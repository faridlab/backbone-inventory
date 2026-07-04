-- Down: drop inventory.stock_ledger_entries table
DROP TABLE IF EXISTS inventory.stock_ledger_entries CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_ledger_entries_audit_timestamp() CASCADE;
