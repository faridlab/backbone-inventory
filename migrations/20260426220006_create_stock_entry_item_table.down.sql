-- Down: drop inventory.stock_entry_items table
DROP TABLE IF EXISTS inventory.stock_entry_items CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_entry_items_audit_timestamp() CASCADE;
