//! GL voucher repository — the hand-written SQL that reconciles a voucher's `posting_state` against
//! the accounting ack, across ALL THREE posting voucher tables.
//!
//! Hand-authored and **user-owned**: this exact path is declared under `user_owned` in
//! `metaphor.codegen.yaml`. It is not schema-derived and has no entity of its own.
//!
//! **Why this file exists (the one place the per-entity repository shape did not fit).** The
//! GL-posting contract is uniform across vouchers: the physical movement (SLE + Bin) commits first,
//! then the post is emitted and the voucher's `posting_state` goes pending→posted|failed. That same
//! reconcile runs against `purchase_receipts`, `delivery_notes` or `stock_reconciliations` depending
//! on which voucher is being posted — so it is nobody's entity SQL, and duplicating it three ways
//! would be three places to get the idempotency guard wrong. The table name is chosen ONLY from the
//! closed [`GlVoucher`] enum — never interpolated from a caller string — so the `format!`s below
//! cannot carry untrusted input.

use sqlx::PgPool;
use uuid::Uuid;

use backbone_orm::company_scope;

/// Which voucher table a GL post reconciles against. A closed enum, so the interpolated table name
/// can only ever be one of three compile-time literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlVoucher {
    PurchaseReceipt,
    DeliveryNote,
    StockReconciliation,
}

impl GlVoucher {
    fn table(self) -> &'static str {
        match self {
            GlVoucher::PurchaseReceipt => "purchase_receipts",
            GlVoucher::DeliveryNote => "delivery_notes",
            GlVoucher::StockReconciliation => "stock_reconciliations",
        }
    }
}

/// A voucher's GL settlement state, as the repost path reads it. Shared by every voucher header
/// projection that carries it — the shape is identical across the three tables.
pub struct GlSettlementState {
    /// `posting_state::text` — "pending" | "posted" | "failed" | "not_applicable".
    pub posting_state: String,
    pub journal_id: Option<Uuid>,
    pub accounting_post_id: Option<Uuid>,
}

/// The GL reconcile's SQL. Stateless: it holds no pool, because both methods run outside the
/// movement's transaction — the physical movement is already committed by the time a post is
/// emitted, and the reconcile is deliberately a separate write (the GL leg is eventually
/// consistent).
pub struct GlVoucherRepository;

impl Default for GlVoucherRepository {
    fn default() -> Self { Self::new() }
}

impl GlVoucherRepository {
    pub fn new() -> Self { Self }

    /// Reconcile a voucher from the accounting ack — the pending→posted transition.
    ///
    /// Guarded on `posting_state <> 'posted'` so a repost of an already-posted voucher cannot
    /// overwrite the original journal's ids: accounting dedupes on
    /// `(company, source_type, source_id, posting_type)` and hands back the ORIGINAL journal, and
    /// this guard is what keeps the recorded ids stable through that.
    ///
    /// Runs `execute_scoped` on the pool; the caller wraps it in `with_company_scope(Some(company))`
    /// (the company comes off the post envelope) so the UPDATE passes the RLS fence (ADR-0008).
    pub async fn mark_posted(
        &self,
        pool: &PgPool,
        voucher: GlVoucher,
        voucher_id: Uuid,
        journal_id: Uuid,
        post_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let sql = format!(
            "UPDATE inventory.{} SET posting_state='posted'::gl_posting_state, journal_id=$2, accounting_post_id=$3, posted_at=now() WHERE id=$1 AND posting_state <> 'posted'::gl_posting_state",
            voucher.table(),
        );
        company_scope::execute_scoped(
            pool,
            sqlx::query(&sql).bind(voucher_id).bind(journal_id).bind(post_id),
        )
        .await?;
        Ok(())
    }

    /// Record a GL rejection. The physical movement is NOT rolled back — it really happened; the
    /// voucher parks in `failed` and is re-drivable via the service's `repost_*` entrypoints.
    ///
    /// Caller supplies the company scope, as [`Self::mark_posted`].
    pub async fn mark_failed(
        &self,
        pool: &PgPool,
        voucher: GlVoucher,
        voucher_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let sql = format!(
            "UPDATE inventory.{} SET posting_state='failed'::gl_posting_state WHERE id=$1",
            voucher.table(),
        );
        company_scope::execute_scoped(pool, sqlx::query(&sql).bind(voucher_id)).await?;
        Ok(())
    }
}
