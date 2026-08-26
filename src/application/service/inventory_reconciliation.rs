//! The ONE adjustment door + the StockReconciliation voucher over it (hand-authored,
//! user-owned).
//!
//! **Quant-driven adjustment (spec stock-business-logic.md §5.2 — there is NO
//! `stock.inventory` model).** A counted quantity STAGES on the quant row
//! (`inventory_quantity`), the diff is a stored compute (`inventory_diff_quantity =
//! counted − quantity`, T4), and `inventory_quantity_set` gates Apply. Applying mints an
//! `is_inventory` move between the counted location and the inventory-loss location THROUGH
//! the move engine ([`super::inventory_move_engine`]) — never a second writer of the quant:
//! the engine's two-step `_synchronize_quant` flips the on-hand to exactly the count, mints
//! the SLE pair, and reblends the Bin (the moving-average core owns the money path). The
//! staged count is consumed on apply, so a second apply finds the gate down and is a NO-OP;
//! a move landing between count and apply is refused LOUDLY as an outdated count (spec
//! `is_outdated`), and a crash between the move's commit and the staging consume is
//! recovered idempotently (on-hand already equals the count → consume, mint nothing).
//!
//! **The voucher converges onto the door.** `submit_reconciliation` keeps its document
//! identity verbatim — header, item rows, the single value-diff `AccountingPost`
//! (`Dr Inventory · Cr Adjustment` up / reversed down; net zero → `not_applicable`), the
//! posting-state lifecycle, `repost_reconciliation`, and the `StockReconciled` event — but
//! every line's physical movement is now quant-staged and move-minted through the door. The
//! voucher's value-diff leg posts EXACTLY ONCE per voucher (idempotency key = the
//! reconciliation id, deduped on `(company, source_type, source_id, posting_type)`); the
//! per-line moves mint with a directive carrying no GL accounts, so the door never posts a
//! second envelope under the voucher. A count carrying an explicit counted RATE is refused
//! (the door values the diff at the current moving average — a rate revaluation is a
//! valuation-overlay concern, never done silently here).
//!
//! Guard map (schema/hooks/stock.hook.yaml): R24 `no_count_while_reserved` (a quant holding
//! reservations cannot be staged or applied), R13 (only stockable locations hold counts),
//! R26 (the quant's company follows its location), T4 (the stored diff compute).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! [`super::super::super::infrastructure::persistence::StockAdjustmentRepository`] (quant
//! lock / stage / consume), `StockPickingRepository` (location resolution, the
//! mint-once/resume gate) and the engine's repositories; every write takes this service's
//! transaction so the staging, the move mint, and the voucher rows commit in bounded units.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    NewReconciliationItemRow, NewReconciliationRow, QuantSelector, StagedCountRow,
};

use super::inventory_events::{InventoryEvent, StockReconciled};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective};
use super::inventory_write_service::{
    is_dup, money, InventoryError, InventoryWriteService, NewReconciliation, SubmitOutcome,
};

// --- door vocabulary ----------------------------------------------------------

/// A staged count: the counted level, the on-hand it was staged against, and the stored
/// diff compute (T4) now sitting on the quant row.
#[derive(Debug, Clone)]
pub struct StagedCount {
    pub quant_id: Uuid,
    pub location_id: Uuid,
    pub on_hand_qty: Decimal,
    pub counted_qty: Decimal,
    pub diff_qty: Decimal,
}

/// The outcome of an apply: what the door did. `applied = false, move_id = None` is the
/// IDEMPOTENT no-op (nothing staged, or the count already reconciled); `move_id = Some(_)`
/// is the `is_inventory` move the door minted (or resumed) and validated.
#[derive(Debug, Clone)]
pub struct AppliedCount {
    pub quant_id: Uuid,
    pub location_id: Uuid,
    pub counted_qty: Decimal,
    pub diff_qty: Decimal,
    pub move_id: Option<Uuid>,
    pub applied: bool,
}

impl InventoryWriteService {
    // ---- the adjustment door: stage -------------------------------------------

