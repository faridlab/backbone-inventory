-- Valuation-overlay columns on existing tables (the P2 schema additions the generator
-- cannot emit: it does not rewrite existing table migrations).
--
--   stock_items.weight_per_unit        — the per-unit shipping weight, the WEIGHT basis of
--                                        landed-cost splits. 0 = unweighted; an all-zero
--                                        weight basis over the target lines is the loud
--                                        zero-denominator case (no silent equal split).
--   locations.valuation_account_id     — per-location valuation account override. When set,
--                                        inventory GL legs that touch this location resolve
--                                        to it ahead of the door-header account
--                                        (smallest-first override: location beats header).
--   purchase_receipt_items.is_landed_costs_line — the landed-cost service seam, owned by
--                                        inventory: the receipt door SKIPS move-minting for
--                                        flagged lines — they carry cost into a LandedCost
--                                        document, not stock.

ALTER TABLE inventory.stock_items
    ADD COLUMN IF NOT EXISTS weight_per_unit NUMERIC(18, 6) NOT NULL DEFAULT 0;

ALTER TABLE inventory.locations
    ADD COLUMN IF NOT EXISTS valuation_account_id UUID;

ALTER TABLE inventory.purchase_receipt_items
    ADD COLUMN IF NOT EXISTS is_landed_costs_line BOOLEAN NOT NULL DEFAULT FALSE;
