# ADR-001: Inventory owns the Stock Ledger + moving-average valuation; it is the supply-chain GL producer

**Status**: Accepted — Applied 2026-07-04
**Deciders**: Farid (owner), build session 2026-07-04
**Related**: `docs/erp/supply-chain.md`, `docs/erp/gl-posting-contract.md`, ADR-002 (the GL seam)

## Context

`backbone-inventory` is the Tier-1 spine of the supply-chain pillar and the **only supply-chain
emitter of `AccountingPost`**. It is to physical goods what `backbone-accounting` is to money: an
append-only `StockLedgerEntry` (SLE) that every movement writes, plus a per-(item,warehouse) `Bin`
running balance. Selling/Buying create intent; inventory owns the physical move and its COGS /
asset-valuation effect.

Inventory owns exactly one master — **Warehouse** (a per-company tree). Everything else is a logical
FK: `item_id`→catalog (held as a `StockItem` ACL projection, never the god-entity), company/branch
→organization, and the GL account ids →accounting (referenced by the post, never imported). Zero
horizontal Cargo edges.

## Decision

1. **Moving-average valuation is the default** (FIFO opt-in deferred). The `Bin` holds
   `(actual_qty, valuation_rate, stock_value)`:
   - **Receipt:** `value += money(qty·rate); qty += qty; rate = value/qty` — new cost blends in.
   - **Delivery:** `cogs = money(qty·rate); value -= cogs; qty -= qty` — an outflow **does not
     change the rate** (it consumes the current average). **Residual flush (council 2026-07-04):** on
     the FINAL outflow (`new_qty == 0`) the last units absorb the moving-average rounding residual —
     `cogs = remaining value`, so `stock_value` returns to **exactly 0** and the inventory subledger
     ties out with the GL at zero stock (no stranded/negative cent). Same for the transfer-out leg.
   - **Transfer:** paired out/in SLE at the source rate — value-neutral, no GL post.
   - **Reconciliation:** set qty/value to the counted figures; the signed delta is posted.
   Standard costing is **cut** (SMB uses actual cost). `stock_value`/GL amounts are 2dp (half-up);
   `valuation_rate` is 6dp.
2. **The SLE is append-only; the Bin is the authoritative current balance.** Every valuation-changing
   movement writes an immutable SLE (`actual_qty` signed, `qty_after_txn`, `valuation_rate`,
   `stock_value`, `stock_value_difference`) and updates the Bin in the same transaction. Corrections
   are new entries, never edits.
3. **Physical movement commits before the GL post.** The SLE + Bin write commit first
   (`status=submitted`); the GL post is then attempted and the voucher's `posting_state` goes
   pending→posted|failed. A GL rejection leaves the real stock movement intact and the post
   retryable — the eventually-consistent model the GL-posting contract prescribes.
4. **Insufficient stock is rejected**, not allowed negative (SMB default): a delivery/transfer of
   more than on-hand fails `insufficient_stock` with no partial movement.

## Consequences

- The valuation math + SLE integrity are locked by `tests/valuation_golden_cases.rs` (7 cases:
  moving-average blend, outflow-rate-invariance, Σ-SLE=Bin, insufficient-stock, value-neutral
  transfer, signed reconciliation, validation).
- Inventory unblocks selling's deferred delivery half: `StockDelivered` carries the source SO so
  selling can advance `delivered_qty`; `StockReceived` lets buying reconcile a PO.
- Deferred (Tier-3, per the brief): serial/batch bundle depth, the repost engine + a **global
  per-(item,warehouse) SLE sequence** for backdated replay (today `sle_no` is per-voucher; the Bin
  is the source of truth), Landed Cost Voucher, PickList/PutawayRule, FIFO, hard reservation.
