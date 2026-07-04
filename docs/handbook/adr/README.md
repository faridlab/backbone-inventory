# Architecture Decision Records

One decision per record: context, decision, alternatives, consequences. **Immutable once accepted** —
to change a decision, write a new ADR that supersedes the old one and update its Status line; never
edit an accepted decision in place.

Inventory has **two sets** of ADRs. The framework decisions (below) are inherited by every Backbone
module. The **domain decisions** — how inventory values stock and posts to the GL — live one level up
in [`docs/adr/`](../../adr/) and are the ones to read first for this module.

## Domain decisions (inventory-specific)

| ADR | Decision | Status |
|-----|----------|--------|
| [ADR-001](../../adr/ADR-001-inventory-boundary-and-valuation.md) | Inventory owns the Stock Ledger + moving-average valuation; it is the supply-chain GL producer | Accepted (2026-07-04) |
| [ADR-002](../../adr/ADR-002-gl-posting-seam.md) | The supply-chain GL seam — COGS + asset-receipt posts via envelope + `GlPostSink` + ACL, eventually consistent, repostable | Accepted (2026-07-04) |

## Framework decisions (inherited by every module)

| ADR | Decision | Status |
|-----|----------|--------|
| [0001](adr-0001-schema-yaml-ssot.md) | Schema YAML is the single source of truth | Accepted |
| [0002](adr-0002-generic-crud.md) | CRUD is inherited from generics, not written per entity | Accepted |
| [0003](adr-0003-custom-markers.md) | Regen-safety via CUSTOM markers and `user_owned` | Accepted |
</content>
