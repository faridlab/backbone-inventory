# backbone-inventory — Handbook

The documentation set for **`backbone-inventory`**: the supply-chain **stock ledger of record**.
It owns an append-only Stock Ledger (SLE) + moving-average valuation, keeps a per-(item, warehouse)
`Bin` running balance, and is the supply-chain pillar's **only emitter of accounting posts** — a
Purchase Receipt posts `Dr Inventory · Cr GR/IR`, a Delivery Note posts `Dr COGS · Cr Inventory`,
a Stock Reconciliation posts the value difference. (See [ADR-001](adr/ADR-001-inventory-boundary-and-valuation.md)
and [ADR-002](adr/ADR-002-gl-posting-seam.md).)

It is a **Backbone Framework domain module** — a `[lib]`-only crate in 4-layer DDD, schema-driven:
the entity structs, DTOs, migrations, repositories, CRUD services, and read handlers are generated
from `schema/models/*.model.yaml`. The valuation engine, the GL-posting seam, and the guarded write
surface are **hand-authored, regeneration-safe** code. This handbook explains both halves.

Every page below names **one reader** and **one Diátaxis mode** at its top. Find your reader, follow
the path.

## Find your path

| You are… | You want to… | Start here |
|----------|--------------|-----------|
| **Evaluator** | Decide whether to build on this | [Philosophy](handbook/01-philosophy.md) → [Background](handbook/02-background.md) → [Technology](handbook/03-technology.md) |
| **App developer** | Integrate inventory into a service and move stock | [Developer Guide](handbook/06-developer-guide.md) |
| **Maintainer** | Understand the machine and extend it safely | [Architecture](handbook/04-architecture.md) → [Maintainer Guide](handbook/05-maintainer-guide.md) |
| **Contributor** | Open a correct PR | [Contributing](handbook/07-contributing.md) |
| **Anyone** | Agree on what a word means | [Glossary](handbook/08-glossary.md) |

## The handbook

1. [Philosophy & motivation](handbook/01-philosophy.md) — *Evaluator.* Why an inventory subledger of record, the worldview (schema-generated plumbing + a hand-written valuation core), and the non-goals.
2. [Background & prior art](handbook/02-background.md) — *Evaluator.* Where the SLE/Bin/moving-average design comes from, and what it borrows and rejects from ERP and from generic CRUD frameworks.
3. [Technology & the "why"](handbook/03-technology.md) — *Evaluator + Maintainer.* The stack, each choice with a rationale and a rejected alternative — including why `rust_decimal` and why the accounting edge is a **dev-dependency only**.
4. [Architecture](handbook/04-architecture.md) — *Maintainer.* C4 view: context (accounting, catalog, organization, party, selling/buying), the generated cake vs. the hand-written engine, and a Purchase Receipt traced from HTTP to a balanced GL post.
5. [Maintainer Guide](handbook/05-maintainer-guide.md) — *Maintainer.* Schema-YAML SSoT, regeneration, where the valuation/GL code lives, how to add an entity, the guarded-router composition, and reposting a stuck GL post.
6. [Developer Guide](handbook/06-developer-guide.md) — *App developer.* Install → quickstart (warehouse → receipt → submit → deliver → check availability) → recipes → configuration → troubleshooting.
7. [Contributing](handbook/07-contributing.md) — *Contributor.* Dev setup, commit/PR conventions, the golden-case oracle, tests and lint, review checklist.
8. [Glossary](handbook/08-glossary.md) — *All.* One term, one meaning — SLE, Bin, moving average, voucher, GL post, `posting_state`, and the framework terms.
9. [Architecture Decision Records](handbook/adr/) — *Maintainer.* The framework decisions (schema SSoT, generic CRUD, CUSTOM markers) **and** the two domain decisions ([inventory boundary & valuation](adr/ADR-001-inventory-boundary-and-valuation.md), [the GL seam](adr/ADR-002-gl-posting-seam.md)).

## Related, already-written docs

This handbook is the *narrative*. Reference sets live alongside it — link out, don't duplicate:

- **[Business flows](business-flows/README.md)** and the **[golden cases](business-flows/golden-cases.md)** — the numeric oracle (IVC-1…9 valuation, ISEAM-1…7 GL seam, IIP-1…4 route surface), mirrored one-to-one by the tests.
- **[Schema DSL reference](schema/README.md)** — the exact YAML grammar: [types](schema/TYPES.md), [model rules](schema/RULE_FORMAT_MODELS.md), [generation targets](schema/GENERATION.md), [error codes](schema/ERROR_CODES.md), [examples](schema/EXAMPLES.md). This is the *Reference* corner of Diátaxis; the handbook explains the *why*.

## Conventions this handbook follows

- **Reader + mode named** at the top of every page.
- **Commands are real.** Every `metaphor …` command was run against `metaphor 0.2.0` while writing. Where a command in the top-level [README](../README.md) is stale, the handbook flags it and gives the working form.
- **Code wins over docs.** When a doc and the schema/code disagree, the schema YAML (the source of truth) wins — the doc is the bug.
</content>
</invoke>
