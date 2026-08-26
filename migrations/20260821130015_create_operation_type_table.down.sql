-- Down: drop inventory.operation_types table
DROP TABLE IF EXISTS inventory.operation_types CASCADE;
DROP FUNCTION IF EXISTS inventory.operation_types_audit_timestamp() CASCADE;
