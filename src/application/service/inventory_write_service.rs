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
    StockEntryItemRepository, StockEntryRepository, StockItemRepository, StockLedgerEntryRepository,
    StockReconciliationItemRepository, StockReconciliationRepository, WarehouseRepository,
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
}
