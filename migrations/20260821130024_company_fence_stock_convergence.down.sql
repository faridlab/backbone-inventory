-- Reverse the company RLS fence for the stock-convergence tables.

DROP POLICY IF EXISTS route_rules_company_isolation ON inventory.route_rules;
ALTER TABLE inventory.route_rules NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.route_rules DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS routes_company_isolation ON inventory.routes;
ALTER TABLE inventory.routes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.routes DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS operation_types_company_isolation ON inventory.operation_types;
ALTER TABLE inventory.operation_types NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.operation_types DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS packages_company_isolation ON inventory.packages;
ALTER TABLE inventory.packages NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.packages DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS lots_company_isolation ON inventory.lots;
ALTER TABLE inventory.lots NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.lots DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS locations_company_isolation ON inventory.locations;
ALTER TABLE inventory.locations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.locations DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS reordering_rules_company_isolation ON inventory.reordering_rules;
ALTER TABLE inventory.reordering_rules NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.reordering_rules DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS stock_quants_company_isolation ON inventory.stock_quants;
ALTER TABLE inventory.stock_quants NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_quants DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS stock_move_lines_company_isolation ON inventory.stock_move_lines;
ALTER TABLE inventory.stock_move_lines NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_move_lines DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS stock_moves_company_isolation ON inventory.stock_moves;
ALTER TABLE inventory.stock_moves NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_moves DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS transfers_company_isolation ON inventory.transfers;
ALTER TABLE inventory.transfers NO FORCE ROW LEVEL SECURITY;
ALTER TABLE inventory.transfers DISABLE ROW LEVEL SECURITY;
