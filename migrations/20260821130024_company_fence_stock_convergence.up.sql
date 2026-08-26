-- Company RLS fence for the stock-convergence tables (ADR-0008, ADR-0014).
-- Hand-authored: the generated enable_company_rls migration predates these tables and
-- a regen does not rewrite it, so this carries the fence for them explicitly.
--
-- The module declares `company_fence: shared_blank` (schema/models/index.model.yaml).
-- The predicate is the shared_blank template for every table:
--     company_id = NULLIF(current_setting('app.company_id', true), '')::uuid
--     OR company_id IS NULL
-- On the NOT NULL company_id tables (transfers, stock_moves, stock_move_lines,
-- stock_quants, reordering_rules) the IS NULL arm is dead — those tables are
-- behaviorally strict. On the nullable master-data tables (locations, lots,
-- packages, operation_types, routes, route_rules) NULL company rows are shared
-- rows visible to every company session (the port of the [False] escape).
--
-- Requires the app to connect as a non-superuser role; migrations/seeders run as
-- the owner and bypass.

-- transactions (strict posture via NOT NULL column) --------------------------------

ALTER TABLE inventory.transfers ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.transfers FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS transfers_company_isolation ON inventory.transfers;
CREATE POLICY transfers_company_isolation ON inventory.transfers
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.stock_moves ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_moves FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS stock_moves_company_isolation ON inventory.stock_moves;
CREATE POLICY stock_moves_company_isolation ON inventory.stock_moves
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.stock_move_lines ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_move_lines FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS stock_move_lines_company_isolation ON inventory.stock_move_lines;
CREATE POLICY stock_move_lines_company_isolation ON inventory.stock_move_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.stock_quants ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.stock_quants FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS stock_quants_company_isolation ON inventory.stock_quants;
CREATE POLICY stock_quants_company_isolation ON inventory.stock_quants
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.reordering_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.reordering_rules FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reordering_rules_company_isolation ON inventory.reordering_rules;
CREATE POLICY reordering_rules_company_isolation ON inventory.reordering_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

-- master data (shared_blank posture — NULL company = shared row) ---------------------

ALTER TABLE inventory.locations ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.locations FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS locations_company_isolation ON inventory.locations;
CREATE POLICY locations_company_isolation ON inventory.locations
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.lots ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.lots FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lots_company_isolation ON inventory.lots;
CREATE POLICY lots_company_isolation ON inventory.lots
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.packages ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.packages FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS packages_company_isolation ON inventory.packages;
CREATE POLICY packages_company_isolation ON inventory.packages
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.operation_types ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.operation_types FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS operation_types_company_isolation ON inventory.operation_types;
CREATE POLICY operation_types_company_isolation ON inventory.operation_types
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.routes ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.routes FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS routes_company_isolation ON inventory.routes;
CREATE POLICY routes_company_isolation ON inventory.routes
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.route_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.route_rules FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS route_rules_company_isolation ON inventory.route_rules;
CREATE POLICY route_rules_company_isolation ON inventory.route_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);
