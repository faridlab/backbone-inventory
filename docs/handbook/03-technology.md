<!-- Reader: Evaluator + Maintainer · Mode: Explanation -->
# Technology & the "why"

Every dependency in [`Cargo.toml`](../../Cargo.toml) earns its place. This page gives each
significant choice a one-line rationale and names the alternative that was rejected, so an evaluator
can judge the stack and a maintainer knows *why* not to swap a piece out casually.

Versions below are what `backbone-inventory` pins at **v0.1.3**; where behavior is version-specific,
the version is called out.

## The choices

| Layer | Choice | Why | Rejected alternative |
|-------|--------|-----|----------------------|
| Language | **Rust 2021**, `[lib]` only | Memory safety + a type system strong enough to make generated code *provably* consistent; no GC pauses in a service hot path | Go (weaker types for the generated-DTO story), Kotlin (the mobile edge, not the domain core) |
| Async runtime | **Tokio 1.x** (`full`) | The de-facto async runtime; Axum and SQLx both build on it, so there is one reactor | `async-std` (smaller ecosystem, no Axum/SQLx alignment) |
| HTTP | **Axum 0.7** (+ `tower`, `tower-http`) | Composes as a plain `Router` — exactly what the read handlers and the guarded write routes return and the module merges; Tower middleware and first-class extractors | `actix-web` (its actor model fights the compose-a-Router design) |
| Database | **PostgreSQL** via **SQLx 0.8** | Queries checked at compile time; native `enum`, `uuid`, `jsonb`, and — critically here — **`decimal`** support via the `rust_decimal` feature | Diesel (heavier macro layer, less async-native), a runtime-only ORM |
| **Money & quantity** | **`rust_decimal` 1.36** (with the SQLx `rust_decimal` feature) | Inventory is *all* money and quantity: `stock_value`/GL amounts at **2dp half-up**, `valuation_rate` at **6dp**. Base-10 decimals make the residual-flush-to-zero rule exact | **`f64` money — categorically rejected.** Binary floats cannot represent `0.01` exactly; a moving-average subledger built on them will not tie to the GL |
| Domain errors | **`thiserror` 1.0** | Typed domain/service errors; the write service's `InventoryError` maps each variant to a stable `code()` (`insufficient_stock`, `empty_document`, `not_draft`, …) and an HTTP status | `anyhow` for domain errors (loses the typed variants the handler matches on) |
| Boundary errors | **`anyhow` 1.0** | Right tool at the *composition* boundary (`ModuleBuilder::build` returns `anyhow::Result`) where a typed enum adds no value | `thiserror` everywhere (ceremony with no payoff at the boundary) |
| Serialization | **`serde` / `serde_json`** | DTOs and the GL-post envelope derive `Serialize`/`Deserialize`; `#[serde(rename_all = "camelCase")]` gives a stable JSON wire shape | manual (de)serialization (error-prone, defeats codegen) |
| IDs / time | **`uuid` v4**, **`chrono`** | UUID keys avoid enumeration and merge cleanly across modules (every cross-module ref is a UUID logical FK); `chrono::NaiveDate` for `posting_date`, `DateTime` for audit stamps | integer PKs (leak ordinality, collide across modules) |
| Config | **`config` 0.14** + **`serde_yaml`** | Layered YAML (`application.yml` + env overrides) matches the `config/` convention; `DATABASE_URL` overrides at runtime | hardcoded config, bespoke env parsing |
| Validation | **`validator` 0.16** (feature-gated) | Generated DTO field rules (`@max(40)` → `#[validate(length(max = 40))]`) enforced at the edge; the hand-written write path adds the domain guards validation can't express (stock sufficiency, draft-state) | hand-written guard clauses scattered across handlers |
| gRPC / proto | **`tonic` 0.12` / `prost`** (present, **not generated**) | Kept in the dependency set so a service *can* light up a second transport, but the proto/gRPC/GraphQL generators are **disabled** for this module (see below) | forcing three transports on a module that ships one |
| Logging | **`tracing`** (+ `tracing-subscriber`) | Structured, async-aware spans; the service host installs the subscriber | `log` (no span/async context) |

## Generators this module turns off

[`schema/models/index.model.yaml`](../../schema/models/index.model.yaml) disables three generators:

```yaml
config:
  generators:
    disabled: [graphql, grpc, proto]
