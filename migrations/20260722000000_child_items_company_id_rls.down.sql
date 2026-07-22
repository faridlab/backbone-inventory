-- Reverse ADR-0010 Decision A: drop the child-table RLS fence and the company_id column.
-- WARNING: this reopens the cross-tenant read path on child rows — only run when rolling back
-- the migration is explicitly intended.

-- ---------------------------------------------------------------------------
-- stock_reconciliation_items
-- ---------------------------------------------------------------------------
DROP POLICY IF EXISTS stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items;
ALTER TABLE inventory.stock_reconciliation_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_reconciliation_items DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS inventory.idx_stock_reconciliation_items_company_id;
ALTER TABLE inventory.stock_reconciliation_items DROP COLUMN IF EXISTS company_id;

-- ---------------------------------------------------------------------------
-- stock_entry_items
-- ---------------------------------------------------------------------------
DROP POLICY IF EXISTS stock_entry_items_company_isolation ON inventory.stock_entry_items;
ALTER TABLE inventory.stock_entry_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_entry_items DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS inventory.idx_stock_entry_items_company_id;
ALTER TABLE inventory.stock_entry_items DROP COLUMN IF EXISTS company_id;

-- ---------------------------------------------------------------------------
-- purchase_receipt_items
-- ---------------------------------------------------------------------------
DROP POLICY IF EXISTS purchase_receipt_items_company_isolation ON inventory.purchase_receipt_items;
ALTER TABLE inventory.purchase_receipt_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.purchase_receipt_items DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS inventory.idx_purchase_receipt_items_company_id;
ALTER TABLE inventory.purchase_receipt_items DROP COLUMN IF EXISTS company_id;

-- ---------------------------------------------------------------------------
-- delivery_note_items
-- ---------------------------------------------------------------------------
DROP POLICY IF EXISTS delivery_note_items_company_isolation ON inventory.delivery_note_items;
ALTER TABLE inventory.delivery_note_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.delivery_note_items DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS inventory.idx_delivery_note_items_company_id;
ALTER TABLE inventory.delivery_note_items DROP COLUMN IF EXISTS company_id;
