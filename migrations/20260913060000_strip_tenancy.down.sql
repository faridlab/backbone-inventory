-- Hand-authored (user-owned). Not regenerated.
--
-- Reverse the tenancy strip: restore the module-native company fence the module
-- declared before ADR-0029 — strict NOT NULL company_id on the transaction and
-- configuration tables; nullable shared_blank company_id on the masters
-- (locations, lots, packages, routes, route_rules, operation_types,
-- scrap_reason_tags and the storage family). Rows the decorator moved to
-- org_unit_id keep their org anchor — this down file only re-adds the columns,
-- indexes and policies; it does not move data back.

-- Shared masters: nullable (NULL = the one shared set).
ALTER TABLE inventory.locations                   ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.lots                        ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.packages                    ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.operation_types             ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.routes                      ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.route_rules                 ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.scrap_reason_tags           ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.package_types               ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.storage_categories          ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.storage_category_capacities ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE inventory.putaway_rules               ADD COLUMN IF NOT EXISTS company_id UUID;

-- Transaction and configuration tables: strict.
ALTER TABLE inventory.warehouses                   ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_items                  ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.transfers                    ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.reordering_rules             ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_moves                  ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_move_lines             ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_quants                 ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_ledger_entries         ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.bins                         ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_entries                ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_entry_items            ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.purchase_receipts            ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.purchase_receipt_items       ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.delivery_notes               ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.delivery_note_items          ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_reconciliations        ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.stock_reconciliation_items   ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.inventory_company_settings   ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.landed_costs                 ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.landed_cost_lines            ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.landed_cost_adjustment_lines ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.picking_batches             ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE inventory.scraps                       ADD COLUMN IF NOT EXISTS company_id UUID NOT NULL DEFAULT gen_random_uuid();

