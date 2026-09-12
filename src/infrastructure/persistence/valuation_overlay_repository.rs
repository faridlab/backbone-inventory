//! Valuation-overlay repository (hand-authored, user-owned) — the SQL behind the posting
//! posture and the landed-cost document family.
//!
//! Not schema-derived: this file exists for the same reason `gl_voucher_repository.rs` does —
//! the reads and writes here cut across the generated per-entity shapes. The settings row is a
//! one-per-org-unit posture read (not a CRUD list), the landed-cost draft write mints header +
//! lines as one unit, and the allocation worksheet is a TRANSIENT recompute surface
//! (delete + recreate, ordered) that must never be written row-by-row by clients.
//!
//! Per the module's 4-layer rule the services orchestrate and this file holds the SQL. Every
//! method takes the CALLER'S connection (or runs `*_scoped` on the pool). Tenancy is
//! composition-installed (ADR-0029): the composing service's org fence bounds every read and
//! write, so no statement keys on tenancy.

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The valuation posture, as the settings row stores it. An ABSENT row means the
/// runtime defaults (`average` / `perpetual` / anglo off) — exactly the pre-overlay behavior.
#[derive(Debug, Clone)]
pub struct PostureRow {
    /// `average` | `fifo` | `standard` (only `average` has a costing engine today).
    pub cost_method: String,
    /// `perpetual` | `periodic`.
    pub valuation_policy: String,
    pub anglo_saxon_accounting: bool,
    pub stock_interim_delivered_account_id: Option<Uuid>,
}

/// A landed-cost document header, as the validate/cancel verbs read it.
#[derive(Debug, Clone)]
pub struct LcHeaderRow {
    pub id: Uuid,
    pub lc_number: String,
    pub branch_id: Option<Uuid>,
    pub target_receipt_id: Uuid,
    pub currency: String,
    pub posting_date: chrono::NaiveDate,
    /// `draft` | `done` | `cancel`.
    pub state: String,
    /// `not_applicable` | `pending` | `posted` | `failed`.
    pub posting_state: String,
    /// The recorded GL settlement ids (a `posted` document's repost short-circuit).
    pub journal_id: Option<Uuid>,
    pub accounting_post_id: Option<Uuid>,
}

/// One landed-cost charge line.
#[derive(Debug, Clone)]
pub struct LcLineRow {
    pub id: Uuid,
    pub name: String,
    pub account_id: Uuid,
    /// `quantity` | `value` | `weight`.
    pub split_method: String,
    pub amount: Decimal,
}

/// The target receipt of a landed cost, as its validation reads it.
#[derive(Debug, Clone)]
pub struct LcTargetReceiptRow {
    pub receipt_number: String,
    pub warehouse_id: Uuid,
    /// `draft` | `submitted` | `cancelled`.
    pub status: String,
    /// The receipt header's inventory account — the fallback of the landed-cost debit leg's
    /// account-resolution chain (location override first, this second).
    pub inventory_account_id: Uuid,
}

/// One transient allocation-worksheet row, as the recompute writes it and tests read it back.
#[derive(Debug, Clone)]
pub struct WorksheetRow {
    pub move_line_id: Uuid,
    pub cost_line_id: Uuid,
    /// This cost line's rounded allocation onto the target line.
    pub share: Decimal,
    /// CUMULATIVE landed cost allocated onto this target line across ALL cost lines.
    pub additional_landed_cost: Decimal,
    /// The target line's still-on-hand quantity at validate time (read-only FIFO attribution).
    pub remaining_qty: Decimal,
}

/// The draft header a `create_landed_cost` mints.
pub struct NewLandedCostRow<'a> {
    pub id: Uuid,
    pub lc_number: &'a str,
    pub branch_id: Option<Uuid>,
    pub target_receipt_id: Uuid,
    pub currency: &'a str,
    pub posting_date: chrono::NaiveDate,
    pub notes: Option<&'a str>,
}

/// One cost line of a draft landed cost.
pub struct NewLcLineRow<'a> {
    pub id: Uuid,
    pub lc_id: Uuid,
    pub name: &'a str,
    pub account_id: Uuid,
    pub split_method: &'a str,
    pub amount: Decimal,
}

/// One worksheet row insert. Rows are written ordered by `move_line_id` (the caller sorts) so
/// the last row is a deterministic rounding-diff recipient.
pub struct NewWorksheetRow {
    pub lc_id: Uuid,
    pub move_line_id: Uuid,
    pub cost_line_id: Uuid,
    pub share: Decimal,
    pub additional_landed_cost: Decimal,
    pub remaining_qty: Decimal,
}

/// Hand-owned SQL for the valuation overlay. Stateless (methods take the caller's connection or
/// run scoped on the pool), mirroring [`super::GlVoucherRepository`].
pub struct ValuationOverlayRepository;

impl Default for ValuationOverlayRepository {
    fn default() -> Self { Self::new() }
}

impl ValuationOverlayRepository {
    pub fn new() -> Self { Self }

