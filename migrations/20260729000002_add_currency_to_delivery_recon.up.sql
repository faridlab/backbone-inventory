-- Migration: add a `currency` column to the delivery-note and stock-reconciliation headers
-- (council 2026-07-29, finding #5).
--
-- `purchase_receipts` already carries `currency` (default 'IDR'); `delivery_notes` and
-- `stock_reconciliations` did not, so their GL posts hardcoded 'IDR' at the wire — a non-IDR
-- company would post mislabeled currency. This adds the column (default 'IDR' to match the
-- existing single-currency behavior) so the write service can thread the document's currency into
-- the AccountingPostEnvelope instead of a literal.

ALTER TABLE inventory.delivery_notes
    ADD COLUMN IF NOT EXISTS currency TEXT NOT NULL DEFAULT 'IDR';

ALTER TABLE inventory.stock_reconciliations
    ADD COLUMN IF NOT EXISTS currency TEXT NOT NULL DEFAULT 'IDR';
