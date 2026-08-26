-- Down: drop inventory.locations table
DROP TABLE IF EXISTS inventory.locations CASCADE;
DROP FUNCTION IF EXISTS inventory.locations_audit_timestamp() CASCADE;
