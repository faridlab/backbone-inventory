-- Down: drop inventory.inventory_company_settings table
DROP TABLE IF EXISTS inventory.inventory_company_settings CASCADE;
DROP FUNCTION IF EXISTS inventory.inventory_company_settings_audit_timestamp() CASCADE;