```

Inventory ships a **REST + service-driven** surface. gRPC/GraphQL/Protobuf would be generated
artifacts nobody consumes, so they are off — the `tonic`/`prost` deps stay only so a composing
service can add a transport by hand if it ever needs one. The Cargo `[features]` (`events`, `auth`,
`grpc`, `openapi`, `validation`) all default to empty; they gate optional layers, not the core.

## The framework crates

Four crates carry the leverage. They are **git dependencies** on the public framework repo, pinned to
`branch = "main"`:

```toml
backbone-core      = { git = "https://github.com/faridlab/backbone-framework", branch = "main", features = ["postgres"] }
backbone-orm       = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
backbone-auth      = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
backbone-messaging = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
```

| Crate | Gives the module | Seen as |
|-------|------------------|---------|
| **`backbone-core`** | `GenericCrudService`, `BackboneCrudHandler`, `PersistentEntity`, `FromCreateDto` / `ApplyUpdateDto`, `ServiceError` / `ServiceResult` | the twelve service type aliases, the read handlers, DTO conversions |
| **`backbone-orm`** | `GenericCrudRepository`, `SoftDelete`, `EntityRepoMeta`, pagination | the repository newtypes |
| **`backbone-auth`** | identity / permission primitives | the `application/auth/` per-entity guards |
| **`backbone-messaging`** | message-bus adapters | reserved for the event/subscription layer |

> **Reproducibility note.** `branch = "main"` is convenient but *not reproducible* — a fresh
> `cargo build` can pull a newer commit. For anything you ship, pin to a tag/rev (`tag = "vX.Y.Z"` or
> `rev = "<sha>"`). `Cargo.lock` is committed and pins transitively, but the git ref is what
> `cargo update` moves.

## The one edge that must stay a dev-dependency: `backbone-accounting`

The most important line in `Cargo.toml` is under `[dev-dependencies]`:

```toml
# TEST-ONLY edge: the GL-posting seam test drives the REAL backbone-accounting PostingService via an
# in-test adapter (envelope -> PostingRequest). Dev-dependency ONLY — the shipped inventory library
# has ZERO normal edge to accounting.
backbone-accounting = { path = "../backbone-accounting" }
```

Inventory posts to accounting, but it does **not** depend on accounting. It emits a serialized
`AccountingPostEnvelope` through the `GlPostSink` trait (`src/application/service/inventory_gl.rs`);
the adapter that maps that envelope onto accounting's `PostingRequest` lives on the consumer/test
side. The proof is a command a maintainer can run:

```bash
cargo tree -e normal -i backbone-accounting   # empty → no normal edge
```

Accounting appears only as a `dev-dependency`, so [`tests/gl_posting_seam.rs`](../../tests/gl_posting_seam.rs)
can drive the *real* ledger and prove the posts balance end-to-end — without coupling the shipped
library to it. This is the seam that keeps the two modules independently deployable
([ADR-002](../adr/ADR-002-gl-posting-seam.md)).

## The CLI: `metaphor`, not `backbone-schema`

Generation, migration, and testing go through the **`metaphor`** binary (v0.2.0 at time of writing),
which dispatches to plugins (`metaphor-schema`, `metaphor-codegen`, `metaphor-dev`).

> ⚠️ **Doc drift flagged.** The top-level [README](../../README.md) still describes the *skeleton*
> this module was stamped from (an `Example` entity, `backbone-schema`/`backbone migration` commands,
> and "update `path = …`" for the deps). Those are stale here: `backbone-schema` is not on `PATH`, the
> deps are git not path, and the entities are the real inventory documents. The working forms are
> `metaphor schema schema generate …` and `metaphor migration run`; the
> [Developer Guide](06-developer-guide.md) and [Maintainer Guide](05-maintainer-guide.md) use the
> verified commands throughout.

Why a workspace CLI instead of raw `cargo`/`sqlx`? A module never lives alone — it is one project in a
multi-project workspace, and `metaphor` applies workspace-wide policy (affected-only builds,
cross-project codegen, plugin discovery). See the schema docs' [INTEGRATION](../schema/INTEGRATION.md).

---

Next: [Architecture](04-architecture.md) — the C4 view and a Purchase Receipt traced to a balanced journal.
</content>
