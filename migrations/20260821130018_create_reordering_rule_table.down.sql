-- Down: drop inventory.reordering_rules table
DROP TABLE IF EXISTS inventory.reordering_rules CASCADE;
DROP FUNCTION IF EXISTS inventory.reordering_rules_audit_timestamp() CASCADE;
