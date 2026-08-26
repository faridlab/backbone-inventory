//! The per-company valuation posting posture (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk: the single resolution of the company's valuation
//! settings row ([`Posture`]) that every GL-posting surface consults — the four voucher-door
//! envelope builders (receipt + delivery, submit + repost), the cancellation compensations,
//! and the move engine's envelope builder. One helper, consulted everywhere, so the posture
//! cannot drift between a door's submit and its repost/cancel legs.
//!
//! What the posture switches:
//!
//! - **Anglo-saxon delivery debit** — when ON, the delivery door's DEBIT leg swaps
//!   `cogs_account_id` for the settings' `stock_interim_delivered_account_id` (the GR/IR-family
//!   interim leg; COGS recognition is deferred to the invoice side). The receipt side is
//!   unchanged in both postures — its `grir_account_id` header account IS the
//!   interim-received leg. There is NO second posting path: the posture only rewrites WHICH
//!   account a leg uses inside the same envelope.
//! - **Periodic valuation policy** — suppresses real-time stock posts entirely: the doors
//!   build no envelope and retire the voucher to `not_applicable`; the engine's move legs do
//!   the same. The periodic closing flow (which would book those legs) is a later increment.
//! - **Location valuation-account override** — the documented account-resolution chain per
//!   inventory leg: the location's `valuation_account_id` when set, else the door-header
//!   account. Smallest-first override: a location beats the header.
//!
//! An ABSENT settings row means the runtime defaults (`average` / perpetual / anglo off) —
//! exactly the pre-overlay posting shapes — so rolling the module out is a no-op for
//! companies that never configure it.
//!
//! Per the module's 4-layer rule this file holds no SQL — the reads live on
//! [`crate::infrastructure::persistence::ValuationOverlayRepository`].

use rust_decimal::Decimal;
use uuid::Uuid;

use super::inventory_write_service::{InventoryError, InventoryWriteService};

/// The resolved per-company posting posture. Built from the settings row, or the runtime
/// defaults when the company has no row.
#[derive(Debug, Clone)]
pub struct Posture {
    /// `average` | `fifo` | `standard` — settings vocabulary. `average` (the moving-average
    /// SLE engine) is the only costing engine shipped; nothing in the posting path branches
    /// on this yet (a `standard` company is refused loudly at landed-cost validation, which
    /// owns that seam).
    pub cost_method: String,
    /// `valuation_policy = 'periodic'`: real-time stock posts suppressed (doors + engine legs
    /// `not_applicable`); the closing flow that books them is a later increment.
    pub periodic: bool,
    /// `anglo_saxon_accounting`: the delivery debit swaps COGS for the interim-delivered leg.
    pub anglo_saxon: bool,
    /// The interim-delivered account the delivery debits under the anglo-saxon posture.
    /// Required loudly (fail-closed) when the posture is ON.
    pub stock_interim_delivered_account_id: Option<Uuid>,
}

impl Default for Posture {
    /// The runtime defaults — an absent settings row behaves exactly like the pre-overlay
    /// module (moving average, perpetual real-time posting, no anglo-saxon swap).
    fn default() -> Self {
        Posture {
            cost_method: "average".into(),
            periodic: false,
            anglo_saxon: false,
            stock_interim_delivered_account_id: None,
        }
    }
}

impl InventoryWriteService {
    /// Read the company's valuation settings row and resolve it into a [`Posture`]. `None`
    /// from the repository (no row for the company) resolves to [`Posture::default`] — the
    /// rollout no-op contract.
    pub(super) async fn posting_posture(
        &self,
        company_id: Uuid,
    ) -> Result<Posture, InventoryError> {
        let row = backbone_orm::company_scope::with_company_scope(
            Some(company_id),
            self.valuation_overlay.fetch_posture(&self.db_pool, company_id),
        )
        .await?;
        Ok(match row {
            None => Posture::default(),
            Some(r) => Posture {
                cost_method: r.cost_method,
                periodic: r.valuation_policy == "periodic",
                anglo_saxon: r.anglo_saxon_accounting,
                stock_interim_delivered_account_id: r.stock_interim_delivered_account_id,
            },
        })
    }

    /// The same posture resolution on the CALLER'S connection — the variant the move engine
    /// uses inside its open movement transaction (the company is already bound on the
    /// connection, so the strict-fenced settings read is RLS-correct without a second
    /// transaction).
    pub(super) async fn posting_posture_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
    ) -> Result<Posture, InventoryError> {
        let row = self.valuation_overlay.fetch_posture_on(conn, company_id).await?;
        Ok(match row {
            None => Posture::default(),
            Some(r) => Posture {
                cost_method: r.cost_method,
                periodic: r.valuation_policy == "periodic",
                anglo_saxon: r.anglo_saxon_accounting,
                stock_interim_delivered_account_id: r.stock_interim_delivered_account_id,
            },
        })
    }

    /// The delivery door's DEBIT leg under the posture: the anglo-saxon
    /// interim-delivered account when the posture is ON, else the header COGS account
    /// unchanged. Fail-closed: the posture ON with no interim account configured is
    /// [`InventoryError::AngloPostureUnconfigured`] — no envelope, no partial post — because a
    /// silent COGS fallback would book the interim leg to the wrong account on every
    /// delivery. The SAME resolution is used by the delivery cancel compensation (the credit
    /// side mirrors the debit the original post used), so submit/repost/cancel stay
    /// symmetric.
    pub(super) fn delivery_debit_account(
        &self,
        company_id: Uuid,
        posture: &Posture,
        cogs_account_id: Uuid,
    ) -> Result<Uuid, InventoryError> {
        if !posture.anglo_saxon {
            return Ok(cogs_account_id);
        }
        posture
            .stock_interim_delivered_account_id
            .ok_or(InventoryError::AngloPostureUnconfigured { company_id })
    }

    /// The account-resolution chain for one inventory leg: the location's
    /// `valuation_account_id` override when set, else the door-header account the leg would
    /// use today. The documented chain, smallest-first — a location beats the header.
    pub(super) async fn inventory_leg_account(
        &self,
        location_id: Uuid,
        header_account_id: Uuid,
    ) -> Result<Uuid, InventoryError> {
        Ok(self
            .valuation_overlay
            .fetch_location_valuation_override(&self.db_pool, location_id)
            .await?
            .unwrap_or(header_account_id))
    }

    /// The explicit account-move gate (the `_should_create_account_move` port): a posting
    /// surface builds an envelope only when the thing being posted carries something — value
    /// OR quantity — and every account its shape needs is configured. A voucher/move that
    /// carries neither (all-zero lines) builds no envelope and its posting_state stays
    /// `not_applicable`. Consulted by the door envelope builders (with the voucher totals)
    /// and by the engine's [`super::inventory_move_engine`] envelope builder (with the
    /// move's leg values); the accounts-configured half is also what the directive's
    /// optional accounts express inside the engine's shape branches.
    pub(super) fn should_create_account_move(
        value: Decimal,
        qty: Decimal,
        accounts_configured: bool,
    ) -> bool {
        accounts_configured && !(value.is_zero() && qty.is_zero())
    }
}
