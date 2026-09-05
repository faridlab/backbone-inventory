//! The two-scope availability read (hand-authored, user-owned; see `metaphor.codegen.yaml`).
//!
//! The storefront-facing availability surface: ONE warehouse pivot, TWO read
//! scopes, both computed FRESH per call off the reservation triangle
//! (`stock_quants.quantity − reserved_quantity` summed over the pivot
//! warehouse's internal locations). Nothing about a readiness check is ever
//! persisted — no scope columns, no warning rows, no materialized verdicts
//! (the upstream payment-readiness gate that persisted `shop_warning` writes
//! during a validation probe is the anti-shape; a validation read that
//! mutates rows is refused by construction here — these methods only SELECT).
//!
//! The two scopes, deliberately separate methods (upstream resolves them
//! through deliberately different resolvers and reserves the checkout arm
//! for a pickup-warehouse override):
//!   - **Display** — what shop grids and product pages show: fresh
//!     availability per item at the pivot warehouse.
//!   - **Checkout** — what the cart clamp and the payment gate consult: the
//!     same fresh availability MINUS the quantities the caller's own cart
//!     already holds, floored at zero. The caller supplies its held
//!     quantities; this module never reads (or knows about) carts.
//!
//! Both scopes are ADVISORY check-then-act reads: nothing is reserved, and
//! enforcement remains where it already lives (the move engine's reservation
//! and picking edges). Two concurrent carts can both pass — that is the
//! recorded upstream posture, re-expressed deliberately rather than silently
//! "fixed" by a half-reservation here.
//!
//! Fail-loud posture (the port pattern the composing service fills):
//!   - The warehouse pivot is a REQUIRED `Uuid`. There is no unset-pivot
//!     overload and no all-warehouses arm on this surface — a caller without
//!     a pivot must refuse on its own side; it can never ask this read to
//!     silently sum every warehouse.
//!   - A pivot warehouse that does not exist, is soft-deleted, or belongs to
//!     another company refuses typed (`UnknownWarehouse`) rather than
//!     projecting an all-sold-out storefront off a zero-row read.
//!   - A GROUPING warehouse node refuses typed as a pivot (`GroupPivot`) —
//!     pointing a shop at a group node is a misconfiguration that would
//!     otherwise read as permanently sold out.
//!   - An empty item set refuses typed (`EmptyItemSet`): a sold-out verdict
//!     over zero variants would be vacuously true.
//!   - The sold-out verdict evaluates EVERY item id the caller passes. The
//!     caller resolves the variant set (the template's variants or the single
//!     item) and passes all of them; this surface has no first-variant
//!     shortcut to get wrong — sold out means NO member of the set has
//!     availability.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{QuantRepository, WarehouseRepository};

/// One item's fresh availability at the pivot warehouse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopedAvailability {
    pub item_id: Uuid,
    /// SUM(quantity) over the pivot warehouse's internal quant rows.
    pub on_hand_qty: Decimal,
    /// SUM(reserved_quantity) over the same rows — the authoritative
    /// reservation counter, read off.
    pub reserved_qty: Decimal,
    /// `on_hand_qty − reserved_qty`, computed per call. Never read off the
    /// stored `available_quantity` column — freshness over cheapness.
    pub available_qty: Decimal,
}

/// One checkout-scope demand: the item plus the quantity the CALLER's own
/// cart already holds for it. The caller sums its own lines per item before
/// passing them here; this module never reads carts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutDemand {
    pub item_id: Uuid,
    pub held_qty: Decimal,
}

/// One checkout-scope answer: fresh availability at the pivot warehouse and
/// the free quantity after the caller's own holdings, floored at zero.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutFreeQty {
    pub item_id: Uuid,
    pub available_qty: Decimal,
    /// `max(available_qty − held_qty, 0)` — the cart clamp's input. Never
    /// negative.
    pub free_qty: Decimal,
}

/// The sold-out verdict over a WHOLE item set (a template's full variant set,
/// or a single item). `sold_out` is true only when EVERY member reads
/// availability ≤ 0 — an absent (never-received) item counts as unavailable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoldOutVerdict {
    pub sold_out: bool,
    /// Complete over the requested set, in input order, zeroed for items
    /// with no quants at the pivot warehouse.
    pub per_item: Vec<ScopedAvailability>,
}

