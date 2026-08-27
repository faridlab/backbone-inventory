-- Down: drop the batch membership surface in reverse order.
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS scraps_positive_qty;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_move_id;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_picking_id;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_package_id;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_lot_id;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_scrap_location_id;
ALTER TABLE inventory.scraps DROP CONSTRAINT IF EXISTS fk_scraps_location_id;
DROP POLICY IF EXISTS scraps_company_isolation ON inventory.scraps;
ALTER TABLE inventory.scraps DISABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS picking_batches_company_isolation ON inventory.picking_batches;
ALTER TABLE inventory.picking_batches DISABLE ROW LEVEL SECURITY;
DROP TRIGGER IF EXISTS batch_member_company_guard ON inventory.transfers;
DROP FUNCTION IF EXISTS inventory.batch_member_company_guard();
ALTER TABLE inventory.transfers DROP CONSTRAINT IF EXISTS fk_transfers_batch_id;
DROP INDEX IF EXISTS idx_transfers_batch_id;
ALTER TABLE inventory.transfers DROP COLUMN IF EXISTS batch_id;
