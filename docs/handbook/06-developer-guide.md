<!-- Reader: App developer · Mode: Tutorial → How-to -->
# Developer Guide

Get from an empty service to moving stock: compose the module, mount its guarded routes, receive
goods, deliver them, and read live availability. The tutorial part holds your hand once; the recipes
assume you know your way around.

Commands here were run against `metaphor 0.2.0`. The top-level [README](../../README.md) still
describes the *skeleton* this module came from (an `Example` entity, `backbone-schema` commands) —
ignore that and use the `metaphor` forms below.

## Prerequisites

- **Rust** (2021 edition) and **Cargo**.
- The **`metaphor`** CLI on your `PATH` (`metaphor --version` → `metaphor 0.2.0` or newer).
- A reachable **PostgreSQL** instance.
- A composing **`backend-service`** (inventory is a library — it does not run alone), and something
  that plays the **general ledger**: for real posting, `backbone-accounting`; for a first run, a stub
  `GlPostSink` is enough.

## Two things to understand before you start

1. **Inventory is a library you compose, not a server you launch.** Your service builds an
   `InventoryModule`, mounts its router, and supplies a `GlPostSink`.
2. **Creating a document ≠ moving stock.** A Purchase Receipt or Delivery Note is created as a
   **draft** (HTTP). The movement — writing the Stock Ledger, updating the Bin, and posting to the
   GL — happens when the document is **submitted**, and submit needs a `GlPostSink`, so it is
   **service/job-driven, not a bare HTTP route.** The guarded HTTP surface gives you creates + reads;
   the submit is a Rust call.

## Install — compose the module into a service

```toml
# In your backend-service Cargo.toml
[dependencies]
backbone-inventory = { path = "../backbone-inventory" }   # or git, pinned to a tag for releases
```

Prepare the database (from the module directory, or point the CLI at it):

```bash
export DATABASE_URL="postgresql://root:password@localhost:5432/inventorydb"
metaphor schema schema validate     # check the schema YAML
metaphor migration run              # CREATE SCHEMA inventory + all tables
```

## Wire it up — the composition root

```rust
use backbone_inventory::InventoryModule;
use backbone_inventory::presentation::http::guarded_routes::create_guarded_inventory_routes;
use backbone_auth::tenant::TenantVerifier;

// pool: sqlx::PgPool
let inventory = InventoryModule::builder()
    .with_database(pool.clone())
    .build()?;

// RECOMMENDED: read models + validated, tenant-scoped creates. No direct SLE/Bin writes, no
// generic delete. Writes derive `company_id` from the signed Bearer token, never from the body.
let verifier = TenantVerifier::hs256(jwt_secret.as_bytes());
let router = create_guarded_inventory_routes(&inventory, pool.clone(), verifier);
//    → mount under /api/v1 in your service's Axum router.

// Do NOT use inventory.all_crud_routes() in production — that is the unguarded admin surface.
```

To actually post to the ledger, implement the sink over your GL (the adapter, not the module, knows
accounting):

```rust
use backbone_inventory::application::service::inventory_gl::{
    GlPostSink, AccountingPostEnvelope, GlPostAck, GlPostRejected,
};

struct AccountingSink { /* an accounting PostingService handle */ }

#[async_trait::async_trait]
impl GlPostSink for AccountingSink {
    async fn post(&self, env: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        // map env (source_type="inventory", balanced debit/credit lines) → your PostingRequest,
        // call the ledger, return the journal id. Dedupe is on (company, source_type, source_id).
    }
}
```

## Quickstart — receive, submit, deliver, read

### 1. Create a warehouse (HTTP, validated)

```bash
curl -s -X POST localhost:8080/api/v1/warehouses \
  -H 'content-type: application/json' \
  -d '{"companyId":"<company-uuid>","code":"WH-MAIN","name":"Main Store"}'
# → 201 { "id": "<warehouse-uuid>" }
```

### 2. Create a Purchase Receipt draft (HTTP, validated)

