-- Down: drop inventory.stock_reconciliations table
DROP TABLE IF EXISTS inventory.stock_reconciliations CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_reconciliations_audit_timestamp() CASCADE;
