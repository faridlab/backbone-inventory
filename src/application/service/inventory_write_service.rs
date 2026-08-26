//! Validated write path + valuation engine for inventory (hand-authored, user-owned).
//!
//! The core is the **moving-average valuation engine** over an append-only Stock Ledger (SLE) and a
//! per-(item,warehouse) `Bin` running balance:
//!   - **Receipt:** `value += qty·rate; qty += qty; rate = value/qty` — new cost blends in.
//!   - **Delivery:** `cogs = qty · rate; value -= cogs; qty -= qty` — rate is unchanged by an
//!     outflow; COGS consumes the current average.
//!   - **Transfer:** paired out/in SLE at the source rate — value-neutral, no GL.
//!   - **Reconciliation:** set qty/value to the counted figures; the delta is the posted difference.
//! Every valuation-changing movement writes an immutable SLE and emits a balanced `AccountingPost`
//! (Dr Inventory·Cr GR/IR on receipt; Dr COGS·Cr Inventory on delivery; the signed value diff on
//! reconciliation). The physical movement (SLE+Bin) commits first; the GL post is eventually
//! consistent (`posting_state` pending→posted|failed), per the GL-posting contract.
//!
//! Money: `stock_value`/GL amounts are 2dp (half-up); `valuation_rate` is 6dp.
//!
//! **Layering (the module's 4-layer rule).** This service ORCHESTRATES: it owns the valuation
//! arithmetic, the unit of work (`begin`/`commit`), the ORDER the bin locks are taken in, the
//! company-scope decisions (ADR-0008) and the seam events. It holds no SQL — every statement lives
//! in `infrastructure::persistence`, and the repository methods that participate in a movement take
//! THIS service's connection so the SLE + Bin writes commit together with their voucher.
//!
//! **This file is the hub:** it holds the module's vocabulary (input structs, outcomes, errors), the
//! ctor, and the shared GL-emit/reconcile helper used by every submit/repost path. The write surface
//! is chunked into focused siblings, each an `impl InventoryWriteService` block over these same
//! types:
//!
//! - [`super::inventory_masters`] — warehouse + stock-item setup (`create_warehouse`,
//!   `create_stock_item`).
//! - [`super::inventory_receipt`] — Purchase Receipt: draft → submit → repost.
//! - [`super::inventory_delivery`] — Delivery Note: draft → submit → repost.
//! - [`super::inventory_transfer`] — Stock Entry (warehouse-to-warehouse move; value-neutral, no GL).
//! - [`super::inventory_reconciliation`] — Stock Reconciliation (physical count → value diff).

use backbone_orm::company_scope;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    BinRepository, DeliveryNoteItemRepository, DeliveryNoteRepository, GlSettlementState, GlVoucher,
    GlVoucherRepository, PurchaseReceiptItemRepository, PurchaseReceiptRepository,
    QuantRepository, StockAdjustmentRepository, StockEntryItemRepository, StockEntryRepository,
    StockItemRepository, StockLedgerEntryRepository, StockMoveLineRepository, StockMoveRepository,
    StockPickingRepository, StockReconciliationItemRepository, StockReconciliationRepository,
    ValuationOverlayRepository, WarehouseRepository,
};

use super::inventory_events::{InventoryEventSink, LoggingSink};
use super::inventory_gl::{AccountingPostEnvelope, GlPostSink};

pub(super) fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}
pub(super) fn rate6(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(6, RoundingStrategy::MidpointAwayFromZero)
}

// --- input structs -----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NewWarehouse {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub warehouse_type: Option<String>,
    pub parent_warehouse_id: Option<Uuid>,
    pub is_group: bool,
}

#[derive(Debug, Clone)]
pub struct NewStockItem {
    pub item_id: Uuid,
    pub company_id: Uuid,
    pub stock_uom: String,
    pub valuation_method: Option<String>,
    pub reorder_level: Decimal,
}

