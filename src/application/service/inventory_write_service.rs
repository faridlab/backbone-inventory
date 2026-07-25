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

use backbone_orm::company_scope;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    BinRepository, DeliveryNoteItemRepository, DeliveryNoteRepository, GlSettlementState, GlVoucher,
    GlVoucherRepository, NewDeliveryItemRow, NewDeliveryRow, NewReceiptItemRow, NewReceiptRow,
    NewReconciliationItemRow, NewReconciliationRow, NewSleRow, NewStockEntryItemRow, NewStockItemRow,
    NewTransferRow, NewWarehouseRow, PurchaseReceiptItemRepository, PurchaseReceiptRepository,
    StockEntryItemRepository, StockEntryRepository, StockItemRepository, StockLedgerEntryRepository,
    StockReconciliationItemRepository, StockReconciliationRepository, WarehouseRepository,
};

use super::inventory_events::{
    InventoryEvent, InventoryEventSink, LoggingSink, StockDelivered, StockMoved, StockReceived,
    StockReconciled,
};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};

fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}
fn rate6(v: Decimal) -> Decimal {
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
fn is_dup(e: &sqlx::Error) -> bool {
    e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false)
}

#[derive(Clone)]
pub struct InventoryWriteService {
    db_pool: PgPool,
    sink: Arc<dyn InventoryEventSink>,
    warehouses: Arc<WarehouseRepository>,
    stock_items: Arc<StockItemRepository>,
    bins: Arc<BinRepository>,
    sles: Arc<StockLedgerEntryRepository>,
    receipts: Arc<PurchaseReceiptRepository>,
    receipt_items: Arc<PurchaseReceiptItemRepository>,
    deliveries: Arc<DeliveryNoteRepository>,
    delivery_items: Arc<DeliveryNoteItemRepository>,
    entries: Arc<StockEntryRepository>,
    entry_items: Arc<StockEntryItemRepository>,
    recons: Arc<StockReconciliationRepository>,
    recon_items: Arc<StockReconciliationItemRepository>,
    gl: Arc<GlVoucherRepository>,
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

    // ---- masters ------------------------------------------------------------

