-- Down: drop inventory.lots table
DROP TABLE IF EXISTS inventory.lots CASCADE;
DROP FUNCTION IF EXISTS inventory.lots_audit_timestamp() CASCADE;
