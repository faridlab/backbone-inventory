# PRD — backbone-inventory

> Tier 1 (spine) · Supply Chain pillar · Indonesia-first ERP suite. Status: built. Date: 2026-07-04.

## Problem & intent
Businesses that hold stock need an accurate, auditable **quantity + valuation ledger** and correct
**COGS / inventory** accounting — without every app re-implementing valuation math or GL posting.
`backbone-inventory` owns the Stock Ledger (SLE) and moving-average valuation, and is the only
supply-chain module that posts to the GL (COGS on delivery, asset on receipt).

## Goals
- Own **Warehouse** (per-company tree) + the append-only **SLE** + per-(item,warehouse) **Bin**.
- Compute **moving-average valuation** server-side; every valuation-changing movement writes an SLE
  and emits a balanced `AccountingPost` (Dr Inventory·Cr GR/IR on receipt; Dr COGS·Cr Inventory on
  delivery; signed difference on reconciliation).
- Hold catalog Item as a read-only **StockItem** projection (never the master).
- Expose stable **domain events** (`StockReceived/Delivered/Moved/Reconciled`) so selling advances
  `delivered_qty` and buying reconciles POs.
- Reject **insufficient stock**; keep the physical movement authoritative even if the GL post fails.

## Non-goals (this phase / deferred)
Serial/batch bundle depth, FIFO, repost engine + global SLE replay ordering, Landed Cost Voucher,
PickList/PutawayRule, hard reservation, standard costing (cut), full bin-level WMS, MRP.

## Personas
- **Warehouse user** — receives, delivers, transfers, counts stock.
- **Finance user** — relies on correct COGS and inventory valuation in the GL.
- **Integrating engineer** — consumes availability + events (selling/buying), extends via events.

## Success criteria
- Moving-average math + SLE integrity locked by a numeric oracle (7 cases).
- COGS + asset-receipt + adjustment posts proven end-to-end against the real accounting ledger (5).
- Zero horizontal Cargo edge; guarded surface blocks direct SLE/Bin writes.
- Indonesia-ready hooks: clean 2dp COGS rounding; import-duty-into-landed-cost is a future overlay.
