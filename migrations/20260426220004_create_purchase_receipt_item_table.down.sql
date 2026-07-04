-- Down: drop inventory.purchase_receipt_items table
DROP TABLE IF EXISTS inventory.purchase_receipt_items CASCADE;
DROP FUNCTION IF EXISTS inventory.purchase_receipt_items_audit_timestamp() CASCADE;
