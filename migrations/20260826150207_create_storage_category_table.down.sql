-- Down: drop inventory.storage_categories table
DROP TABLE IF EXISTS inventory.storage_categories CASCADE;
DROP FUNCTION IF EXISTS inventory.storage_categories_audit_timestamp() CASCADE;
