-- Down: drop inventory.landed_costs table
DROP TABLE IF EXISTS inventory.landed_costs CASCADE;
DROP FUNCTION IF EXISTS inventory.landed_costs_audit_timestamp() CASCADE;
