-- Down: drop inventory.route_rules table
DROP TABLE IF EXISTS inventory.route_rules CASCADE;
DROP FUNCTION IF EXISTS inventory.route_rules_audit_timestamp() CASCADE;
