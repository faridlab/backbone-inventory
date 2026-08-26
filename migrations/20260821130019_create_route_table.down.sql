-- Down: drop inventory.routes table
DROP TABLE IF EXISTS inventory.routes CASCADE;
DROP FUNCTION IF EXISTS inventory.routes_audit_timestamp() CASCADE;
