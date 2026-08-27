-- Packages/storage/putaway master-data guards — the SC-6 constraint family the register
-- deferred (R7/R8/R9/R10 uniqueness rides the schema-declared partial indexes the table
-- migrations already carry; this migration carries the positivity CHECKs, the mutual-
-- exclusion guard, the T12 derivation trigger, the intra-module FKs the generator does
-- not emit across files, and the shared_blank fences).
--
-- Positivity + XOR are `enforcement: both` (ADR-0015): the service layer refuses loudly,
-- and these CHECKs are what a raw-SQL writer (a job, a seeder, a psql session) cannot
-- skip — the same posture as the R11 trigger.

-- R16 — package-type dimensions/weight are non-negative when present.
ALTER TABLE inventory.package_types
    DROP CONSTRAINT IF EXISTS package_types_non_negative_dims;
ALTER TABLE inventory.package_types
    ADD CONSTRAINT package_types_non_negative_dims CHECK (
        (length   IS NULL OR length   >= 0)
    AND (width    IS NULL OR width    >= 0)
    AND (height   IS NULL OR height   >= 0)
    AND (max_weight IS NULL OR max_weight >= 0)
    );

-- R17 — storage-category max weight is non-negative when present.
ALTER TABLE inventory.storage_categories
    DROP CONSTRAINT IF EXISTS storage_categories_non_negative_weight;
ALTER TABLE inventory.storage_categories
    ADD CONSTRAINT storage_categories_non_negative_weight CHECK (
        max_weight IS NULL OR max_weight >= 0
    );

-- R15 — a capacity row carries a strictly positive quantity.
ALTER TABLE inventory.storage_category_capacities
    DROP CONSTRAINT IF EXISTS storage_capacity_positive_quantity;
ALTER TABLE inventory.storage_category_capacities
    ADD CONSTRAINT storage_capacity_positive_quantity CHECK (quantity > 0);

-- The capacity row's two targets are mutually exclusive (an item row OR a package-type
-- row — the Odoo shape: product_capacity_ids / package_capacity_ids are disjoint).
ALTER TABLE inventory.storage_category_capacities
    DROP CONSTRAINT IF EXISTS storage_capacity_target_xor;
ALTER TABLE inventory.storage_category_capacities
    ADD CONSTRAINT storage_capacity_target_xor CHECK (
        (item_id IS NOT NULL)::int + (package_type_id IS NOT NULL)::int = 1
    );

-- Intra-module real FKs the generator does not emit across schema files. The two columns
-- themselves are schema additions to EXISTING tables (packages.package_type_id,
-- locations.storage_category_id — tracking.model.yaml / location.model.yaml): the
-- generator never rewrites an existing table migration, so the ALTERs live here.
ALTER TABLE inventory.packages
    ADD COLUMN IF NOT EXISTS package_type_id UUID;
CREATE INDEX IF NOT EXISTS idx_packages_package_type_id ON inventory.packages (package_type_id);
ALTER TABLE inventory.locations
    ADD COLUMN IF NOT EXISTS storage_category_id UUID;
CREATE INDEX IF NOT EXISTS idx_locations_storage_category_id ON inventory.locations (storage_category_id);

ALTER TABLE inventory.packages
    ADD CONSTRAINT fk_packages_package_type_id
    FOREIGN KEY (package_type_id) REFERENCES inventory.package_types (id);
ALTER TABLE inventory.locations
    ADD CONSTRAINT fk_locations_storage_category_id
    FOREIGN KEY (storage_category_id) REFERENCES inventory.storage_categories (id);
ALTER TABLE inventory.storage_category_capacities
    ADD CONSTRAINT fk_storage_capacity_category_id
    FOREIGN KEY (storage_category_id) REFERENCES inventory.storage_categories (id);
ALTER TABLE inventory.storage_category_capacities
    ADD CONSTRAINT fk_storage_capacity_package_type_id
    FOREIGN KEY (package_type_id) REFERENCES inventory.package_types (id);
ALTER TABLE inventory.putaway_rules
    ADD CONSTRAINT fk_putaway_rules_location_in_id
    FOREIGN KEY (location_in_id) REFERENCES inventory.locations (id);
ALTER TABLE inventory.putaway_rules
    ADD CONSTRAINT fk_putaway_rules_location_out_id
    FOREIGN KEY (location_out_id) REFERENCES inventory.locations (id);
ALTER TABLE inventory.putaway_rules
    ADD CONSTRAINT fk_putaway_rules_package_type_id
    FOREIGN KEY (package_type_id) REFERENCES inventory.package_types (id);
ALTER TABLE inventory.putaway_rules
    ADD CONSTRAINT fk_putaway_rules_storage_category_id
    FOREIGN KEY (storage_category_id) REFERENCES inventory.storage_categories (id);

-- T12 — putaway_rules.storage_category_id is a STORED derived copy of the destination
-- location's category (spec stock-business-logic.md §12 T12). The derivation runs at
-- write time on every destination change and is raw-SQL-writer-proof: whatever a writer
-- claims the column holds, the trigger re-derives it from location_out_id. This is the
-- R27 write-time-derivation posture (never a free column).
CREATE OR REPLACE FUNCTION inventory.putaway_rule_derive_storage_category() RETURNS trigger AS $$
BEGIN
    -- PL/pgSQL does not guarantee short-circuit evaluation, so the INSERT and UPDATE
    -- arms are split: only the UPDATE arm may reference OLD.
    IF TG_OP = 'INSERT' THEN
        NEW.storage_category_id := (
            SELECT storage_category_id FROM inventory.locations WHERE id = NEW.location_out_id
        );
    ELSIF NEW.location_out_id IS DISTINCT FROM OLD.location_out_id THEN
        NEW.storage_category_id := (
            SELECT storage_category_id FROM inventory.locations WHERE id = NEW.location_out_id
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS putaway_rule_derive_storage_category ON inventory.putaway_rules;
CREATE TRIGGER putaway_rule_derive_storage_category
    BEFORE INSERT OR UPDATE OF location_out_id
    ON inventory.putaway_rules
    FOR EACH ROW EXECUTE FUNCTION inventory.putaway_rule_derive_storage_category();

-- Shared_blank company fence (ADR-0008 / ADR-0014) for the five master-data tables:
-- package_types, storage_categories, storage_category_capacities, putaway_rules,
-- scrap_reason_tags. NULL company rows are first-class shared rows (the [False] escape);
-- non-NULL rows stay fenced. Mirrors 20260821130024_company_fence_stock_convergence.

ALTER TABLE inventory.package_types ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.package_types FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS package_types_company_isolation ON inventory.package_types;
CREATE POLICY package_types_company_isolation ON inventory.package_types
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.storage_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.storage_categories FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS storage_categories_company_isolation ON inventory.storage_categories;
CREATE POLICY storage_categories_company_isolation ON inventory.storage_categories
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.storage_category_capacities ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.storage_category_capacities FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS storage_category_capacities_company_isolation ON inventory.storage_category_capacities;
CREATE POLICY storage_category_capacities_company_isolation ON inventory.storage_category_capacities
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.putaway_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.putaway_rules FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS putaway_rules_company_isolation ON inventory.putaway_rules;
CREATE POLICY putaway_rules_company_isolation ON inventory.putaway_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.scrap_reason_tags ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.scrap_reason_tags FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS scrap_reason_tags_company_isolation ON inventory.scrap_reason_tags;
CREATE POLICY scrap_reason_tags_company_isolation ON inventory.scrap_reason_tags
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);
