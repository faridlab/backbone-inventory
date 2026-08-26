-- Down: drop inventory.landed_cost_lines table
DROP TABLE IF EXISTS inventory.landed_cost_lines CASCADE;
DROP FUNCTION IF EXISTS inventory.landed_cost_lines_audit_timestamp() CASCADE;