#[derive(Debug, Clone)]
pub struct ReceiptLine {
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub rate: Decimal,
    /// Landed-cost service line (the seam is owned by inventory): a flagged line carries cost
    /// into a LandedCost document, NOT stock — the receipt door skips move-minting, the
    /// document totals and the cancellation for it. Default `false` everywhere.
    pub is_landed_costs_line: bool,
}
#[derive(Debug, Clone)]
pub struct NewReceipt {
    pub receipt_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub supplier_id: Uuid,
    pub source_po_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    pub currency: String,
    pub inventory_account_id: Uuid,
    pub grir_account_id: Uuid,
    pub lines: Vec<ReceiptLine>,
}

#[derive(Debug, Clone)]
pub struct DeliveryLine {
    pub item_id: Uuid,
    pub quantity: Decimal,
}
#[derive(Debug, Clone)]
pub struct NewDelivery {
    pub delivery_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub source_so_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    pub currency: String,
    pub cogs_account_id: Uuid,
    pub inventory_account_id: Uuid,
    pub lines: Vec<DeliveryLine>,
}

#[derive(Debug, Clone)]
pub struct NewTransfer {
    pub entry_number: String,
    pub company_id: Uuid,
    pub from_warehouse_id: Uuid,
    pub to_warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    pub lines: Vec<DeliveryLine>, // item_id + quantity
}

#[derive(Debug, Clone)]
pub struct ReconLine {
    pub item_id: Uuid,
    pub counted_qty: Decimal,
    pub counted_rate: Decimal, // 0 = keep current rate
}
#[derive(Debug, Clone)]
pub struct NewReconciliation {
    pub recon_number: String,
    pub company_id: Uuid,
    pub warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    pub currency: String,
    pub inventory_account_id: Uuid,
    pub adjustment_account_id: Uuid,
    pub lines: Vec<ReconLine>,
}

/// Outcome of submitting a movement that posts to the GL.
#[derive(Debug, Clone)]
pub struct SubmitOutcome {
    pub voucher_id: Uuid,
    pub posted: bool,
    pub journal_id: Option<Uuid>,
    pub post_id: Option<Uuid>,
    pub gl_amount: Decimal,
}

/// One landed-cost charge line of a draft landed cost. `account_id` is the credit side
/// (required — a line without it is rejected); `split_method` is `quantity` | `value` |
/// `weight`; `amount` may be negative — a negative landed cost is the REVERSAL pattern
/// (a validated landed cost can never cancel, so corrections re-book with swapped legs).
#[derive(Debug, Clone)]
pub struct LcCostLine {
    pub name: String,
    pub account_id: Uuid,
    pub split_method: String,
    pub amount: Decimal,
}

/// A draft landed-cost document: one target purchase receipt + its cost lines. The document
/// revalues the target receipt's DONE moves at validation — see
/// [`super::landed_cost_service_custom`].
#[derive(Debug, Clone)]
pub struct NewLandedCost {
    pub lc_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub target_receipt_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    pub currency: String,
    pub notes: Option<String>,
    pub lines: Vec<LcCostLine>,
}

// --- errors ------------------------------------------------------------------

