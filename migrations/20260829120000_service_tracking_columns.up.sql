-- The service-delivery tracking policy on the product surface: what a selling-order
-- confirm mints for a service item (a task, a project, or nothing). Three columns on
-- stock_items — the ACL projection of the catalog item that every product carries,
-- including services (is_stock_item = false):
--
--   service_tracking            — the four-rung ladder. NOT NULL DEFAULT 'manual':
--                                 an existing service item mints nothing until an
--                                 operator deliberately configures a rung (the
--                                 upstream default task_global_project is NOT
--                                 inherited, so configured stock never starts
--                                 minting projects on its own).
--   service_project_id          — the fixed project anchor for task_global_project.
--                                 Logical FK to project.Project.id (cross-module by
--                                 design: no DB constraint).
--   service_project_template_id — the blueprint anchor for task_in_project /
--                                 project_only forking. Logical FK to
--                                 project.ProjectTemplate.id.
--
-- An item with no stock_items row, or rung 'manual', mints nothing (absence = manual).

-- The rung enum lands UNQUALIFIED on the search path (public), matching every other
-- enum this module creates (valuation_method, warehouse_type, doc_status, ...).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'service_tracking_type') THEN
        CREATE TYPE service_tracking_type AS ENUM ('task_global_project', 'task_in_project', 'project_only', 'manual');
    END IF;
END
$$;

ALTER TABLE inventory.stock_items
    ADD COLUMN IF NOT EXISTS service_tracking service_tracking_type NOT NULL DEFAULT 'manual',
    ADD COLUMN IF NOT EXISTS service_project_id UUID,
    ADD COLUMN IF NOT EXISTS service_project_template_id UUID;
