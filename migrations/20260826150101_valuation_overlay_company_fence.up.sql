-- Strict company RLS fence for the valuation-overlay tables (ADR-0008, ADR-0014).
-- Hand-authored: the generated enable_company_rls migration predates these tables and
-- a regen does not rewrite it, so this carries the fence for them explicitly
-- (mirrors 20260821130024_company_fence_stock_convergence).
--
-- STRICT posture — no shared_blank escape: every one of these tables carries a NOT NULL
-- company_id and its rows are one tenant's by definition (settings are per-company; a
-- landed cost books in its acting company; the cost lines and the allocation worksheet
-- are children of a landed cost). The predicate has no `OR company_id IS NULL` arm.

-- Requires the app to connect as a non-superuser role; migrations/seeders run as
-- the owner and bypass.

ALTER TABLE inventory.inventory_company_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.inventory_company_settings FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS inventory_company_settings_company_isolation ON inventory.inventory_company_settings;
CREATE POLICY inventory_company_settings_company_isolation ON inventory.inventory_company_settings
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

ALTER TABLE inventory.landed_costs ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_costs FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS landed_costs_company_isolation ON inventory.landed_costs;
CREATE POLICY landed_costs_company_isolation ON inventory.landed_costs
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

ALTER TABLE inventory.landed_cost_lines ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_cost_lines FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS landed_cost_lines_company_isolation ON inventory.landed_cost_lines;
CREATE POLICY landed_cost_lines_company_isolation ON inventory.landed_cost_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

ALTER TABLE inventory.landed_cost_adjustment_lines ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_cost_adjustment_lines FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines;
CREATE POLICY landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