```bash
curl -s -X POST localhost:8080/api/v1/purchase-receipts \
  -H 'content-type: application/json' \
  -d '{
        "receiptNumber":"PR-0001",
        "companyId":"<company-uuid>",
        "supplierId":"<supplier-uuid>",
        "warehouseId":"<warehouse-uuid>",
        "postingDate":"2026-07-04",
        "inventoryAccountId":"<inventory-account-uuid>",
        "grirAccountId":"<grir-account-uuid>",
        "lines":[{"itemId":"<item-uuid>","quantity":"10","rate":"100"}]
      }'
# → 201 { "id": "<receipt-uuid>" }   (status = draft; nothing moved yet)
```

Note the JSON is **camelCase** (`receiptNumber`) though the DB/Rust are snake_case — the generated
`#[serde(rename_all = "camelCase")]` at work. Quantities and rates are decimal **strings** to keep
precision exact.

### 3. Submit it (Rust — writes the SLE, updates the Bin, posts to the GL)

```rust
use backbone_inventory::application::service::inventory_write_service::InventoryWriteService;

let write = InventoryWriteService::new(pool.clone());
let outcome = write.submit_purchase_receipt(receipt_id, &sink).await?;
// → SLE +10 written; Bin(item,WH-MAIN) = qty 10, rate 100.000000, value 1000.00;
//   GL post Dr Inventory 1000 · Cr GR/IR 1000; outcome.posted == true.
```

### 4. Deliver 5 (create draft over HTTP, submit in Rust)

```bash
curl -s -X POST localhost:8080/api/v1/delivery-notes \
  -H 'content-type: application/json' \
  -d '{"deliveryNumber":"DN-0001","companyId":"<company-uuid>","customerId":"<customer-uuid>",
       "warehouseId":"<warehouse-uuid>","postingDate":"2026-07-04",
       "cogsAccountId":"<cogs-account-uuid>","inventoryAccountId":"<inventory-account-uuid>",
       "lines":[{"itemId":"<item-uuid>","quantity":"5"}]}'
# → 201 { "id": "<delivery-uuid>" }
```

```rust
let outcome = write.submit_delivery_note(delivery_id, &sink).await?;
// → COGS = 5 × 110 (moving average, if a second receipt at 120 blended it) ; Bin qty drops by 5,
//   rate UNCHANGED; GL post Dr COGS · Cr Inventory. Delivering more than on-hand → InventoryError::
//   InsufficientStock (no partial movement).
```

### 5. Read live availability (HTTP)

```bash
curl -s 'localhost:8080/api/v1/availability?companyId=<company-uuid>&itemId=<item-uuid>&warehouseId=<warehouse-uuid>'
# → 200 { on-hand qty, reserved qty, valuation rate, stock value }  — the surface selling checks.
```

Expected numbers for the canonical sequence are the [golden cases](../business-flows/golden-cases.md)
(IVC-1…9) — that file is the oracle the tests assert against.

## Key concepts

- **Schema YAML is the source of truth.** You edit [`schema/models/*.model.yaml`](../schema/RULE_FORMAT_MODELS.md);
  entities, DTOs, migrations, repositories, and read handlers are generated. The valuation engine and
  GL seam are hand-written and regen-safe. ([Philosophy](01-philosophy.md).)
- **The Bin is the current balance; the SLE is the history.** Every movement writes an immutable SLE
  and updates the Bin in one transaction. `Σ SLE = Bin`, always.
- **Moving average.** A receipt blends the new cost into the rate; a delivery/transfer consumes the
  current average and leaves the rate unchanged.
- **Submit, then post.** The physical movement commits first; the GL post is eventually consistent
  (`posting_state` pending→posted|failed) and repostable.
- **Mount the guarded router.** `create_guarded_inventory_routes()` — not `all_crud_routes()` — is the
  production surface. ([Architecture](04-architecture.md).)

## Recipes

### How do I check stock before promising a sale?

Call `GET /availability` (above), or in-process `InventoryReadService::availability(company, item,
warehouse)`. This is exactly the surface `selling` consumes.

