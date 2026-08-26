-- Explicit total value a move carries (a document reversal valued at its original
-- amount). Nullable: ordinary moves leave it NULL and are valued by the moving-average
-- core (price/average-derived); a reversal move sets it so the reversed estate returns
-- to exactly its pre-movement state.
ALTER TABLE inventory.stock_moves ADD COLUMN IF NOT EXISTS forced_value NUMERIC(18,2);
