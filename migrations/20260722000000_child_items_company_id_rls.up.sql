-- ADR-0010 Decision A: direct company_id + FORCE RLS on inventory child (line) tables.
--
-- The four child tables (delivery_note_items, purchase_receipt_items, stock_entry_items,
-- stock_reconciliation_items) previously had NO company_id column and so could not be fenced
-- by RLS — a cross-tenant read could list another tenant's lines if it could guess a parent id.
-- The parents are already fenced (migration 20260426220013). This migration backfills each
-- child from its parent, marks the column NOT NULL, and applies the ADR-0008 invariant #1 fence
-- (FORCE RLS + USING/WITH CHECK against app.company_id).
--
-- No FK to a companies table is added: company_id is a LOGICAL FK only
-- (@exclude_from_foreign_key_check in schema YAML), consistent with every other company_id
-- in this module (no companies table exists in the inventory schema; the parents don't have
-- such an FK either).
--
-- Reversible by the companion .down.sql.

-- ---------------------------------------------------------------------------
-- delivery_note_items
-- ---------------------------------------------------------------------------
ALTER TABLE inventory.delivery_note_items ADD COLUMN company_id UUID;

UPDATE inventory.delivery_note_items AS c
   SET company_id = p.company_id
  FROM inventory.delivery_notes AS p
 WHERE c.delivery_id = p.id
   AND c.company_id IS NULL;

ALTER TABLE inventory.delivery_note_items ALTER COLUMN company_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_delivery_note_items_company_id
    ON inventory.delivery_note_items (company_id);

ALTER TABLE inventory.delivery_note_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.delivery_note_items FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS delivery_note_items_company_isolation ON inventory.delivery_note_items;
CREATE POLICY delivery_note_items_company_isolation ON inventory.delivery_note_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- ---------------------------------------------------------------------------
-- purchase_receipt_items
-- ---------------------------------------------------------------------------
ALTER TABLE inventory.purchase_receipt_items ADD COLUMN company_id UUID;

UPDATE inventory.purchase_receipt_items AS c
   SET company_id = p.company_id
  FROM inventory.purchase_receipts AS p
 WHERE c.receipt_id = p.id
   AND c.company_id IS NULL;

ALTER TABLE inventory.purchase_receipt_items ALTER COLUMN company_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_purchase_receipt_items_company_id
    ON inventory.purchase_receipt_items (company_id);

ALTER TABLE inventory.purchase_receipt_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.purchase_receipt_items FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS purchase_receipt_items_company_isolation ON inventory.purchase_receipt_items;
CREATE POLICY purchase_receipt_items_company_isolation ON inventory.purchase_receipt_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- ---------------------------------------------------------------------------
-- stock_entry_items
-- ---------------------------------------------------------------------------
ALTER TABLE inventory.stock_entry_items ADD COLUMN company_id UUID;

UPDATE inventory.stock_entry_items AS c
   SET company_id = p.company_id
  FROM inventory.stock_entries AS p
 WHERE c.entry_id = p.id
   AND c.company_id IS NULL;

ALTER TABLE inventory.stock_entry_items ALTER COLUMN company_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_stock_entry_items_company_id
    ON inventory.stock_entry_items (company_id);

ALTER TABLE inventory.stock_entry_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_entry_items FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS stock_entry_items_company_isolation ON inventory.stock_entry_items;
CREATE POLICY stock_entry_items_company_isolation ON inventory.stock_entry_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- ---------------------------------------------------------------------------
-- stock_reconciliation_items
-- ---------------------------------------------------------------------------
ALTER TABLE inventory.stock_reconciliation_items ADD COLUMN company_id UUID;

UPDATE inventory.stock_reconciliation_items AS c
   SET company_id = p.company_id
  FROM inventory.stock_reconciliations AS p
 WHERE c.reconciliation_id = p.id
   AND c.company_id IS NULL;

ALTER TABLE inventory.stock_reconciliation_items ALTER COLUMN company_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_stock_reconciliation_items_company_id
    ON inventory.stock_reconciliation_items (company_id);

ALTER TABLE inventory.stock_reconciliation_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_reconciliation_items FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items;
CREATE POLICY stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
