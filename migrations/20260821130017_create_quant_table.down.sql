-- Down: drop inventory.stock_quants table
DROP TABLE IF EXISTS inventory.stock_quants CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_quants_audit_timestamp() CASCADE;
