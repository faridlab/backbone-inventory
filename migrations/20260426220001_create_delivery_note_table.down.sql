-- Down: drop inventory.delivery_notes table
DROP TABLE IF EXISTS inventory.delivery_notes CASCADE;
DROP FUNCTION IF EXISTS inventory.delivery_notes_audit_timestamp() CASCADE;
