-- Reverse: restore the `fifo` variant to `valuation_method`.
--
-- Recreates the two-variant type (moving_average, fifo) and re-casts the column back. Existing rows
-- are unaffected (they are all `moving_average` after the up migration); no row is set to `fifo`.

ALTER TYPE valuation_method RENAME TO valuation_method_new;

CREATE TYPE valuation_method AS ENUM ('moving_average', 'fifo');

ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method DROP DEFAULT;
ALTER TABLE inventory.stock_items
    ALTER COLUMN valuation_method TYPE valuation_method
    USING (valuation_method_new::text)::valuation_method;
ALTER TABLE inventory.stock_items ALTER COLUMN valuation_method SET DEFAULT 'moving_average';

DROP TYPE valuation_method_new;
