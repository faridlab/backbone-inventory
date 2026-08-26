-- Reverse the valuation-overlay columns.

ALTER TABLE inventory.purchase_receipt_items DROP COLUMN IF EXISTS is_landed_costs_line;
ALTER TABLE inventory.locations DROP COLUMN IF EXISTS valuation_account_id;
ALTER TABLE inventory.stock_items DROP COLUMN IF EXISTS weight_per_unit;
