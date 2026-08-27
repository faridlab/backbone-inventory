-- Down: drop inventory.package_types table
DROP TABLE IF EXISTS inventory.package_types CASCADE;
DROP FUNCTION IF EXISTS inventory.package_types_audit_timestamp() CASCADE;
