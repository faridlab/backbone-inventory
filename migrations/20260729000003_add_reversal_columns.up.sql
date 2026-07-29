-- Migration: record the GL reversal post on a cancelled receipt/delivery (council 2026-07-29, #3).
--
-- A cancellation appends compensating SLEs (reversing the physical movement) and emits a
-- `posting_type='reversal'` GL post that references the original via `reverses_post_id`. The original
-- `journal_id`/`accounting_post_id` stay intact (the original post really happened); the reversal's
-- own ids land in these new columns so both legs are auditable and a re-drive is idempotent.

ALTER TABLE inventory.purchase_receipts
    ADD COLUMN IF NOT EXISTS reversal_journal_id UUID,
    ADD COLUMN IF NOT EXISTS reversal_accounting_post_id UUID;

ALTER TABLE inventory.delivery_notes
    ADD COLUMN IF NOT EXISTS reversal_journal_id UUID,
    ADD COLUMN IF NOT EXISTS reversal_accounting_post_id UUID;
