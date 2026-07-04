<!-- Reader: All · Mode: Reference -->
# Glossary — ubiquitous language

One term, one meaning, used everywhere in this handbook and in the code. When a term names a type or
file, that name is exact. If a doc uses a different word for one of these, the doc is the bug.

## Inventory domain terms

### Stock Ledger Entry (SLE)
The append-only, immutable ledger entry every valuation-changing movement writes — inventory's
equivalent of a GL journal line. One per (item, warehouse) per movement, carrying `actual_qty`
(signed), `qty_after_txn`, `valuation_rate`, `stock_value`, and `stock_value_difference` (the GL
amount). Corrections are *new* entries, never edits. Entity `StockLedgerEntry`, table
`inventory.stock_ledger_entries`. `Σ SLE` always reconstructs the `Bin`.

### Bin
The running per-(item, warehouse) balance: `actual_qty`, `reserved_qty`, moving-average
`valuation_rate`, and `stock_value`. The **authoritative current balance**, updated in lockstep with
every SLE inside one transaction. Entity `Bin`, table `inventory.bins`. Loaded `FOR UPDATE` during a
movement so concurrent movements serialize (no oversell).

### Moving average
The default valuation method. A **receipt** blends the new cost into the rate
(`rate = value / qty`); an **outflow** (delivery/transfer) consumes the current average as COGS and
**does not change the rate**. Other methods: `fifo` (opt-in, deferred), standard costing (cut).
Enum `ValuationMethod`.

### Residual flush
The rule that keeps the subledger tying to the GL at zero stock: on the final outflow that drains a
bin to `qty == 0`, the last units absorb the moving-average rounding residual — COGS = the entire
remaining value — so `stock_value` returns to **exactly 0.00**, never a stranded fraction of a cent.
Golden case IVC-8.

### Voucher / voucher type
A source document that produces SLEs: **Purchase Receipt** (inbound), **Delivery Note** (outbound),
**Stock Entry** (internal transfer), **Stock Reconciliation** (count adjustment). Enum `VoucherType`;
`voucher_no` is the human reference; `sle_no` sequences the SLEs within one voucher and, with
`(voucher_type, voucher_id)`, forms the idempotency key.

### GR/IR clearing
Goods-Received / Invoice-Received: a non-party current-liability account credited by a Purchase
Receipt (`Dr Inventory · Cr GR/IR`) and cleared later when the supplier invoice posts. Being
non-party satisfies the GL contract's "party required iff AR/AP" rule without a party on the line.

### COGS
Cost of Goods Sold — the expense a Delivery Note debits (`Dr COGS · Cr Inventory`) at the consumed
moving-average value.

### AccountingPost / envelope
The balanced posting inventory emits to the GL, carried as an `AccountingPostEnvelope` (in
`inventory_gl.rs`): `source_type = "inventory"`, `source_id = voucher_id`, and debit/credit
`GlPostLine`s that sum equal (`is_balanced()`). Accounting dedupes on `(company, source_type,
source_id, posting_type)`, making re-emits idempotent.

### `GlPostSink`
The outbound port (a trait) through which inventory emits its envelope. A composing service or the
seam test implements it over accounting's `PostingService`. The shipped library has **zero normal
Cargo edge** to accounting; the envelope is the wire contract, the sink is the seam.

### `posting_state`
A voucher's GL reconciliation state: `pending` (not yet posted) → `posted` | `failed`, or
`not_applicable` (value-neutral movement, e.g. a transfer). Enum `GlPostingState`. A `failed` voucher
is **not terminal** — see *repost*.

### Repost
Re-driving a stuck GL post for a voucher whose physical movement committed but whose post is `failed`
or crash-window `pending` (`repost_purchase_receipt` / `repost_delivery_note`). Rebuilds the *same*
envelope, so accounting's dedupe returns the original journal — never a second. Golden cases
ISEAM-6/7.

### Eventually consistent (movement vs. post)
The physical movement (SLE + Bin) commits first, in one transaction; the GL post is emitted *after*
and reconciled into `posting_state`. A GL failure never rolls back the stock movement. This is the
model the GL-posting contract prescribes ([ADR-002](../adr/ADR-002-gl-posting-seam.md)).

### Warehouse
The **one master inventory owns** — a per-company tree (`parent_warehouse_id`, `is_group`). Group
nodes hold no stock. Entity `Warehouse`, table `inventory.warehouses`. Enum `WarehouseType`
(`stock`/`transit`/`wip`/`finished_goods`/`rejected`).

### StockItem (projection)
A read-only ACL projection of `catalog.Item` — the stock-relevant slice (`stock_uom`, `has_batch`,
`valuation_method`, `reorder_level`), refreshed by catalog events. **Never** the catalog god-entity;
inventory owns the projection, not the item.

### Guarded routes vs. unguarded CRUD
`create_guarded_inventory_routes()` — the **recommended** production surface: read models + validated
creates, no direct SLE/Bin write, no generic delete. `InventoryModule::all_crud_routes()` — the
**unguarded** admin/seeding surface (twelve generic CRUD endpoints per entity); its `routes()` alias
is `#[deprecated]`.

