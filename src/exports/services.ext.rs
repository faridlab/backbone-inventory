//! The extension of `services.rs` (hand-written; ADR-0031).

#[allow(unused_imports)]
use super::*;

// The two-scope availability read (display vs checkout at one warehouse pivot, computed
// fresh per call): the port contract, its fail-loud refusing default, the database-backed
// implementation, and the DTOs. Stable consumption surface for composing services — the
// storefront, a product feed, a pickup flow.
pub use crate::application::service::availability_scope::{
    AvailabilityScopeError, AvailabilityScopePort, AvailabilityScopeRead, CheckoutDemand,
    CheckoutFreeQty, RefusingAvailabilityScopePort, ScopedAvailability, SoldOutVerdict,
};
