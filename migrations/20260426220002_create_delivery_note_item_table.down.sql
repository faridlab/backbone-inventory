-- Down: drop inventory.delivery_note_items table
DROP TABLE IF EXISTS inventory.delivery_note_items CASCADE;
DROP FUNCTION IF EXISTS inventory.delivery_note_items_audit_timestamp() CASCADE;
