//! Regression guard (ADR-0010 Decision A + council 2026-07-29): the unguarded
//! `all_crud_routes()` composer must NOT mount generic CRUD on engine-owned tables — the child
//! line tables (`delivery_note_items`, `purchase_receipt_items`, `stock_entry_items`,
//! `stock_reconciliation_items`), the running-balance `bins` table, and the stock-convergence
//! engine state (`stock_moves`, `stock_move_lines`, `stock_quants`). Line items are owned by
//! their parent document, and `bins` is the moving-average balance that must stay in lockstep with
//! the append-only SLE and the GL. The move/line/quant tables carry the 7-state lifecycle and the
//! reservation triangle; driving them through generic CRUD would bypass the guarded transitions,
//! the picking projection re-derivation, and the SLE/GL minting. All must be written only through
//! `InventoryWriteService` / the move engine (the surface exposed by
//! `create_guarded_inventory_routes`); generic CRUD on them is never a
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
    // Stock-convergence engine state (spec: stock move lifecycle / reservation triangle):
    // `stock_moves` carries the 7-state lifecycle driven only by the engine's guarded
    // transitions (schema/models/move.model.yaml: "never free-hand-set over HTTP");
    // `stock_move_lines` is the reservation mirror; `stock_quants.reserved_quantity` is the
    // authoritative reservation counter (ONE writer: the move engine). Generic CRUD on any
    // of them would bypass the picking projection re-derivation, the SLE minting, and the
    // GL post. Reads stay exposed via `readonly_routes()`.
    "create_stock_move_routes(self.stock_move_service",
    "create_stock_move_line_routes(self.stock_move_line_service",
    "create_quant_routes(self.quant_service",
    // Append-only ledger and GL-posting adjustments (RIDER 2 QUANT-SURFACE HARDENING):
    // `stock_ledger_entries` is the append-only ledger the move engine mints — generic
    // writes would bypass the SLE/GL seam and the accounting integration. `stock_reconciliations`
    // are the GL-posting adjustments that must post only through the engine's GL surface.
    // Reads stay exposed via `readonly_routes()`.
    "create_stock_ledger_entry_routes(self.stock_ledger_entry_service",
    "create_stock_reconciliation_routes(self.stock_reconciliation_service",
    // Landed-cost children (the valuation overlay): the cost lines are owned by their
    // landed-cost document (writes ride its validate/cancel verbs, which alone run the
    // allocation + revaluation through the move engine), and the adjustment lines are the
    // TRANSIENT allocation worksheet the validate verb deletes and recreates — a generic
    // write would forge allocations no SLE/bin write backs. Reads stay exposed via
    // `readonly_routes()`.
    "create_landed_cost_line_routes(self.landed_cost_line_service",
    "create_landed_cost_adjustment_line_routes(self.landed_cost_adjustment_line_service",
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