### DeliveryIntake
The selling↔inventory seam (`inventory_intake.rs`): consumes a `DeliveryRequested` (from a sales
order) and creates the delivery draft. Exposed at `POST /delivery-requests`.

### Inventory events
`StockReceived` / `StockDelivered` / `StockMoved` / `StockReconciled` (`inventory_events.rs`), emitted
after a movement. `StockDelivered` carries the source SO and `StockReceived` the source PO, so selling
and buying can reconcile their documents.

## Framework terms

### Aggregate / Entity
A domain object with identity and a lifecycle, defined by one `schema/models/<name>.model.yaml`.
Generated into `src/domain/entity/<name>.rs` with a strongly-typed id, a builder, `apply_patch`, and
audit accessors.

### Application / Domain / Infrastructure / Presentation layers
The four DDD layers. **Domain** (`src/domain/`): entities, enums, repository traits — depends on
nothing. **Application** (`src/application/`): services, DTOs, and the hand-written engine — depends on
domain. **Infrastructure** (`src/infrastructure/`): repository impls — depends on domain/application.
**Presentation** (`src/presentation/`, `src/routes/`): handlers and route composition — depends on
application.

### Audit metadata
The `metadata` JSONB field (`created_at`, `updated_at`, `deleted_at`, `created_by`, `updated_by`,
`deleted_by`) added by `config.audit: true`. Timestamps are trigger-set; the `*_by` actors are logical
FKs to `sapiens.User.id`.

### `BackboneCrudHandler`
The `backbone-core` type that produces an Axum `Router` with all **twelve** CRUD endpoints for an
entity. Backs the read handlers and the unguarded `all_crud_routes()`; you never hand-write these.

### Bounded context
The single business domain a module owns. One module = one bounded context. Inventory references other
modules by logical FK; it never edits their schema.

### Composition root
**`InventoryModule` + `InventoryModuleBuilder` in [`src/lib.rs`](../../src/lib.rs)** — wires each
service to its repository and composes routers. (`src/module.rs` is vestigial skeleton residue, not
the live root.)

### CUSTOM marker
A `// <<< CUSTOM … // END CUSTOM` region inside a generated file whose content survives regeneration.
Spelling varies per file — match what's already there.

### DTO
A wire-shape struct in `src/application/dto/` (and `src/presentation/dto/`): `Create…Dto`, `Update…Dto`,
`Patch…Dto`, `…ResponseDto`, `…Summary`, `…ListResponse`. Serialized `camelCase`; generated with
`From`/`Apply` conversions.

### `GenericCrudRepository` / `GenericCrudService`
The `backbone-orm` / `backbone-core` generics carrying standard CRUD. A repository is a **newtype**
over `GenericCrudRepository<Entity, SoftDelete>`; a service is a **type alias** over
`GenericCrudService<…>`. Inherited, never re-implemented.

### Logical foreign key
A cross-module reference declared with `@foreign_key(module.Type.field)` or
`@exclude_from_foreign_key_check`. Documents the relationship; **not** enforced by a DB constraint, so
modules stay independently deployable. Inventory's `item_id`, `company_id`, `supplier_id`, and the GL
account ids are all logical FKs.

### `metaphor`
The workspace CLI (v0.2.0) orchestrating projects and dispatching to plugins (`metaphor-schema`,
`metaphor-codegen`, `metaphor-dev`). Prefer it over raw `cargo`/`sqlx`. The `backbone-schema` binary the
top-level README mentions is **not** installed; use `metaphor schema schema …`.

### Module
A **library crate** owning one bounded context in 4-layer DDD, schema-driven. `[lib]` only — no
`main.rs`. Composed into a `backend-service`; never run alone.

### Own schema (per module)
Inventory gets its own Postgres schema (`schema: inventory`). Migrations `CREATE SCHEMA inventory` and
qualify tables as `inventory.<table>`, so modules never collide on a table name. Not to be confused
with the schema-YAML SSoT.

### Port / Adapter
The DDD names for the two repositories per entity: the **port** is the domain-layer `trait`; the
**adapter** is the infrastructure-layer `struct` (the Postgres implementation). The `GlPostSink` is
also a port — its adapter lives on the consumer side.

### Regeneration (regen)
Re-running `metaphor schema schema generate … --force` to rebuild downstream code from the schema.
Overwrites everything **outside** a protected region (CUSTOM markers, hand-authored files, `user_owned`
globs).

### Schema (the SSoT)
`schema/models/*.model.yaml` — the single source of truth. Every entity struct, DTO, migration,
repository, service, and read handler is generated from it.

### Soft delete
Marking a row deleted (`metadata.deleted_at` set) instead of removing it, enabled by
`config.soft_delete: true`. Every inventory read path filters `metadata->>'deleted_at' IS NULL`.

### Twelve endpoints
The standard CRUD surface every entity gets from `BackboneCrudHandler`: `list`, `create`, `get`,
`update`, `patch`, `soft_delete`, `restore`, `empty_trash`, `bulk_create`, `upsert`, `find_by_id`,
`list_deleted`. Mounted only by the *unguarded* surface.

### `user_owned`
The `metaphor.codegen.yaml` key listing glob paths the generator skips wholesale. Inventory protects
the engine files, `guarded_routes.rs`, `tests/features/**`, and `docs/**`.
</content>
