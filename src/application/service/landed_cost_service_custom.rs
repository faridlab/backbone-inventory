//! Landed-cost documents (hand-authored, user-owned): draft → validate → done, cancel from
//! draft only (hand_set state machine, ADR-0016 document vocabulary).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in
//! [`super::inventory_write_service`]. A landed cost revalues the DONE moves of ONE target
//! purchase receipt:
//!
//! 1. **Split** each cost line's amount over the target lines by its basis — `quantity`
//!    (done qty), `value` (carried value), or `weight` (done qty × the item master's
//!    per-unit weight). HALF-UP at the currency minor unit, the LAST target line (by
//!    `move_line_id`) eats the rounding diff. A basis that sums to ZERO across every target
//!    line is rejected LOUDLY ([`InventoryError::LandedCostZeroSplitBasis`]) — no silent
//!    equal-split fallback, no partial worksheet rows; the deliberate deviation from the
//!    reference ERP's quiet fallback, which masked unweighted item data.
//! 2. **Revalue** the remaining stock through the move engine's adjustment verb
//!    ([`InventoryWriteService::adjust_move_value`]) — one value-only SLE row + bin reblend
//!    per target line. The engine stays the single SLE/bin writer; this door never touches
//!    `insert_sle`/`update_balance` itself.
//! 3. **Post** ONE balanced envelope through the posting_state machinery the voucher doors
//!    use (`Dr inventory valuation / Cr per cost line account`), both sides carrying only the
//!    remaining-share value.
//!
//! **Retroactive-revaluation asymmetry (deliberate, preserved).** Only the still-on-hand
//! portion of a target line revalues: `delta = allocation × remaining_qty / done_qty`, where
//! `remaining_qty` is a read-only FIFO ATTRIBUTION over the SLE history (attribution, NOT
//! FIFO costing — the moving-average engine stays the only costing writer). The
//! already-consumed portion produces NO correcting entry — no COGS true-up, no journal leg.
//! See the full rationale at the engine verb's site ([`super::inventory_move_engine`]).
//!
//! **Worksheet, not a ledger.** The allocation worksheet (`landed_cost_adjustment_lines`) is
//! deleted and recreated on every validation, rows written ordered by `move_line_id` so the
//! last-line-eats-the-diff recipient is deterministic. The only accounting identity is the
//! posted journal entry; the worksheet is a read-back surface (and the GL repost's
//! credit-split source).
//!
//! **Reversal pattern.** A `done` landed cost can NEVER cancel — a correction is a
//! negative-amount landed cost whose debit/credit legs are swapped verbatim (a negative
//! document lowers the bin value and posts Dr cost lines / Cr inventory).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! [`crate::infrastructure::persistence::ValuationOverlayRepository`] and the engine's
//! repositories.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    GlVoucher, LcLineRow, MoveRow, NewLcLineRow, NewLandedCostRow, NewWorksheetRow,
};

use super::inventory_events::{InventoryEvent, LandedCostValidated};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_posture::Posture;
use super::inventory_write_service::{
    is_dup, money, InventoryError, InventoryWriteService, NewLandedCost, SubmitOutcome,
};

/// One target line of the split: the DONE move behind it, its single execution line, and the
/// basis vectors the three split methods read. The receipt door mints exactly one move per
/// stock line (one execution line each), so one move = one target line here.
#[derive(Clone)]
struct TargetLine {
    mv: MoveRow,
    move_line_id: Uuid,
    /// The move's destination (the warehouse's stock location) — where the debit leg's
    /// account-resolution chain starts.
    dest_location_id: Uuid,
    /// `done_qty` — the move's done quantity (basis of the quantity/weight splits, denominator
    /// of the remaining-share ratio).
    done_qty: Decimal,
    /// The value the move's IN leg blended into its bin (basis of the value split).
    carried_value: Decimal,
    /// The item master's per-unit shipping weight (basis of the weight split).
    weight_per_unit: Decimal,
    /// How many of the move's units are still on hand (read-only FIFO attribution at validate
    /// time — the revaluation's effective share).
    remaining_qty: Decimal,
}

/// One worksheet row of the computed plan (per cost line × target line).
struct PlanRow {
    move_line_id: Uuid,
    cost_line_id: Uuid,
    /// This cost line's rounded allocation onto the target line (the last line by
    /// `move_line_id` eats the rounding diff).
    share: Decimal,
    /// The target line's CUMULATIVE allocation across ALL cost lines (repeated on each of the
    /// line's rows — the worksheet reads cumulative per move line).
    cumulative: Decimal,
    remaining_qty: Decimal,
}

