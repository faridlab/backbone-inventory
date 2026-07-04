<!-- Reader: Evaluator · Mode: Explanation -->
# Background & prior art

Nothing here is novel for novelty's sake. The inventory design borrows a well-worn ERP pattern; the
code generation borrows from a lineage of scaffolders and ORMs. This page credits both honestly and
says what `backbone-inventory` takes and what it leaves.

## Part 1 — the inventory design

### Perpetual inventory, and why not periodic

Two ways to know what stock is worth:

- **Periodic:** count everything at period-end, back into COGS from opening + purchases − closing.
  Cheap to run, blind between counts — you never know today's margin, and shrinkage hides until the
  count.
- **Perpetual:** every movement updates quantity *and* value continuously, so the balance sheet and
  COGS are always live.

`backbone-inventory` is **perpetual**. The cost is that every movement must do real work
(valuation + a ledger write + a GL post); the payoff is that "what do I have and what is it worth"
is answerable at any instant, and shrinkage surfaces the moment a physical count disagrees with the
system.

### The Stock Ledger + Bin pattern (ERPNext lineage)

The append-only **Stock Ledger Entry (SLE)** plus a per-(item, warehouse) **Bin** running balance is
the pattern ERPNext uses, and it is the one adopted here — because it is the correct one.

- **What's good:** the SLE is an immutable audit trail (`actual_qty`, `qty_after_txn`,
  `valuation_rate`, `stock_value`, `stock_value_difference`), so `Σ SLE` always reconstructs the
  `Bin`. Corrections are new entries; history is never rewritten. Every valuation-changing SLE has a
  known GL amount (`stock_value_difference`), so the subledger ties to the ledger by construction.
- **What's borrowed:** the SLE/Bin split, the voucher-typed movements (Purchase Receipt, Delivery
  Note, Stock Entry, Stock Reconciliation), and moving-average as the default cost method.
- **What's rejected / deferred:** ERPNext's full depth — serial/batch bundles, a global
  per-(item, warehouse) sequence with a backdated **repost engine**, Landed Cost Vouchers, pick
  lists, and FIFO lots — is Tier-3 and parked ([ADR-001](../adr/ADR-001-inventory-boundary-and-valuation.md)).
  Today `sle_no` is per-voucher and the `Bin` is the source of truth; backdated replay is future work.

### The valuation-method choice

| Method | What a delivery costs the P&L | Verdict here |
|--------|-------------------------------|--------------|
| **Moving average** | The current blended average at the moment of outflow | **Default.** Simplest correct answer for SMB; one rate per (item, warehouse). |
| **FIFO** | The cost of the oldest lots still on hand | **Opt-in, deferred.** `ValuationMethod::fifo` exists in the schema; the lot machinery does not yet. |
| **Standard costing** | A fixed planned cost, with variances booked separately | **Cut.** SMBs use actual cost; variance accounting is overhead they don't want. |

The subtle rule that makes moving-average *tie out*: an outflow consumes the average but **does not
change the rate**, and the final outflow that empties a bin flushes the entire remaining value to
COGS so `stock_value` lands on exactly 0. Without that residual flush, rounding leaves a stranded
fraction of a cent and the subledger slowly drifts from the GL. (Golden case IVC-8 locks this.)

### Inventory as a subledger to the GL

The oldest idea here: inventory is a **subledger**. The warehouse owns the physical truth; the
general ledger records its money effect. So inventory *emits* postings — it does not keep its own
parallel set of books:

- **Purchase Receipt** → `Dr Inventory · Cr GR/IR clearing` (an asset arrives, the goods-received/
  invoice-received clearing account is credited until the invoice lands).
- **Delivery Note** → `Dr COGS · Cr Inventory` (the asset leaves, expensed at the consumed average).
- **Stock Reconciliation** → the signed value difference (`Dr Inventory · Cr Adjustment` on a gain,
  the reverse on shrinkage).