#[derive(Debug)]
pub enum InventoryError {
    EmptyDocument,
    NegativeQuantity,
    InsufficientStock { item_id: Uuid, warehouse_id: Uuid, available: Decimal, requested: Decimal },
    DuplicateNumber(String),
    NotFound(Uuid),
    NotDraft(String),
    SameWarehouse,
    /// A cancellation was attempted on a voucher whose status is not `submitted` (e.g. a draft —
    /// delete it instead — or an already-cancelled one that has no GL post to recover).
    NotSubmitted(Uuid),
    /// A cancellation needs the original GL post to be `posted` (so it can be reversed); the original
    /// is still `pending`/`failed`. Repost the voucher first.
    GlNotPosted(Uuid),
    /// A receipt cancellation would push a bin's quantity below zero (the received stock was already
    /// issued). Reverse the issue first, or correct via a reconciliation.
    InsufficientStockToReverse { item_id: Uuid, warehouse_id: Uuid, available: Decimal, requested: Decimal },
    GlRejected { code: String, message: String },
    // ---- stock-move pipeline (the converged lifecycle engine) -----------------------------
    /// The move is not in the state this action requires (the guarded state machine rejected the
    /// transition — e.g. `_action_assign` on a draft move, or `_action_done` on a cancelled one).
    WrongMoveState { move_id: Uuid, action: &'static str, current: String },
    /// A move whose source and destination location are the same row (a physical no-op — rejected,
    /// stock.hook.yaml R9).
    SameLocation { move_id: Uuid, location_id: Uuid },
    /// `_action_done` requires at least one execution line (the picking's done-needs-lines guard,
    /// R24 at move grain).
    MoveLinesRequired { move_id: Uuid },
    /// Every done line must carry a non-negative quantity and the move a positive total (R23)
    /// — reuses [`InventoryError::NegativeQuantity`] for the plain negative case.
    /// The source location of a quant write is a view location (view nodes hold no stock — R13).
    ViewLocationHoldsNoStock { location_id: Uuid },
    /// The location a move/quant references does not exist (or is soft-deleted).
    LocationNotFound(Uuid),
    /// A quant's company must follow its location's company (R26/T3 — derived at write time).
    QuantCompanyMismatch { location_id: Uuid, location_company: Option<Uuid>, move_company: Uuid },
    // ---- quant-driven adjustment door (spec §5.2 — no stock.inventory model) ----------------
    /// A count cannot be staged or applied while the quant holds reservations (stock.hook.yaml
    /// R24 `no_count_while_reserved`) — release the reservations first.
    CountReserved { quant_id: Uuid, reserved_qty: Decimal },
    /// The staged count is stale: the quant's on-hand moved between the count and the apply
    /// (spec §5.2 `is_outdated` — a move landed in the window; the user must re-count).
    OutdatedCount { quant_id: Uuid, counted_qty: Decimal, on_hand_qty: Decimal },
    /// The reconciliation write surface only carries counted QUANTITY; an explicit counted
    /// RATE is a valuation-overlay concern (the quant-driven door values the diff at the
    /// current moving average — never a silent rate revaluation).
    CountedRateUnsupported,
    // ---- valuation overlay (per-company posting posture + landed costs) -------------------
    /// The company's anglo-saxon delivery-debit posture is ON but no stock interim delivered
    /// account is configured. Fail-closed: the delivery door refuses to operate (no envelope,
    /// no partial post) until the account is set — a silent COGS fallback would post the
    /// interim leg to the wrong account on every delivery.
    AngloPostureUnconfigured { company_id: Uuid },
    /// A landed-cost validation found no DONE moves under its target receipt — nothing to
    /// revalue (the receipt was never submitted, or its lines were all zero / landed-cost
    /// service lines that mint no stock).
    LandedCostNoValuedTargets { receipt_id: Uuid },
    /// A landed-cost line carries no credit account (the split's credit side is undefined).
    LandedCostLineNeedsAccount { line_id: Uuid },
    /// The company's cost method is `standard`: landed costs refuse to validate loudly. Under
    /// standard costing a receipt's value comes from the item's standard price and a landed
    /// cost would introduce a variance the standard-recompute engine (a later increment)
    /// would have to absorb — refusing beats silently diverging.
    LandedCostRequiresCostMethod { company_id: Uuid, cost_method: String },
    /// The split basis sums to zero across every target line (e.g. a `weight` split where no
    /// item carries a per-unit weight, or a `value` split over zero-valued lines). This is a
    /// LOUD rejection by decision: the historical silent equal-split fallback masked
    /// misconfigured weight data. No fallback, no partial worksheet rows — the transaction
    /// rolls back.
    LandedCostZeroSplitBasis { basis: String, target_receipt_id: Uuid },
    /// The landed cost is not in `draft` (validate and cancel only ever start from `draft`;
    /// a `done` landed cost can never cancel — the reversal pattern is a negative-amount
    /// landed cost).
    LandedCostNotDraft { lc_id: Uuid, state: String },
    /// A draft landed cost with no cost lines has nothing to allocate.
    LandedCostNoLines { lc_id: Uuid },
    /// A target receipt line resolves no inventory valuation account for the landed-cost
    /// debit leg (neither a per-location override nor the receipt's header inventory
    /// account).
    LandedCostNoValuationAccount { move_id: Uuid },
    /// The worksheet's Σ allocations does not equal the Σ cost line amounts — an internal
    /// arithmetic invariant broke (each cost line's shares, with the last target line eating
    /// the rounding diff, must sum to exactly its amount).
    LandedCostAllocationMismatch { lc_id: Uuid, allocated: Decimal, declared: Decimal },
    Db(sqlx::Error),
}

impl InventoryError {
    pub fn code(&self) -> String {
        match self {
            InventoryError::EmptyDocument => "empty_document".into(),
            InventoryError::NegativeQuantity => "negative_quantity".into(),
            InventoryError::InsufficientStock { .. } => "insufficient_stock".into(),
            InventoryError::DuplicateNumber(_) => "duplicate_number".into(),
            InventoryError::NotFound(_) => "not_found".into(),
            InventoryError::NotDraft(_) => "not_draft".into(),
            InventoryError::NotSubmitted(_) => "not_submitted".into(),
            InventoryError::GlNotPosted(_) => "gl_not_posted".into(),
            InventoryError::InsufficientStockToReverse { .. } => "insufficient_stock_to_reverse".into(),
            InventoryError::SameWarehouse => "same_warehouse".into(),
            InventoryError::GlRejected { code, .. } => code.clone(),
            InventoryError::WrongMoveState { .. } => "wrong_move_state".into(),
            InventoryError::SameLocation { .. } => "same_location".into(),
            InventoryError::MoveLinesRequired { .. } => "move_lines_required".into(),
            InventoryError::ViewLocationHoldsNoStock { .. } => "quant_on_view_location".into(),
            InventoryError::LocationNotFound(_) => "location_not_found".into(),
            InventoryError::QuantCompanyMismatch { .. } => "quant_company_mismatch".into(),
            InventoryError::CountReserved { .. } => "quant_reserved".into(),
            InventoryError::OutdatedCount { .. } => "outdated_count".into(),
            InventoryError::CountedRateUnsupported => "counted_rate_unsupported".into(),
            InventoryError::AngloPostureUnconfigured { .. } => "anglo_posture_unconfigured".into(),
            InventoryError::LandedCostNoValuedTargets { .. } => "landed_cost_no_valued_targets".into(),
            InventoryError::LandedCostLineNeedsAccount { .. } => "landed_cost_line_needs_account".into(),
            InventoryError::LandedCostRequiresCostMethod { .. } => "landed_cost_requires_cost_method".into(),
            InventoryError::LandedCostZeroSplitBasis { .. } => "landed_cost_zero_split_basis".into(),
            InventoryError::LandedCostNotDraft { .. } => "landed_cost_not_draft".into(),
            InventoryError::LandedCostNoLines { .. } => "landed_cost_no_lines".into(),
            InventoryError::LandedCostNoValuationAccount { .. } => "landed_cost_no_valuation_account".into(),
            InventoryError::LandedCostAllocationMismatch { .. } => "landed_cost_allocation_mismatch".into(),
            InventoryError::Db(_) => "internal_error".into(),
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            InventoryError::NotFound(_) => 404,
            InventoryError::Db(_) => 500,
            _ => 422,
        }
    }
}
impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InventoryError::GlRejected { code, message } => write!(f, "{code}: {message}"),
            other => write!(f, "{}", other.code()),
        }
    }
}
impl std::error::Error for InventoryError {}
impl From<sqlx::Error> for InventoryError {
    fn from(e: sqlx::Error) -> Self { InventoryError::Db(e) }
}

