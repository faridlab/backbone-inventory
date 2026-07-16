-- Down: remove the company RLS fence for inventory module

-- Reverse the company RLS fence for inventory.delivery_notes
DROP POLICY IF EXISTS delivery_notes_company_isolation ON inventory.delivery_notes;
ALTER TABLE inventory.delivery_notes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.delivery_notes DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.purchase_receipts
DROP POLICY IF EXISTS purchase_receipts_company_isolation ON inventory.purchase_receipts;
ALTER TABLE inventory.purchase_receipts NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.purchase_receipts DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.stock_entries
DROP POLICY IF EXISTS stock_entries_company_isolation ON inventory.stock_entries;
ALTER TABLE inventory.stock_entries NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_entries DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.stock_ledger_entries
DROP POLICY IF EXISTS stock_ledger_entries_company_isolation ON inventory.stock_ledger_entries;
ALTER TABLE inventory.stock_ledger_entries NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_ledger_entries DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.bins
DROP POLICY IF EXISTS bins_company_isolation ON inventory.bins;
ALTER TABLE inventory.bins NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.bins DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.stock_reconciliations
DROP POLICY IF EXISTS stock_reconciliations_company_isolation ON inventory.stock_reconciliations;
ALTER TABLE inventory.stock_reconciliations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_reconciliations DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.warehouses
DROP POLICY IF EXISTS warehouses_company_isolation ON inventory.warehouses;
ALTER TABLE inventory.warehouses NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.warehouses DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for inventory.stock_items
DROP POLICY IF EXISTS stock_items_company_isolation ON inventory.stock_items;
ALTER TABLE inventory.stock_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_items DISABLE ROW LEVEL SECURITY;

