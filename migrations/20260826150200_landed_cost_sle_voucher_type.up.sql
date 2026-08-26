-- Landed-cost revaluation SLE rows: extend the stock ledger's `voucher_type` vocabulary
-- with the 'landed_cost' member. The move engine's landed-cost adjustment verb mints one
-- value-only SLE row per target receipt line under this type (voucher_id = the landed cost,
-- voucher_no = '{lc_number}/{move_name}/{move_line_id}'); the append-only ledger, the bins and
-- the moving-average engine are untouched — this is a new producer label on the EXISTING
-- single-writer estate, not a second ledger.
--
-- The estate-divergence detector treats the type as an allow-listed non-door signature: a
-- landed-cost row changes a bin's value without changing its quantity, so the detector's
-- three-way estate check must read it as a legitimate valuation write.

ALTER TYPE voucher_type ADD VALUE IF NOT EXISTS 'landed_cost';