/// The effective revaluation of one target line (Σ over cost lines of the remaining-share
/// portions of its allocations).
struct PlanDelta {
    move_line_id: Uuid,
    delta: Decimal,
}

/// The computed validation: everything the write phase and the GL phase need, derived in ONE
/// read pass so every loud rejection fires BEFORE any row is written.
struct LcPlan {
    lc_id: Uuid,
    lc_number: String,
    company_id: Uuid,
    branch_id: Option<Uuid>,
    currency: String,
    posting_date: chrono::NaiveDate,
    target_receipt_id: Uuid,
    posture: Posture,
    /// Target lines in WRITE order (sorted by `move_line_id` — the worksheet's insert order,
    /// which fixes the deterministic rounding-diff recipient).
    targets: Vec<TargetLine>,
    rows: Vec<PlanRow>,
    deltas: Vec<PlanDelta>,
    /// Debit legs pre-resolved onto the account chain (location override → receipt header
    /// inventory account), aggregated per account in target order.
    debits: Vec<(Uuid, Decimal)>,
    /// Credit legs per cost line (its account, its Σ remaining-share delta), in line order.
    credits: Vec<(Uuid, Decimal)>,
    /// Σ cost line amounts (the document total).
    amount_total: Decimal,
    /// Σ effective deltas — the value actually revalued onto the bins (what the GL carries).
    revalued_total: Decimal,
}

/// Accumulate `amt` onto the `acct` entry, preserving insertion order.
fn push_amount(legs: &mut Vec<(Uuid, Decimal)>, acct: Uuid, amt: Decimal) {
    if amt.is_zero() { return }
    if let Some(e) = legs.iter_mut().find(|(a, _)| *a == acct) {
        e.1 += amt;
    } else {
        legs.push((acct, amt));
    }
}

impl InventoryWriteService {
    // ---- create ---------------------------------------------------------------

