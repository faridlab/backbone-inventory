-- Down: drop inventory.stock_move_lines table
DROP TABLE IF EXISTS inventory.stock_move_lines CASCADE;
DROP FUNCTION IF EXISTS inventory.stock_move_lines_audit_timestamp() CASCADE;