/// The availability scope read's typed refusals.
#[derive(Debug, thiserror::Error)]
pub enum AvailabilityScopeError {
    /// The pivot warehouse does not exist, is soft-deleted, or belongs to
    /// another company. Carries (warehouse_id, company_id).
    #[error("availability pivot warehouse {0} is unknown to company {1}")]
    UnknownWarehouse(Uuid, Uuid),
    /// The pivot warehouse is a grouping node, not a concrete stock
    /// warehouse. Carries the warehouse id.
    #[error("availability pivot warehouse {0} is a warehouse group, not a stock warehouse")]
    GroupPivot(Uuid),
    /// The item set (or demand list) is empty — a vacuous read that would
    /// answer "sold out" over nothing.
    #[error("the availability item set is empty: a sold-out verdict over no items would be vacuously true")]
    EmptyItemSet,
    /// The same item appears twice in one request. The caller must sum its
    /// own per-item quantities before asking.
    #[error("item {0} appears more than once in one availability request")]
    DuplicateItem(Uuid),
    /// A demand carries a negative held quantity.
    #[error("held quantity for item {0} is negative")]
    NegativeHeld(Uuid),
    /// The port has no adapter wired in this composition — the fail-loud
    /// default. No availability number is fabricated, so no surface can
    /// render a guessed stock state.
    #[error("availability scope port unwired: {0}")]
    Unwired(&'static str),
    /// The database transport.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The two-scope availability read a composing service (the storefront, a
/// feed renderer, a pickup flow) consults. Every method is a pure read over
/// the stock estate; nothing is reserved and nothing is persisted.
#[async_trait::async_trait]
pub trait AvailabilityScopePort: Send + Sync {
    /// Display scope: fresh per-item availability at the pivot warehouse,
    /// complete over `item_ids` in input order (absent items zeroed — an
    /// unreceived item is unavailable, not an error).
    async fn display_availability(
        &self,
        company_id: Uuid,
        item_ids: &[Uuid],
        warehouse_id: Uuid,
    ) -> Result<Vec<ScopedAvailability>, AvailabilityScopeError>;

    /// Checkout scope: fresh per-item availability at the (possibly
    /// pickup-overridden) pivot warehouse minus the caller's own held
    /// quantities, floored at zero. Complete over the demand list's items.
    async fn checkout_free_qty(
        &self,
        company_id: Uuid,
        demands: &[CheckoutDemand],
        warehouse_id: Uuid,
    ) -> Result<Vec<CheckoutFreeQty>, AvailabilityScopeError>;

    /// Sold-out verdict over the WHOLE item set (WSC's first-variant-only
    /// template check is the bug class this refuses to re-express):
    /// `sold_out` = no member has availability. Policy (may an out-of-stock
    /// item still be ordered?) stays with the caller — this is raw stock
    /// truth.
    async fn sold_out(
        &self,
        company_id: Uuid,
        item_ids: &[Uuid],
        warehouse_id: Uuid,
    ) -> Result<SoldOutVerdict, AvailabilityScopeError>;
}

/// The refusing default: no availability read ever succeeds. Fail-loud — an
/// unwired composition surfaces the typed `Unwired` refusal on every scope,
/// never a guessed zero (which would render the whole storefront sold out)
/// and never a guessed abundance (which would oversell).
#[derive(Debug, Default, Clone, Copy)]
pub struct RefusingAvailabilityScopePort;

impl RefusingAvailabilityScopePort {
    fn refused() -> AvailabilityScopeError {
        AvailabilityScopeError::Unwired(
            "no availability scope adapter is installed in this composition",
        )
    }
}

#[async_trait::async_trait]
impl AvailabilityScopePort for RefusingAvailabilityScopePort {
    async fn display_availability(
        &self,
        _company_id: Uuid,
        _item_ids: &[Uuid],
        _warehouse_id: Uuid,
    ) -> Result<Vec<ScopedAvailability>, AvailabilityScopeError> {
        Err(Self::refused())
    }

    async fn checkout_free_qty(
        &self,
        _company_id: Uuid,
        _demands: &[CheckoutDemand],
        _warehouse_id: Uuid,
    ) -> Result<Vec<CheckoutFreeQty>, AvailabilityScopeError> {
        Err(Self::refused())
    }

    async fn sold_out(
        &self,
        _company_id: Uuid,
        _item_ids: &[Uuid],
        _warehouse_id: Uuid,
    ) -> Result<SoldOutVerdict, AvailabilityScopeError> {
        Err(Self::refused())
    }
}

/// The real, database-backed availability scope read. Implements
/// [`AvailabilityScopePort`] so a composition can hold the trait and swap
/// adapters (the refusing default in tests, this service in production).
#[derive(Clone)]
pub struct AvailabilityScopeRead {
    db_pool: PgPool,
    quants: std::sync::Arc<QuantRepository>,
    warehouses: std::sync::Arc<WarehouseRepository>,
}

impl AvailabilityScopeRead {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            quants: std::sync::Arc::new(QuantRepository::new(db_pool.clone())),
            warehouses: std::sync::Arc::new(WarehouseRepository::new(db_pool.clone())),
            db_pool,
        }
    }

    /// Fail-loud pivot validation: the warehouse must exist, be live, belong
    /// to the company, and be a concrete stock warehouse.
    async fn validate_pivot(&self, company_id: Uuid, warehouse_id: Uuid) -> Result<(), AvailabilityScopeError> {
        match self.warehouses.fetch_pivot_warehouse(&self.db_pool, company_id, warehouse_id).await? {
            None => Err(AvailabilityScopeError::UnknownWarehouse(warehouse_id, company_id)),
            Some(true) => Err(AvailabilityScopeError::GroupPivot(warehouse_id)),
            Some(false) => Ok(()),
        }
    }

