<!-- Reader: Maintainer · Mode: How-to -->
# Maintainer Guide

How to maintain `backbone-inventory` and add features without breaking the regeneration machine. The
one rule to keep: **edit the schema YAML, then regenerate; put hand-written code only where the
generator promises not to touch it.** In this module that hand-written code is not a footnote — it is
the valuation engine and the GL seam, so this guide spends most of its length on *where it lives and
how it survives regen*.

All commands below were run against `metaphor 0.2.0`. Where the top-level README differs (it still
describes the skeleton), this guide has the working form.

## Before you touch anything

- Read this project's [`CLAUDE.md`](../../CLAUDE.md), [ADR-001](../adr/ADR-001-inventory-boundary-and-valuation.md),
  and [ADR-002](../adr/ADR-002-gl-posting-seam.md) — they encode the invariants you must not break.
- Confirm the project type is **`module`**: a **library** (`[lib]` only). Never add `main.rs` or a
  binary target.
- Internalize the source of truth: **`schema/models/<entity>.model.yaml`**. Generated code is
  downstream.

## Where code goes (and what it may depend on)

| Layer | Directory | Generated | Your hand-written code goes… |
|-------|-----------|-----------|------------------------------|
| Domain | `src/domain/` | entities, enums, repository traits | rarely — invariants that belong on the entity, via CUSTOM markers |
| Application | `src/application/` | `<Entity>Service` aliases, DTOs, validators, auth | **here** — `inventory_write_service.rs`, `inventory_gl.rs`, `inventory_read.rs`, `inventory_intake.rs`, `inventory_events.rs` |
| Infrastructure | `src/infrastructure/` | repository newtypes | custom repo methods `GenericCrudRepository` can't express |
| Presentation | `src/presentation/`, `src/routes/` | per-entity read handlers, DTOs | **`http/guarded_routes.rs`** |
| Composition | `src/lib.rs` | `InventoryModule` + builder | inside the `// <<< CUSTOM` builder hooks |

Dependency arrows point inward. If the domain layer imports `axum` or `sqlx`, something is in the
wrong layer.

## The composition root is `lib.rs`, not `module.rs`

