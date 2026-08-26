-- Down: drop inventory.transfers table
DROP TABLE IF EXISTS inventory.transfers CASCADE;
DROP FUNCTION IF EXISTS inventory.transfers_audit_timestamp() CASCADE;
