-- Reverse: drop the `currency` column added to delivery-note and stock-reconciliation headers.

ALTER TABLE inventory.stock_reconciliations DROP COLUMN IF EXISTS currency;
ALTER TABLE inventory.delivery_notes DROP COLUMN IF EXISTS currency;
