# ADR-002: The supply-chain GL seam — COGS + asset-receipt posts via envelope + ACL

**Status**: Accepted — Applied 2026-07-04 (the second `AccountingPost` producer, proven end-to-end)
**Deciders**: Farid (owner), build session 2026-07-04
**Related**: `docs/erp/gl-posting-contract.md`, backbone-selling ADR-002 (the reference implementation), ADR-001

## Context

`backbone-selling` proved the GL-posting seam once (revenue). Inventory is the **second** producer
and the one the supply-chain pillar hinges on: it posts the COGS and asset-valuation effects of
physical movements. This ADR records that it reuses selling's seam verbatim.

## Decision

1. **Inventory emits a serialized `AccountingPostEnvelope`** (its own copy of the contract shape —
   the wire type is the contract, duplicated per producer) reached only through a `GlPostSink`
   trait. The ACL adapter that names accounting's `PostingRequest` lives on the consumer/test side.
   The shipped library has **zero normal Cargo edge** to accounting (`cargo tree -e normal -i
   backbone-accounting` is empty; accounting is a dev-dependency for the seam test only).
2. **The three posts, balanced by construction:**
   - **Purchase Receipt:** `Dr Inventory · Cr GR/IR clearing` = Σ line `money(qty·rate)`.
   - **Delivery Note:** `Dr COGS · Cr Inventory` = Σ line `money(qty·valuation_rate)`.
   - **Stock Reconciliation:** signed net difference — `Dr Inventory · Cr Adjustment` (gain) or the
     reverse (shrinkage).
   `source_type = "inventory"`, `source_id = voucher_id`; accounting dedupes on
   `(company, source_type, source_id, posting_type)` (the idempotency identity).
3. **Eventually consistent, with a real recovery path.** The SLE+Bin transaction commits first; the
   post is emitted after and the voucher's `posting_state` reconciled from the ack (posted) or
   rejection (failed). A GL rejection never rolls back the physical movement. A `failed` (or a
   crash-window `pending`) voucher is **not terminal**: `repost_purchase_receipt` /
   `repost_delivery_note` re-drive the post (rebuilding the SAME envelope from the stored header).
   Idempotent against the "physically-posted-but-status-not-updated" window — accounting dedupes on
   `source_id`, so a re-emit returns the original journal, never a second (council 2026-07-04).
4. **GR/IR clearing is a non-party liability** (`current_liability`), so the contract's "party
   required iff AR/AP" rule is satisfied without a party on the clearing line.

## Consequences

- Proven, not asserted: `tests/gl_posting_seam.rs` drives the real `PostingService` and asserts the
  balanced journals — receipt (Dr Inventory 1,000 · Cr GR/IR 1,000), delivery (Dr COGS 550 · Cr
  Inventory 550 at moving-average), reconciliation shrinkage (Dr Adjustment 200 · Cr Inventory 200),
  a rejection leaving the movement intact but `posting_state=failed`, and re-submit refused.
- This is the reference the rest of the pillar (buying receipts, manufacturing WIP/FG) reuses.
- Residual / parking lot: async/durable posting + retry behind `failed`; reversal on cancellation
  (`posting_type=reversal`); landed-cost repost; Indonesia COGS rounding overlay + import-duty into
  landed cost.
