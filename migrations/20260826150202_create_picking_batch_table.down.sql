-- Down: drop inventory.picking_batches table
DROP TABLE IF EXISTS inventory.picking_batches CASCADE;
DROP FUNCTION IF EXISTS inventory.picking_batches_audit_timestamp() CASCADE;
