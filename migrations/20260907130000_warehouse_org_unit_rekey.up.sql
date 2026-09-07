-- Re-key inventory.warehouses from company_id to org_unit_id (ADR-0028).
--
-- org_units ids are UUID-stable: a company node's id IS the company id it was
-- seeded from (organization's spine migration). So the backfill is a COPY, not
-- a remap — every existing warehouses.company_id already names a company node.
--
-- What this migration does, in order:
--   1. add org_unit_id, copy company_id into it, tighten to NOT NULL
--   2. write-path kind guard: org_unit_id must name a company or branch node
--      (a root node or an unknown id is rejected — root is for tenant-shared
--      rows, and warehouses are never tenant-shared)
--   3. re-key the partial unique indexes (code, name) from company_id to
--      org_unit_id and add a plain index for scope scans
--   4. swap the RLS fence: entitlement-union over app.scope_unit_ids instead
--      of equality on app.company_id. The session resolver sets
--      app.scope_unit_ids to the union of the entitled subtrees (which always
--      includes the root node). Unset or empty var = zero rows, fail-closed.
--   5. drop company_id — org_unit_id is the only scoping key (ADR-0028).
--
-- Requires the organization schema (org_units + spine) to exist in this
-- database; in composed services both module schemas live in the same DB.

-- 1. Column + backfill (copy, not remap) + NOT NULL.
ALTER TABLE inventory.warehouses ADD COLUMN org_unit_id UUID;
UPDATE inventory.warehouses SET org_unit_id = company_id;
ALTER TABLE inventory.warehouses ALTER COLUMN org_unit_id SET NOT NULL;

-- 2. Write-path kind guard.
CREATE OR REPLACE FUNCTION inventory.warehouses_org_unit_kind_guard() RETURNS trigger AS $$
DECLARE
    v_kind text;
BEGIN
    SELECT kind::text INTO v_kind FROM organization.org_units WHERE id = NEW.org_unit_id;
    IF v_kind IS NULL THEN
        RAISE EXCEPTION 'warehouses.org_unit_id % does not reference an organization.org_units node', NEW.org_unit_id;
    END IF;
    IF v_kind NOT IN ('company', 'branch') THEN
        RAISE EXCEPTION 'warehouses.org_unit_id must reference a company or branch node, got a % node (%)', v_kind, NEW.org_unit_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS warehouses_org_unit_kind_guard ON inventory.warehouses;
CREATE TRIGGER warehouses_org_unit_kind_guard
    BEFORE INSERT OR UPDATE OF org_unit_id ON inventory.warehouses
    FOR EACH ROW EXECUTE FUNCTION inventory.warehouses_org_unit_kind_guard();

-- 3. Unique indexes re-keyed to the org node.
DROP INDEX IF EXISTS inventory.idx_warehouses_company_id_code;
DROP INDEX IF EXISTS inventory.idx_warehouses_company_id_name;
CREATE UNIQUE INDEX idx_warehouses_org_unit_id_code
    ON inventory.warehouses (org_unit_id, code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX idx_warehouses_org_unit_id_name
    ON inventory.warehouses (org_unit_id, name) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX idx_warehouses_org_unit_id ON inventory.warehouses (org_unit_id);

-- 4. Entitlement-union fence (ADR-0028).
--    Unset var -> NULL array -> no rows visible; empty string -> empty array
--    -> no rows visible. Both fail closed.
DROP POLICY IF EXISTS warehouses_company_isolation ON inventory.warehouses;
CREATE POLICY warehouses_org_unit_isolation ON inventory.warehouses
    FOR ALL
    USING      (org_unit_id = ANY(string_to_array(current_setting('app.scope_unit_ids', true), ',')::uuid[]))
    WITH CHECK (org_unit_id = ANY(string_to_array(current_setting('app.scope_unit_ids', true), ',')::uuid[]));

-- 5. The old key is gone; org_unit_id is the only scoping key.
ALTER TABLE inventory.warehouses DROP COLUMN company_id;
