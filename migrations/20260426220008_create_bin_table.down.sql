-- Down: drop inventory.bins table
DROP TABLE IF EXISTS inventory.bins CASCADE;
DROP FUNCTION IF EXISTS inventory.bins_audit_timestamp() CASCADE;
