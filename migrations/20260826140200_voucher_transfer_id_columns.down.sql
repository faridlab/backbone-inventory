DROP INDEX IF EXISTS inventory.idx_stock_reconciliations_transfer_id;
DROP INDEX IF EXISTS inventory.idx_stock_entries_transfer_id;
DROP INDEX IF EXISTS inventory.idx_delivery_notes_transfer_id;
DROP INDEX IF EXISTS inventory.idx_purchase_receipts_transfer_id;

ALTER TABLE inventory.stock_reconciliations DROP COLUMN IF EXISTS transfer_id;
ALTER TABLE inventory.stock_entries         DROP COLUMN IF EXISTS transfer_id;
ALTER TABLE inventory.delivery_notes        DROP COLUMN IF EXISTS transfer_id;
ALTER TABLE inventory.purchase_receipts     DROP COLUMN IF EXISTS transfer_id;