This is the reference pattern that the rest of the supply-chain pillar (buying receipts,
manufacturing WIP/FG) reuses ([ADR-002](../adr/ADR-002-gl-posting-seam.md)).

## Part 2 — the code-generation design

The physical entities and the twelve CRUD endpoints per entity are *generated*. That design has its
own prior art.

### 1. Hand-rolled layers

Write entity, DTOs, migration, repository, service, and handler by hand for every entity.

- **Good:** total control, no magic. **Breaks:** does not scale past a handful of entities — every
  one re-litigates pagination, soft-delete, and error shape, and they drift.
- **Kept:** the explicit, readable 4-layer structure. **Rejected:** *writing* the mechanical 95% by
  hand.

### 2. Heavyweight ORMs (Rails, Django, Hibernate)

A base class gives you CRUD, migrations, and query building.

- **Good:** enormous leverage. **Breaks:** the magic is at *runtime* — invisible SQL, fat models that
  couple domain to persistence, weak/reflective typing.
- **Kept:** the leverage — generic CRUD you inherit. **Rejected:** runtime magic. Backbone generates
  *visible Rust you can read and step through*, keeps the domain layer free of persistence, and uses
  SQLx so queries are checked at **compile time**.

### 3. Schema-first codegen (OpenAPI, Prisma, protobuf)

Describe the data once; generate types and servers.

- **Good:** one source of truth, no drift *if* you never hand-edit. **Breaks:** the "never hand-edit"
  clause — custom logic forces you to fork the output or bolt it on awkwardly.
- **Kept:** the single source of truth and full-artifact generation. **Rejected:** the all-or-nothing
  edit boundary. The `// <<< CUSTOM` marker and `user_owned` files let generated and hand-written
  code coexist — which is exactly what lets the valuation engine live *beside* generated CRUD in the
  same tree ([ADR-0003](adr/adr-0003-custom-markers.md)).

### 4. Laravel-style scaffolders (`make:*`)

A generator writes starter files once; then they're yours.

- **Good:** fast start (Backbone mirrors it with `metaphor make entity`). **Breaks:** one-shot — the
  files drift from any spec the moment you edit them.
- **Kept:** the ergonomic `make` entry point. **Rejected:** the one-shot nature. Backbone's generation
  is idempotent and repeatable for the life of the module.

### What Backbone synthesizes

| From | Borrowed | Rejected |
|------|----------|----------|
| Hand-rolled layers | Explicit, readable 4-layer DDD | Writing the boilerplate by hand |
| Heavyweight ORMs | Inherited generic CRUD | Runtime magic; domain/DB coupling |
| Schema-first codegen | One source of truth; full-artifact generation | The all-or-nothing edit boundary |
| Laravel scaffolders | Ergonomic `make` entry point | One-shot, non-repeatable generation |

The result: **repeatable, compile-time-checked, regen-safe scaffolding over a strict DDD skeleton**,
with a protected region where a real domain engine lives. Inventory is the proof that the second half
matters — remove the valuation engine and the GL seam and you have a stock table, not a stock ledger.

## Where it sits in the Metaphor workspace

Inventory is one **`module`** among the project types the [Metaphor CLI](../schema/INTEGRATION.md)
orchestrates:

- **`crate`** — a focused Rust library.
- **`module`** — *this* — a bounded domain library (4-layer DDD, schema-driven), **consumed by
  services, never run alone.**
- **`backend-service`** — a runnable Axum/SQLx/Tonic server that *composes* modules and supplies the
  `GlPostSink` that carries inventory's posts into `backbone-accounting`.

Inventory references identity, catalog, party, and accounts from sibling modules by logical FK, and is
wired into a service by that service's composition root. The [Architecture](04-architecture.md) page
shows exactly where the seams are.

---

Next: [Technology & the "why"](03-technology.md) — the stack, choice by choice.
</content>
