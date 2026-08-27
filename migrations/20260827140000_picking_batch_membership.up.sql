-- Picking-batch membership + fences (the stock.picking.batch satellite, SB-1).
-- Hand-authored: the generator does not rewrite existing table migrations, so the
-- membership column the schema declares on Transfer (schema/models/operation.model.yaml,
-- transfers.batch_id) lands here, together with the strict company fence for the two
-- transaction tables and the DB backstop of the batch-member company guard.
--
-- SB-1 (ADR-0016): picking_batches.state is a STORED COMPUTE over its member pickings.
-- Nothing in the DDL writes it — the only writers are the batch recompute in
-- src/infrastructure/persistence/picking_batch_projection_repository.rs (fired by the
-- membership verbs) and the picking-projection cascade in stock_move_repository.rs (fired
-- on every member-state change). This migration only carries the projection's INPUTS.

-- Membership: one batch per picking (Odoo's to-batch domain allows at most one).
ALTER TABLE inventory.transfers
    ADD COLUMN IF NOT EXISTS batch_id UUID;

-- Plain index (no WHERE) — the schema declares the batch_id index unconditionally, and
-- the schema-to-DDL drift check compares the WHERE clauses exactly.
CREATE INDEX IF NOT EXISTS idx_transfers_batch_id
    ON inventory.transfers (batch_id);

-- Intra-module real FK (the cross-module convention stays logical-FK + comment).
ALTER TABLE inventory.picking_batches
    DROP CONSTRAINT IF EXISTS fk_picking_batches_members;
ALTER TABLE inventory.transfers
    ADD CONSTRAINT fk_transfers_batch_id
    FOREIGN KEY (batch_id) REFERENCES inventory.picking_batches (id);

-- Batch-member company consistency — the DB backstop half of the `enforcement: both`
-- pair (the same posture as the R11 rule-company trigger). A picking may only join a
-- batch of its own company: the batch is strictly fenced (NOT NULL company_id) and a
-- cross-company member would leak one tenant's transfer into another tenant's wave.
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

-- Strict company fence (ADR-0008 / ADR-0014) for the batch and scrap transaction tables.
-- The generated enable_company_rls migration predates these tables and a regen does not
-- rewrite it, so the fence DDL is hand-owned here, mirroring
-- 20260821130024_company_fence_stock_convergence. NOT NULL company_id makes the IS NULL
-- arm dead — behaviorally strict.

ALTER TABLE inventory.picking_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.picking_batches FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS picking_batches_company_isolation ON inventory.picking_batches;
CREATE POLICY picking_batches_company_isolation ON inventory.picking_batches
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

ALTER TABLE inventory.scraps ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory.scraps FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS scraps_company_isolation ON inventory.scraps;
CREATE POLICY scraps_company_isolation ON inventory.scraps
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

-- Intra-module real FKs for the scrap document (generated cross-file FKs are logical;
-- these targets are all inside this module, so they become real constraints).
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_location_id
    FOREIGN KEY (location_id) REFERENCES inventory.locations (id);
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_scrap_location_id
    FOREIGN KEY (scrap_location_id) REFERENCES inventory.locations (id);
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_lot_id
    FOREIGN KEY (lot_id) REFERENCES inventory.lots (id);
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_package_id
    FOREIGN KEY (package_id) REFERENCES inventory.packages (id);
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_picking_id
    FOREIGN KEY (picking_id) REFERENCES inventory.transfers (id);
ALTER TABLE inventory.scraps
    ADD CONSTRAINT fk_scraps_move_id
    FOREIGN KEY (move_id) REFERENCES inventory.stock_moves (id);

-- Scrap positivity (spec stock-business-logic.md §8): a disposal record always carries
-- a strictly positive quantity — the DB half of the door's loud refusal.
ALTER TABLE inventory.scraps
    DROP CONSTRAINT IF EXISTS scraps_positive_qty;
ALTER TABLE inventory.scraps
    ADD CONSTRAINT scraps_positive_qty CHECK (scrap_qty > 0);
