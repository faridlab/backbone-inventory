-- Down: drop inventory.scrap_reason_tags table
DROP TABLE IF EXISTS inventory.scrap_reason_tags CASCADE;
DROP FUNCTION IF EXISTS inventory.scrap_reason_tags_audit_timestamp() CASCADE;
