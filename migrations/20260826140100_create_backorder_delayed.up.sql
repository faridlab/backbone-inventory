-- The deferred-reservation backorder policy: mint the backorder transfer on a partial
-- validate, but do not reserve it — the scheduler's assign sweep picks it up. Extends
-- the operation type's partial-validate policy vocabulary (ask / always / never).
ALTER TYPE create_backorder ADD VALUE IF NOT EXISTS 'delayed' BEFORE 'never';
