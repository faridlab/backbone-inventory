-- Down: drop inventory.landed_cost_adjustment_lines table
DROP TABLE IF EXISTS inventory.landed_cost_adjustment_lines CASCADE;
DROP FUNCTION IF EXISTS inventory.landed_cost_adjustment_lines_audit_timestamp() CASCADE;
