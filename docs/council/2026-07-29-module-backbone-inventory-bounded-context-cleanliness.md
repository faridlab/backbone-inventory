<!-- date: 2026-07-29 | repo-type: module | unit: backbone-inventory | focus: bounded-context-cleanliness
     roster: chair, skeptic, steelman (subagents) + ddd-bounded-context, contract-seat, yagni-business, domain-expert (in-context) -->

# Council — module:backbone-inventory — focus: bounded-context-cleanliness

## Best call

**Strip generic CRUD over `bins` off the module's public route surface, and demote `BinService` to `pub(crate)`.** Concretely: remove `create_bin_routes(self.bin_service.clone())` from `InventoryModule::all_crud_routes()` (`src/lib.rs:92-121`) and from the deprecated `routes()` alias (`src/lib.rs:128-131`), and change `pub use BinService` (`src/lib.rs:41`) plus `pub bin_service` field exposure to `pub(crate)`. The bounded context's central invariant — *subledger ties to GL because lock_or_init-FOR-UPDATE → append-SLE → balanced-post is the only path that touches a bin* — is today enforced by the **caller's restraint**, not by the module's published contract. A single `PATCH /bins/{id}` against `all_crud_routes()` rewrites `actual_qty`/`valuation_rate`/`stock_value` with no row lock, no SLE, no GL post, and `repost_*` cannot repair it (it rebuilds from header, never re-touches SLE/Bin). ADR-0008 RLS does not save you — the leak runs under the app's own scoped role. This is the one move that converts a convention-held invariant into an enforced contract; everything else is downstream.

- **Residual negative value:** ~2-4h to land (refactor + extend `route_surface_guard.rs` to assert `create_bin_routes` is absent from the composed surface, plus a `consumer_surface` test that the only public bin mutation path is via `submit_*`). You lose the rarely-legitimate operator affordance of editing a bin row over HTTP; the genuine admin need (correcting a mis-seeded bin) is better served by a scoped `bin_adjust` endpoint that itself emits an SLE. Tiny risk a sibling workspace crate was reaching for `BinService` directly — `cargo check --workspace` surfaces that in minutes.
- **Reversibility:** easy. If a real workflow needs direct bin writes, re-introduce them behind a dedicated, SLE-emitting handler — never the generic 12-endpoint surface.
- **What would flip this:** the 10-min probe (scratch DB, seed bin, `UPDATE bins SET stock_value = stock_value + 1` via the app role, `SELECT b.stock_value - SUM(s.stock_value_difference) AS drift …`) returning **non-zero in production today**. That would promote "data reconciliation + backfill" ahead of the route fix, because the damage is already in the books. Or: a named operator workflow whose documented requirement is hand-editing bin balances — then the call inverts to "keep the route, wrap it in SLE emission."

Direct answer to the user's two questions, on this lens:
- **Complete?** No. Three concrete gaps surfaced by multiple seats independently: reversal/cancellation paths, FIFO declared but unimplemented, currency hardcoded to `IDR`. Plus the contract leak above. None are cosmetic.
- **Can we improve?** Yes — and the highest-leverage improvement is closing the leak, because it is the *only* finding that makes the bounded context a real bounded context rather than a polite one.

## Disagreement map

