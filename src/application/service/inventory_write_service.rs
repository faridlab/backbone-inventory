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

use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

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

/// A Bin's running balance, loaded FOR UPDATE inside a movement transaction.
struct BinState {
    qty: Decimal,
    rate: Decimal,
    value: Decimal,
}

#[derive(Clone)]
pub struct InventoryWriteService {
    db_pool: PgPool,
    sink: Arc<dyn InventoryEventSink>,
}

impl InventoryWriteService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool, sink: Arc::new(LoggingSink) }
    }
    pub fn with_sink(db_pool: PgPool, sink: Arc<dyn InventoryEventSink>) -> Self {
        Self { db_pool, sink }
    }

    // ---- masters ------------------------------------------------------------

    pub async fn create_warehouse(&self, w: NewWarehouse) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let wt = w.warehouse_type.unwrap_or_else(|| "stock".into());
        let r = sqlx::query(
            r#"INSERT INTO inventory.warehouses
                (id, company_id, code, name, warehouse_type, parent_warehouse_id, is_group)
               VALUES ($1,$2,$3,$4,$5::warehouse_type,$6,$7)"#,
        )
        .bind(id).bind(w.company_id).bind(&w.code).bind(&w.name).bind(&wt)
        .bind(w.parent_warehouse_id).bind(w.is_group)
        .execute(&self.db_pool).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(w.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_stock_item(&self, s: NewStockItem) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let vm = s.valuation_method.unwrap_or_else(|| "moving_average".into());
        let r = sqlx::query(
            r#"INSERT INTO inventory.stock_items
                (id, item_id, company_id, stock_uom, is_stock_item, has_batch, valuation_method, reorder_level)
               VALUES ($1,$2,$3,$4,TRUE,FALSE,$5::valuation_method,$6)"#,
        )
        .bind(id).bind(s.item_id).bind(s.company_id).bind(&s.stock_uom).bind(&vm).bind(s.reorder_level)
        .execute(&self.db_pool).await;
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
        let ins = sqlx::query(
            r#"INSERT INTO inventory.purchase_receipts
                (id, receipt_number, company_id, branch_id, supplier_id, source_po_id, warehouse_id,
                 posting_date, currency, total_value, inventory_account_id, grir_account_id,
                 status, posting_state)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'IDR',$9,$10,$11,'draft'::doc_status,'pending'::gl_posting_state)"#,
        )
        .bind(id).bind(&r.receipt_number).bind(r.company_id).bind(r.branch_id).bind(r.supplier_id)
        .bind(r.source_po_id).bind(r.warehouse_id).bind(r.posting_date).bind(total)
        .bind(r.inventory_account_id).bind(r.grir_account_id)
        .execute(&mut *tx).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.receipt_number) } else { e.into() });
        }
        for l in &r.lines {
            sqlx::query(
                r#"INSERT INTO inventory.purchase_receipt_items (id, receipt_id, item_id, quantity, rate, amount)
                   VALUES ($1,$2,$3,$4,$5,$6)"#,
            )
            .bind(Uuid::new_v4()).bind(id).bind(l.item_id).bind(l.quantity).bind(l.rate).bind(money(l.quantity * l.rate))
            .execute(&mut *tx).await?;
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
        let ins = sqlx::query(
            r#"INSERT INTO inventory.delivery_notes
                (id, delivery_number, company_id, branch_id, customer_id, source_so_id, warehouse_id,
                 posting_date, total_cogs, cogs_account_id, inventory_account_id, status, posting_state)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,0,$9,$10,'draft'::doc_status,'pending'::gl_posting_state)"#,
        )
        .bind(id).bind(&d.delivery_number).bind(d.company_id).bind(d.branch_id).bind(d.customer_id)
        .bind(d.source_so_id).bind(d.warehouse_id).bind(d.posting_date)
        .bind(d.cogs_account_id).bind(d.inventory_account_id)
        .execute(&mut *tx).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(d.delivery_number) } else { e.into() });
        }
        for l in &d.lines {
            sqlx::query(
                r#"INSERT INTO inventory.delivery_note_items (id, delivery_id, item_id, quantity, valuation_rate, cogs_amount)
                   VALUES ($1,$2,$3,$4,0,0)"#,
            )
            .bind(Uuid::new_v4()).bind(id).bind(l.item_id).bind(l.quantity)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- bin helper (call inside a transaction) ----------------------------

    /// Load a bin FOR UPDATE, creating a zeroed one if absent. Returns its current balance.
    async fn load_or_init_bin(
        tx: &mut sqlx::PgConnection,
        company: Uuid,
        item: Uuid,
        warehouse: Uuid,
    ) -> Result<BinState, InventoryError> {
        let row = sqlx::query(
            r#"SELECT actual_qty, valuation_rate, stock_value FROM inventory.bins
               WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3 AND (metadata->>'deleted_at') IS NULL
               FOR UPDATE"#,
        )
        .bind(company).bind(item).bind(warehouse).fetch_optional(&mut *tx).await?;
        if let Some(r) = row {
            return Ok(BinState { qty: r.get("actual_qty"), rate: r.get("valuation_rate"), value: r.get("stock_value") });
        }
        sqlx::query(
            r#"INSERT INTO inventory.bins (id, company_id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
               VALUES ($1,$2,$3,$4,0,0,0,0)"#,
        )
        .bind(Uuid::new_v4()).bind(company).bind(item).bind(warehouse).execute(&mut *tx).await?;
        Ok(BinState { qty: Decimal::ZERO, rate: Decimal::ZERO, value: Decimal::ZERO })
    }

    async fn set_bin(
        tx: &mut sqlx::PgConnection,
        company: Uuid, item: Uuid, warehouse: Uuid,
        qty: Decimal, rate: Decimal, value: Decimal,
    ) -> Result<(), InventoryError> {
        sqlx::query(
            r#"UPDATE inventory.bins SET actual_qty=$4, valuation_rate=$5, stock_value=$6
               WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company).bind(item).bind(warehouse).bind(qty).bind(rate).bind(value)
        .execute(&mut *tx).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_sle(
        tx: &mut sqlx::PgConnection,
        company: Uuid, item: Uuid, warehouse: Uuid, posting_date: chrono::NaiveDate,
        actual_qty: Decimal, qty_after: Decimal, incoming_rate: Decimal, valuation_rate: Decimal,
        stock_value: Decimal, value_diff: Decimal, voucher_type: &str, voucher_id: Uuid,
        voucher_no: &str, sle_no: i32,
    ) -> Result<(), InventoryError> {
        sqlx::query(
            r#"INSERT INTO inventory.stock_ledger_entries
                (id, company_id, item_id, warehouse_id, posting_date, actual_qty, qty_after_txn,
                 incoming_rate, valuation_rate, stock_value, stock_value_difference, voucher_type,
                 voucher_id, voucher_no, sle_no, is_cancelled)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12::voucher_type,$13,$14,$15,FALSE)"#,
        )
        .bind(Uuid::new_v4()).bind(company).bind(item).bind(warehouse).bind(posting_date)
        .bind(actual_qty).bind(qty_after).bind(incoming_rate).bind(valuation_rate).bind(stock_value)
        .bind(value_diff).bind(voucher_type).bind(voucher_id).bind(voucher_no).bind(sle_no)
        .execute(&mut *tx).await?;
        Ok(())
    }

    // ---- submit: Purchase Receipt (asset post) -----------------------------

    pub async fn submit_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let hdr = sqlx::query(
            r#"SELECT receipt_number, company_id, branch_id, warehouse_id, posting_date, source_po_id,
                      status::text AS st, inventory_account_id, grir_account_id
               FROM inventory.purchase_receipts WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(id).fetch_optional(&self.db_pool).await?.ok_or(InventoryError::NotFound(id))?;
        if hdr.get::<String, _>("st") != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company: Uuid = hdr.get("company_id");
        let branch: Option<Uuid> = hdr.get("branch_id");
        let warehouse: Uuid = hdr.get("warehouse_id");
        let posting_date: chrono::NaiveDate = hdr.get("posting_date");
        let voucher_no: String = hdr.get("receipt_number");
        let inv_acct: Uuid = hdr.get("inventory_account_id");
        let grir_acct: Uuid = hdr.get("grir_account_id");
        let source_po: Option<Uuid> = hdr.get("source_po_id");

        let items = sqlx::query(
            r#"SELECT item_id, quantity, rate FROM inventory.purchase_receipt_items
               WHERE receipt_id=$1 AND (metadata->>'deleted_at') IS NULL ORDER BY id"#,
        )
        .bind(id).fetch_all(&self.db_pool).await?;

        // ---- physical movement: SLE + Bin, one transaction ----
        let mut tx = self.db_pool.begin().await?;
        let mut total_debit = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let item: Uuid = it.get("item_id");
            let qty: Decimal = it.get("quantity");
            let in_rate: Decimal = it.get("rate");
            let bin = Self::load_or_init_bin(&mut tx, company, item, warehouse).await?;
            let in_amount = money(qty * in_rate);
            let new_qty = bin.qty + qty;
            let new_value = bin.value + in_amount;
            let new_rate = if new_qty > Decimal::ZERO { rate6(new_value / new_qty) } else { Decimal::ZERO };
            Self::set_bin(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            sle_no += 1;
            Self::write_sle(&mut tx, company, item, warehouse, posting_date, qty, new_qty, in_rate,
                new_rate, new_value, in_amount, "purchase_receipt", id, &voucher_no, sle_no).await?;
            total_debit += in_amount;
        }
        sqlx::query("UPDATE inventory.purchase_receipts SET status='submitted'::doc_status WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
        tx.commit().await?;

        // ---- GL post (eventually consistent) ----
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: "IDR".into(), posting_type: "original".into(),
            description: Some("Goods receipt".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, total_debit).with_description("Inventory"),
                GlPostLine::credit(grir_acct, total_debit).with_description("GR/IR clearing"),
            ],
        };
        let outcome = self.emit_and_reconcile("purchase_receipts", id, &env, sink, total_debit).await?;
        self.sink.publish(InventoryEvent::StockReceived(StockReceived {
            receipt_id: id, company_id: company, warehouse_id: warehouse, source_po_id: source_po,
            total_value: total_debit,
        }));
        Ok(outcome)
    }

    // ---- submit: Delivery Note (COGS post) ---------------------------------

    pub async fn submit_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let hdr = sqlx::query(
            r#"SELECT delivery_number, company_id, branch_id, warehouse_id, posting_date, source_so_id,
                      status::text AS st, cogs_account_id, inventory_account_id
               FROM inventory.delivery_notes WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(id).fetch_optional(&self.db_pool).await?.ok_or(InventoryError::NotFound(id))?;
        if hdr.get::<String, _>("st") != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company: Uuid = hdr.get("company_id");
        let branch: Option<Uuid> = hdr.get("branch_id");
        let warehouse: Uuid = hdr.get("warehouse_id");
        let posting_date: chrono::NaiveDate = hdr.get("posting_date");
        let voucher_no: String = hdr.get("delivery_number");
        let cogs_acct: Uuid = hdr.get("cogs_account_id");
        let inv_acct: Uuid = hdr.get("inventory_account_id");
        let source_so: Option<Uuid> = hdr.get("source_so_id");

        let items = sqlx::query(
            r#"SELECT id, item_id, quantity FROM inventory.delivery_note_items
               WHERE delivery_id=$1 AND (metadata->>'deleted_at') IS NULL ORDER BY id"#,
        )
        .bind(id).fetch_all(&self.db_pool).await?;

        let mut tx = self.db_pool.begin().await?;
        let mut total_cogs = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let line_id: Uuid = it.get("id");
            let item: Uuid = it.get("item_id");
            let qty: Decimal = it.get("quantity");
            let bin = Self::load_or_init_bin(&mut tx, company, item, warehouse).await?;
            if bin.qty < qty {
                return Err(InventoryError::InsufficientStock { item_id: item, warehouse_id: warehouse, available: bin.qty, requested: qty });
            }
            let new_qty = bin.qty - qty;
            // On the FINAL outflow (bin drained to 0), the last units absorb the moving-average
            // rounding residual: COGS = the entire remaining value, so stock_value returns to
            // EXACTLY 0 and the Inventory subledger ties out with the GL at zero stock (council
            // 2026-07-04). Otherwise COGS consumes the current 2dp-rounded average.
            let cogs = if new_qty.is_zero() { bin.value } else { money(qty * bin.rate) };
            let new_value = bin.value - cogs;
            // Moving-average: rate is unchanged by an outflow.
            let new_rate = if new_qty > Decimal::ZERO { bin.rate } else { Decimal::ZERO };
            Self::set_bin(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            sqlx::query("UPDATE inventory.delivery_note_items SET valuation_rate=$2, cogs_amount=$3 WHERE id=$1")
                .bind(line_id).bind(bin.rate).bind(cogs).execute(&mut *tx).await?;
            sle_no += 1;
            Self::write_sle(&mut tx, company, item, warehouse, posting_date, -qty, new_qty, Decimal::ZERO,
                new_rate, new_value, -cogs, "delivery_note", id, &voucher_no, sle_no).await?;
            total_cogs += cogs;
        }
        sqlx::query("UPDATE inventory.delivery_notes SET status='submitted'::doc_status, total_cogs=$2 WHERE id=$1")
            .bind(id).bind(total_cogs).execute(&mut *tx).await?;
        tx.commit().await?;

        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: "IDR".into(), posting_type: "original".into(),
            description: Some("Delivery COGS".into()),
            lines: vec![
                GlPostLine::debit(cogs_acct, total_cogs).with_description("COGS"),
                GlPostLine::credit(inv_acct, total_cogs).with_description("Inventory"),
            ],
        };
        let outcome = self.emit_and_reconcile("delivery_notes", id, &env, sink, total_cogs).await?;
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
        let ins = sqlx::query(
            r#"INSERT INTO inventory.stock_entries
                (id, entry_number, company_id, stock_entry_type, from_warehouse_id, to_warehouse_id,
                 posting_date, status, posting_state)
               VALUES ($1,$2,$3,'transfer'::stock_entry_type,$4,$5,$6,'submitted'::doc_status,'not_applicable'::gl_posting_state)"#,
        )
        .bind(id).bind(&t.entry_number).bind(t.company_id).bind(t.from_warehouse_id).bind(t.to_warehouse_id).bind(t.posting_date)
        .execute(&mut *tx).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(t.entry_number) } else { e.into() });
        }
        let mut sle_no = 0i32;
        for l in &t.lines {
            sqlx::query("INSERT INTO inventory.stock_entry_items (id, entry_id, item_id, quantity) VALUES ($1,$2,$3,$4)")
                .bind(Uuid::new_v4()).bind(id).bind(l.item_id).bind(l.quantity).execute(&mut *tx).await?;
            // OUT of source
            let from = Self::load_or_init_bin(&mut tx, t.company_id, l.item_id, t.from_warehouse_id).await?;
            if from.qty < l.quantity {
                return Err(InventoryError::InsufficientStock { item_id: l.item_id, warehouse_id: t.from_warehouse_id, available: from.qty, requested: l.quantity });
            }
            let from_qty = from.qty - l.quantity;
            // Same residual flush as delivery: when the source bin drains to 0, the move carries
            // its entire remaining value so the source ends at exactly 0 (and the target receives
            // the exact value moved — total value conserved to the cent).
            let move_value = if from_qty.is_zero() { from.value } else { money(l.quantity * from.rate) };
            let from_value = from.value - move_value;
            let from_rate = if from_qty > Decimal::ZERO { from.rate } else { Decimal::ZERO };
            Self::set_bin(&mut tx, t.company_id, l.item_id, t.from_warehouse_id, from_qty, from_rate, from_value).await?;
            sle_no += 1;
            Self::write_sle(&mut tx, t.company_id, l.item_id, t.from_warehouse_id, t.posting_date, -l.quantity, from_qty, Decimal::ZERO, from_rate, from_value, -move_value, "stock_entry", id, &t.entry_number, sle_no).await?;
            // IN to target at the source rate (value conserved)
            let to = Self::load_or_init_bin(&mut tx, t.company_id, l.item_id, t.to_warehouse_id).await?;
            let to_qty = to.qty + l.quantity;
            let to_value = to.value + move_value;
            let to_rate = if to_qty > Decimal::ZERO { rate6(to_value / to_qty) } else { Decimal::ZERO };
            Self::set_bin(&mut tx, t.company_id, l.item_id, t.to_warehouse_id, to_qty, to_rate, to_value).await?;
            sle_no += 1;
            Self::write_sle(&mut tx, t.company_id, l.item_id, t.to_warehouse_id, t.posting_date, l.quantity, to_qty, from.rate, to_rate, to_value, move_value, "stock_entry", id, &t.entry_number, sle_no).await?;
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
        let ins = sqlx::query(
            r#"INSERT INTO inventory.stock_reconciliations
                (id, recon_number, company_id, warehouse_id, posting_date, net_difference,
                 inventory_account_id, adjustment_account_id, status, posting_state)
               VALUES ($1,$2,$3,$4,$5,0,$6,$7,'submitted'::doc_status,'pending'::gl_posting_state)"#,
        )
        .bind(id).bind(&r.recon_number).bind(r.company_id).bind(r.warehouse_id).bind(r.posting_date)
        .bind(r.inventory_account_id).bind(r.adjustment_account_id)
        .execute(&mut *tx).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.recon_number) } else { e.into() });
        }
        let mut net = Decimal::ZERO;
        let mut sle_no = 0i32;
        for l in &r.lines {
            let bin = Self::load_or_init_bin(&mut tx, r.company_id, l.item_id, r.warehouse_id).await?;
            let target_rate = if l.counted_rate > Decimal::ZERO { l.counted_rate } else { bin.rate };
            let new_value = money(l.counted_qty * target_rate);
            let value_diff = new_value - bin.value;
            let qty_diff = l.counted_qty - bin.qty;
            Self::set_bin(&mut tx, r.company_id, l.item_id, r.warehouse_id, l.counted_qty, target_rate, new_value).await?;
            sqlx::query(
                r#"INSERT INTO inventory.stock_reconciliation_items
                    (id, reconciliation_id, item_id, counted_qty, counted_rate, qty_difference, value_difference)
                   VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
            )
            .bind(Uuid::new_v4()).bind(id).bind(l.item_id).bind(l.counted_qty).bind(l.counted_rate).bind(qty_diff).bind(value_diff)
            .execute(&mut *tx).await?;
            sle_no += 1;
            Self::write_sle(&mut tx, r.company_id, l.item_id, r.warehouse_id, r.posting_date, qty_diff, l.counted_qty, Decimal::ZERO, target_rate, new_value, value_diff, "stock_reconciliation", id, &r.recon_number, sle_no).await?;
            net += value_diff;
        }
        sqlx::query("UPDATE inventory.stock_reconciliations SET net_difference=$2 WHERE id=$1")
            .bind(id).bind(net).execute(&mut *tx).await?;
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
                posting_date: r.posting_date, currency: "IDR".into(), posting_type: "original".into(),
                description: Some("Stock reconciliation".into()), lines,
            };
            self.emit_and_reconcile("stock_reconciliations", id, &env, sink, net.abs()).await?;
        } else {
            sqlx::query("UPDATE inventory.stock_reconciliations SET posting_state='not_applicable'::gl_posting_state WHERE id=$1")
                .bind(id).execute(&self.db_pool).await?;
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
        let h = sqlx::query(
            r#"SELECT company_id, branch_id, receipt_number, posting_date, total_value,
                      inventory_account_id, grir_account_id, posting_state::text AS ps, journal_id, accounting_post_id
               FROM inventory.purchase_receipts WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        ).bind(id).fetch_optional(&self.db_pool).await?.ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h, id) { return Ok(o); }
        let amt: Decimal = h.get("total_value");
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.get("company_id"), branch_id: h.get("branch_id"),
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.get("receipt_number")),
            posting_date: h.get("posting_date"), currency: "IDR".into(), posting_type: "original".into(),
            description: Some("Goods receipt (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.get("inventory_account_id"), amt).with_description("Inventory"),
                GlPostLine::credit(h.get("grir_account_id"), amt).with_description("GR/IR clearing"),
            ],
        };
        self.emit_and_reconcile("purchase_receipts", id, &env, sink, amt).await
    }

    pub async fn repost_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let h = sqlx::query(
            r#"SELECT company_id, branch_id, delivery_number, posting_date, total_cogs,
                      cogs_account_id, inventory_account_id, posting_state::text AS ps, journal_id, accounting_post_id
               FROM inventory.delivery_notes WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        ).bind(id).fetch_optional(&self.db_pool).await?.ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h, id) { return Ok(o); }
        let amt: Decimal = h.get("total_cogs");
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.get("company_id"), branch_id: h.get("branch_id"),
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.get("delivery_number")),
            posting_date: h.get("posting_date"), currency: "IDR".into(), posting_type: "original".into(),
            description: Some("Delivery COGS (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.get("cogs_account_id"), amt).with_description("COGS"),
                GlPostLine::credit(h.get("inventory_account_id"), amt).with_description("Inventory"),
            ],
        };
        self.emit_and_reconcile("delivery_notes", id, &env, sink, amt).await
    }

    /// Short-circuit a repost when the voucher is already settled: `posted` → return the recorded
    /// ids; `not_applicable` → no-op. Returns None when a (re-)emit is actually needed.
    fn already_settled(h: &sqlx::postgres::PgRow, id: Uuid) -> Option<SubmitOutcome> {
        match h.get::<String, _>("ps").as_str() {
            "posted" => Some(SubmitOutcome {
                voucher_id: id, posted: true,
                journal_id: h.get("journal_id"), post_id: h.get("accounting_post_id"), gl_amount: Decimal::ZERO,
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
        &self, table: &str, voucher_id: Uuid, env: &AccountingPostEnvelope, sink: &dyn GlPostSink, gl_amount: Decimal,
    ) -> Result<SubmitOutcome, InventoryError> {
        debug_assert!(env.is_balanced());
        match sink.post(env).await {
            Ok(ack) => {
                let sql = format!(
                    "UPDATE inventory.{table} SET posting_state='posted'::gl_posting_state, journal_id=$2, accounting_post_id=$3, posted_at=now() WHERE id=$1 AND posting_state <> 'posted'::gl_posting_state"
                );
                sqlx::query(&sql).bind(voucher_id).bind(ack.journal_id).bind(ack.post_id).execute(&self.db_pool).await?;
                Ok(SubmitOutcome { voucher_id, posted: true, journal_id: Some(ack.journal_id), post_id: Some(ack.post_id), gl_amount })
            }
            Err(rej) => {
                let sql = format!("UPDATE inventory.{table} SET posting_state='failed'::gl_posting_state WHERE id=$1");
                let _ = sqlx::query(&sql).bind(voucher_id).execute(&self.db_pool).await;
                Err(InventoryError::GlRejected { code: rej.code, message: rej.message })
            }
        }
    }
}
