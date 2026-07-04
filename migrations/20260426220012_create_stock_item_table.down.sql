-- Down: drop inventory.stock_items table
DROP TABLE IF EXISTS inventory.stock_items CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_items_audit_timestamp() CASCADE;
