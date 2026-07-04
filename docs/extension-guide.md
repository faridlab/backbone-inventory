# Extension Guide — backbone-inventory

> How to embed and extend `backbone-inventory` without forking it. Public contract per
> `docs/erp/extension-contract.md`.

## The public surface (stable)

> The stable, regen-safe public path is **`backbone_inventory::application::service::*`** (the
> CUSTOM-protected re-exports). The generated `src/exports/` tree is unwired scaffolding
> (`pub mod exports` is absent from lib.rs), so `application::service` is the authoritative surface.

**A. Availability read-model** (`InventoryReadService`) — what selling/buying consume to source stock:
`availability(company, item, warehouse) → AvailabilityView { actual_qty, reserved_qty, available_qty }`
(`available = actual − reserved`), `availability_across_warehouses(...)`, and
`stock_balance(...) → StockBalance`. HTTP: `GET /availability?companyId&itemId&warehouseId`.

**B. `DeliveryRequested` intake** (`DeliveryIntake::on_delivery_requested`) — the trigger of the
selling→inventory delivery seam: turns a `DeliveryRequested` into a **draft** Delivery Note (submit
is a separate `submit_delivery_note(sink)` step). HTTP: `POST /delivery-requests`.

**C. Domain events** (`application::service` — semantic `InventoryEvent`, NOT the generated CRUD one):

| Event | Fires when | Carries |
|-------|-----------|---------|
| `StockReceived` | a purchase receipt is submitted | receipt_id, company_id, warehouse_id, source_po_id, total_value |
| `StockDelivered` | a delivery note is submitted | delivery_id, company_id, warehouse_id, source_so_id, total_cogs |
| `StockMoved` | a transfer is submitted | entry_id, company_id, from_warehouse_id, to_warehouse_id |
| `StockReconciled` | a reconciliation is submitted | reconciliation_id, company_id, warehouse_id, net_difference |

`StockDelivered.source_so_id` is how `backbone-selling` advances its `delivered_qty`;
`StockReceived.source_po_id` is how buying reconciles a PO.

**B. The outbound GL port** — `inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink,
GlPostAck, GlPostRejected}`. A composing service implements `GlPostSink` (map envelope → its ledger);
the shipped library has zero normal Cargo edge to accounting.

## How a consumer extends
1. **Subscribe to a domain event** — implement `InventoryEventSink` in your crate / a `*_custom.rs`
   sibling and pass it to `InventoryWriteService::with_sink(pool, my_sink)`.
2. **Provide the GL sink** — implement `GlPostSink` over your accounting adapter; pass it to the
   `submit_*` methods (or a posting job).
3. **Keep your logic in `user_owned` / `*_custom.rs`** — regen skips those files, so your rules
   survive a module regeneration. Never edit generated code outside `// <<< CUSTOM` markers.

## Composing the HTTP surface
Mount `presentation::http::create_guarded_inventory_routes(&module, pool)` — read + validated create.
Direct SLE/Bin writes and generic mutation are not exposed; submitting is service/job-driven.

## Not a contract
`// <<< CUSTOM` blocks inside generated files (own edits only); internal repositories/services; the
generated CRUD events — prefer the semantic domain events above.

## Deferred surfaces (not yet stable)
Inbound projection sync (`ItemCreated` → StockItem), buying's `ReceiptExpected` intake, FIFO,
serial/batch, repost + global SLE ordering, landed cost — additive when built. (The §5
consumer-rule-survives-regen round-trip is built during the selling↔inventory wiring, once a real
consumer subscribes.)
