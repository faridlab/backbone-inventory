-- Reverse the warehouses org_unit re-key (ADR-0028).
--
-- Restores company_id by walking each warehouse's node up the org tree to the
-- first company-kind ancestor (a warehouse under a branch node belongs to that
-- branch's company). Best-effort: requires the organization org_units spine
-- to still exist. The org fence policy, kind-guard trigger, and org_unit
-- indexes are dropped; the original company-scoped fence and indexes are
-- recreated.

DROP POLICY IF EXISTS warehouses_org_unit_isolation ON inventory.warehouses;
DROP TRIGGER IF EXISTS warehouses_org_unit_kind_guard ON inventory.warehouses;
DROP FUNCTION IF EXISTS inventory.warehouses_org_unit_kind_guard();

DROP INDEX IF EXISTS inventory.idx_warehouses_org_unit_id_code;
DROP INDEX IF EXISTS inventory.idx_warehouses_org_unit_id_name;
DROP INDEX IF EXISTS inventory.idx_warehouses_org_unit_id;

ALTER TABLE inventory.warehouses ADD COLUMN company_id UUID;

UPDATE inventory.warehouses w
SET company_id = (
    WITH RECURSIVE up AS (
        SELECT o.id, o.parent_id, o.kind::text AS kind
        FROM organization.org_units o WHERE o.id = w.org_unit_id
        UNION ALL
        SELECT p.id, p.parent_id, p.kind::text
        FROM organization.org_units p JOIN up u ON p.id = u.parent_id
    )
    SELECT up.id FROM up WHERE up.kind = 'company' LIMIT 1
);

-- Warehouses whose node chain has no company ancestor (or whose node is gone)
-- cannot map back; that is data loss territory and must not be silently
-- coerced. Fail loudly instead of writing NULLs past NOT NULL below.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM inventory.warehouses WHERE company_id IS NULL) THEN
        RAISE EXCEPTION 'warehouses rows exist whose org_unit chain has no company ancestor; cannot reverse the re-key';
    END IF;
END $$;

ALTER TABLE inventory.warehouses ALTER COLUMN company_id SET NOT NULL;
ALTER TABLE inventory.warehouses DROP COLUMN org_unit_id;

CREATE UNIQUE INDEX idx_warehouses_company_id_code
    ON inventory.warehouses (company_id, code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX idx_warehouses_company_id_name
    ON inventory.warehouses (company_id, name) WHERE (metadata->>'deleted_at') IS NULL;

CREATE POLICY warehouses_company_isolation ON inventory.warehouses
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