/// Discriminate a unique violation out of a raw `sqlx::Error`.
///
/// This is why the repositories' write methods leak `sqlx::Error` rather than a typed repo error: the
/// service turns a re-used receipt number into `DuplicateNumber`, and a typed error would have thrown
/// that information away.
pub(super) fn is_dup(e: &sqlx::Error) -> bool {
    e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false)
}

/// The GL-posting tripwire for voucher doors whose legs are minted through the move engine.
/// A re-wired door (receipt / delivery / their cancels) owns ONE GL post per voucher — its
/// envelope, `posting_state`, repost and reversal machinery key on the voucher id — so its
/// moves are minted with an empty [`super::inventory_move_engine::MoveGlDirective`] (a leg
/// whose accounts are absent posts nothing). This sink exists so a move can never post GL
/// SILENTLY under such a door: if a directive ever grows accounts and the engine builds an
/// envelope, the loud rejection surfaces it instead of a phantom second journal.
pub(in crate::application::service) struct DoorOwnedGlSink;
#[async_trait::async_trait]
impl super::inventory_gl::GlPostSink for DoorOwnedGlSink {
    async fn post(
        &self,
        _e: &super::inventory_gl::AccountingPostEnvelope,
    ) -> Result<super::inventory_gl::GlPostAck, super::inventory_gl::GlPostRejected> {
        Err(super::inventory_gl::GlPostRejected {
            code: "gl_owned_by_voucher_door".into(),
            message: "voucher doors post one door-owned GL envelope; their moves post none".into(),
        })
    }
}

