-- Down: drop inventory.storage_category_capacities table
DROP TABLE IF EXISTS inventory.storage_category_capacities CASCADE;
DROP FUNCTION IF EXISTS inventory.storage_category_capacities_audit_timestamp() CASCADE;
