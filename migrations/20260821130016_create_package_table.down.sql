-- Down: drop inventory.packages table
DROP TABLE IF EXISTS inventory.packages CASCADE;
DROP FUNCTION IF EXISTS inventory.packages_audit_timestamp() CASCADE;
