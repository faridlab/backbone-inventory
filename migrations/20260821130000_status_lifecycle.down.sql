-- Down: restore the stock-ledger-entry cancellation boolean from the status enum
-- Only 'cancelled' rows are written back to TRUE; 'active' rows ride the boolean DEFAULT FALSE.

ALTER TABLE inventory.stock_ledger_entries ADD COLUMN is_cancelled BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE inventory.stock_ledger_entries SET is_cancelled = TRUE WHERE status = 'cancelled';
ALTER TABLE inventory.stock_ledger_entries DROP COLUMN status;

DROP TYPE IF EXISTS stock_ledger_status;