#[derive(Clone)]
pub struct InventoryWriteService {
    pub(super) db_pool: PgPool,
    pub(super) sink: Arc<dyn InventoryEventSink>,
    pub(super) warehouses: Arc<WarehouseRepository>,
    pub(super) stock_items: Arc<StockItemRepository>,
    pub(super) bins: Arc<BinRepository>,
    pub(super) sles: Arc<StockLedgerEntryRepository>,
    pub(super) receipts: Arc<PurchaseReceiptRepository>,
    pub(super) receipt_items: Arc<PurchaseReceiptItemRepository>,
    pub(super) deliveries: Arc<DeliveryNoteRepository>,
    pub(super) delivery_items: Arc<DeliveryNoteItemRepository>,
    pub(super) entries: Arc<StockEntryRepository>,
    pub(super) entry_items: Arc<StockEntryItemRepository>,
    pub(super) recons: Arc<StockReconciliationRepository>,
    pub(super) recon_items: Arc<StockReconciliationItemRepository>,
    pub(super) gl: Arc<GlVoucherRepository>,
    // The stock-convergence engine's repositories (quant reservation apex, move lifecycle,
    // move-line mirror) — same construction pattern as the voucher doors above.
    pub(super) quants: Arc<QuantRepository>,
    pub(super) moves: Arc<StockMoveRepository>,
    pub(super) move_lines: Arc<StockMoveLineRepository>,
    // The picking-as-projection + quant-driven adjustment companions (transfer header mint,
    // projection probes, location resolution, quant-surface heal; count staging/consume).
    pub(super) pickings: Arc<StockPickingRepository>,
    pub(super) adjustments: Arc<StockAdjustmentRepository>,
    // The valuation overlay (per-company posting posture + landed-cost document family).
    // Stateless hand-owned SQL, same construction pattern as the GL voucher repository.
    pub(super) valuation_overlay: Arc<ValuationOverlayRepository>,
}

impl InventoryWriteService {
    pub fn new(db_pool: PgPool) -> Self {
        Self::with_sink(db_pool, Arc::new(LoggingSink))
    }
    pub fn with_sink(db_pool: PgPool, sink: Arc<dyn InventoryEventSink>) -> Self {
        Self {
            warehouses: Arc::new(WarehouseRepository::new(db_pool.clone())),
            stock_items: Arc::new(StockItemRepository::new(db_pool.clone())),
            bins: Arc::new(BinRepository::new(db_pool.clone())),
            sles: Arc::new(StockLedgerEntryRepository::new(db_pool.clone())),
            receipts: Arc::new(PurchaseReceiptRepository::new(db_pool.clone())),
            receipt_items: Arc::new(PurchaseReceiptItemRepository::new(db_pool.clone())),
            deliveries: Arc::new(DeliveryNoteRepository::new(db_pool.clone())),
            delivery_items: Arc::new(DeliveryNoteItemRepository::new(db_pool.clone())),
            entries: Arc::new(StockEntryRepository::new(db_pool.clone())),
            entry_items: Arc::new(StockEntryItemRepository::new(db_pool.clone())),
            recons: Arc::new(StockReconciliationRepository::new(db_pool.clone())),
            recon_items: Arc::new(StockReconciliationItemRepository::new(db_pool.clone())),
            gl: Arc::new(GlVoucherRepository::new()),
            quants: Arc::new(QuantRepository::new(db_pool.clone())),
            moves: Arc::new(StockMoveRepository::new(db_pool.clone())),
            move_lines: Arc::new(StockMoveLineRepository::new(db_pool.clone())),
            pickings: Arc::new(StockPickingRepository::new()),
            adjustments: Arc::new(StockAdjustmentRepository::new()),
            valuation_overlay: Arc::new(ValuationOverlayRepository::new()),
            db_pool,
            sink,
        }
    }

