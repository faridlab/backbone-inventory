-- Warehouse names are unique per company among non-deleted rows (the name half of
-- the per-company uniqueness rule; the code half ships with the base table).
CREATE UNIQUE INDEX IF NOT EXISTS idx_warehouses_company_id_name
    ON inventory.warehouses (company_id, name)
    WHERE (metadata->>'deleted_at') IS NULL;
