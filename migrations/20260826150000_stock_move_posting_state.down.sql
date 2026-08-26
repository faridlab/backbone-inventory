DROP INDEX IF EXISTS inventory.idx_stock_moves_posting_state;

ALTER TABLE inventory.stock_moves DROP COLUMN IF EXISTS posting_state;
