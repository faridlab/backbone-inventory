-- The voucher -> minted-transfer seam (declared in schema/models/*.model.yaml as
-- `transfer_id uuid?` + `idx_*_transfer_id`): the transfer a submitted voucher mints,
-- linking each voucher header to the picking projection that groups its StockMoves.
-- NULL = not yet minted (draft). The link is a plain indexed column, not a physical
-- FK constraint — the transfer projection is derived from the moves, and a hard FK
-- would couple the voucher write path to the projection's lifecycle.
ALTER TABLE inventory.purchase_receipts     ADD COLUMN IF NOT EXISTS transfer_id UUID;
ALTER TABLE inventory.delivery_notes        ADD COLUMN IF NOT EXISTS transfer_id UUID;
ALTER TABLE inventory.stock_entries         ADD COLUMN IF NOT EXISTS transfer_id UUID;
ALTER TABLE inventory.stock_reconciliations ADD COLUMN IF NOT EXISTS transfer_id UUID;

-- Minted-transfer lookup (voucher -> picking projection), one per voucher table.
CREATE INDEX IF NOT EXISTS idx_purchase_receipts_transfer_id
    ON inventory.purchase_receipts (transfer_id);
CREATE INDEX IF NOT EXISTS idx_delivery_notes_transfer_id
    ON inventory.delivery_notes (transfer_id);
CREATE INDEX IF NOT EXISTS idx_stock_entries_transfer_id
    ON inventory.stock_entries (transfer_id);
CREATE INDEX IF NOT EXISTS idx_stock_reconciliations_transfer_id
    ON inventory.stock_reconciliations (transfer_id);