    // ---- shared: repost short-circuit + GL emit/reconcile ------------------

    /// Short-circuit a repost when the voucher is already settled: `posted` → return the recorded
    /// ids; `not_applicable` → no-op. Returns None when a (re-)emit is actually needed.
    pub(super) fn already_settled(gl: &GlSettlementState, id: Uuid) -> Option<SubmitOutcome> {
        match gl.posting_state.as_str() {
            "posted" => Some(SubmitOutcome {
                voucher_id: id, posted: true,
                journal_id: gl.journal_id, post_id: gl.accounting_post_id, gl_amount: Decimal::ZERO,
            }),
            "not_applicable" => Some(SubmitOutcome { voucher_id: id, posted: false, journal_id: None, post_id: None, gl_amount: Decimal::ZERO }),
            _ => None,
        }
    }

    /// Emit the envelope through `sink` and reconcile the voucher's posting_state. The physical
    /// movement is already committed; on GL failure the voucher is `failed` — now re-drivable via
    /// `repost_*` (never rolled back).
    pub(super) async fn emit_and_reconcile(
        &self, voucher: GlVoucher, voucher_id: Uuid, env: &AccountingPostEnvelope, sink: &dyn GlPostSink, gl_amount: Decimal,
    ) -> Result<SubmitOutcome, InventoryError> {
        debug_assert!(env.is_balanced());
        match sink.post(env).await {
            Ok(ack) => {
                company_scope::with_company_scope(
                    Some(env.company_id),
                    self.gl.mark_posted(&self.db_pool, voucher, voucher_id, ack.journal_id, ack.post_id),
                ).await?;
                Ok(SubmitOutcome { voucher_id, posted: true, journal_id: Some(ack.journal_id), post_id: Some(ack.post_id), gl_amount })
            }
            Err(rej) => {
                let _ = company_scope::with_company_scope(
                    Some(env.company_id),
                    self.gl.mark_failed(&self.db_pool, voucher, voucher_id),
                ).await;
                Err(InventoryError::GlRejected { code: rej.code, message: rej.message })
            }
        }
    }

    /// Emit a `posting_type='reversal'` post and record its ids in the voucher's
    /// `reversal_*` columns (council 2026-07-29, #3). Distinct from [`Self::emit_and_reconcile`]:
    /// the original post is already `posted` and its ids stay intact, so on success we call
    /// `mark_reversal_posted` (no `posting_state` guard); on failure we do NOT `mark_failed` (that
    /// would clobber the original's `posted` state) — the voucher stays `cancelled` with a NULL
    /// `reversal_accounting_post_id`, and re-calling cancel re-emits only this GL leg.
    pub(super) async fn emit_reversal_and_reconcile(
        &self, voucher: GlVoucher, voucher_id: Uuid, env: &AccountingPostEnvelope, sink: &dyn GlPostSink, gl_amount: Decimal,
    ) -> Result<SubmitOutcome, InventoryError> {
        debug_assert!(env.is_balanced());
        match sink.post(env).await {
            Ok(ack) => {
                company_scope::with_company_scope(
                    Some(env.company_id),
                    self.gl.mark_reversal_posted(&self.db_pool, voucher, voucher_id, ack.journal_id, ack.post_id),
                ).await?;
                Ok(SubmitOutcome { voucher_id, posted: true, journal_id: Some(ack.journal_id), post_id: Some(ack.post_id), gl_amount })
            }
            Err(rej) => Err(InventoryError::GlRejected { code: rej.code, message: rej.message }),
        }
    }

