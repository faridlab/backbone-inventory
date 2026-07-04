# FSD — backbone-inventory

> Functional Spec. Tier 1 · Supply Chain. Date: 2026-07-04. Maps rules (BRD) to entities, the
> valuation engine, endpoints, states, and seams.

## Entities (schema/models/*.model.yaml — SSoT)
| Entity | Table | Notes |
|--------|-------|-------|
| Warehouse | `inventory.warehouses` | owned master; per-company tree (parent/is_group) |
| StockItem | `inventory.stock_items` | ACL projection of catalog.Item (logical `item_id`) |
| StockLedgerEntry | `inventory.stock_ledger_entries` | append-only; unique `(voucher_type,voucher_id,sle_no)` |
| Bin | `inventory.bins` | running `(actual_qty, valuation_rate, stock_value)` per (item,warehouse) |
| PurchaseReceipt / …Item | `inventory.purchase_receipts` | inbound; inventory+grir account refs; posting_state |
| DeliveryNote / …Item | `inventory.delivery_notes` | outbound; cogs+inventory account refs; posting_state |
| StockEntry / …Item | `inventory.stock_entries` | internal transfer (value-neutral) |
| StockReconciliation / …Item | `inventory.stock_reconciliations` | count adjustment; inventory+adjustment account refs |

All cross-module ids are logical FKs (`@exclude_from_foreign_key_check`): `item_id`→catalog,
company/branch→organization, supplier/customer→party, GL accounts→accounting, source PO/SO→buying/selling.

## Services (application/service — hand-authored, user_owned)
- `InventoryWriteService` — the valuation engine + validated writes:
  - masters: `create_warehouse`, `create_stock_item`
  - drafts: `create_purchase_receipt`, `create_delivery_note`
  - submits: `submit_purchase_receipt(sink)` (SLE in + Bin + asset post), `submit_delivery_note(sink)`
    (SLE out + Bin + COGS post), `submit_transfer` (paired SLE, no GL), `submit_reconciliation(sink)`
    (SLE adjust + Bin + difference post)
  - helpers: `load_or_init_bin` (FOR UPDATE), `set_bin`, `write_sle`, `emit_and_reconcile`
- `inventory_gl` — outbound GL port: `AccountingPostEnvelope`, `GlPostLine`, `GlPostSink`, ack/reject.
- `inventory_events` — domain events + `InventoryEventSink`.

## HTTP surface (presentation/http/guarded_routes.rs)
`create_guarded_inventory_routes(&InventoryModule, pool)` — read (warehouse/bin/SLE/receipt/delivery)
+ validated create (warehouse, purchase-receipt, delivery-note). Direct SLE/Bin writes and generic
mutation are NOT mounted. Submitting a movement needs a `GlPostSink` → service/job-driven.

## State machines
- Receipt / Delivery / Reconciliation: `draft → submitted` (→ cancelled). `posting_state`
  (pending → posted | failed | not_applicable) is an independent GL-reconciliation axis.

## Integration seams
- **Outbound GL (proven):** `submit_*` → `GlPostSink` → accounting `PostingService` (envelope →
  PostingRequest ACL). `source_type=inventory`, idempotent on voucher id. See ADR-002,
  `tests/gl_posting_seam.rs`.
- **Outbound events:** `InventoryEventSink` publishes the 4 domain events.
- **Read-model (built):** `InventoryReadService.availability/stock_balance` → `AvailabilityView`/
  `StockBalance` (`GET /availability`) — the surface selling/buying source stock from.
- **Inbound intake (built):** `DeliveryIntake.on_delivery_requested` (`POST /delivery-requests`) →
  a draft Delivery Note (selling→inventory delivery seam trigger).
- **Inbound (future):** `ItemCreated/Updated` → StockItem projection; buying's `ReceiptExpected`.

## Test oracle
`valuation_golden_cases` (7), `gl_posting_seam` (5, real ledger), `integrity_probes` (4). **16 tests.**
