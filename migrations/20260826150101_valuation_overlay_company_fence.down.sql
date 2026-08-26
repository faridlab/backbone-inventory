-- Reverse the strict company RLS fence for the valuation-overlay tables.

DROP POLICY IF EXISTS landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines;
ALTER TABLE inventory.landed_cost_adjustment_lines NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_cost_adjustment_lines DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS landed_cost_lines_company_isolation ON inventory.landed_cost_lines;
ALTER TABLE inventory.landed_cost_lines NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_cost_lines DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS landed_costs_company_isolation ON inventory.landed_costs;
ALTER TABLE inventory.landed_costs NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.landed_costs DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS inventory_company_settings_company_isolation ON inventory.inventory_company_settings;
ALTER TABLE inventory.inventory_company_settings NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.inventory_company_settings DISABLE ROW LEVEL SECURITY;
