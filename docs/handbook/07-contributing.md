<!-- Reader: Contributor · Mode: How-to -->
# Contributing

How to land a change in `backbone-inventory` — dev setup, conventions, and the checklist a reviewer
will hold you to. The single hardest rule to remember: **commit messages carry no Claude or
co-author signature.** Everything else is standard.

## Dev setup

```bash
# 1. Toolchain
rustup show                 # Rust 2021 edition toolchain
metaphor --version          # metaphor 0.2.0+ on PATH

# 2. A database for tests
export DATABASE_URL="postgresql://root:password@localhost:5432/inventorydb"
metaphor migration run

# 3. Prove a clean baseline before you change anything
metaphor dev test
metaphor lint check
```

If `metaphor` is not installed, see the workspace root `metaphor.yaml` / plugin discovery order
(`$PATH` → `$METAPHOR_PLUGIN_BIN_DIR` → `~/.metaphor/bin/`).

## The golden rule of module changes

You are almost never editing generated Rust directly. Before writing code, ask: *does this belong in
the schema?* If it changes an entity's shape, the answer is yes — edit `schema/models/*.model.yaml`,
regenerate, and commit the regenerated output together with the schema change. A PR that hand-edits a
generated struct will be sent back.

The exception that defines this module: the **valuation engine and GL seam** (`inventory_write_service.rs`,
`inventory_gl.rs`, `inventory_read.rs`, `inventory_intake.rs`, `inventory_events.rs`, and
`guarded_routes.rs`) are hand-written and `user_owned`. You edit those directly — that is where the
domain logic lives. See the [Maintainer Guide](05-maintainer-guide.md).

## Behavior first: the flow ↔ feature ↔ golden-case oracle

Inventory's correctness is defined *before* the Rust, as three artifacts kept in lockstep:

1. **Business flow** — `docs/business-flows/*.md`: actors, preconditions, rules, postconditions.
2. **Golden case** — [`docs/business-flows/golden-cases.md`](../business-flows/golden-cases.md): the
   exact expected numbers/statuses/error codes (IVC-1…9 valuation, ISEAM-1…7 GL seam, IIP-1…4 route
   surface).
3. **Executable oracle** — `tests/valuation_golden_cases.rs`, `tests/gl_posting_seam.rs`,
   `tests/integrity_probes.rs`, which assert exactly those numbers.

**If you change a valuation or posting behavior, change all three in the same PR.** A new movement
type, a rounding rule, a posting shape — each gets a golden case with real numbers and a test that
fails without your change. This is not optional for engine changes; it is the review gate.

## Branch & commit conventions

- **Branch** off `main`. Never commit directly to `main`.
- **Conventional commits.** `type(scope): summary` — e.g. `feat(delivery): flush residual to zero on
  final outflow`, `fix(bin): serialize concurrent deliveries with FOR UPDATE`, `docs(handbook): adapt
  architecture to real entities`. Types drive versioning: `fix:` → patch, `feat:` → minor, `feat!:` /
  `BREAKING CHANGE:` → major.
- **One concern per commit.** Group by functionality; keep large regenerated files in their own commit
  rather than mixed with hand-written logic.
- **Message says *why*, not "update".** No filler (`wip`, `fix stuff`, `changes`).
- **NO signatures.** Never append `Co-Authored-By`, `Generated with…`, or any trailer. Hard workspace
  rule (root `CLAUDE.md`).

```
feat(reconciliation): post signed value difference on physical count

A count below system qty is shrinkage: Dr Adjustment · Cr Inventory. Locked by
golden case ISEAM-3 and valuation_golden_cases IVC-6.
```

## Before you open a PR — the checklist

- [ ] Change started in the **schema YAML** if it touches an entity's shape.
- [ ] `metaphor schema schema validate` passes.
- [ ] Regenerated code committed alongside the schema change (no hand-edits outside CUSTOM regions).
- [ ] Engine/GL logic lives in a `user_owned` file or a `// <<< CUSTOM` marker — never in generated
      territory.
- [ ] **Valuation invariants intact:** `Σ SLE = Bin`; outflow leaves the rate unchanged; residual
      flush lands `stock_value` on exactly 0 at zero stock; movement commits before the GL post;
      `insufficient_stock` rejects with no partial move.
- [ ] **Golden case added/updated** for any behavior change, with a matching test.
- [ ] `cargo tree -e normal -i backbone-accounting` is **empty** (no normal edge to accounting).
- [ ] No `main.rs` / binary target added (this is a **library**).
- [ ] Production route surface unchanged or still guarded (`create_guarded_inventory_routes`, not
      `all_crud_routes`).
- [ ] No sibling module's schema touched; cross-module references are logical FKs.
- [ ] `metaphor dev test` green; `metaphor lint check` clean.
- [ ] Migrations have both `*.up.sql` and `*.down.sql`.
- [ ] Docs updated if behavior changed (this handbook, the golden cases, or `docs/schema/`).
- [ ] Conventional-commit messages, **no signatures**.

## Tests

- Unit + integration + golden cases + the GL seam test all run through `metaphor dev test`.
- The **GL seam test** (`tests/gl_posting_seam.rs`) drives the *real* `backbone-accounting`
  `PostingService` through an in-test adapter — proving the posts balance end-to-end. Accounting is a
  **dev-dependency only**; keep it that way.
- Per-entity API tests live under `tests/integration/`; behavior/BDD features under
  `tests/features/**` (a `user_owned` path the generator never touches).

## Review expectations

A reviewer checks, in order:

1. **Did the change start in the right place?** Schema for shape; the engine files for logic.
2. **Regen-safety.** Nothing valuable sits where the next `generate --force` would eat it.
3. **Invariants.** The valuation/posting rules above still hold; the golden cases prove it.
4. **Boundary.** No normal Cargo edge to accounting; no sibling schema touched; the guarded surface
   still guards.
5. **Proof.** Golden case + test exist and pass; migrations are reversible.

Expect a request to move logic into a protected region if it is in generated territory, and a request
for a golden case if you changed engine behavior without one — neither is a nit.

## Architectural changes

If your change is a *decision* (a new dependency, a new movement type, a posting-shape shift), write
an ADR. The domain ADRs live in [`docs/adr/`](../adr/) (numbered `ADR-001`, `ADR-002`, …); the
framework ADRs in [`adr/`](adr/) (`adr-0001`…). ADRs are **immutable once accepted** — supersede
rather than edit.

---

Related: [Glossary](08-glossary.md) · [Maintainer Guide](05-maintainer-guide.md) · [ADRs](adr/) ·
[domain ADRs](../adr/).
</content>