CREATE INDEX IF NOT EXISTS _product_location_index ON inventory.stock_moves USING btree (item_id, location_id, location_dest_id, company_id, state);
CREATE UNIQUE INDEX IF NOT EXISTS idx_bins_company_id_item_id_warehouse_id ON inventory.bins USING btree (company_id, item_id, warehouse_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_delivery_note_items_company_id ON inventory.delivery_note_items USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_delivery_notes_company_id_customer_id_status ON inventory.delivery_notes USING btree (company_id, customer_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_inventory_company_settings_company_id ON inventory.inventory_company_settings USING btree (company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE UNIQUE INDEX IF NOT EXISTS idx_landed_costs_company_id_lc_number ON inventory.landed_costs USING btree (company_id, lc_number) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE UNIQUE INDEX IF NOT EXISTS idx_locations_barcode_company_id ON inventory.locations USING btree (barcode, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_locations_company_id ON inventory.locations USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_lots_company_id ON inventory.lots USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_lots_name_item_id_company_id ON inventory.lots USING btree (name, item_id, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_operation_types_company_id ON inventory.operation_types USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_package_types_company_id ON inventory.package_types USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_packages_company_id ON inventory.packages USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_picking_batches_company_id ON inventory.picking_batches USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_picking_batches_name_company_id ON inventory.picking_batches USING btree (name, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_purchase_receipt_items_company_id ON inventory.purchase_receipt_items USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_purchase_receipts_company_id_supplier_id_status ON inventory.purchase_receipts USING btree (company_id, supplier_id, status);
CREATE INDEX IF NOT EXISTS idx_putaway_rules_company_id ON inventory.putaway_rules USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_reordering_rules_company_id ON inventory.reordering_rules USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reordering_rules_item_id_location_id_company_id ON inventory.reordering_rules USING btree (item_id, location_id, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_route_rules_company_id ON inventory.route_rules USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_routes_company_id ON inventory.routes USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_scrap_reason_tags_company_id ON inventory.scrap_reason_tags USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scrap_reason_tags_name ON inventory.scrap_reason_tags USING btree (name) WHERE (((metadata ->> 'deleted_at'::text) IS NULL) AND (company_id IS NULL));
CREATE UNIQUE INDEX IF NOT EXISTS idx_scrap_reason_tags_name_company_id ON inventory.scrap_reason_tags USING btree (name, company_id) WHERE (((metadata ->> 'deleted_at'::text) IS NULL) AND (company_id IS NOT NULL));
CREATE INDEX IF NOT EXISTS idx_scraps_company_id ON inventory.scraps USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scraps_name_company_id ON inventory.scraps USING btree (name, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_stock_entries_company_id_stock_entry_type_status ON inventory.stock_entries USING btree (company_id, stock_entry_type, status);
CREATE INDEX IF NOT EXISTS idx_stock_entry_items_company_id ON inventory.stock_entry_items USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_stock_items_company_id_item_id ON inventory.stock_items USING btree (company_id, item_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE INDEX IF NOT EXISTS idx_stock_ledger_entries_company_id_item_id_warehouse_id_postin ON inventory.stock_ledger_entries USING btree (company_id, item_id, warehouse_id, posting_date);
CREATE INDEX IF NOT EXISTS idx_stock_move_lines_company_id ON inventory.stock_move_lines USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_stock_moves_company_id ON inventory.stock_moves USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_stock_quants_company_id ON inventory.stock_quants USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_stock_reconciliation_items_company_id ON inventory.stock_reconciliation_items USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_stock_reconciliations_company_id_status ON inventory.stock_reconciliations USING btree (company_id, status);
CREATE INDEX IF NOT EXISTS idx_storage_categories_company_id ON inventory.storage_categories USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_storage_category_capacities_company_id ON inventory.storage_category_capacities USING btree (company_id);
CREATE INDEX IF NOT EXISTS idx_transfers_company_id ON inventory.transfers USING btree (company_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_transfers_name_company_id ON inventory.transfers USING btree (name, company_id) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE UNIQUE INDEX IF NOT EXISTS idx_warehouses_company_id_code ON inventory.warehouses USING btree (company_id, code) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);
CREATE UNIQUE INDEX IF NOT EXISTS idx_warehouses_company_id_name ON inventory.warehouses USING btree (company_id, name) WHERE ((metadata ->> 'deleted_at'::text) IS NULL);

-- Strict tables: the one-company predicate.
DROP POLICY IF EXISTS warehouses_company_isolation                ON inventory.warehouses;
CREATE POLICY warehouses_company_isolation ON inventory.warehouses
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_items_company_isolation              ON inventory.stock_items;
CREATE POLICY stock_items_company_isolation ON inventory.stock_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS transfers_company_isolation                ON inventory.transfers;
CREATE POLICY transfers_company_isolation ON inventory.transfers
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS reordering_rules_company_isolation         ON inventory.reordering_rules;
CREATE POLICY reordering_rules_company_isolation ON inventory.reordering_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_moves_company_isolation              ON inventory.stock_moves;
CREATE POLICY stock_moves_company_isolation ON inventory.stock_moves
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_move_lines_company_isolation         ON inventory.stock_move_lines;
CREATE POLICY stock_move_lines_company_isolation ON inventory.stock_move_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_quants_company_isolation             ON inventory.stock_quants;
CREATE POLICY stock_quants_company_isolation ON inventory.stock_quants
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_ledger_entries_company_isolation     ON inventory.stock_ledger_entries;
CREATE POLICY stock_ledger_entries_company_isolation ON inventory.stock_ledger_entries
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS bins_company_isolation                     ON inventory.bins;
CREATE POLICY bins_company_isolation ON inventory.bins
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_entries_company_isolation            ON inventory.stock_entries;
CREATE POLICY stock_entries_company_isolation ON inventory.stock_entries
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_entry_items_company_isolation        ON inventory.stock_entry_items;
CREATE POLICY stock_entry_items_company_isolation ON inventory.stock_entry_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS purchase_receipts_company_isolation        ON inventory.purchase_receipts;
CREATE POLICY purchase_receipts_company_isolation ON inventory.purchase_receipts
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS purchase_receipt_items_company_isolation   ON inventory.purchase_receipt_items;
CREATE POLICY purchase_receipt_items_company_isolation ON inventory.purchase_receipt_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS delivery_notes_company_isolation           ON inventory.delivery_notes;
CREATE POLICY delivery_notes_company_isolation ON inventory.delivery_notes
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS delivery_note_items_company_isolation      ON inventory.delivery_note_items;
CREATE POLICY delivery_note_items_company_isolation ON inventory.delivery_note_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_reconciliations_company_isolation    ON inventory.stock_reconciliations;
CREATE POLICY stock_reconciliations_company_isolation ON inventory.stock_reconciliations
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items;
CREATE POLICY stock_reconciliation_items_company_isolation ON inventory.stock_reconciliation_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS inventory_company_settings_company_isolation ON inventory.inventory_company_settings;
CREATE POLICY inventory_company_settings_company_isolation ON inventory.inventory_company_settings
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS landed_costs_company_isolation             ON inventory.landed_costs;
CREATE POLICY landed_costs_company_isolation ON inventory.landed_costs
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS landed_cost_lines_company_isolation        ON inventory.landed_cost_lines;
CREATE POLICY landed_cost_lines_company_isolation ON inventory.landed_cost_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines;
CREATE POLICY landed_cost_adjustment_lines_company_isolation ON inventory.landed_cost_adjustment_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS picking_batches_company_isolation          ON inventory.picking_batches;
CREATE POLICY picking_batches_company_isolation ON inventory.picking_batches
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

DROP POLICY IF EXISTS scraps_company_isolation                   ON inventory.scraps;
CREATE POLICY scraps_company_isolation ON inventory.scraps
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Shared masters: the shared_blank shape (NULL = the one shared set).
DROP POLICY IF EXISTS locations_company_isolation                ON inventory.locations;
CREATE POLICY locations_company_isolation ON inventory.locations
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS lots_company_isolation                     ON inventory.lots;
CREATE POLICY lots_company_isolation ON inventory.lots
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS packages_company_isolation                 ON inventory.packages;
CREATE POLICY packages_company_isolation ON inventory.packages
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS operation_types_company_isolation          ON inventory.operation_types;
CREATE POLICY operation_types_company_isolation ON inventory.operation_types
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS routes_company_isolation                   ON inventory.routes;
CREATE POLICY routes_company_isolation ON inventory.routes
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS route_rules_company_isolation              ON inventory.route_rules;
CREATE POLICY route_rules_company_isolation ON inventory.route_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS scrap_reason_tags_company_isolation        ON inventory.scrap_reason_tags;
CREATE POLICY scrap_reason_tags_company_isolation ON inventory.scrap_reason_tags
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS package_types_company_isolation            ON inventory.package_types;
CREATE POLICY package_types_company_isolation ON inventory.package_types
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS storage_categories_company_isolation       ON inventory.storage_categories;
CREATE POLICY storage_categories_company_isolation ON inventory.storage_categories
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS storage_category_capacities_company_isolation ON inventory.storage_category_capacities;
CREATE POLICY storage_category_capacities_company_isolation ON inventory.storage_category_capacities
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS putaway_rules_company_isolation            ON inventory.putaway_rules;
CREATE POLICY putaway_rules_company_isolation ON inventory.putaway_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

-- Restore the two guard triggers the strip retired (their bodies read the
-- re-added company columns). The batch-member guard comes back whole; the rule
-- guard comes back with its full original body (R11 company-consistency arms
-- included). On a database whose rows kept only their org anchor these can
-- misfire on new writes — the honest rollback runs on a pre-strip backup.
CREATE OR REPLACE FUNCTION inventory.batch_member_company_guard() RETURNS trigger AS $$
DECLARE
    batch_company UUID;
BEGIN
    IF NEW.batch_id IS NULL THEN
        RETURN NEW;
    END IF;
    SELECT company_id INTO batch_company FROM inventory.picking_batches
     WHERE id = NEW.batch_id;
    IF batch_company IS NULL OR batch_company <> NEW.company_id THEN
        RAISE EXCEPTION 'batch_member_company_mismatch: picking company % cannot join batch % (company %)',
            NEW.company_id, NEW.batch_id, batch_company;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS batch_member_company_guard ON inventory.transfers;
CREATE TRIGGER batch_member_company_guard
    BEFORE INSERT OR UPDATE OF batch_id, company_id
    ON inventory.transfers
    FOR EACH ROW EXECUTE FUNCTION inventory.batch_member_company_guard();

CREATE OR REPLACE FUNCTION inventory.route_rule_company_consistency() RETURNS trigger AS $$
DECLARE
    pt_company UUID;
    wh_company UUID;
    dest_usage TEXT;
BEGIN
    SELECT company_id INTO pt_company FROM inventory.operation_types
     WHERE id = NEW.picking_type_id;
    SELECT company_id INTO wh_company FROM inventory.warehouses
     WHERE id = NEW.warehouse_id;
    SELECT usage::text INTO dest_usage FROM inventory.locations
     WHERE id = NEW.location_dest_id;

    IF NEW.company_id IS NOT NULL THEN
        IF pt_company IS NOT NULL AND pt_company <> NEW.company_id THEN
            RAISE EXCEPTION 'rule_company_mismatch: rule company % differs from operation type company %', NEW.company_id, pt_company;
        END IF;
        IF wh_company IS NOT NULL AND wh_company <> NEW.company_id THEN
            RAISE EXCEPTION 'rule_company_mismatch: rule company % differs from warehouse company %', NEW.company_id, wh_company;
        END IF;
    END IF;

    IF pt_company IS NOT NULL AND wh_company IS NOT NULL AND pt_company <> wh_company THEN
        RAISE EXCEPTION 'rule_company_mismatch: operation type company % differs from warehouse company %', pt_company, wh_company;
    END IF;

    IF dest_usage = 'view' THEN
        RAISE EXCEPTION 'rule_dest_is_view: rule destination % is a view location', NEW.location_dest_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS route_rule_company_consistency ON inventory.route_rules;
CREATE TRIGGER route_rule_company_consistency
    BEFORE INSERT OR UPDATE OF company_id, picking_type_id, warehouse_id, location_dest_id
    ON inventory.route_rules
    FOR EACH ROW EXECUTE FUNCTION inventory.route_rule_company_consistency();
