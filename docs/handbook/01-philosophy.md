<!-- Reader: Evaluator · Mode: Explanation -->
# Philosophy & motivation

**`backbone-inventory` is the stock ledger of record.** It is to physical goods what the general
ledger is to money: an append-only log of every movement, from which the current on-hand quantity
and value are derived — never guessed, never patched in place. Selling and buying create *intent*;
inventory owns the *physical move* and the accounting effect that follows from it.

Two convictions sit underneath, and they pull in opposite directions on purpose:

1. **The mechanical 95% is generated.** Twelve CRUD endpoints per entity, DTOs, migrations,
   repositories, read handlers — all downstream artifacts of one schema file. Nobody hand-writes them.
2. **The valuable 5% is hand-written, and protected from the generator.** The moving-average
   valuation engine, the SLE integrity rules, and the GL-posting seam are the reason this module
   exists. They live in regeneration-safe files the generator never touches.

A module that is *only* generated CRUD is a thin thing. Inventory is not that. Understanding the
seam between the two halves is the whole point of this handbook.

## The problem this module solves

Ask a small business "how much stock do you have, and what is it worth?" and the naive system
answers with two mutable numbers per item: a quantity and a cost. Both are wrong within a week.

- **Mutable balances drift.** A quantity you increment and decrement in place has no history; when
  it disagrees with the shelf, there is nothing to audit. Corrections overwrite the crime scene.
- **Cost is not a number, it is a *method*.** The moment two receipts arrive at different prices,
  "unit cost" is ambiguous. What does a later delivery cost the P&L — the first price, the last, the
  average? Get it wrong and COGS (and therefore profit) is wrong.
- **Stock value and the ledger must tie out.** The inventory asset on the balance sheet *is* the sum
  of what's on the shelves. If the warehouse says one number and accounting says another, the books
  are broken — and reconciling them by hand is a monthly tax.

## The worldview

Six ideas shape every design decision here. The first three are the framework's; the last three are
inventory's own, recorded in [ADR-001](../adr/ADR-001-inventory-boundary-and-valuation.md) and
[ADR-002](../adr/ADR-002-gl-posting-seam.md).

1. **The schema is the single source of truth.** [`schema/models/*.model.yaml`](../schema/RULE_FORMAT_MODELS.md)
   is authoritative. The entity structs, DTOs, migrations, repositories, and read handlers are
   *regenerated*, never hand-maintained. If code and schema disagree, the schema is right.
   ([ADR-0001](adr/adr-0001-schema-yaml-ssot.md).)

2. **Boilerplate is generic, so make it generic once.** Standard CRUD is *inherited* from
   `GenericCrudService` / `GenericCrudRepository`, not written per entity. A service is a **type
   alias**; a repository is a **newtype**. ([ADR-0002](adr/adr-0002-generic-crud.md).)

3. **Hand-written code must survive regeneration.** Business logic lives inside `// <<< CUSTOM …
   // END CUSTOM` markers, in whole `*_custom.rs`-style files the generator never emits, or under a
   `user_owned` glob in `metaphor.codegen.yaml`. ([ADR-0003](adr/adr-0003-custom-markers.md).)

4. **The Stock Ledger is append-only; the Bin is the authoritative current balance.** Every
   valuation-changing movement writes one immutable `StockLedgerEntry` (SLE) and updates the
   per-(item, warehouse) `Bin` — quantity, moving-average rate, and value — in the *same* database
   transaction. Corrections are new entries, never edits. `Σ SLE` ties to the `Bin`, always.

5. **Moving-average valuation is the default, and it must tie to zero.** A receipt blends the new
   cost into the average; an outflow (delivery or transfer) consumes the current average as COGS and
   **does not change the rate**. On the final outflow that drains a bin to zero, the last units
   absorb the rounding residual so `stock_value` returns to *exactly* 0 — the subledger ties out with
   the GL at zero stock, no stranded cent. Standard costing is deliberately cut; FIFO is opt-in and
   deferred.

6. **Physical movement commits before the accounting post; the post is eventually consistent.** The
   SLE + Bin write commits first (`status = submitted`). The GL post is emitted *after*, and the
   voucher's `posting_state` moves `pending → posted | failed`. A GL rejection never rolls back the
   real stock movement — it leaves a `failed` voucher that a `repost_*` call re-drives. Inventory
   owns the physical truth; accounting is reconciled to it, not the reverse.

The payoff: the *shape* of every entity is consistent and free, and the *judgment* — how value moves,
when the ledger posts, what a caller is allowed to do — is written once, tested against a numeric
oracle, and safe from the next regeneration.

## The 4-layer discipline

A module is Domain-Driven Design's four layers; dependency arrows point only inward:

```
Presentation  →  Application  →  Domain  ←  Infrastructure
   (HTTP)          (services)     (entities)    (Postgres)
```

- **Domain** knows nothing about HTTP or SQL — entities (`Warehouse`, `StockLedgerEntry`, `Bin`, the
  documents) and their invariants.
- **Application** orchestrates use cases: the generated CRUD services *and* the hand-written
  `InventoryWriteService` (the valuation engine) and `InventoryReadService` (availability).
- **Infrastructure** adapts the domain to Postgres.
- **Presentation** exposes read models and validated creates over Axum.

The [Architecture](04-architecture.md) page traces a Purchase Receipt through all four, from HTTP to
a balanced journal.

## What this module deliberately does **not** do

Non-goals are why the boundary stays clean — see [ADR-001](../adr/ADR-001-inventory-boundary-and-valuation.md).

- **It is not a service.** A module is a **library crate** (`[lib]` only, no `main.rs`). A
  `backend-service` composes it, hands it a pool and a `GlPostSink`, and mounts its router.
- **It does not own the catalog, companies, parties, or accounts.** Inventory owns exactly **one**
  master — `Warehouse`. Everything else is a *logical foreign key*: `item_id` → catalog (held as a
  read-only `StockItem` projection, never the god-entity), company/branch → organization, supplier/
  customer → party, and the GL account ids → accounting. There are **zero horizontal Cargo edges** to
  those modules.
- **It does not depend on accounting to post to it.** Inventory emits a serialized
  `AccountingPostEnvelope` through a `GlPostSink` trait; the adapter that speaks accounting's API
  lives on the *consumer* side. `cargo tree -e normal -i backbone-accounting` is empty
  ([ADR-002](../adr/ADR-002-gl-posting-seam.md)).
- **It does not allow negative stock.** A delivery or transfer of more than on-hand fails
  `insufficient_stock` with no partial movement — the SMB default.
- **It does not ship every ERP feature.** Serial/batch bundle depth, FIFO, landed-cost vouchers,
  pick lists, hard reservation, and a global backdated-replay repost engine are explicitly parked as
  Tier-3 ([ADR-001](../adr/ADR-001-inventory-boundary-and-valuation.md)). The module does the
  spine correctly before it does the frills at all.

## When this is the wrong tool

Be honest before adopting:

- If you need **FIFO / lot-level costing today**, or standard costing, this module defaults to
  moving-average and defers the rest — you would be building on a deliberately unfinished corner.
- If you want inventory to be the **system of record for the item catalog** (descriptions, pricing,
  UoM conversions), it is not — that lives in `catalog`, and inventory holds only a projection.
- If your books are not double-entry, or you have **no general ledger to post to**, the GL seam —
  half the value here — buys you nothing.

For a small-to-mid business that moves physical goods and wants its stock value to tie to its books
automatically, this is exactly the shape that pays off.

---

Next: [Background & prior art](02-background.md) — where the SLE/Bin/moving-average design comes from.
</content>
