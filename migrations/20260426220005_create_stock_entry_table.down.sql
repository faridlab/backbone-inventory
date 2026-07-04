-- Down: drop inventory.stock_entries table
DROP TABLE IF EXISTS inventory.stock_entries CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_entries_audit_timestamp() CASCADE;