    pub async fn create_warehouse(&self, w: NewWarehouse) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let wt = w.warehouse_type.unwrap_or_else(|| "stock".into());
        // RLS scope (ADR-0008): company on the DTO.
        let r = company_scope::with_company_scope(
            Some(w.company_id),
            self.warehouses.insert_warehouse(&self.db_pool, &NewWarehouseRow {
                id,
                company_id: w.company_id,
                code: &w.code,
                name: &w.name,
                warehouse_type: &wt,
                parent_warehouse_id: w.parent_warehouse_id,
                is_group: w.is_group,
            }),
        ).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(w.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_stock_item(&self, s: NewStockItem) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let vm = s.valuation_method.unwrap_or_else(|| "moving_average".into());
        // RLS scope (ADR-0008): company on the DTO.
        let r = company_scope::with_company_scope(
            Some(s.company_id),
            self.stock_items.insert_stock_item(&self.db_pool, &NewStockItemRow {
                id,
                item_id: s.item_id,
                company_id: s.company_id,
                stock_uom: &s.stock_uom,
                valuation_method: &vm,
                reorder_level: s.reorder_level,
            }),
        ).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(s.item_id.to_string())),
            Err(e) => Err(e.into()),
        }
    }

    // ---- create (draft) documents ------------------------------------------

    pub async fn create_purchase_receipt(&self, r: NewReceipt) -> Result<Uuid, InventoryError> {
        if r.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &r.lines {
            if l.quantity < Decimal::ZERO || l.rate < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let total: Decimal = r.lines.iter().map(|l| money(l.quantity * l.rate)).sum();
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, r.company_id).await?;
        let ins = self.receipts.insert_draft(&mut tx, &NewReceiptRow {
            id,
            receipt_number: &r.receipt_number,
            company_id: r.company_id,
            branch_id: r.branch_id,
            supplier_id: r.supplier_id,
            source_po_id: r.source_po_id,
            warehouse_id: r.warehouse_id,
            posting_date: r.posting_date,
            total_value: total,
            inventory_account_id: r.inventory_account_id,
            grir_account_id: r.grir_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.receipt_number) } else { e.into() });
        }
        for l in &r.lines {
            self.receipt_items.insert_item(&mut tx, &NewReceiptItemRow {
                id: Uuid::new_v4(),
                receipt_id: id,
                company_id: r.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
                rate: l.rate,
                amount: money(l.quantity * l.rate),
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn create_delivery_note(&self, d: NewDelivery) -> Result<Uuid, InventoryError> {
        if d.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &d.lines {
            if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, d.company_id).await?;
        let ins = self.deliveries.insert_draft(&mut tx, &NewDeliveryRow {
            id,
            delivery_number: &d.delivery_number,
            company_id: d.company_id,
            branch_id: d.branch_id,
            customer_id: d.customer_id,
            source_so_id: d.source_so_id,
            warehouse_id: d.warehouse_id,
            posting_date: d.posting_date,
            cogs_account_id: d.cogs_account_id,
            inventory_account_id: d.inventory_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(d.delivery_number) } else { e.into() });
        }
        for l in &d.lines {
            self.delivery_items.insert_item(&mut tx, &NewDeliveryItemRow {
                id: Uuid::new_v4(),
                delivery_id: id,
                company_id: d.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- submit: Purchase Receipt (asset post) -----------------------------

    pub async fn submit_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let hdr = self.receipts.fetch_submit_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if hdr.status != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company = hdr.company_id;
        let branch = hdr.branch_id;
        let warehouse = hdr.warehouse_id;
        let posting_date = hdr.posting_date;
        let voucher_no = hdr.receipt_number;
        let inv_acct = hdr.inventory_account_id;
        let grir_acct = hdr.grir_account_id;
        let source_po = hdr.source_po_id;

        let items = company_scope::with_company_scope(
            Some(company),
            self.receipt_items.fetch_items(&self.db_pool, id),
        ).await?;

        // ---- physical movement: SLE + Bin, one transaction ----
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty.
        company_scope::bind_company_on(&mut tx, company).await?;
        let mut total_debit = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let (item, qty, in_rate) = (it.item_id, it.quantity, it.rate);
            let bin = self.bins.lock_or_init(&mut tx, company, item, warehouse).await?;
            let in_amount = money(qty * in_rate);
            let new_qty = bin.actual_qty + qty;
            let new_value = bin.stock_value + in_amount;
            let new_rate = if new_qty > Decimal::ZERO { rate6(new_value / new_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: company, item_id: item, warehouse_id: warehouse, posting_date,
                actual_qty: qty, qty_after_txn: new_qty, incoming_rate: in_rate,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: in_amount,
                voucher_type: "purchase_receipt", voucher_id: id, voucher_no: &voucher_no, sle_no,
            }).await?;
            total_debit += in_amount;
        }
        self.receipts.mark_submitted(&mut tx, id).await?;
        tx.commit().await?;

        // ---- GL post (eventually consistent) ----
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Goods receipt".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, total_debit).with_description("Inventory"),
                GlPostLine::credit(grir_acct, total_debit).with_description("GR/IR clearing"),
            ],
        };
        let outcome = self.emit_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, total_debit).await?;
        self.sink.publish(InventoryEvent::StockReceived(StockReceived {
            receipt_id: id, company_id: company, warehouse_id: warehouse, source_po_id: source_po,
            total_value: total_debit,
        }));
        Ok(outcome)
    }

    // ---- submit: Delivery Note (COGS post) ---------------------------------

    pub async fn submit_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let hdr = self.deliveries.fetch_submit_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if hdr.status != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company = hdr.company_id;
        let branch = hdr.branch_id;
        let warehouse = hdr.warehouse_id;
        let posting_date = hdr.posting_date;
        let voucher_no = hdr.delivery_number;
        let cogs_acct = hdr.cogs_account_id;
        let inv_acct = hdr.inventory_account_id;
        let source_so = hdr.source_so_id;

        let items = company_scope::with_company_scope(
            Some(company),
            self.delivery_items.fetch_items(&self.db_pool, id),
        ).await?;

        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty (and every delivery
        // would then fail the availability check).
        company_scope::bind_company_on(&mut tx, company).await?;
        let mut total_cogs = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let (line_id, item, qty) = (it.id, it.item_id, it.quantity);
            let bin = self.bins.lock_or_init(&mut tx, company, item, warehouse).await?;
            if bin.actual_qty < qty {
                return Err(InventoryError::InsufficientStock { item_id: item, warehouse_id: warehouse, available: bin.actual_qty, requested: qty });
            }
            let new_qty = bin.actual_qty - qty;
            // On the FINAL outflow (bin drained to 0), the last units absorb the moving-average
            // rounding residual: COGS = the entire remaining value, so stock_value returns to
            // EXACTLY 0 and the Inventory subledger ties out with the GL at zero stock (council
            // 2026-07-04). Otherwise COGS consumes the current 2dp-rounded average.
            let cogs = if new_qty.is_zero() { bin.stock_value } else { money(qty * bin.valuation_rate) };
            let new_value = bin.stock_value - cogs;
            // Moving-average: rate is unchanged by an outflow.
            let new_rate = if new_qty > Decimal::ZERO { bin.valuation_rate } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            self.delivery_items.update_valuation(&mut tx, line_id, bin.valuation_rate, cogs).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: company, item_id: item, warehouse_id: warehouse, posting_date,
                actual_qty: -qty, qty_after_txn: new_qty, incoming_rate: Decimal::ZERO,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: -cogs,
                voucher_type: "delivery_note", voucher_id: id, voucher_no: &voucher_no, sle_no,
            }).await?;
            total_cogs += cogs;
        }
        self.deliveries.mark_submitted_with_cogs(&mut tx, id, total_cogs).await?;
        tx.commit().await?;

        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS".into()),
            lines: vec![
                GlPostLine::debit(cogs_acct, total_cogs).with_description("COGS"),
                GlPostLine::credit(inv_acct, total_cogs).with_description("Inventory"),
            ],
        };
        let outcome = self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, total_cogs).await?;
        self.sink.publish(InventoryEvent::StockDelivered(StockDelivered {
            delivery_id: id, company_id: company, warehouse_id: warehouse, source_so_id: source_so,
            total_cogs,
        }));
        Ok(outcome)
    }

    // ---- submit: Stock Entry (transfer — value-neutral, no GL) --------------

    pub async fn submit_transfer(&self, t: NewTransfer) -> Result<Uuid, InventoryError> {
        if t.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        if t.from_warehouse_id == t.to_warehouse_id { return Err(InventoryError::SameWarehouse); }
        for l in &t.lines { if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); } }

        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty.
        company_scope::bind_company_on(&mut tx, t.company_id).await?;
        let ins = self.entries.insert_transfer(&mut tx, &NewTransferRow {
            id,
            entry_number: &t.entry_number,
            company_id: t.company_id,
            from_warehouse_id: t.from_warehouse_id,
            to_warehouse_id: t.to_warehouse_id,
            posting_date: t.posting_date,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(t.entry_number) } else { e.into() });
        }
        let mut sle_no = 0i32;
        for l in &t.lines {
            self.entry_items.insert_item(&mut tx, &NewStockEntryItemRow {
                id: Uuid::new_v4(),
                entry_id: id,
                company_id: t.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
            // OUT of source — the SOURCE bin is always locked before the target. Both bins are locked
            // on this one transaction; keep this order (it is what stops two transfers touching the
            // same pair of warehouses from deadlocking against each other).
            let from = self.bins.lock_or_init(&mut tx, t.company_id, l.item_id, t.from_warehouse_id).await?;
            if from.actual_qty < l.quantity {
                return Err(InventoryError::InsufficientStock { item_id: l.item_id, warehouse_id: t.from_warehouse_id, available: from.actual_qty, requested: l.quantity });
            }
            let from_qty = from.actual_qty - l.quantity;
            // Same residual flush as delivery: when the source bin drains to 0, the move carries
            // its entire remaining value so the source ends at exactly 0 (and the target receives
            // the exact value moved — total value conserved to the cent).
            let move_value = if from_qty.is_zero() { from.stock_value } else { money(l.quantity * from.valuation_rate) };
            let from_value = from.stock_value - move_value;
            let from_rate = if from_qty > Decimal::ZERO { from.valuation_rate } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, t.company_id, l.item_id, t.from_warehouse_id, from_qty, from_rate, from_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: t.company_id, item_id: l.item_id, warehouse_id: t.from_warehouse_id,
                posting_date: t.posting_date, actual_qty: -l.quantity, qty_after_txn: from_qty,
                incoming_rate: Decimal::ZERO, valuation_rate: from_rate, stock_value: from_value,
                stock_value_difference: -move_value, voucher_type: "stock_entry", voucher_id: id,
                voucher_no: &t.entry_number, sle_no,
            }).await?;
            // IN to target at the source rate (value conserved)
            let to = self.bins.lock_or_init(&mut tx, t.company_id, l.item_id, t.to_warehouse_id).await?;
            let to_qty = to.actual_qty + l.quantity;
            let to_value = to.stock_value + move_value;
            let to_rate = if to_qty > Decimal::ZERO { rate6(to_value / to_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, t.company_id, l.item_id, t.to_warehouse_id, to_qty, to_rate, to_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: t.company_id, item_id: l.item_id, warehouse_id: t.to_warehouse_id,
                posting_date: t.posting_date, actual_qty: l.quantity, qty_after_txn: to_qty,
                incoming_rate: from.valuation_rate, valuation_rate: to_rate, stock_value: to_value,
                stock_value_difference: move_value, voucher_type: "stock_entry", voucher_id: id,
                voucher_no: &t.entry_number, sle_no,
            }).await?;
        }
        tx.commit().await?;
        self.sink.publish(InventoryEvent::StockMoved(StockMoved {
            entry_id: id, company_id: t.company_id, from_warehouse_id: Some(t.from_warehouse_id), to_warehouse_id: Some(t.to_warehouse_id),
        }));
        Ok(id)
    }

    // ---- submit: Stock Reconciliation (value-difference post) ---------------

    pub async fn submit_reconciliation(&self, r: NewReconciliation, sink: &dyn GlPostSink) -> Result<Uuid, InventoryError> {
        if r.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &r.lines {
            if l.counted_qty < Decimal::ZERO || l.counted_rate < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty and the count would
        // book the entire counted quantity as a difference.
        company_scope::bind_company_on(&mut tx, r.company_id).await?;
        let ins = self.recons.insert_submitted(&mut tx, &NewReconciliationRow {
            id,
            recon_number: &r.recon_number,
            company_id: r.company_id,
            warehouse_id: r.warehouse_id,
            posting_date: r.posting_date,
            inventory_account_id: r.inventory_account_id,
            adjustment_account_id: r.adjustment_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.recon_number) } else { e.into() });
        }
        let mut net = Decimal::ZERO;
        let mut sle_no = 0i32;
        for l in &r.lines {
            let bin = self.bins.lock_or_init(&mut tx, r.company_id, l.item_id, r.warehouse_id).await?;
            let target_rate = if l.counted_rate > Decimal::ZERO { l.counted_rate } else { bin.valuation_rate };
            let new_value = money(l.counted_qty * target_rate);
            let value_diff = new_value - bin.stock_value;
            let qty_diff = l.counted_qty - bin.actual_qty;
            self.bins.update_balance(&mut tx, r.company_id, l.item_id, r.warehouse_id, l.counted_qty, target_rate, new_value).await?;
            self.recon_items.insert_item(&mut tx, &NewReconciliationItemRow {
                id: Uuid::new_v4(),
                reconciliation_id: id,
                company_id: r.company_id,
                item_id: l.item_id,
                counted_qty: l.counted_qty,
                counted_rate: l.counted_rate,
                qty_difference: qty_diff,
                value_difference: value_diff,
            }).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: r.company_id, item_id: l.item_id, warehouse_id: r.warehouse_id,
                posting_date: r.posting_date, actual_qty: qty_diff, qty_after_txn: l.counted_qty,
                incoming_rate: Decimal::ZERO, valuation_rate: target_rate, stock_value: new_value,
                stock_value_difference: value_diff, voucher_type: "stock_reconciliation",
                voucher_id: id, voucher_no: &r.recon_number, sle_no,
            }).await?;
            net += value_diff;
        }
        self.recons.update_net_difference(&mut tx, id, net).await?;
        tx.commit().await?;

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
                posting_date: r.posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
                description: Some("Stock reconciliation".into()), lines,
            };
            self.emit_and_reconcile(GlVoucher::StockReconciliation, id, &env, sink, net.abs()).await?;
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

    // ---- repost: re-drive a stuck GL post (pending/failed) ------------------

    /// Re-emit the GL post for a voucher whose physical movement committed but whose post is
    /// `pending` or `failed` (a transient accounting outage, or a crash between commit and the
    /// status update). This is the exit from a stuck `failed` voucher (council 2026-07-04) — without
    /// it, `(submitted, failed)` is terminal and the subledger silently stops tying to the GL.
    ///
    /// **Idempotent** against the "physically-posted-but-status-not-updated" crash window: it rebuilds
    /// the SAME envelope (`source_id = voucher_id`), and accounting dedupes on
    /// `(company, source_type, source_id, posting_type)` — so a re-emit of an already-posted voucher
    /// returns the original journal, never a second one. An already-`posted` voucher short-circuits
    /// (returns the recorded ids); `not_applicable` is a no-op. Rebuilds the envelope from the stored
    /// header (never re-touches the SLE/Bin — the physical movement already happened).
    pub async fn repost_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let h = self.receipts.fetch_repost_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h.gl, id) { return Ok(o); }
        let amt = h.total_value;
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number),
            posting_date: h.posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Goods receipt (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.inventory_account_id, amt).with_description("Inventory"),
                GlPostLine::credit(h.grir_account_id, amt).with_description("GR/IR clearing"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, amt).await
    }

    pub async fn repost_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let h = self.deliveries.fetch_repost_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h.gl, id) { return Ok(o); }
        let amt = h.total_cogs;
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number),
            posting_date: h.posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.cogs_account_id, amt).with_description("COGS"),
                GlPostLine::credit(h.inventory_account_id, amt).with_description("Inventory"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, amt).await
    }

    /// Short-circuit a repost when the voucher is already settled: `posted` → return the recorded
    /// ids; `not_applicable` → no-op. Returns None when a (re-)emit is actually needed.
    fn already_settled(gl: &GlSettlementState, id: Uuid) -> Option<SubmitOutcome> {
        match gl.posting_state.as_str() {
            "posted" => Some(SubmitOutcome {
                voucher_id: id, posted: true,
                journal_id: gl.journal_id, post_id: gl.accounting_post_id, gl_amount: Decimal::ZERO,
            }),
            "not_applicable" => Some(SubmitOutcome { voucher_id: id, posted: false, journal_id: None, post_id: None, gl_amount: Decimal::ZERO }),
            _ => None,
        }
    }

    // ---- shared GL emit + reconcile ----------------------------------------

    /// Emit the envelope through `sink` and reconcile the voucher's posting_state. The physical
    /// movement is already committed; on GL failure the voucher is `failed` — now re-drivable via
    /// `repost_*` (never rolled back).
    async fn emit_and_reconcile(
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
}