### How do I move stock between warehouses?

A transfer is a `StockEntry` and is **value-neutral** — no GL post. Drive it in Rust:

```rust
write.submit_transfer(NewTransfer {
    entry_number: "SE-0001".into(), company_id, from_warehouse_id, to_warehouse_id,
    posting_date, lines: vec![DeliveryLine { item_id, quantity: dec!(4) }],
}).await?;   // paired out/in SLE at the source rate; total value conserved. Same warehouse → SameWarehouse.
```

### How do I correct stock to a physical count?

A `StockReconciliation` sets each bin to the counted figure and posts the signed value difference:

```rust
write.submit_reconciliation(NewReconciliation {
    recon_number: "SR-0001".into(), company_id, warehouse_id, posting_date,
    inventory_account_id, adjustment_account_id,
    lines: vec![ReconLine { item_id, counted_qty: dec!(8), counted_rate: dec!(0) }], // 0 = keep rate
}, &sink).await?;   // shrinkage → Dr Adjustment · Cr Inventory; gain → the reverse.
```

### How do I recover a delivery/receipt whose GL post failed?

Re-drive it — idempotent, returns the original journal if it actually posted:

```rust
write.repost_purchase_receipt(receipt_id, &sink).await?;   // or repost_delivery_note(...)
```

### How does selling trigger a delivery?

Through `DeliveryIntake` — `POST /delivery-requests` (or the in-process `on_delivery_requested`)
creates the delivery draft from a `DeliveryRequested`. The submit still runs service-side with a sink.

## Configuration

Defaults live in [`config/application.yml`](../../config/application.yml); override per environment.

| Option | Default | When to change |
|--------|---------|----------------|
| `server.host` | `0.0.0.0` | Bind to a specific interface. |
| `server.port` | `8080` | Port conflicts / multi-service hosts. |
| `database.url` | `postgresql://root:password@localhost:5432/skeletondb` | **Always** in real deployments — override with the `DATABASE_URL` env var, which takes precedence. |
| `database.max_connections` | `10` | Tune to your Postgres pool budget. |
| `logging.level` | `info` | `debug`/`trace` when diagnosing. |
| `features.workflows` | `true` | The goods-movement workflow. |

Layering: `application.yml` (base) → `application-dev.yml` / `application-prod.yml` (overrides).
`DATABASE_URL` in the environment always wins over the YAML.

## Troubleshooting

| Symptom / error `code` | Cause | Fix |
|------------------------|-------|-----|
| `insufficient_stock` (422) | Delivery/transfer exceeds on-hand; no partial movement is made | Receive or transfer stock first; negative stock is disallowed by design. |
| `empty_document` (422) | A receipt/delivery/recon submitted with no lines | Add at least one line. |
| `not_draft` (422) | Submitting a document that is already `submitted`/`cancelled` | A movement posts once; use `repost_*` if the *GL post* (not the movement) is stuck. |
| `same_warehouse` (422) | Transfer with `from == to` | Pick distinct warehouses. |
| `duplicate_number` (422) | `receipt_number`/`delivery_number`/`recon_number`/warehouse `code` collides | Numbers are unique per document type; pick a fresh one. |
| `posting_state = failed` after submit | The GL rejected the post (e.g. `non_postable_account`); **the stock movement stood** | Fix the account, then `repost_*`. The SLE + Bin are intact. |
| `405/404` on `POST /stock-ledger-entries` or `DELETE /purchase-receipts/{id}` | The guarded surface deliberately does not expose raw SLE writes or generic delete | Correct: use submit/reconciliation, never a direct ledger write (golden IIP-1…2). |
| `backbone-schema: command not found` | Following the stale README | Use `metaphor schema schema …`. |
| Amounts off by a cent vs. the GL | Expecting `f64` math | All money is `rust_decimal` (2dp) / rate (6dp); send quantities/rates as strings. |

---

Next: [Contributing](07-contributing.md) to send a change back, or the [Glossary](08-glossary.md) to
pin down a term.
</content>