    /// Open a DRAFT landed cost: header + cost lines as one unit. Guards: at least one line
    /// (nothing to allocate); every line carries a credit account (a line without one is
    /// rejected loudly — its split's credit side would be undefined). Negative amounts are
    /// allowed: a negative landed cost is the reversal pattern.
    pub async fn create_landed_cost(&self, lc: NewLandedCost) -> Result<Uuid, InventoryError> {
        if lc.lines.is_empty() {
            return Err(InventoryError::LandedCostNoLines { lc_id: Uuid::new_v4() });
        }
        for l in &lc.lines {
            if l.account_id.is_nil() {
                return Err(InventoryError::LandedCostLineNeedsAccount { line_id: Uuid::new_v4() });
            }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, lc.company_id).await?;
        let ins = self.valuation_overlay.insert_landed_cost_draft(&mut tx, &NewLandedCostRow {
            id,
            lc_number: &lc.lc_number,
            company_id: lc.company_id,
            branch_id: lc.branch_id,
            target_receipt_id: lc.target_receipt_id,
            currency: &lc.currency,
            posting_date: lc.posting_date,
            notes: lc.notes.as_deref(),
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(lc.lc_number) } else { e.into() });
        }
        for l in &lc.lines {
            // An invalid split method fails the DB's enum cast loudly (a typed mapping would
            // swallow the offending value); the vocabulary is quantity|value|weight.
            self.valuation_overlay.insert_lc_line(&mut tx, &NewLcLineRow {
                id: Uuid::new_v4(),
                lc_id: id,
                company_id: lc.company_id,
                name: &l.name,
                account_id: l.account_id,
                split_method: &l.split_method,
                amount: l.amount,
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- validate -------------------------------------------------------------

    /// Validate the DRAFT landed cost: split → revalue → post, the full verb. The physical
    /// revaluation (worksheet rebuild + value-only SLE rows + bin reblends + `state=done` +
    /// GL leg armed `pending`) commits FIRST; the GL post is eventually consistent, exactly
    /// like the voucher doors. Service/job-driven (it needs the composing service's
    /// `GlPostSink`).
    pub async fn validate_landed_cost(
        &self,
        lc_id: Uuid,
        sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        let plan = self.lc_compute(lc_id).await?;
        self.lc_apply(&plan).await?;
        self.lc_post(&plan, Some(sink)).await
    }

    /// The HTTP-shaped validate: the SAME physical revalidation, but the GL leg stays ARMED
    /// `pending` for a service-driven [`Self::repost_landed_cost`] to drive (the composing
    /// service's sink is not available on a bare route — the module's standing posture for
    /// GL-posting verbs). Documents that structurally post nothing (a `periodic` company, an
    /// all-consumed allocation) retire to `not_applicable` immediately.
    pub async fn validate_landed_cost_deferred(&self, lc_id: Uuid) -> Result<SubmitOutcome, InventoryError> {
        let plan = self.lc_compute(lc_id).await?;
        self.lc_apply(&plan).await?;
        self.lc_post(&plan, None).await
    }

    // ---- cancel ---------------------------------------------------------------

    /// Cancel a DRAFT landed cost. A `done` landed cost can never cancel — its bins and ledger
    /// already revalued; the correction pattern is a NEGATIVE landed cost (swapped legs).
    pub async fn cancel_landed_cost(&self, lc_id: Uuid) -> Result<(), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        let hdr = self.valuation_overlay
            .fetch_lc_header(&mut tx, lc_id).await?
            .ok_or(InventoryError::NotFound(lc_id))?;
        if hdr.state != "draft" {
            return Err(InventoryError::LandedCostNotDraft { lc_id, state: hdr.state });
        }
        company_scope::bind_company_on(&mut tx, hdr.company_id).await?;
        self.valuation_overlay.delete_worksheet(&mut tx, lc_id).await?;
        self.valuation_overlay.mark_lc_cancelled(&mut tx, lc_id).await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- GL repost: re-drive a stuck post --------------------------------------

    /// Re-drive the GL leg of a DONE landed cost whose `posting_state` is `pending` or
    /// `failed` (a rejected or interrupted post — the physical revaluation already committed
    /// and is never re-touched). The envelope is rebuilt from the MINTED ledger and the
    /// worksheet: debit legs from the landed-cost SLE rows (each row's signed delta, its
    /// target line resolving the valuation-account chain), credit legs from the worksheet's
    /// per-cost-line shares scaled by each row's snapshotted remaining share (a done move's
    /// quantity is frozen, so the rebuild is exact). Idempotent: `posted` short-circuits with
    /// the recorded ids, `not_applicable` is a no-op, and the envelope re-uses the document's
    /// source identity so accounting's dedupe on `(company, source_type, source_id,
    /// posting_type)` can never double-post.
    pub async fn repost_landed_cost(
        &self,
        lc_id: Uuid,
        sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        let (hdr, receipt_inventory_account_id, lines, worksheet, sles) = {
            let mut conn = self.db_pool.acquire().await?;
            let hdr = self.valuation_overlay
                .fetch_lc_header(&mut conn, lc_id).await?
                .ok_or(InventoryError::NotFound(lc_id))?;
            company_scope::bind_company_on(&mut conn, hdr.company_id).await?;
            let target = self.valuation_overlay
                .fetch_lc_target_receipt(&mut conn, hdr.target_receipt_id).await?
                .ok_or(InventoryError::NotFound(hdr.target_receipt_id))?;
            let lines = self.valuation_overlay.fetch_lc_lines(&mut conn, lc_id).await?;
            let worksheet = self.valuation_overlay.fetch_worksheet(&mut conn, lc_id).await?;
            let sles = self.sles.fetch_lc_revaluations(&mut conn, hdr.company_id, lc_id).await?;
            (hdr, target.inventory_account_id, lines, worksheet, sles)
        };
        match hdr.posting_state.as_str() {
            "posted" => return Ok(SubmitOutcome {
                voucher_id: lc_id, posted: true,
                journal_id: hdr.journal_id, post_id: hdr.accounting_post_id,
                gl_amount: Decimal::ZERO,
            }),
            "not_applicable" => return Ok(SubmitOutcome {
                voucher_id: lc_id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            }),
            _ => {} // pending | failed → re-drive below
        }
        if hdr.state != "done" {
            // posting_state only leaves not_applicable inside a validate transaction, so this
            // is a defensive guard — but a hand-mangled row should fail loudly, not post.
            return Err(InventoryError::LandedCostNotDraft { lc_id, state: hdr.state });
        }

        // Debit legs: one per minted SLE row, resolved onto the account chain
        // (target line's location override → the receipt header's inventory account).
        let mut debits: Vec<(Uuid, Decimal)> = Vec::new();
        let mut credits: Vec<(Uuid, Decimal)> = Vec::new();
        {
            let mut conn = self.db_pool.acquire().await?;
            for (voucher_no, delta) in &sles {
                // voucher_no = {lc_number}/{move_name}/{move_line_id} — the trailing id is the
                // stable target grain.
                let line_id = voucher_no.rsplit('/').next()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or(InventoryError::LandedCostNoValuationAccount { move_id: lc_id })?;
                let line = self.move_lines.fetch_line(&mut conn, line_id).await?
                    .ok_or(InventoryError::LandedCostNoValuationAccount { move_id: lc_id })?;
                let acct = self.inventory_leg_account(hdr.company_id, line.location_dest_id, receipt_inventory_account_id).await?;
                push_amount(&mut debits, acct, *delta);
            }
            // Credit legs: per cost line, its worksheet shares scaled by the snapshotted
            // remaining share over the (frozen) done qty — the exact validate-time delta.
            for w in &worksheet {
                let Some(line) = lines.iter().find(|l| l.id == w.cost_line_id) else { continue };
                let target_line = self.move_lines.fetch_line(&mut conn, w.move_line_id).await?
                    .ok_or(InventoryError::LandedCostNoValuationAccount { move_id: lc_id })?;
                if target_line.quantity.is_zero() { continue }
                let delta = money(w.share * w.remaining_qty / target_line.quantity);
                push_amount(&mut credits, line.account_id, delta);
            }
        }
        let revalued_total: Decimal = sles.iter().map(|(_, d)| *d).sum();
        let env = Self::lc_envelope(
            lc_id, &hdr.lc_number, hdr.company_id, hdr.branch_id, hdr.posting_date, &hdr.currency,
            &debits, &credits, revalued_total,
        )?;
        self.emit_and_reconcile(GlVoucher::LandedCost, lc_id, &env, sink, revalued_total).await
    }

    // ---- the shared compute (read phase + split math; rejects BEFORE any write) ----

    /// Resolve the draft, its cost lines, the target receipt's DONE moves and every basis
    /// vector, run the split math, and return the full plan. Every loud guard fires here — a
    /// rejected validation has written nothing at all.
    async fn lc_compute(&self, lc_id: Uuid) -> Result<LcPlan, InventoryError> {
        let (hdr, lines, mut targets, posture, receipt_inventory_account_id) = {
            let mut conn = self.db_pool.acquire().await?;
            let hdr = self.valuation_overlay
                .fetch_lc_header(&mut conn, lc_id).await?
                .ok_or(InventoryError::NotFound(lc_id))?;
            if hdr.state != "draft" {
                return Err(InventoryError::LandedCostNotDraft { lc_id, state: hdr.state });
            }
            company_scope::bind_company_on(&mut conn, hdr.company_id).await?;
            let target = self.valuation_overlay
                .fetch_lc_target_receipt(&mut conn, hdr.target_receipt_id).await?
                .ok_or(InventoryError::NotFound(hdr.target_receipt_id))?;
            let lines = self.valuation_overlay.fetch_lc_lines(&mut conn, lc_id).await?;
            let posture = self.posting_posture_on(&mut conn, hdr.company_id).await?;

            // The target lines: the receipt's DONE moves (the door stamps the receipt number
            // as every line move's `origin`), each with its single execution line.
            let mut targets: Vec<TargetLine> = Vec::new();
            let moves = self.moves.fetch_moves_by_origin(&mut conn, hdr.company_id, &target.receipt_number).await?;
            for mv in moves {
                if mv.state != "done" || mv.quantity <= Decimal::ZERO { continue }
                let Some(line) = self.move_lines.fetch_lines_for_move(&mut conn, mv.id).await?
                    .into_iter().find(|l| l.quantity > Decimal::ZERO) else { continue };
                let carried = self.sles.sum_move_in_value(&mut conn, hdr.company_id, mv.id).await?;
                let weight = self.valuation_overlay
                    .fetch_item_weight(&mut conn, hdr.company_id, mv.item_id).await?;
                targets.push(TargetLine {
                    move_line_id: line.id,
                    dest_location_id: mv.location_dest_id,
                    done_qty: mv.quantity,
                    carried_value: carried,
                    weight_per_unit: weight,
                    remaining_qty: Decimal::ZERO, // filled by the attribution below
                    mv,
                });
            }
            (hdr, lines, targets, posture, target.inventory_account_id)
        };
        if targets.is_empty() {
            // Nothing DONE to revalue: the receipt was never submitted, or every line was
            // zero / a landed-cost service line (the seam — those mint no stock).
            return Err(InventoryError::LandedCostNoValuedTargets { receipt_id: hdr.target_receipt_id });
        }
        if lines.is_empty() {
            return Err(InventoryError::LandedCostNoLines { lc_id });
        }
        for l in &lines {
            if l.account_id.is_nil() {
                return Err(InventoryError::LandedCostLineNeedsAccount { line_id: l.id });
            }
        }
        // A `standard` company refuses loudly: under standard costing a landed cost would
        // introduce a variance the standard-recompute engine (a later increment) must absorb —
        // refusing beats silently diverging. `average` (and the `fifo` vocabulary) pass.
        if posture.cost_method == "standard" {
            return Err(InventoryError::LandedCostRequiresCostMethod {
                company_id: hdr.company_id, cost_method: posture.cost_method.clone(),
            });
        }

        // Read-only FIFO attribution (the remaining share per move) — attribution, NOT costing.
        let move_ids: Vec<Uuid> = targets.iter().map(|t| t.mv.id).collect();
        let remaining = company_scope::with_company_scope(
            Some(hdr.company_id),
            self.sles.remaining_qty_for_moves(&self.db_pool, hdr.company_id, &move_ids),
        ).await?;
        for t in targets.iter_mut() {
            t.remaining_qty = remaining.get(&t.mv.id).copied().unwrap_or(Decimal::ZERO);
        }

        // Deterministic write order: by `move_line_id` (the worksheet's insert order — the
        // LAST line by move_line_id is the rounding-diff recipient), then cost-line order.
        targets.sort_by(|a, b| a.move_line_id.cmp(&b.move_line_id));

        // ---- the split math (one pass per cost line) --------------------------------------
        let mut rows: Vec<PlanRow> = Vec::new();
        let mut delta_by_line: HashMap<Uuid, Decimal> = HashMap::new();
        let mut credits: Vec<(Uuid, Decimal)> = Vec::new();
        for c in &lines {
            let basis: Vec<Decimal> = targets.iter().map(|t| match c.split_method.as_str() {
                "value" => t.carried_value,
                "weight" => t.done_qty * t.weight_per_unit,
                _ => t.done_qty, // quantity
            }).collect();
            let denominator: Decimal = basis.iter().sum();
            // The LOUD zero-denominator rejection — the decided deviation: no equal-split
            // fallback, no partial worksheet rows. An all-zero weight basis (no item carries
            // a per-unit weight) or an all-zero value basis (zero-valued target lines) is a
            // configuration defect the operator must see.
            if denominator.is_zero() {
                return Err(InventoryError::LandedCostZeroSplitBasis {
                    basis: c.split_method.clone(),
                    target_receipt_id: hdr.target_receipt_id,
                });
            }
            let mut allocated = Decimal::ZERO;
            let mut line_delta_total = Decimal::ZERO;
            for (i, t) in targets.iter().enumerate() {
                let share = if i == targets.len() - 1 {
                    // The last line (by move_line_id) eats the rounding diff — the worksheet's
                    // insert order makes this recipient deterministic.
                    c.amount - allocated
                } else {
                    let s = money(c.amount * basis[i] / denominator);
                    allocated += s;
                    s
                };
                // THE RETROACTIVE-REVALUATION ASYMMETRY: only the remaining share of the
                // allocation revalues the bin; the consumed portion produces NO correcting
                // entry (no COGS true-up, no journal leg). Deliberate — see the engine verb.
                let delta = if t.done_qty.is_zero() {
                    Decimal::ZERO
                } else {
                    money(share * t.remaining_qty / t.done_qty)
                };
                line_delta_total += delta;
                *delta_by_line.entry(t.move_line_id).or_insert(Decimal::ZERO) += delta;
                rows.push(PlanRow {
                    move_line_id: t.move_line_id,
                    cost_line_id: c.id,
                    share,
                    cumulative: Decimal::ZERO, // filled after every cost line has run
                    remaining_qty: t.remaining_qty,
                });
            }
            // G2 check (a): the worksheet's Σ allocations must equal the Σ cost amounts (the
            // last-line-eats-diff rule makes the identity exact; a violation is internal).
            let line_shares: Decimal = rows.iter()
                .filter(|r| r.cost_line_id == c.id).map(|r| r.share).sum();
            if line_shares != c.amount {
                return Err(InventoryError::LandedCostAllocationMismatch {
                    lc_id, allocated: line_shares, declared: c.amount,
                });
            }
            push_amount(&mut credits, c.account_id, line_delta_total);
        }
        // The cumulative per move line (across ALL cost lines), stamped on each of its rows.
        let mut cumulative_by_line: HashMap<Uuid, Decimal> = HashMap::new();
        for r in &rows {
            *cumulative_by_line.entry(r.move_line_id).or_insert(Decimal::ZERO) += r.share;
        }
        for r in rows.iter_mut() {
            r.cumulative = cumulative_by_line[&r.move_line_id];
        }

        let deltas: Vec<PlanDelta> = targets.iter().map(|t| PlanDelta {
            move_line_id: t.move_line_id,
            delta: delta_by_line.get(&t.move_line_id).copied().unwrap_or(Decimal::ZERO),
        }).collect();
        let amount_total: Decimal = lines.iter().map(|c| c.amount).sum();
        let revalued_total: Decimal = delta_by_line.values().copied().sum();

        // Debit legs pre-resolved onto the account chain: the target line's destination
        // location override, else the receipt header's inventory account.
        let mut debits: Vec<(Uuid, Decimal)> = Vec::new();
        for t in &targets {
            let Some(d) = deltas.iter().find(|d| d.move_line_id == t.move_line_id) else { continue };
            if d.delta.is_zero() { continue }
            let acct = self.inventory_leg_account(hdr.company_id, t.dest_location_id, receipt_inventory_account_id).await?;
            push_amount(&mut debits, acct, d.delta);
        }

        Ok(LcPlan {
            lc_id,
            lc_number: hdr.lc_number,
            company_id: hdr.company_id,
            branch_id: hdr.branch_id,
            currency: hdr.currency,
            posting_date: hdr.posting_date,
            target_receipt_id: hdr.target_receipt_id,
            posture,
            targets,
            rows,
            deltas,
            debits,
            credits,
            amount_total,
            revalued_total,
        })
    }

    // ---- the write phase (one transaction: worksheet + revaluation + state) ----------

    /// Apply a computed plan in ONE transaction: rebuild the worksheet (delete + recreate,
    /// ordered by `move_line_id`), mint the value-only revaluation SLE rows + bin reblends
    /// through the engine verb, flip the document `done` and arm its GL leg `pending`. A crash
    /// before commit rolls the whole unit back; a crash after it leaves the armed leg for
    /// [`Self::repost_landed_cost`].
    async fn lc_apply(&self, plan: &LcPlan) -> Result<(), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, plan.company_id).await?;
        self.valuation_overlay.delete_worksheet(&mut tx, plan.lc_id).await?;
        for r in &plan.rows {
            self.valuation_overlay.insert_worksheet_row(&mut tx, &NewWorksheetRow {
                lc_id: plan.lc_id,
                company_id: plan.company_id,
                move_line_id: r.move_line_id,
                cost_line_id: r.cost_line_id,
                share: r.share,
                additional_landed_cost: r.cumulative,
                remaining_qty: r.remaining_qty,
            }).await?;
        }
        for t in &plan.targets {
            let Some(d) = plan.deltas.iter().find(|d| d.move_line_id == t.move_line_id) else { continue };
            // The engine owns the SLE/bin writes — the door calls the verb (one-writer rule).
            // A zero delta mints nothing (an all-consumed target line revalues nothing).
            self.adjust_move_value(&mut tx, &t.mv, t.move_line_id, d.delta, plan.lc_id, &plan.lc_number).await?;
        }
        self.valuation_overlay.mark_lc_validated(&mut tx, plan.lc_id, plan.amount_total).await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- the GL phase (shared by both validate entrypoints) -------------------------

    /// `Some(sink)` emits the envelope and reconciles; `None` (the deferred/HTTP validate)
    /// leaves the armed `pending` leg for a repost drive. Documents that structurally post
    /// nothing — a `periodic` company, an all-consumed allocation (Σ remaining-share = 0) —
    /// retire to `not_applicable` either way.
    async fn lc_post(
        &self,
        plan: &LcPlan,
        sink: Option<&dyn GlPostSink>,
    ) -> Result<SubmitOutcome, InventoryError> {
        let event = || InventoryEvent::LandedCostValidated(LandedCostValidated {
            lc_id: plan.lc_id,
            company_id: plan.company_id,
            target_receipt_id: plan.target_receipt_id,
            amount_total: plan.amount_total,
            revalued_value: plan.revalued_total,
        });
        if plan.posture.periodic || plan.revalued_total.is_zero() {
            company_scope::with_company_scope(
                Some(plan.company_id),
                self.gl.mark_not_applicable(&self.db_pool, GlVoucher::LandedCost, plan.lc_id),
            ).await?;
            self.sink.publish(event());
            return Ok(SubmitOutcome {
                voucher_id: plan.lc_id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
        let env = Self::lc_envelope(
            plan.lc_id, &plan.lc_number, plan.company_id, plan.branch_id, plan.posting_date,
            &plan.currency, &plan.debits, &plan.credits, plan.revalued_total,
        )?;
        match sink {
            None => {
                // Deferred validate: the leg stays armed `pending`; the event still fires —
                // the physical revaluation happened.
                self.sink.publish(event());
                Ok(SubmitOutcome {
                    voucher_id: plan.lc_id, posted: false, journal_id: None, post_id: None,
                    gl_amount: plan.revalued_total,
                })
            }
            Some(sink) => {
                let out = self.emit_and_reconcile(
                    GlVoucher::LandedCost, plan.lc_id, &env, sink, plan.revalued_total,
                ).await?;
                self.sink.publish(event());
                Ok(out)
            }
        }
    }

    /// Build the balanced landed-cost envelope. Debit legs are the target lines' revaluation
    /// deltas on the account-resolution chain (location override → receipt header inventory
    /// account); credit legs carry each cost line's remaining-share total — BOTH sides carry
    /// only the remaining-portion value, so the post balances without inventing the missing
    /// COGS leg the consumed portion would need. A NEGATIVE document (the reversal pattern)
    /// swaps the sides verbatim: absolute amounts on the opposite legs. G2 check (b): the
    /// journal's Σ must equal Σ deltas — by construction both sides sum `|total|`; the assert
    /// keeps a future edit from silently unbalancing it.
    fn lc_envelope(
        lc_id: Uuid,
        lc_number: &str,
        company_id: Uuid,
        branch_id: Option<Uuid>,
        posting_date: chrono::NaiveDate,
        currency: &str,
        debits: &[(Uuid, Decimal)],
        credits: &[(Uuid, Decimal)],
        total: Decimal,
    ) -> Result<AccountingPostEnvelope, InventoryError> {
        let (dr_side, cr_side) = if total >= Decimal::ZERO {
            (debits, credits)
        } else {
            // Reversal: debit the cost-line accounts, credit inventory — swapped verbatim.
            (credits, debits)
        };
        let abs = |v: Decimal| v.abs();
        let lines: Vec<GlPostLine> = dr_side.iter()
            .map(|(a, amt)| GlPostLine::debit(*a, abs(*amt)).with_description("Inventory valuation"))
            .chain(cr_side.iter()
                .map(|(a, amt)| GlPostLine::credit(*a, abs(*amt)).with_description("Landed cost")))
            .collect();
        let env = AccountingPostEnvelope {
            idempotency_key: lc_id.to_string(),
            company_id,
            branch_id,
            source_type: "inventory".into(),
            source_id: lc_id,
            source_reference: Some(lc_number.to_string()),
            posting_date,
            currency: currency.to_string(),
            posting_type: "original".into(),
            reverses_post_id: None,
            description: Some("Landed cost".into()),
            lines,
        };
        // G2 (b): the journal must carry exactly the revalued value, on both sides.
        let dr_sum: Decimal = env.lines.iter().map(|l| l.debit).sum();
        let cr_sum: Decimal = env.lines.iter().map(|l| l.credit).sum();
        if dr_sum != abs(total) || cr_sum != abs(total) {
            return Err(InventoryError::LandedCostAllocationMismatch {
                lc_id, allocated: dr_sum, declared: abs(total),
            });
        }
        Ok(env)
    }
}

/// Exposed for tests: the split math's HALF-UP rounding helper is the service-wide `money`.
#[allow(dead_code)]
fn _money_is_half_up(v: Decimal) -> Decimal { money(v) }
