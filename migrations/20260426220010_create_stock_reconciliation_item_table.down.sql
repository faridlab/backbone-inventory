-- Down: drop inventory.stock_reconciliation_items table
DROP TABLE IF EXISTS inventory.stock_reconciliation_items CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_reconciliation_items_audit_timestamp() CASCADE;
