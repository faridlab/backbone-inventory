-- Down: drop inventory.putaway_rules table
DROP TABLE IF EXISTS inventory.putaway_rules CASCADE;
DROP FUNCTION IF EXISTS inventory.putaway_rules_audit_timestamp() CASCADE;