    /// The shared fresh aggregation: per-item on-hand/reserved sums over the
    /// pivot warehouse's internal quant rows, zero-filled over the requested
    /// set in input order. `item_ids` must be non-empty and duplicate-free
    /// (the caller-side guards have already refused otherwise).
    async fn scoped_rows(
        &self,
        company_id: Uuid,
        item_ids: &[Uuid],
        warehouse_id: Uuid,
    ) -> Result<Vec<ScopedAvailability>, AvailabilityScopeError> {
        let rows = self
            .quants
            .fetch_warehouse_on_hand(&self.db_pool, company_id, item_ids, warehouse_id)
            .await?;
        let by_item: std::collections::HashMap<Uuid, (Decimal, Decimal)> = rows
            .into_iter()
            .map(|r| (r.item_id, (r.on_hand_qty, r.reserved_qty)))
            .collect();
        Ok(item_ids
            .iter()
            .map(|id| {
                let (on_hand, reserved) =
                    by_item.get(id).copied().unwrap_or((Decimal::ZERO, Decimal::ZERO));
                ScopedAvailability {
                    item_id: *id,
                    on_hand_qty: on_hand,
                    reserved_qty: reserved,
                    available_qty: on_hand - reserved,
                }
            })
            .collect())
    }

    /// Refuse an empty or duplicate item set (shared guard for both
    /// item-list methods).
    fn guard_item_set(item_ids: &[Uuid]) -> Result<(), AvailabilityScopeError> {
        if item_ids.is_empty() {
            return Err(AvailabilityScopeError::EmptyItemSet);
        }
        let mut seen = std::collections::HashSet::with_capacity(item_ids.len());
        for id in item_ids {
            if !seen.insert(*id) {
                return Err(AvailabilityScopeError::DuplicateItem(*id));
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AvailabilityScopePort for AvailabilityScopeRead {
    async fn display_availability(
        &self,
        company_id: Uuid,
        item_ids: &[Uuid],
        warehouse_id: Uuid,
    ) -> Result<Vec<ScopedAvailability>, AvailabilityScopeError> {
        Self::guard_item_set(item_ids)?;
        self.validate_pivot(company_id, warehouse_id).await?;
        self.scoped_rows(company_id, item_ids, warehouse_id).await
    }

    async fn checkout_free_qty(
        &self,
        company_id: Uuid,
        demands: &[CheckoutDemand],
        warehouse_id: Uuid,
    ) -> Result<Vec<CheckoutFreeQty>, AvailabilityScopeError> {
        if demands.is_empty() {
            return Err(AvailabilityScopeError::EmptyItemSet);
        }
        let item_ids: Vec<Uuid> = demands.iter().map(|d| d.item_id).collect();
        Self::guard_item_set(&item_ids)?;
        for d in demands {
            if d.held_qty < Decimal::ZERO {
                return Err(AvailabilityScopeError::NegativeHeld(d.item_id));
            }
        }
        self.validate_pivot(company_id, warehouse_id).await?;
        let rows = self.scoped_rows(company_id, &item_ids, warehouse_id).await?;
        Ok(rows
            .into_iter()
            .zip(demands.iter())
            .map(|(row, demand)| CheckoutFreeQty {
                item_id: row.item_id,
                available_qty: row.available_qty,
                free_qty: (row.available_qty - demand.held_qty).max(Decimal::ZERO),
            })
            .collect())
    }

    async fn sold_out(
        &self,
        company_id: Uuid,
        item_ids: &[Uuid],
        warehouse_id: Uuid,
    ) -> Result<SoldOutVerdict, AvailabilityScopeError> {
        Self::guard_item_set(item_ids)?;
        self.validate_pivot(company_id, warehouse_id).await?;
        let per_item = self.scoped_rows(company_id, item_ids, warehouse_id).await?;
        let sold_out = per_item.iter().all(|row| row.available_qty <= Decimal::ZERO);
        Ok(SoldOutVerdict { sold_out, per_item })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unwired_port_refuses_every_scope() {
        let port = RefusingAvailabilityScopePort;
        let id = Uuid::new_v4();
        assert!(matches!(
            port.display_availability(Uuid::new_v4(), &[id], Uuid::new_v4()).await,
            Err(AvailabilityScopeError::Unwired(_))
        ));
        assert!(matches!(
            port.checkout_free_qty(
                Uuid::new_v4(),
                &[CheckoutDemand { item_id: id, held_qty: Decimal::ZERO }],
                Uuid::new_v4()
            )
            .await,
            Err(AvailabilityScopeError::Unwired(_))
        ));
        assert!(matches!(
            port.sold_out(Uuid::new_v4(), &[id], Uuid::new_v4()).await,
            Err(AvailabilityScopeError::Unwired(_))
        ));
    }
}