    // ---- shared: voucher doors minting their legs through the move engine ------------------
    //
    // The receipt/delivery voucher doors (and their cancel verbs) no longer write Bins or SLEs
    // directly: each line mints ONE stock move through the engine (create → confirm → assign →
    // done), so the quant flips, the Bin reblende and the SLE rows come from move application —
    // one stock estate, one writer. The voucher keeps its identity (header, item rows, events,
    // its ONE GL envelope + posting_state/repost/reversal machinery); the moves post no GL
    // (empty directive + the DoorOwnedGlSink tripwire above).

    /// Mint (or resume) the move a voucher line maps to. The mapping is the deterministic move
    /// name — `{voucher}/{seq}` for forward legs, `{voucher}/REV/{seq}` for reversal legs —
    /// looked up among the moves stamped with the voucher number as `origin`:
    ///
    /// - no move under the name → mint a fresh DRAFT move through the engine;
    /// - a DONE move → the line already landed in a prior (crashed) attempt — `None`, the
    ///   caller skips the line (a door is not cross-move atomic; the name mapping is what makes
    ///   a retry converge instead of double-moving stock);
    /// - a LIVE move (draft/confirmed/…) → hand it back for the caller to keep driving;
    /// - a CANCELLED move → refuse loudly: someone cancelled the voucher's leg by hand, and
    ///   silently re-minting under a new name would double-count the line.
    pub(in crate::application::service) async fn mint_line_move(
        &self,
        m: super::inventory_move_engine::NewStockMove,
    ) -> Result<Option<Uuid>, InventoryError> {
        let origin = m.origin.clone().unwrap_or_default();
        let existing = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, m.company_id).await?;
            let rows = self.moves.fetch_moves_by_origin(&mut tx, m.company_id, &origin).await?;
            tx.commit().await?;
            rows
        };
        if let Some(prior) = existing.iter().find(|mv| mv.name == m.name) {
            return match prior.state.as_str() {
                "done" => Ok(None),
                "cancel" => Err(InventoryError::WrongMoveState {
                    move_id: prior.id, action: "mint", current: "cancel".into(),
                }),
                _ => Ok(Some(prior.id)),
            };
        }
        Ok(Some(self.create_move(m).await?))
    }

    /// Advance a door-minted move to the most-reserved state it can reach: confirm (if still
    /// draft), then an assign pass (if confirmed/partial). Returns the post-advance state —
    /// `assigned` when the full demand is covered, less when not. The door decides what a
    /// short advance means (the delivery refuses; the inbound receipt never is).
    pub(in crate::application::service) async fn advance_move_to_assigned(
        &self,
        move_id: Uuid,
    ) -> Result<String, InventoryError> {
        let mut state = self.move_state_of(move_id).await?;
        if state == "draft" {
            self.action_confirm(move_id).await?;
            state = self.move_state_of(move_id).await?;
        }
        if state == "confirmed" || state == "partially_available" {
            let outcome = self.action_assign(move_id).await?;
            state = outcome.state;
        }
        Ok(state)
    }

    /// Read one move's current state under the company fence. The door drives the engine's
    /// verbs; this re-read between verbs is how it follows the state the ENGINE wrote (the
    /// door never derives or asserts move state itself).
    pub(in crate::application::service) async fn move_state_of(
        &self,
        move_id: Uuid,
    ) -> Result<String, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        company_scope::bind_company_on(&mut tx, mv.company_id).await?;
        tx.commit().await?;
        Ok(mv.state)
    }

    /// Resolve the endpoints a voucher door's line moves run between: the warehouse's stock
    /// location (internal, resolve-or-bootstrap) and the company's counterpart partner
    /// location (supplier for receipts, customer for deliveries — resolve-or-bootstrap).
    /// Returns `(partner_location_id, stock_location_id)`.
    pub(in crate::application::service) async fn door_move_endpoints(
        &self,
        company_id: Uuid,
        warehouse_id: Uuid,
        partner_usage: &str,
    ) -> Result<(Uuid, Uuid), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let partner = self.pickings.ensure_partner_location(&mut tx, company_id, partner_usage).await?;
        let stock = self.pickings.ensure_internal_location(&mut tx, warehouse_id, company_id).await?;
        tx.commit().await?;
        Ok((partner, stock))
    }
}