    // ---- posting posture ------------------------------------------------------

    /// Read the caller's org-unit valuation settings row. `None` when the org unit has no row
    /// — the caller applies the runtime defaults (average / perpetual / anglo off), which
    /// makes rolling the module out a no-op for tenants that never configure it.
    pub async fn fetch_posture(
        &self,
        pool: &PgPool,
    ) -> Result<Option<PostureRow>, sqlx::Error> {
        let row = backbone_orm::company_scope::fetch_optional_row_scoped(
            pool,
            sqlx::query(
                r#"SELECT cost_method::text AS cost_method, valuation_policy::text AS valuation_policy,
                          anglo_saxon_accounting, stock_interim_delivered_account_id
                   FROM inventory.inventory_company_settings
                   WHERE (metadata->>'deleted_at') IS NULL"#,
            ),
        )
        .await?;
        Ok(row.map(Self::map_posture_row))
    }

    /// The same posture read on the CALLER'S connection — the variant the move engine uses
    /// inside its open movement transaction (the ambient org scope is already bound there, so
    /// the fenced read is correct without opening a second transaction).
    pub async fn fetch_posture_on(
        &self,
        conn: &mut sqlx::PgConnection,
    ) -> Result<Option<PostureRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT cost_method::text AS cost_method, valuation_policy::text AS valuation_policy,
                      anglo_saxon_accounting, stock_interim_delivered_account_id
               FROM inventory.inventory_company_settings
               WHERE (metadata->>'deleted_at') IS NULL"#,
        )
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(Self::map_posture_row))
    }

    fn map_posture_row(r: sqlx::postgres::PgRow) -> PostureRow {
        PostureRow {
            cost_method: r.get("cost_method"),
            valuation_policy: r.get("valuation_policy"),
            anglo_saxon_accounting: r.get("anglo_saxon_accounting"),
            stock_interim_delivered_account_id: r.get("stock_interim_delivered_account_id"),
        }
    }

    /// A location's valuation-account override, when set. `None` = no override (the caller
    /// falls through to the door-header account).
    ///
    /// The read runs org-scoped: locations are shared masters under the composition's
    /// root-shared fence, so a scoped read resolves the override for tenant-specific and
    /// shared locations alike.
    pub async fn fetch_location_valuation_override(
        &self,
        pool: &PgPool,
        location_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = backbone_orm::company_scope::fetch_optional_row_scoped(
            pool,
            sqlx::query(
                r#"SELECT valuation_account_id FROM inventory.locations
                   WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
            )
            .bind(location_id),
        )
        .await?;
        Ok(row.and_then(|r| r.get("valuation_account_id")))
    }

    /// The per-unit shipping weight of one stock item (0 = unweighted; the weight basis of a
    /// landed-cost split — an all-zero weight basis is the loud zero-denominator case).
    pub async fn fetch_item_weight(
        &self,
        conn: &mut sqlx::PgConnection,
        item_id: Uuid,
    ) -> Result<Decimal, sqlx::Error> {
        let row: Option<Decimal> = sqlx::query_scalar(
            r#"SELECT weight_per_unit FROM inventory.stock_items
               WHERE item_id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(item_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.unwrap_or(Decimal::ZERO))
    }

    // ---- landed-cost document -------------------------------------------------

    /// Read one landed-cost header.
    pub async fn fetch_lc_header(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
    ) -> Result<Option<LcHeaderRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, lc_number, branch_id, target_receipt_id, currency,
                      posting_date, state::text AS state, posting_state::text AS posting_state,
                      journal_id, accounting_post_id
               FROM inventory.landed_costs
               WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(lc_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| LcHeaderRow {
            id: r.get("id"),
            lc_number: r.get("lc_number"),
            branch_id: r.get("branch_id"),
            target_receipt_id: r.get("target_receipt_id"),
            currency: r.get("currency"),
            posting_date: r.get("posting_date"),
            state: r.get("state"),
            posting_state: r.get("posting_state"),
            journal_id: r.get("journal_id"),
            accounting_post_id: r.get("accounting_post_id"),
        }))
    }

    /// Read the landed cost's charge lines.
    pub async fn fetch_lc_lines(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
    ) -> Result<Vec<LcLineRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, name, account_id, split_method::text AS split_method, amount
               FROM inventory.landed_cost_lines
               WHERE lc_id=$1 AND (metadata->>'deleted_at') IS NULL
               ORDER BY (metadata->>'created_at'), id"#,
        )
        .bind(lc_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| LcLineRow {
                id: r.get("id"),
                name: r.get("name"),
                account_id: r.get("account_id"),
                split_method: r.get("split_method"),
                amount: r.get("amount"),
            })
            .collect())
    }

    /// Insert a draft landed-cost header. `posting_state` starts `not_applicable`: a draft has
    /// no GL leg; validation arms `pending` in the same transaction that mints the revaluation
    /// SLE rows (the move-engine arming pattern).
    pub async fn insert_landed_cost_draft(
        &self,
        conn: &mut sqlx::PgConnection,
        lc: &NewLandedCostRow<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.landed_costs
                 (id, lc_number, branch_id, target_receipt_id, currency,
                  posting_date, state, amount_total, posting_state, notes)
               VALUES ($1,$2,$3,$4,$5,$6,'draft',$7,'not_applicable',$8)"#,
        )
        .bind(lc.id)
        .bind(lc.lc_number)
        .bind(lc.branch_id)
        .bind(lc.target_receipt_id)
        .bind(lc.currency)
        .bind(lc.posting_date)
        .bind(Decimal::ZERO)
        .bind(lc.notes)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Insert one cost line of a draft landed cost.
    pub async fn insert_lc_line(
        &self,
        conn: &mut sqlx::PgConnection,
        l: &NewLcLineRow<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.landed_cost_lines
                 (id, lc_id, name, account_id, split_method, amount)
               VALUES ($1,$2,$3,$4,$5::landed_cost_split_method,$6)"#,
        )
        .bind(l.id)
        .bind(l.lc_id)
        .bind(l.name)
        .bind(l.account_id)
        .bind(l.split_method)
        .bind(l.amount)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// The target receipt of a landed cost, as its validation reads it: the receipt number the
    /// door stamped as `origin` on every line move, the header inventory account (the fallback
    /// of the debit leg's account-resolution chain), and the status (a landed cost only ever
    /// targets a receipt whose moves are DONE; the service derives that from the moves
    /// themselves). Takes the caller's connection.
    pub async fn fetch_lc_target_receipt(
        &self,
        conn: &mut sqlx::PgConnection,
        receipt_id: Uuid,
    ) -> Result<Option<LcTargetReceiptRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT receipt_number, warehouse_id, status::text AS st, inventory_account_id
               FROM inventory.purchase_receipts
               WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(receipt_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| LcTargetReceiptRow {
            receipt_number: r.get("receipt_number"),
            warehouse_id: r.get("warehouse_id"),
            status: r.get("st"),
            inventory_account_id: r.get("inventory_account_id"),
        }))
    }

    /// Flip a draft landed cost to `done` and arm its GL leg `pending` — one transaction
    /// together with the revaluation SLE rows the engine verb minted before this call inside
    /// the caller's unit of work. `amount_total` is the document's Σ cost amounts.
    pub async fn mark_lc_validated(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
        amount_total: Decimal,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE inventory.landed_costs
               SET state='done', posting_state='pending', amount_total=$2
               WHERE id=$1 AND state='draft'"#,
        )
        .bind(lc_id)
        .bind(amount_total)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Cancel a DRAFT landed cost (a `done` one can never cancel — the reversal pattern is a
    /// negative-amount landed cost). A draft posted no GL, so its posting state retires to
    /// `not_applicable`.
    pub async fn mark_lc_cancelled(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE inventory.landed_costs
               SET state='cancel', posting_state='not_applicable'
               WHERE id=$1 AND state='draft'"#,
        )
        .bind(lc_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    // ---- the transient allocation worksheet -----------------------------------

    /// Delete the landed cost's worksheet rows. The worksheet is a RECOMPUTE SURFACE, not a
    /// ledger: every (re)validation deletes and recreates it wholesale. The only accounting
    /// identity is the posted journal entry.
    pub async fn delete_worksheet(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM inventory.landed_cost_adjustment_lines WHERE lc_id=$1")
            .bind(lc_id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    /// Insert one worksheet row (the caller writes them ordered by `move_line_id` so the
    /// last-line-eats-the-rounding-diff recipient is deterministic).
    pub async fn insert_worksheet_row(
        &self,
        conn: &mut sqlx::PgConnection,
        w: &NewWorksheetRow,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.landed_cost_adjustment_lines
                 (id, lc_id, move_line_id, cost_line_id, share,
                  additional_landed_cost, remaining_qty)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        )
        .bind(Uuid::new_v4())
        .bind(w.lc_id)
        .bind(w.move_line_id)
        .bind(w.cost_line_id)
        .bind(w.share)
        .bind(w.additional_landed_cost)
        .bind(w.remaining_qty)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Read the landed cost's worksheet back (tests + GL repost reconstruction).
    pub async fn fetch_worksheet(
        &self,
        conn: &mut sqlx::PgConnection,
        lc_id: Uuid,
    ) -> Result<Vec<WorksheetRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT move_line_id, cost_line_id, share, additional_landed_cost, remaining_qty
               FROM inventory.landed_cost_adjustment_lines
               WHERE lc_id=$1
               ORDER BY move_line_id, id"#,
        )
        .bind(lc_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| WorksheetRow {
                move_line_id: r.get("move_line_id"),
                cost_line_id: r.get("cost_line_id"),
                share: r.get("share"),
                additional_landed_cost: r.get("additional_landed_cost"),
                remaining_qty: r.get("remaining_qty"),
            })
            .collect())
    }
}
