# backbone-inventory

The supply-chain **stock ledger of record** — a Backbone Framework domain module.

It owns an **append-only Stock Ledger (SLE)** plus a per-(item, warehouse) **`Bin`** running balance,
values stock with **moving average**, and is the supply-chain pillar's **only emitter of accounting
posts**: a Purchase Receipt posts `Dr Inventory · Cr GR/IR`, a Delivery Note posts `Dr COGS · Cr
Inventory`, and a Stock Reconciliation posts the value difference — into `backbone-accounting` over a
seam that keeps the two modules independently deployable.

It is a **library crate** (`[lib]` only, 4-layer DDD, schema-first). The entity structs, DTOs,
migrations, repositories, and read handlers are **generated** from `schema/models/*.model.yaml`; the
valuation engine, the GL-posting seam, and the guarded write surface are **hand-authored and
regeneration-safe**.

## Start here

**→ [`docs/README.md`](docs/README.md) — the handbook.** Everything below is a summary of it.

| You are… | Read |
|----------|------|
| Evaluating the design | [Philosophy](docs/handbook/01-philosophy.md) · [Background](docs/handbook/02-background.md) · [Technology](docs/handbook/03-technology.md) |
| Integrating the module | [Developer Guide](docs/handbook/06-developer-guide.md) |
| Extending the module | [Architecture](docs/handbook/04-architecture.md) · [Maintainer Guide](docs/handbook/05-maintainer-guide.md) |
| Opening a PR | [Contributing](docs/handbook/07-contributing.md) |
| Deciding what a word means | [Glossary](docs/handbook/08-glossary.md) |

Key decisions: [ADR-001 — inventory boundary & valuation](docs/adr/ADR-001-inventory-boundary-and-valuation.md) ·
[ADR-002 — the GL-posting seam](docs/adr/ADR-002-gl-posting-seam.md).

## The domain at a glance

| Concept | What it is |
|---------|-----------|
| **StockLedgerEntry (SLE)** | Immutable, append-only ledger line per movement. `Σ SLE` reconstructs the `Bin`. Corrections are new entries, never edits. |
| **Bin** | The authoritative current balance: on-hand qty, moving-average rate, value. Updated with each SLE in one transaction. |
| **Purchase Receipt** | Inbound goods → SLE incoming, Bin blended → `Dr Inventory · Cr GR/IR`. |
| **Delivery Note** | Outbound → SLE outgoing at the current average (rate unchanged) → `Dr COGS · Cr Inventory`. |
| **Stock Entry (transfer)** | Internal move → paired out/in SLE at source rate — value-neutral, no GL post. |
| **Stock Reconciliation** | Adjust to a physical count → SLE for the delta → signed value-difference post. |
| **Warehouse** | The one master this module owns (per-company tree). Everything else is a logical FK. |

Inventory owns **only** `Warehouse`. Item (catalog), company/branch (organization), supplier/customer
(party), and GL accounts (accounting) are all logical foreign keys — **zero horizontal Cargo edges**.

## Golden path

```bash
metaphor schema schema validate                 # check the schema YAML (the source of truth)
metaphor migration run                          # CREATE SCHEMA inventory + all tables
metaphor dev test                               # unit + valuation golden cases + GL-seam test
metaphor lint check
```

Never run bare `cargo build`/`cargo test` from the workspace root — use the `metaphor` wrappers so
workspace policy applies. To confirm the accounting edge stays test-only:
`cargo tree -e normal -i backbone-accounting` must be **empty**.

## Layout

```
schema/models/        # ← SOURCE OF TRUTH: the inventory documents, SLE, Bin, Warehouse, enums
migrations/           # generated PostgreSQL migrations (CREATE SCHEMA inventory)
src/
├── domain/           # generated: entities, enums, repository traits
├── application/
│   └── service/      # generated CRUD services + HAND-WRITTEN engine:
│                     #   inventory_write_service.rs  (moving-average SLE engine)
│                     #   inventory_gl.rs             (GlPostSink, AccountingPostEnvelope)
│                     #   inventory_read.rs           (availability read model)
│                     #   inventory_intake.rs         (selling↔inventory delivery seam)
├── infrastructure/   # generated: persistence repositories
├── presentation/
│   └── http/         # generated read handlers + HAND-WRITTEN guarded_routes.rs
├── exports/          # read-only consumer query surface for sibling modules
└── lib.rs            # InventoryModule — the composition root
tests/                # valuation_golden_cases.rs · gl_posting_seam.rs · integrity_probes.rs
docs/                 # the handbook, ADRs, business-flow oracle, schema reference
metaphor.codegen.yaml # user_owned globs — protects the engine, guarded routes, tests, docs from regen
```

## Composing it into a service

```rust
use backbone_inventory::InventoryModule;
use backbone_inventory::presentation::http::create_guarded_inventory_routes;
use backbone_auth::tenant::TenantVerifier;

let inventory = InventoryModule::builder().with_database(pool.clone()).build()?;
// The verifier is built once from your JWT secret; writes take their tenant from the token.
let verifier = TenantVerifier::hs256(jwt_secret.as_bytes());
let router = create_guarded_inventory_routes(&inventory, pool.clone(), verifier); // ← the guarded surface
// Mount under /api/v1. Supply a GlPostSink to post movements to your ledger.
// Do NOT mount InventoryModule::all_crud_routes() in production — that is the unguarded admin surface.
```

See the [Developer Guide](docs/handbook/06-developer-guide.md) for the full receive → submit →
deliver → check-availability walkthrough.

## Regeneration safety

Everything outside a protected region is overwritten by `metaphor schema schema generate --force`.
Hand-written code survives in one of three places: `// <<< CUSTOM … // END CUSTOM` markers, whole
files the generator never emits (the `inventory_*` engine files), or paths listed under `user_owned`
in [`metaphor.codegen.yaml`](metaphor.codegen.yaml). See the
[Maintainer Guide](docs/handbook/05-maintainer-guide.md).
</content>
