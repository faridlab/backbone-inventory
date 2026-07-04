# BRD — backbone-inventory

> Business Requirements & Rules. Tier 1 · Supply Chain. Date: 2026-07-04. Pairs with
> `docs/business-flows/golden-cases.md` — every rule has a numeric oracle.

## Documents
- **Purchase Receipt** — inbound goods (→ SLE in, asset post).
- **Delivery Note** — outbound goods (→ SLE out, COGS post).
- **Stock Entry** — internal transfer (→ paired SLE, value-neutral).
- **Stock Reconciliation** — count adjustment (→ SLE, difference post).
- **Warehouse** (owned master, tree) · **Bin** (running balance) · **StockLedgerEntry** (the ledger).

## Business rules
**BR-1 (moving-average valuation).** Receipt: `value += money(qty·rate); qty += qty; rate =
value/qty`. Delivery: `cogs = money(qty·rate_current); value -= cogs; qty -= qty`; **rate unchanged
by an outflow**. Transfer: out+in at the source rate (value conserved). `stock_value`/GL amounts 2dp
half-up; `valuation_rate` 6dp.

**BR-2 (append-only ledger).** Every valuation-changing movement writes one immutable SLE per line
(`actual_qty` signed, `qty_after_txn`, `valuation_rate`, `stock_value`, `stock_value_difference`) and
updates the Bin in the same transaction. `Σ SLE actual_qty = Bin qty`; `Σ value_difference = Bin
value`. Corrections are new entries.

**BR-3 (sufficient stock).** An outgoing movement of more than on-hand is rejected
(`insufficient_stock`) with no partial movement. Negative stock is not permitted (SMB default).

**BR-4 (non-empty / non-negative).** ≥1 line; no negative quantity or rate. → `empty_document` /
`negative_quantity`. Transfers require distinct warehouses (`same_warehouse`).

**BR-5 (submit once).** Only a `draft` document may be submitted (`not_draft`); a submitted movement
posts exactly once.

**BR-6 (GL posting).** On submit, inventory emits: receipt `Dr Inventory · Cr GR/IR`; delivery `Dr
COGS · Cr Inventory`; reconciliation the signed net difference. Balanced by construction; GR/IR is a
non-party clearing account.

**BR-7 (eventually consistent).** The SLE+Bin transaction commits first; the GL post is emitted after
and `posting_state` goes pending→posted|failed. A GL rejection leaves the physical movement intact
(retryable), never rolled back.

**BR-8 (unique voucher numbers).** receipt/delivery/entry/recon numbers are unique (soft-delete
aware). → `duplicate_number`.

## Events
`StockReceived` (buying reconciles PO), `StockDelivered` (selling advances delivered_qty),
`StockMoved`, `StockReconciled`.

## Deferred (with reason)
FIFO, serial/batch depth, repost + global SLE ordering, landed cost, reservation, standard costing
(cut), MRP — Tier 3 / other pillars / needs no consumer yet.
