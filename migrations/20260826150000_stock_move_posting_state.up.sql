-- GL settlement state on stock moves: not_applicable / pending / posted / failed.
--
-- A move whose done-time AccountingPost was built (the caller's directive supplied
-- accounts) now records the post outcome on the move itself. The physical movement
-- (quant flips, Bin reblende, SLE rows) still commits first and is never rolled back
-- on a GL rejection — the rejection marks the move failed instead of surfacing as an
-- error with no durable trace, and a service-layer repost verb re-drives failed and
-- pending legs. Moves that post no GL of their own (voucher-door moves whose GL is
-- owned by the voucher, value-neutral warehouse-to-warehouse shapes, directives
-- without the needed accounts) stay not_applicable.
--
-- Reuses the existing gl_posting_state enum (the voucher doors' vocabulary) so both
-- posting surfaces share one state type.

ALTER TABLE inventory.stock_moves
    ADD COLUMN IF NOT EXISTS posting_state gl_posting_state NOT NULL DEFAULT 'not_applicable';

-- The pending/failed sweep (the repost worklist): finds moves whose physical leg
-- landed without a settled GL leg.
CREATE INDEX IF NOT EXISTS idx_stock_moves_posting_state
    ON inventory.stock_moves (posting_state);
