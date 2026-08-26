-- Down: drop inventory.stock_moves table
DROP TABLE IF EXISTS inventory.stock_moves CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_moves_audit_timestamp() CASCADE;
