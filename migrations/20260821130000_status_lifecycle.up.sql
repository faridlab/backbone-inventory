-- Migration: replace the stock-ledger-entry cancellation boolean with a status enum
-- inventory.stock_ledger_entries carried `is_cancelled BOOLEAN NOT NULL DEFAULT FALSE`; the
-- tree-wide convention is one `status` enum field per lifecycle (see docs/refactoring-schema in
-- the serpa workspace). Only TRUE rows are written to 'cancelled'; rows at the column default
-- FALSE map to the enum default 'active' via the new column's DEFAULT, so no UPDATE is needed
-- for them. The enum type is created unqualified so it lands beside the module's other enum
-- types (public), where the generated sqlx type_name resolves.

DO $$ BEGIN
    CREATE TYPE stock_ledger_status AS ENUM ('active', 'cancelled');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE inventory.stock_ledger_entries ADD COLUMN status stock_ledger_status NOT NULL DEFAULT 'active';
UPDATE inventory.stock_ledger_entries SET status = 'cancelled' WHERE is_cancelled;
ALTER TABLE inventory.stock_ledger_entries DROP COLUMN is_cancelled;
