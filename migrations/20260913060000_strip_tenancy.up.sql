-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the inventory tables (ADR-0029): the
-- module is tenant-agnostic; org scoping is installed by the COMPOSING service's
-- tenancy decorator, never by the module. Dropped here, per table: the
-- <table>_company_isolation RLS policy, the company-leading indexes, and the
-- company_id column.
--
-- Company-scoped uniqueness is re-declared ORG-SCOPED in the composing
-- decorator (13 re-declarations): warehouses (code) and (name),
-- stock_items (item), lots (name, item), locations (barcode), transfers
-- (name), reordering_rules (item, location), bins (org, item, warehouse),
-- inventory_company_settings (org), landed_costs (lc_number),
-- picking_batches (name), scraps (name), scrap_reason_tags (name — the old
-- split NULL/company arms collapse into one).
--
-- The composed-world shapes: the shared masters (locations, lots, packages,
-- routes, route_rules, operation_types, scrap_reason_tags and the storage
-- family) fence with allow_root — rows the old policies' NULL arm admitted
-- become tenant-root-anchored at backfill, and the scope union (subtree ∪
-- tenant root) carries the shared semantics. Transactions, vouchers and the
-- per-unit configuration rows are plain org-scoped rows (decorator fill stamps
-- the acting unit).
--
-- Ordering guard (the decorator must run FIRST on any database with data): the
-- module never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from
--      company_id — or b) it is empty (a fresh database).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping
-- a column that still holds the only tenancy key. The file is re-runnable
-- (every drop is IF EXISTS and the tracker has no checksums), so a failed run
-- retries cleanly after the decorator lands.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those
-- now.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'warehouses', 'stock_items', 'locations', 'lots', 'packages',
        'operation_types', 'transfers', 'routes', 'route_rules',
        'reordering_rules', 'stock_moves', 'stock_move_lines', 'stock_quants',
        'stock_ledger_entries', 'bins', 'stock_entries', 'stock_entry_items',
        'purchase_receipts', 'purchase_receipt_items', 'delivery_notes',
        'delivery_note_items', 'stock_reconciliations',
        'stock_reconciliation_items', 'inventory_company_settings',
        'landed_costs', 'landed_cost_lines', 'landed_cost_adjustment_lines',
        'picking_batches', 'scraps', 'scrap_reason_tags', 'package_types',
        'storage_categories', 'storage_category_capacities', 'putaway_rules']
    LOOP
        IF to_regclass(format('inventory.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'inventory' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM inventory.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM inventory.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' inventory.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- Guard triggers whose bodies read company columns (they block the DROPs below).
--
-- The picking-batch member guard is PURELY a company check (cross-tenant leak
-- backstop); the decorator's org fence carries that guarantee now — trigger and
-- function both go.
DROP TRIGGER IF EXISTS batch_member_company_guard ON inventory.transfers;
DROP FUNCTION IF EXISTS inventory.batch_member_company_guard();

-- The rule guard carries TWO checks: the R11 company-consistency arms (dead under
-- the org fence) and the R13 destination-not-a-view guard (company-free, still
-- load-bearing). Replace the function with the view-guard-only body and re-arm the
-- trigger without the company column in its UPDATE list.
DROP TRIGGER IF EXISTS route_rule_company_consistency ON inventory.route_rules;
CREATE OR REPLACE FUNCTION inventory.route_rule_company_consistency() RETURNS trigger AS $$
DECLARE
    dest_usage TEXT;
BEGIN
    SELECT usage::text INTO dest_usage FROM inventory.locations
     WHERE id = NEW.location_dest_id;

    IF dest_usage = 'view' THEN
        RAISE EXCEPTION 'rule_dest_is_view: rule destination % is a view location', NEW.location_dest_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER route_rule_company_consistency
    BEFORE INSERT OR UPDATE OF picking_type_id, warehouse_id, location_dest_id
    ON inventory.route_rules
    FOR EACH ROW EXECUTE FUNCTION inventory.route_rule_company_consistency();

-- Policies (the module-native fence): one per table.
DROP POLICY IF EXISTS warehouses_company_isolation                ON inventory.warehouses;
DROP POLICY IF EXISTS stock_items_company_isolation              ON inventory.stock_items;
DROP POLICY IF EXISTS locations_company_isolation                ON inventory.locations;
DROP POLICY IF EXISTS lots_company_isolation                     ON inventory.lots;
DROP POLICY IF EXISTS packages_company_isolation                 ON inventory.packages;
DROP POLICY IF EXISTS operation_types_company_isolation          ON inventory.operation_types;
DROP POLICY IF EXISTS transfers_company_isolation                ON inventory.transfers;
DROP POLICY IF EXISTS routes_company_isolation                   ON inventory.routes;
DROP POLICY IF EXISTS route_rules_company_isolation              ON inventory.route_rules;
DROP POLICY IF EXISTS reordering_rules_company_isolation         ON inventory.reordering_rules;
DROP POLICY IF EXISTS stock_moves_company_isolation              ON inventory.stock_moves;
DROP POLICY IF EXISTS stock_move_lines_company_isolation         ON inventory.stock_move_lines;
DROP POLICY IF EXISTS stock_quants_company_isolation             ON inventory.stock_quants;
DROP POLICY IF EXISTS stock_ledger_entries_company_isolation     ON inventory.stock_ledger_entries;
DROP POLICY IF EXISTS bins_company_isolation                     ON inventory.bins;
DROP POLICY IF EXISTS stock_entries_company_isolation            ON inventory.stock_entries;
DROP POLICY IF EXISTS stock_entry_items_company_isolation        ON inventory.stock_entry_items;
DROP POLICY IF EXISTS purchase_receipts_company_isolation        ON inventory.purchase_receipts;
DROP POLICY IF EXISTS purchase_receipt_items_company_isolation   ON inventory.purchase_receipt_items;
DROP POLICY IF EXISTS delivery_notes_company_isolation           ON inventory.delivery_notes;
DROP POLICY IF EXISTS delivery_note_items_company_isolation      ON inventory.delivery_note_items;
DROP POLICY IF EXISTS stock_reconciliations_company_isolation    ON inventory.stock_reconciliations;
DROP POLICY IF EXISTS stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items;
DROP POLICY IF EXISTS inventory_company_settings_company_isolation ON inventory.inventory_company_settings;
DROP POLICY IF EXISTS landed_costs_company_isolation             ON inventory.landed_costs;
DROP POLICY IF EXISTS landed_cost_lines_company_isolation        ON inventory.landed_cost_lines;
DROP POLICY IF EXISTS landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines;
DROP POLICY IF EXISTS picking_batches_company_isolation          ON inventory.picking_batches;
DROP POLICY IF EXISTS scraps_company_isolation                   ON inventory.scraps;
DROP POLICY IF EXISTS scrap_reason_tags_company_isolation        ON inventory.scrap_reason_tags;
DROP POLICY IF EXISTS package_types_company_isolation            ON inventory.package_types;
DROP POLICY IF EXISTS storage_categories_company_isolation       ON inventory.storage_categories;
DROP POLICY IF EXISTS storage_category_capacities_company_isolation ON inventory.storage_category_capacities;
DROP POLICY IF EXISTS putaway_rules_company_isolation            ON inventory.putaway_rules;

-- Company-leading indexes: every one is dropped (14 uniques re-declare
-- org-scoped in the composing decorator; the rest were company-prefix lookups
-- the org fence supersedes).
DROP INDEX IF EXISTS inventory._product_location_index;
DROP INDEX IF EXISTS inventory.idx_bins_company_id_item_id_warehouse_id;
DROP INDEX IF EXISTS inventory.idx_delivery_note_items_company_id;
DROP INDEX IF EXISTS inventory.idx_delivery_notes_company_id_customer_id_status;
DROP INDEX IF EXISTS inventory.idx_inventory_company_settings_company_id;
DROP INDEX IF EXISTS inventory.idx_landed_costs_company_id_lc_number;
DROP INDEX IF EXISTS inventory.idx_locations_barcode_company_id;
DROP INDEX IF EXISTS inventory.idx_locations_company_id;
DROP INDEX IF EXISTS inventory.idx_lots_company_id;
DROP INDEX IF EXISTS inventory.idx_lots_name_item_id_company_id;
DROP INDEX IF EXISTS inventory.idx_operation_types_company_id;
DROP INDEX IF EXISTS inventory.idx_package_types_company_id;
DROP INDEX IF EXISTS inventory.idx_packages_company_id;
DROP INDEX IF EXISTS inventory.idx_picking_batches_company_id;
DROP INDEX IF EXISTS inventory.idx_picking_batches_name_company_id;
DROP INDEX IF EXISTS inventory.idx_purchase_receipt_items_company_id;
DROP INDEX IF EXISTS inventory.idx_purchase_receipts_company_id_supplier_id_status;
DROP INDEX IF EXISTS inventory.idx_putaway_rules_company_id;
DROP INDEX IF EXISTS inventory.idx_reordering_rules_company_id;
DROP INDEX IF EXISTS inventory.idx_reordering_rules_item_id_location_id_company_id;
DROP INDEX IF EXISTS inventory.idx_route_rules_company_id;
DROP INDEX IF EXISTS inventory.idx_routes_company_id;
DROP INDEX IF EXISTS inventory.idx_scrap_reason_tags_company_id;
DROP INDEX IF EXISTS inventory.idx_scrap_reason_tags_name;
DROP INDEX IF EXISTS inventory.idx_scrap_reason_tags_name_company_id;
DROP INDEX IF EXISTS inventory.idx_scraps_company_id;
DROP INDEX IF EXISTS inventory.idx_scraps_name_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_entries_company_id_stock_entry_type_status;
DROP INDEX IF EXISTS inventory.idx_stock_entry_items_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_items_company_id_item_id;
DROP INDEX IF EXISTS inventory.idx_stock_ledger_entries_company_id_item_id_warehouse_id_postin;
DROP INDEX IF EXISTS inventory.idx_stock_move_lines_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_moves_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_quants_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_reconciliation_items_company_id;
DROP INDEX IF EXISTS inventory.idx_stock_reconciliations_company_id_status;
DROP INDEX IF EXISTS inventory.idx_storage_categories_company_id;
DROP INDEX IF EXISTS inventory.idx_storage_category_capacities_company_id;
DROP INDEX IF EXISTS inventory.idx_transfers_company_id;
DROP INDEX IF EXISTS inventory.idx_transfers_name_company_id;
DROP INDEX IF EXISTS inventory.idx_warehouses_company_id_code;
DROP INDEX IF EXISTS inventory.idx_warehouses_company_id_name;

-- The column itself.
ALTER TABLE inventory.warehouses                   DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_items                  DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.locations                    DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.lots                         DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.packages                     DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.operation_types              DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.transfers                    DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.routes                       DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.route_rules                  DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.reordering_rules             DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_moves                  DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_move_lines             DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_quants                 DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_ledger_entries         DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.bins                         DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_entries                DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_entry_items            DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.purchase_receipts            DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.purchase_receipt_items       DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.delivery_notes               DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.delivery_note_items          DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_reconciliations        DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.stock_reconciliation_items   DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.inventory_company_settings   DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.landed_costs                 DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.landed_cost_lines            DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.landed_cost_adjustment_lines DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.picking_batches             DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.scraps                       DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.scrap_reason_tags            DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.package_types                DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.storage_categories           DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.storage_category_capacities  DROP COLUMN IF EXISTS company_id;
ALTER TABLE inventory.putaway_rules                DROP COLUMN IF EXISTS company_id;
