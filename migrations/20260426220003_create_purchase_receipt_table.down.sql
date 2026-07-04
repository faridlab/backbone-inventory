-- Down: drop inventory.purchase_receipts table
DROP TABLE IF EXISTS inventory.purchase_receipts CASCADE;
DROP FUNCTION IF EXISTS inventory.purchase_receipts_audit_timestamp() CASCADE;
