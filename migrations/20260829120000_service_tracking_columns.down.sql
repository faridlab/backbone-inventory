-- Reverse the service-tracking columns.

ALTER TABLE inventory.stock_items
    DROP COLUMN IF EXISTS service_project_template_id,
    DROP COLUMN IF EXISTS service_project_id,
    DROP COLUMN IF EXISTS service_tracking;

DROP TYPE IF EXISTS service_tracking_type;
