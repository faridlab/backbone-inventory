-- Migration: drop the unimplemented `fifo` variant from `valuation_method` (council 2026-07-29).
--
-- FIFO was declared in the enum and persisted on `stock_items.valuation_method` but the valuation
-- engine is hardwired moving-average — a stock item set to `fifo` was silently valued by moving
-- average. This removes the false option. PostgreSQL has no `ALTER TYPE ... DROP VALUE`, so the type
-- is renamed, recreated with only `moving_average`, the column re-cast, and the old type dropped.
--
-- Wrapped in a DO block that is IDEMPOTENT and recovers from a partial run:
--   * if `valuation_method_old` exists, a prior run renamed+recreated but the column re-cast did not
--     finish — complete it;
--   * else if `fifo` is still a variant, do the full rename/recreate/re-cast.
-- The USING clause casts the COLUMN value (named `valuation_method`) through text — not the type —
-- because the column keeps its name while its type OID follows the rename.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_type WHERE typname = 'valuation_method_old') THEN
        -- Partial-run recovery: the column is still typed valuation_method_old; finish the re-cast.
        ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method DROP DEFAULT;
        ALTER TABLE inventory.stock_items
            ALTER COLUMN valuation_method TYPE valuation_method
            USING (valuation_method::text)::valuation_method;
        ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method SET DEFAULT 'moving_average';
        DROP TYPE valuation_method_old;
    ELSIF EXISTS (
        SELECT 1 FROM pg_enum e
        JOIN pg_type t ON e.enumtypid = t.oid
        WHERE t.typname = 'valuation_method' AND e.enumlabel = 'fifo'
    ) THEN
        -- Fresh run: normalize legacy fifo rows, rename, recreate, re-cast, drop old.
        UPDATE inventory.stock_items
           SET valuation_method = 'moving_average'
         WHERE valuation_method::text = 'fifo';

        ALTER TYPE valuation_method RENAME TO valuation_method_old;
        CREATE TYPE valuation_method AS ENUM ('moving_average');

        ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method DROP DEFAULT;
        ALTER TABLE inventory.stock_items
            ALTER COLUMN valuation_method TYPE valuation_method
            USING (valuation_method::text)::valuation_method;
        ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method SET DEFAULT 'moving_average';

        DROP TYPE valuation_method_old;
    END IF;
END
$$;
