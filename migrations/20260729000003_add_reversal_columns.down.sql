-- Reverse: drop the reversal-post columns from receipt/delivery headers.

ALTER TABLE inventory.delivery_notes
    DROP COLUMN IF EXISTS reversal_accounting_post_id,
    DROP COLUMN IF EXISTS reversal_journal_id;

ALTER TABLE inventory.purchase_receipts
    DROP COLUMN IF EXISTS reversal_accounting_post_id,
    DROP COLUMN IF EXISTS reversal_journal_id;
