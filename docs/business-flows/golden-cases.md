# Inventory — Golden Cases (the numeric oracle)

Exact expected results mirroring `tests/valuation_golden_cases.rs`, `tests/gl_posting_seam.rs`, and
`tests/integrity_probes.rs`. Money: `stock_value`/GL amounts 2dp (half-up); `valuation_rate` 6dp.

## Valuation engine (`tests/valuation_golden_cases.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **IVC-1** | receipts 10@100 then 10@120 | Bin qty 20, rate `110.000000`, value `2200.00` (weighted average). |
| **IVC-2** | then deliver 5 | COGS `550.00` (5×110); Bin qty 15, rate `110` (unchanged by outflow), value `1650.00`. |
| **IVC-3** | two receipts | Σ SLE `actual_qty` = Bin qty; Σ SLE `stock_value_difference` = Bin value; 2 SLE rows. |
| **IVC-4** | receipt 3, deliver 10 | `insufficient_stock`; no stock consumed. |
| **IVC-5** | receipt 10@100 in WH1, transfer 4 → WH2 | WH1 (6, 600.00), WH2 (4, rate 100, 400.00); total value conserved (1000). |
| **IVC-6** | receipt 10@100, reconcile count=8 | `net_difference −200.00`; Bin qty 8, value 800.00. |
| **IVC-7** | empty receipt / same-warehouse transfer | `empty_document` / `same_warehouse`. |
| **IVC-8** | receipt 1@10.00 + 2@10.005 (rate 10.003333), deliver 1×3 | Bin drains to qty 0, value **exactly 0.00**, rate 0; Σ COGS = 30.01 (received value) — residual flush, subledger ties to GL. |
| **IVC-9** | on-hand 10, two concurrent deliveries of 6 | exactly one succeeds; the other `insufficient_stock`; Bin qty 4 (FOR UPDATE serializes, no oversell). |

## GL seam (`tests/gl_posting_seam.rs`) — inventory → the REAL accounting ledger

| Case | Input | Expected |
|------|-------|----------|
| **ISEAM-1** | receipt 10@100 | balanced journal: `Dr Inventory 1,000 · Cr GR/IR 1,000`; receipt `posting_state=posted`. |
| **ISEAM-2** | receipts 10@100+10@120, deliver 5 | `Dr COGS 550 · Cr Inventory 550` (moving-average). |
| **ISEAM-3** | receipt 10@100, reconcile to 8 | `Dr Adjustment 200 · Cr Inventory 200` (shrinkage). |
| **ISEAM-4** | receipt with Inventory = a non-postable header account | GL rejects `non_postable_account`; `posting_state=failed`; **SLE+Bin stand** (qty 10) — eventually consistent, retryable. |
| **ISEAM-5** | submit a submitted receipt again | `not_draft` — the movement posts once. |
| **ISEAM-6** | submit under a transient failure → `failed`; then `repost_purchase_receipt` | movement committed (qty 10) despite the failure; repost re-drives to `posted` (balanced journal). |
| **ISEAM-7** | force a posted voucher back to `failed` (crash window), then repost | dedup on `source_id` returns the SAME journal; still exactly one journal — no double post; an already-`posted` repost is a no-op. |

## Route surface (`tests/integrity_probes.rs`)

| Case | Input via guarded routes | Expected |
|------|--------------------------|----------|
| **IIP-1** | `POST /stock-ledger-entries`, `POST /bins/bulk` | `405/404` — no direct SLE/Bin write. |
| **IIP-2** | `DELETE /purchase-receipts/{id}` | `405/404` — no generic delete. |
| **IIP-3** | `POST /warehouses` well-formed | `201`. |
| **IIP-4** | `POST /purchase-receipts` with `lines:[]` | `422 empty_document`. |

## Conventions
- Every valuation-changing movement writes an immutable SLE and updates the Bin in one transaction;
  the GL post is emitted **after** commit (eventually consistent; `posting_state` pending→posted|failed).
- Moving-average: a receipt blends the rate; an outflow (delivery/transfer) leaves the rate unchanged
  and consumes the current average as COGS.
- The Bin is the authoritative current balance; `sle_no` is per-voucher (global replay ordering is a
  deferred Tier-3 repost concern).
