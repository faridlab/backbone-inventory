-- Down: drop inventory.scraps table
DROP TABLE IF EXISTS inventory.scraps CASCADE;
DROP FUNCTION IF EXISTS inventory.scraps_audit_timestamp() CASCADE;
