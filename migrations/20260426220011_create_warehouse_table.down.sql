-- Down: drop inventory.warehouses table
DROP TABLE IF EXISTS inventory.warehouses CASCADE;
DROP FUNCTION IF EXISTS inventory.warehouses_audit_timestamp() CASCADE;
