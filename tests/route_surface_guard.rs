//! Regression guard (ADR-0010 Decision A + council 2026-07-29): the unguarded
//! `all_crud_routes()` composer must NOT mount generic CRUD on engine-owned tables — the child
//! line tables (`delivery_note_items`, `purchase_receipt_items`, `stock_entry_items`,
//! `stock_reconciliation_items`) AND the running-balance `bins` table. Line items are owned by
//! their parent document, and `bins` is the moving-average balance that must stay in lockstep with
//! the append-only SLE and the GL. Both must be written only through `InventoryWriteService` (the
//! surface exposed by `create_guarded_inventory_routes`); generic CRUD on them is never a
//! legitimate HTTP path. A `PATCH /bins/{id}` would rewrite the balance with no row lock, no SLE,
//! and no GL post — silent subledger drift `repost_*` cannot repair.
//!
//! Why a source-level check: codegen regenerates `all_crud_routes()` wholesale — a `// <<< CUSTOM`
//! block can't suppress it (regen emits its own copy and produces a duplicate-function error), and
//! there is no schema attribute to skip route generation for an entity. So the closure lives in
//! generated territory and a `metaphor schema generate` can silently re-add the excluded routes.
//! This test reads `src/lib.rs` and fails the build if any engine-owned-table route mount reappears,
//! turning a silent reopen into a loud CI failure.
//!
//! Safety note: the RLS fence (NOT NULL `company_id` + FORCE RLS, ADR-0008/0010) already makes any
//! exposure fail-closed — a generic create is rejected, reads are tenant-fenced. This guard is
//! defense-in-depth hygiene, not the security boundary.

const LIB_RS: &str = include_str!("../src/lib.rs");

/// The engine-owned tables whose generic CRUD is deliberately excluded from `all_crud_routes`.
/// Each entry is the exact route-mount call site (function + the service field it would be called
/// with), which only appears inside an `all_crud_routes`-style composer.
const EXCLUDED_ENGINE_OWNED_ROUTE_MOUNTS: &[&str] = &[
    // Child line tables (ADR-0010): owned by their parent document.
    "create_delivery_note_item_routes(self.delivery_note_item_service",
    "create_purchase_receipt_item_routes(self.purchase_receipt_item_service",
    "create_stock_entry_item_routes(self.stock_entry_item_service",
    "create_stock_reconciliation_item_routes(self.stock_reconciliation_item_service",
    // Running balance (council 2026-07-29): must tie to the SLE and GL via the engine only.
    "create_bin_routes(self.bin_service",
];

#[test]
fn all_crud_routes_excludes_engine_owned_tables() {
    for mount in EXCLUDED_ENGINE_OWNED_ROUTE_MOUNTS {
        assert!(
            !LIB_RS.contains(mount),
            "regression: `all_crud_routes` mounts an engine-owned-table route ({mount}). \
             A schema regen has re-added it. Remove the call from `all_crud_routes` in src/lib.rs — \
             child line items and the `bins` running balance must be written via InventoryWriteService only.",
        );
    }
}