- **Convention vs. contract on bin mutation** — Steelman says the context is clean because the engine path is well-formed and tested; Skeptic (backed by the orchestrator's confirmation of `lib.rs:92-121`) says the invariant is convention-only because the public API exports a generic write surface over a running-balance table. **Crux:** does "bounded context" require the invariant to be *unreachable to violate* through the published surface, or merely *correct on the happy path*? On this lens (cleanliness), the former wins — the latter is "clean engine, leaky membrane."

- **Cancel/reversal: build it or delete the vocabulary?** — ddd-bounded-context and domain-expert want `cancel_purchase_receipt` / `cancel_delivery_note` (compensating SLE + `posting_type:"reversal"`); yagni-business agrees reversal is "the next real dollar." ddd-bounded-context floats the alternative: remove `is_cancelled` / `reverses_post_id` / `doc_status::cancelled` until they're real. **Crux:** is there a known production flow (purchase return, customer return, mis-posted receipt) that needs reversal *now*? Domain-expert asserts yes; without that fact, removal is cheaper. This is genuinely blocked on one piece of operator workflow knowledge — but it is a *completeness* question, not a *cleanliness* question, so it ranks below the Best call on this lens.

- **FIFO: rip it out or wire it up?** — yagni-business says remove from enum (one migration, re-add when real); domain-expert lists it as a missing real rule. Both agree the *current* state — enum value persisted on `StockItem`, accepted by `create_stock_item`, silently ignored by `submit_*` — is a latent correctness bug. **Crux:** no seat defends the status quo; the only disagreement is direction. On a cleanliness lens, removal wins (smaller surface, no false promise in the ubiquitous language).

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | **Close the bin-CRUD leak**: drop `create_bin_routes` from `all_crud_routes()` and `routes()`; make `BinService` `pub(crate)`; extend `route_surface_guard.rs` to assert absence. | high | ~2-4h; lose direct-bin-edit affordance; small caller-break risk caught by `cargo check --workspace` | easy | Probe showing prod drift already non-zero → reorder with data fix; or documented operator workflow requiring hand edits |
| 2 | **Remove `fifo` from `valuation_method` enum** (yagni-business) — one migration, eliminates silent misvaluation. | high | One migration + any `StockItem` rows already set to `fifo` (probe: `SELECT count(*) WHERE valuation_method='fifo'`) | easy (re-add when wired) | Any production tenant with `valuation_method='fifo'` set → keep enum, prioritize wiring the branch instead |
| 3 | **Resolve the cancelled-state vocabulary** — either ship `cancel_purchase_receipt`/`cancel_delivery_note` (compensating SLE, inverse residual-flush, `posting_type:"reversal"`) or delete `is_cancelled`/`reverses_post_id`/`doc_status::cancelled`. | med-high | Build path is ~1-2 days; remove path is ~1h. Either fixes the language/behavior collision ddd-seat flagged | easy if remove; costly-but-reversible if build | One fact: does any current operator workflow need returns/reversals now? (cheap probe: ask ops, or grep support tickets) |
| 4 | **Re-export `InventoryWriteService` + `New*` + `InventoryError` at crate root** (contract-seat) — the module's most important contract (validated movement) is currently neither an HTTP promise nor a prominent library entry point. | med | ~30 min; tiny increase in public API surface | easy (additive) | Evidence that downstream callers are expected to compose their own service rather than consume this one |
| 5 | **Thread currency off the hardcoded `"IDR"`** in all 5 envelopes (domain-expert). | med | ~1-3h; touches every envelope + tests | easy | If module is genuinely single-tenant single-currency by design (then hardcode becomes a documented invariant, not a bug) |
| 6 | **Add a reconciler backstop**: scheduled diff of `bin.stock_value` vs `SUM(sle.stock_value_difference) BY (item, warehouse)`, alert on non-zero. | med | ~1 day; defense-in-depth, not prevention — only *detects* the leak #1 prevents | easy | If #1 is rejected for operator-edit reasons, this becomes the primary control instead |
| 7 | **Tactical cleanups** (Steelman's honest list): typed 500 in availability read; split `NewTransfer` off `DeliveryLine` (no `rate` in a transfer); classify `GlRejected` transient/permanent; replace string-match `route_surface_guard` with parsed route registry; add HTTP-level negative-qty guard alongside the service one. | low | ~half-day batch | easy | None — these are unambiguously net-positive, just low-leverage vs. the invariant fix |

## Maturity scorecard

Focus is **bounded-context-cleanliness**, not maturity — scorecard skipped per contract.

## Parking lot

- **Transfer-transfer deadlock** on opposing directions (A→B and B→A same item, same voucher) — raised by Skeptic; source-before-target only orders within one voucher. Scope: inventory engine, not the membrane. Out of lens.
- **No `repost_reconciliation`** — a crash between `tx.commit` and `mark_not_applicable` leaves a net==0 recon permanently in limbo. Raised by Skeptic. Out of lens (engine robustness, not context boundary).
- **`is_dup` deliberately leaks `sqlx::Error`** — Steelman flagged as justified trade-off. Out of lens.
- **Logical-only FK independence** — RI is the app's job; document but don't change. Out of lens.
- **`route_layer`-not-`layer` discipline, `all_crud_routes` admin-only status** — depend on the Best call landing cleanly. Tracked under #1.

---

## Relevant paths

- `src/lib.rs` — lines 41, 92-121, 128-131 (the leak)
- `src/application/service/inventory_write_service.rs` — the engine hub whose uniqueness the leak undermines
- `tests/route_surface_guard.rs` — the test to extend for move #1
- `schema/models/` — `valuation_method` enum (move #2) and `doc_status`/`is_cancelled`/`reverses_post_id` (move #3)
- `src/application/service/inventory_gl.rs` — the 13-line seam whose guarantees are silently undermined while the leak stands