Wiring lives in **[`src/lib.rs`](../../src/lib.rs)** — `InventoryModule` holds the twelve services and
`InventoryModuleBuilder::build()` constructs them from the pool. (The file
[`src/module.rs`](../../src/module.rs) is leftover skeleton — a single-`Example` `Module` that is not
in the crate's module tree. Ignore it; do not wire into it.)

Two route surfaces come off the module:

- `InventoryModule::all_crud_routes()` — **unguarded** generic CRUD on every entity. Admin/seeding
  only. The plain `routes()` alias is `#[deprecated]` because a naive mount exposes unvalidated writes.
- `create_guarded_inventory_routes(&module, pool)` in `presentation/http/guarded_routes.rs` — the
  **recommended** production surface: read models + validated creates, no direct SLE/Bin write.

## Adding a new entity (the golden path)

Say you want a `Batch`.

```bash
# 1. Describe it. Either scaffold a schema stub…
metaphor make entity Batch --module inventory
#    …or copy schema/models/warehouse.model.yaml → batch.model.yaml and edit it,
#    then add `- batch.model.yaml` under `imports:` in schema/models/index.model.yaml
#    (parents before children).

# 2. Validate the schema before generating.
metaphor schema schema validate inventory

# 3. Generate all artifacts (entity, DTOs, repo, service, read handler, routes).
metaphor schema schema generate inventory --target all --force

# 4. Generate the migration for the new entity.
metaphor migration generate Batch inventory

# 5. Apply migrations.
metaphor migration run

# 6. Register the service in the composition root (see below), then:
metaphor dev test
```

> `--target` accepts a comma-separated subset (e.g. `--target dto,handler`) to regenerate part of the
> cake. Run `metaphor schema schema generate --help` for the full target list, and `--dry-run` to see
> changes before writing.

### Step 6 in detail — wire the service into `InventoryModule`

Generation does **not** edit the composition root. Open [`src/lib.rs`](../../src/lib.rs) and follow the
existing twelve-service pattern:

```rust
pub struct InventoryModule {
    // …existing services…
    pub batch_service: Arc<BatchService>,          // ← add the field
}

// in InventoryModuleBuilder::build():
let batch_repository = Arc::new(BatchRepository::new(db_pool.clone()));
let batch_service    = Arc::new(BatchService::with_repository(batch_repository.clone()));
Ok(InventoryModule { /* …existing…, */ batch_service })   // ← return it

// add its re-export at the top of lib.rs:
pub use application::service::BatchService;
```

Then decide its route surface: add a generated `create_batch_read_routes(...)` merge to
`create_guarded_inventory_routes` if callers should read it, and only add it to `all_crud_routes()` if
unguarded CRUD is genuinely wanted.

## Maintaining the valuation engine & GL seam (the part unique to inventory)

The domain logic lives in five hand-authored, user-owned files under `src/application/service/`:

| File | Owns |
|------|------|
| `inventory_write_service.rs` | The moving-average engine: `submit_purchase_receipt`, `submit_delivery_note`, `submit_transfer`, `submit_reconciliation`, `repost_*`, plus the `load_or_init_bin` / `set_bin` / `write_sle` helpers. `InventoryError` and its `code()`/`http_status()`. |
| `inventory_gl.rs` | The outbound port: `GlPostSink` trait, `AccountingPostEnvelope`, `GlPostLine`, `GlPostAck`/`GlPostRejected`. The `is_balanced()` invariant. |
| `inventory_read.rs` | `InventoryReadService::availability(...)` — the read model selling consumes. |
| `inventory_intake.rs` | `DeliveryIntake` — the selling↔inventory delivery seam (`DeliveryRequested`). |
| `inventory_events.rs` | `InventoryEvent` (`StockReceived` / `StockDelivered` / `StockMoved` / `StockReconciled`) and the sink. |

**Rules for changing them:**

1. **Preserve the invariants the golden cases lock.** Any edit here must keep: `Σ SLE = Bin`; outflow
   leaves the rate unchanged; the residual flush lands `stock_value` on exactly 0 at zero stock; the
   physical movement commits *before* the GL post; `insufficient_stock` rejects with no partial move.
   The oracle is [`docs/business-flows/golden-cases.md`](../business-flows/golden-cases.md) mirrored by
   `tests/valuation_golden_cases.rs`, `tests/gl_posting_seam.rs`, and `tests/integrity_probes.rs`.
   Change the behavior → change the golden case *and* the test in the same PR.
2. **Money math goes through `money()` (2dp half-up) and `rate6()` (6dp).** Never introduce an `f64`
   or a bare division for a stored value.
3. **Every valuation-changing movement writes an SLE and updates the Bin in one transaction**, then
   emits a balanced envelope. If you add a movement type, follow the receipt/delivery pattern exactly.
4. **New GL post?** Build an `AccountingPostEnvelope` with `source_type = "inventory"`,
   `source_id = <voucher_id>`, balanced lines, and route it through `emit_and_reconcile` so
   `posting_state` and repost behavior stay consistent.

## Regen-safety — the rules that keep your logic alive

Regeneration **overwrites everything outside a protected region.** Three mechanisms; know which one
you're using.

### 1. `// <<< CUSTOM … // END CUSTOM` markers (inside generated files)

The generator preserves whatever sits between the markers. `lib.rs` ships builder hooks:

```rust
// in InventoryModuleBuilder::build()
// <<< CUSTOM
// END CUSTOM
```

Marker spelling varies by file (`// <<< CUSTOM METHODS START >>>`, `// <<< CUSTOM SERVICES START >>>`,
…). **Match the spelling already in the file**; add code between the existing pair, don't invent new
marker text. Use markers for small additions.

### 2. Whole hand-authored files (never generated, never overwritten)

The valuation engine files above are the canonical example — the generator never emits a file named
`inventory_write_service.rs`, so it never touches it. They're wired in through the surrounding
`mod.rs` under a `// <<< CUSTOM` marker so the `mod` declaration survives too.

### 3. `user_owned` globs in `metaphor.codegen.yaml`

[`metaphor.codegen.yaml`](../../metaphor.codegen.yaml) lists paths the generator skips **wholesale** —
never reads, merges, or deletes. This module protects the hand-written services, the guarded routes,
`tests/features/**`, and `docs/**`. Add a path here when you want a whole file immune to generation:

```yaml
user_owned:
  - "src/application/service/inventory_write_service.rs"
  - "src/application/service/inventory_gl.rs"
  - "src/presentation/http/guarded_routes.rs"
  - "tests/features/**"
  - "docs/**"
```

**Which to reach for:** a few lines → a CUSTOM marker; a cohesive unit of logic → a whole file listed
in `user_owned`; an entire hand-owned subtree → a glob.

## Changing an existing entity

1. Edit the field in `schema/models/<entity>.model.yaml` (the SSoT — never the generated struct).
2. `metaphor schema schema validate inventory`.
3. `metaphor migration generate <Entity> inventory` (or a schema-diff migration against a live DB via
   `metaphor schema schema migration inventory --database-url …`).
4. `metaphor schema schema generate inventory --target all --force`.
5. `metaphor migration run && metaphor dev test`.

If the changed entity is one the write service reads or writes with **raw SQL** (`StockLedgerEntry`,
`Bin`, the documents), grep `inventory_write_service.rs` for the column and update the hand-written
query — the generator won't do it for you, and a compile-time SQLx check or a golden case will catch
you if you forget.

## Reposting a stuck GL post

A voucher whose movement committed but whose post is `failed` (or a crash-window `pending`) is **not
terminal**. Re-drive it — this rebuilds the *same* envelope, so accounting's dedupe returns the
original journal, never a second:

```rust
// service/job code, given the same GlPostSink the submit used
let outcome = write_service.repost_purchase_receipt(receipt_id, &sink).await?;
// or: write_service.repost_delivery_note(delivery_id, &sink).await?;
```

An already-`posted` voucher short-circuits (returns the recorded ids); `not_applicable` is a no-op.
Proven by golden cases ISEAM-6 / ISEAM-7.

## Build, test, lint

```bash
metaphor dev test          # unit + integration + golden cases + the GL seam test
metaphor lint check        # clippy + fmt policy
metaphor dev serve         # run the composing service locally
```

Never run bare `cargo build`/`cargo test` from the workspace root — each project has its own
`Cargo.toml`; use the `metaphor` wrappers so workspace policy applies. Inside *this* module directory,
`cargo test` works but `metaphor dev test` is preferred. To confirm the accounting edge stays
dev-only: `cargo tree -e normal -i backbone-accounting` must be empty.

## Versioning & release

- Versioned in [`Cargo.toml`](../../Cargo.toml) (`0.1.3` today). Bump per conventional-commits:
  `fix:` → patch, `feat:` → minor, `feat!:` / `BREAKING CHANGE` → major.
- Before releasing: `metaphor dev test` and `metaphor lint check` clean; the empty-accounting-edge
  check passes.
- Pin the `backbone-*` git deps to a tag/rev for any release build (see [Technology](03-technology.md)).
- Conventional commits, **no Claude / co-author signature** — see [Contributing](07-contributing.md).

## What will break things

- **Editing generated code outside a CUSTOM marker** — silently overwritten on the next
  `generate --force`. The number-one regression.
- **Breaking a valuation invariant** — an outflow that changes the rate, a movement that skips the
  SLE, a value that ties to 0.01 instead of 0.00. The golden cases exist to catch this; run them.
- **Giving inventory a normal Cargo edge to accounting** — the seam depends on there being *none*.
  Keep accounting a dev-dependency.
- **Adding `main.rs` / a binary target** — wrong project type; a module is a library.
- **Mounting `all_crud_routes()` in production** — that's the unguarded surface. Use the guarded one.
- **Touching a sibling module's schema** — reference other modules by logical FK, never edit theirs.

---

Next: [Developer Guide](06-developer-guide.md) if you are integrating inventory rather than maintaining it.
</content>