    /// Stage a counted quantity on a quant (spec §5.2): the count is a LEVEL (the counted
    /// on-hand), staged as `inventory_quantity` with its stored diff compute (T4) and the
    /// `inventory_quantity_set` gate that [`Self::apply_inventory`] demands. Staging
    /// OVERWRITES any prior staging — the count is not a delta, so re-counting replaces the
    /// previous count. Guards: R24 (the quant holds no reservations), R13 (only internal
    /// locations are countable), R26 (the location belongs to the caller's company). A
    /// quant that does not exist yet is initialized at zero — the count surface exists even
    /// before any stock does.
    pub async fn stage_quant_count(
        &self,
        company_id: Uuid,
        item_id: Uuid,
        location_id: Uuid,
        counted_qty: Decimal,
    ) -> Result<StagedCount, InventoryError> {
        if counted_qty < Decimal::ZERO {
            return Err(InventoryError::NegativeQuantity);
        }
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): bound before any fenced read — an unbound connection sees
        // zero quants and would initialize a duplicate count surface.
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let existing = self.adjustments.lock_quant(
            &mut tx, QuantSelector::ItemLocation { item_id, location_id },
        ).await?;
        let quant = match existing {
            Some(row) => {
                self.check_countable(&row, company_id)?;
                row
            }
            None => {
                // Bootstrap the count surface: the location must be a stockable location of
                // this company (R13 / R26) before a quant may exist on it. The two-id facts
                // read resolves the same location twice — it is a single-location lookup.
                let locs = self.moves.fetch_move_locations(&mut tx, location_id, location_id).await?;
                let facts = locs.0.ok_or(InventoryError::LocationNotFound(location_id))?;
                if facts.usage == "view" {
                    return Err(InventoryError::ViewLocationHoldsNoStock { location_id });
                }
                if facts.usage != "internal" {
                    // View locations are the named R13 case; every other non-internal usage
                    // (supplier/customer/inventory-loss/transit/production) holds no
                    // countable stock of ours either — the shared error vocabulary covers
                    // the whole class under the view-location code.
                    return Err(InventoryError::ViewLocationHoldsNoStock { location_id });
                }
                if facts.company_id != Some(company_id) {
                    return Err(InventoryError::QuantCompanyMismatch {
                        location_id,
                        location_company: facts.company_id,
                        move_company: company_id,
                    });
                }
                let id = self.adjustments.init_quant(&mut tx, item_id, location_id, company_id).await?;
                self.adjustments.lock_quant(&mut tx, QuantSelector::ById(id)).await?
                    .ok_or(InventoryError::NotFound(id))?
            }
        };
        let diff = counted_qty - quant.quantity;
        self.adjustments.stage_count(&mut tx, quant.id, counted_qty, diff).await?;
        tx.commit().await?;
        Ok(StagedCount {
            quant_id: quant.id,
            location_id,
            on_hand_qty: quant.quantity,
            counted_qty,
            diff_qty: diff,
        })
    }

    // ---- the adjustment door: apply (_apply_inventory) -------------------------

    /// Apply a staged count (spec §5.2 `_apply_inventory`): mint (or resume) the
    /// `is_inventory` move that reconciles the quant's on-hand to the count, and validate
    /// it through the engine pipeline — the move is the ONE writer of the quant's
    /// `quantity`; this method never touches it. Idempotent by construction:
    ///   - nothing staged (gate down) → no-op;
    ///   - on-hand already equals the count (a crash between the move's commit and the
    ///     staging consume, or offsetting activity) → consume the staging, mint nothing;
    ///   - a live `is_inventory` move for the grain already exists (a crash mid-pipeline)
    ///     → resume IT, never mint a second (the mint-once gate);
    ///   - a move landed between the count and here → LOUD `OutdatedCount` (the staged
    ///     diff no longer reconciles the count; re-stage against the new on-hand).
    ///
    /// GL: with accounts in `gl`, the engine posts the adjustment leg per move
    /// (`Dr Inventory · Cr Adjustment` up / reversed down, idempotency key = move id); a
    /// directive without accounts posts nothing (the physical movement is unaffected) —
    /// the voucher path drives the door that way and posts its own single net envelope.
    pub async fn apply_inventory(
        &self,
        company_id: Uuid,
        selector: QuantSelector,
        gl: &MoveGlDirective,
        sink: &dyn GlPostSink,
    ) -> Result<AppliedCount, InventoryError> {
        // -- gate + guards under the quant's FOR UPDATE lock --------------------------------
        let (quant_id, location_id, counted, staged_diff) = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let quant = self.adjustments.lock_quant(&mut tx, selector).await?
                .ok_or_else(|| match selector {
                    QuantSelector::ById(id) => InventoryError::NotFound(id),
                    QuantSelector::ItemLocation { location_id, .. } =>
                        InventoryError::LocationNotFound(location_id),
                })?;
            self.check_countable(&quant, company_id)?;
            if !quant.inventory_quantity_set {
                tx.commit().await?;
                return Ok(AppliedCount {
                    quant_id: quant.id, location_id: quant.location_id,
                    counted_qty: quant.quantity, diff_qty: Decimal::ZERO,
                    move_id: None, applied: false,
                });
            }
            let counted = quant.inventory_quantity.unwrap_or(quant.quantity);
            let staged_diff = quant.inventory_diff_quantity.unwrap_or(Decimal::ZERO);
            if quant.quantity == counted {
                // Already reconciled (crash-window recovery or offsetting activity): the
                // on-hand IS the count — consume the staging, mint nothing.
                self.adjustments.consume_staged_count(&mut tx, quant.id).await?;
                tx.commit().await?;
                return Ok(AppliedCount {
                    quant_id: quant.id, location_id: quant.location_id,
                    counted_qty: counted, diff_qty: Decimal::ZERO,
                    move_id: None, applied: false,
                });
            }
            let on_hand_at_stage = counted - staged_diff;
            if quant.quantity != on_hand_at_stage {
                return Err(InventoryError::OutdatedCount {
                    quant_id: quant.id, counted_qty: counted, on_hand_qty: quant.quantity,
                });
            }
            tx.commit().await?;
            (quant.id, quant.location_id, counted, staged_diff)
        };

        // -- zero diff: the count matches the on-hand — consume, no move --------------------
        if staged_diff == Decimal::ZERO {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            self.adjustments.consume_staged_count(&mut tx, quant_id).await?;
            tx.commit().await?;
            return Ok(AppliedCount {
                quant_id, location_id, counted_qty: counted, diff_qty: Decimal::ZERO,
                move_id: None, applied: true,
            });
        }

        // -- resolve the inventory-loss location (the far end of every adjustment move) ----
        let loss_location = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let loc = self.pickings.ensure_inventory_loss_location(&mut tx, company_id).await?;
            tx.commit().await?;
            loc
        };

        // -- mint-once / resume gate --------------------------------------------------------
        let item_id = match selector {
            QuantSelector::ById(_) => {
                // The locked row carried it; re-read the grain off the quant we just locked.
                let mut tx = self.db_pool.begin().await?;
                company_scope::bind_company_on(&mut tx, company_id).await?;
                let quant = self.adjustments.lock_quant(&mut tx, QuantSelector::ById(quant_id)).await?
                    .ok_or(InventoryError::NotFound(quant_id))?;
                tx.commit().await?;
                quant.item_id
            }
            QuantSelector::ItemLocation { item_id, .. } => item_id,
        };
        let existing = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let found = self.pickings.find_adjustment_moves(
                &mut tx, company_id, item_id, location_id, loss_location,
            ).await?;
            tx.commit().await?;
            found
        };
        let move_id = match existing.first() {
            // A live adjustment move for this grain: resume it (a crash between mint and
            // done leaves exactly this state — minting again would double-apply).
            Some(prior) if prior.state != "done" && prior.state != "cancel" => prior.id,
            _ => {
                let (src, dst, qty) = if staged_diff > Decimal::ZERO {
                    (loss_location, location_id, staged_diff)
                } else {
                    (location_id, loss_location, -staged_diff)
                };
                let warehouse = {
                    let mut tx = self.db_pool.begin().await?;
                    company_scope::bind_company_on(&mut tx, company_id).await?;
                    let locs = self.moves.fetch_move_locations(&mut tx, location_id, location_id).await?;
                    tx.commit().await?;
                    locs.0.and_then(|f| f.warehouse_id)
                };
                self.create_move(super::inventory_move_engine::NewStockMove {
                    name: format!("ADJ/{}/{}", item_id.simple(), location_id.simple()),
                    company_id,
                    item_id,
                    demand_qty: qty,
                    price_unit: Decimal::ZERO, // the diff is valued at the current average
                    procure_method: "make_to_stock".into(),
                    picking_id: None,
                    origin: None,
                    location_id: src,
                    location_dest_id: dst,
                    partner_id: None,
                    warehouse_id: warehouse,
                    orderpoint_id: None,
                    move_orig_ids: vec![],
                    move_dest_ids: vec![],
                    is_inventory: true,
                    scrapped: false,
                    forced_value: None, // ordinary demand: the diff is valued at the current average
                }).await?
            }
        };

        // -- drive the move to done through the engine pipeline -----------------------------
        // Sequence the verbs, re-reading the state between them: the engine owns the state
        // (this method never asserts it), and the assign step is what mints the execution
        // line the done verb requires (R24 at move grain).
        let mut state = self.move_state_of(move_id).await?;
        if state == "draft" {
            self.action_confirm(move_id).await?;
            state = self.move_state_of(move_id).await?;
        }
        if state == "confirmed" || state == "partially_available" {
            self.action_assign(move_id).await?;
            state = self.move_state_of(move_id).await?;
        }
        if state != "assigned" {
            // An adjustment must land WHOLE: a partial draw would leave the on-hand short of
            // the count while the staging is gone. Loud refusal, staging survives for a retry.
            return Err(InventoryError::WrongMoveState {
                move_id, action: "apply", current: state,
            });
        }
        self.action_done(move_id, BackorderPolicy::Never, gl, sink).await?;

        // -- consume the staging (the gate drops AFTER the move landed) ---------------------
        {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            self.adjustments.consume_staged_count(&mut tx, quant_id).await?;
            tx.commit().await?;
        }
        Ok(AppliedCount {
            quant_id, location_id, counted_qty: counted, diff_qty: staged_diff,
            move_id: Some(move_id), applied: true,
        })
    }

    /// The pending-count worklist at a location (the partial index
    /// `WHERE inventory_quantity_set = true` serves this read): every staged-but-unapplied
    /// count, with the stored diff compute. A probe — it derives nothing, writes nothing.
    pub async fn staged_counts(
        &self,
        company_id: Uuid,
        location_id: Uuid,
    ) -> Result<Vec<StagedCountRow>, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let rows = self.adjustments.staged_counts_at_location(&mut tx, location_id).await?;
        tx.commit().await?;
        Ok(rows)
    }

    /// The guards every staging/apply shares: R24 (no count while the quant holds
    /// reservations — release them first), R13 (only stockable locations hold counts),
    /// R26 (the location belongs to the count's company).
    fn check_countable(
        &self,
        quant: &crate::infrastructure::persistence::QuantCountRow,
        company_id: Uuid,
    ) -> Result<(), InventoryError> {
        if quant.location_usage == "view" || quant.location_usage != "internal" {
            return Err(InventoryError::ViewLocationHoldsNoStock { location_id: quant.location_id });
        }
        if quant.location_company_id != Some(company_id) {
            return Err(InventoryError::QuantCompanyMismatch {
                location_id: quant.location_id,
                location_company: quant.location_company_id,
                move_company: company_id,
            });
        }
        if quant.reserved_quantity > Decimal::ZERO {
            return Err(InventoryError::CountReserved {
                quant_id: quant.id, reserved_qty: quant.reserved_quantity,
            });
        }
        Ok(())
    }

    // ---- submit: the StockReconciliation voucher over the door -------------------

    /// Submit a physical-count voucher (converged onto the adjustment door). Document
    /// identity verbatim: header + item rows + the single value-diff GL envelope + the
    /// `StockReconciled` event. Physical: per line, the count stages on the warehouse stock
    /// location's quant and applies through [`Self::apply_inventory`] — the minted
    /// `is_inventory` moves carry the SLE/Bin legs (voucher-type `stock_entry`, voucher id
    /// = the move id: the ledger contract is the append-only SLE + moving average, which
    /// the engine owns). The voucher's GL leg posts exactly once (recon-keyed idempotency);
    /// the door is driven with a no-accounts directive so no per-move GL duplicates it.
    ///
    /// Sequencing: the physical applications run FIRST, the voucher rows are recorded LAST
    /// — a failure mid-way leaves no half-written voucher (the same number can be
    /// re-submitted), and the door's idempotency (gate consumed per applied line; fresh
    /// staging per unapplied line) makes the re-run converge instead of double-applying.
    /// Not atomic across lines: an applied line's stock movement stays applied (the SLE is
    /// append-only truth); the retry re-stages every line against the then-current on-hand.
    pub async fn submit_reconciliation(&self, r: NewReconciliation, sink: &dyn GlPostSink) -> Result<Uuid, InventoryError> {
        if r.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &r.lines {
            if l.counted_qty < Decimal::ZERO || l.counted_rate < Decimal::ZERO {
                return Err(InventoryError::NegativeQuantity);
            }
            if l.counted_rate > Decimal::ZERO {
                return Err(InventoryError::CountedRateUnsupported);
            }
        }

        // The warehouse's stock location: the quant grain the counts stage on (the voucher
        // grain was the warehouse; the door grain is the location — bootstrapped per
        // warehouse on first use, the same resolution the transfer voucher uses).
        let stock_location = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, r.company_id).await?;
            let loc = self.pickings.ensure_internal_location(&mut tx, r.warehouse_id, r.company_id).await?;
            tx.commit().await?;
            loc
        };

        // ---- physical: stage + apply per line through the door ---------------------------
        let mut net = Decimal::ZERO;
        let mut applied: Vec<(Uuid, Decimal, Decimal, Decimal)> = Vec::with_capacity(r.lines.len());
        for l in &r.lines {
            // Heal the quant surface if the stock predates the converged model (Bins
            // without quants) — the door reads and writes the quant grain.
            {
                let mut tx = self.db_pool.begin().await?;
                company_scope::bind_company_on(&mut tx, r.company_id).await?;
                self.pickings.ensure_quant_surface(
                    &mut tx, r.company_id, l.item_id, stock_location, Some(r.warehouse_id),
                ).await?;
                tx.commit().await?;
            }
            // The pre-move moving average: the value-diff the door produces (the count is
            // valued at the current average — a counted RATE is refused at the door).
            let rate = {
                let mut tx = self.db_pool.begin().await?;
                company_scope::bind_company_on(&mut tx, r.company_id).await?;
                let bal = self.bins.lock_or_init(&mut tx, r.company_id, l.item_id, r.warehouse_id).await?;
                tx.commit().await?;
                bal.valuation_rate
            };
            let staged = self.stage_quant_count(r.company_id, l.item_id, stock_location, l.counted_qty).await?;
            let outcome = self.apply_inventory(
                r.company_id,
                QuantSelector::ItemLocation { item_id: l.item_id, location_id: stock_location },
                &MoveGlDirective::default(), // no per-move GL: the voucher posts the one net envelope
                sink,
            ).await?;
            let qty_diff = if outcome.applied { outcome.diff_qty } else { staged.diff_qty };
            let value_diff = if qty_diff == Decimal::ZERO {
                Decimal::ZERO
            } else {
                money(qty_diff * rate)
            };
            net += value_diff;
            applied.push((l.item_id, l.counted_qty, qty_diff, value_diff));
        }

        // ---- voucher record: header + items + net, one transaction -----------------------
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, r.company_id).await?;
        let ins = self.recons.insert_submitted(&mut tx, &NewReconciliationRow {
            id,
            recon_number: &r.recon_number,
            company_id: r.company_id,
            warehouse_id: r.warehouse_id,
            posting_date: r.posting_date,
            currency: &r.currency,
            inventory_account_id: r.inventory_account_id,
            adjustment_account_id: r.adjustment_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.recon_number) } else { e.into() });
        }
        for (item_id, counted_qty, qty_diff, value_diff) in &applied {
            self.recon_items.insert_item(&mut tx, &NewReconciliationItemRow {
                id: Uuid::new_v4(),
                reconciliation_id: id,
                company_id: r.company_id,
                item_id: *item_id,
                counted_qty: *counted_qty,
                counted_rate: Decimal::ZERO,
                qty_difference: *qty_diff,
                value_difference: *value_diff,
            }).await?;
        }
        self.recons.update_net_difference(&mut tx, id, net).await?;
        tx.commit().await?;

        // ---- GL: the single value-diff envelope, exactly as the proven contract ------------
        if net != Decimal::ZERO {
            // net > 0: stock increased → Dr Inventory · Cr Adjustment; net < 0: the reverse.
            let (inv, adj) = (r.inventory_account_id, r.adjustment_account_id);
            let lines = if net > Decimal::ZERO {
                vec![GlPostLine::debit(inv, net).with_description("Inventory"), GlPostLine::credit(adj, net).with_description("Stock adjustment")]
            } else {
                let amt = -net;
                vec![GlPostLine::debit(adj, amt).with_description("Stock adjustment"), GlPostLine::credit(inv, amt).with_description("Inventory")]
            };
            let env = AccountingPostEnvelope {
                idempotency_key: id.to_string(), company_id: r.company_id, branch_id: None,
                source_type: "inventory".into(), source_id: id, source_reference: Some(r.recon_number.clone()),
                posting_date: r.posting_date, currency: r.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
                description: Some("Stock reconciliation".into()), lines,
            };
            self.emit_and_reconcile(crate::infrastructure::persistence::GlVoucher::StockReconciliation, id, &env, sink, net.abs()).await?;
        } else {
            company_scope::with_company_scope(
                Some(r.company_id),
                self.recons.mark_not_applicable(&self.db_pool, id),
            ).await?;
        }
        self.sink.publish(InventoryEvent::StockReconciled(StockReconciled {
            reconciliation_id: id, company_id: r.company_id, warehouse_id: r.warehouse_id, net_difference: net,
        }));
        Ok(id)
    }

    // ---- repost: re-drive a stuck reconciliation post -----------------------------

    /// Re-drive the GL leg for a reconciliation whose physical movement committed but whose post is
    /// `pending` or `failed`, OR — the case this exists for — a `net==0` recon that crashed between
    /// the voucher record and `mark_not_applicable` and is stuck in `(submitted, pending)`. Mirrors the
    /// receipt/delivery repost: rebuilds from the stored header, never re-touches the physical
    /// movement (the door's staging gate is consumed and the moves are done — the apply is a no-op,
    /// so a repost can never double-apply the counts).
    ///
    /// `net != 0` → re-emit the value-diff envelope (idempotent: accounting dedupes on
    /// `(company, source_type, source_id, posting_type)`); `net == 0` → reconcile to
    /// `not_applicable`. Already-`posted`/`not_applicable` short-circuits via `already_settled`.
    pub async fn repost_reconciliation(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let h = self.recons.fetch_repost_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h.gl, id) { return Ok(o); }

        if h.net_difference.is_zero() {
            // net==0 carries no value to post; the recovery is the mark_not_applicable that the
            // crash skipped. No event re-publish — the physical movement already committed.
            company_scope::with_company_scope(
                Some(h.company_id),
                self.recons.mark_not_applicable(&self.db_pool, id),
            ).await?;
            return Ok(SubmitOutcome {
                voucher_id: id, posted: false, journal_id: None, post_id: None, gl_amount: Decimal::ZERO,
            });
        }

        let net = h.net_difference;
        let (inv, adj) = (h.inventory_account_id, h.adjustment_account_id);
        let lines = if net > Decimal::ZERO {
            vec![GlPostLine::debit(inv, net).with_description("Inventory"), GlPostLine::credit(adj, net).with_description("Stock adjustment")]
        } else {
            let amt = -net;
            vec![GlPostLine::debit(adj, amt).with_description("Stock adjustment"), GlPostLine::credit(inv, amt).with_description("Inventory")]
        };
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: None,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.recon_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Stock reconciliation (repost)".into()), lines,
        };
        self.emit_and_reconcile(crate::infrastructure::persistence::GlVoucher::StockReconciliation, id, &env, sink, net.abs()).await
    }
}
