-- =============================================================================
-- Reference Data Seeds
-- =============================================================================
-- Global reference data that every company needs (shared_blank pattern):
--   1. Virtual location trees (loss/scrap, inventory adjustment, production, transit)
--   2. Global MTO route + rule (company-agnostic, activated by replenish-on-order items)
--
-- These rows carry no tenant of their own and are visible to every scope
-- (the port of Odoo's [False] escape, ADR-0014 shared_blank posture).
-- =============================================================================

-- =============================================================================
-- 1. Virtual Location Trees
-- =============================================================================
-- The root locations that every warehouse references. These are shared
-- (no tenant of their own) so every company can use them. Individual companies
-- create their own customer/supplier/internal location subtrees under these roots.

-- Virtual location roots (location_id = NULL → these are the global trees)
-- `company_id` is gone: inventory is tenant-agnostic and the organization key
-- is installed by the composing service's decorator, which fills it from the
-- session's acting unit. These five are the virtual roots every scope reads.
INSERT INTO inventory.locations (id, name, complete_name, active, usage, location_id, parent_path, warehouse_id, metadata) VALUES
-- Supplier virtual root (all vendor receipts flow from here, per Odoo)
-- The supplier root is the source for all purchase receipts
('2c72e32f-0000-0000-0000-000000000001', 'Vendors', 'Vendors', true, 'supplier', NULL, '/2c72e32f-0000-0000-0000-000000000001', NULL,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb),
-- Customer virtual root (all deliveries flow to here, per Odoo)
-- The customer root is the destination for all sales deliveries
('2c72e32f-0000-0000-0000-000000000002', 'Customers', 'Customers', true, 'customer', NULL, '/2c72e32f-0000-0000-0000-000000000002', NULL,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb),
-- Inventory loss / scrap location (the adjustment sink)
-- Quant adjustments and stock write-offs flow here
('2c72e32f-0000-0000-0000-000000000003', 'Inventory Loss', '/Inventory Loss', true, 'inventory', NULL, '/2c72e32f-0000-0000-0000-000000000003', NULL,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb),
-- Production location (virtual WIP staging)
-- Manufacturing operations use this as the staging location
('2c72e32f-0000-0000-0000-000000000004', 'Production', '/Production', true, 'production', NULL, '/2c72e32f-0000-0000-0000-000000000004', NULL,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb),
-- Transit location (inter-company holding)
-- Stock in transit between companies sits here
('2c72e32f-0000-0000-0000-000000000005', 'Transit', '/Transit', true, 'transit', NULL, '/2c72e32f-0000-0000-0000-000000000005', NULL,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb)
ON CONFLICT (id) DO NOTHING;

-- =============================================================================
-- 2. Global MTO Route + Rule
-- =============================================================================
-- The Make-To-Order route is activated per-item by the replenish-on-order flag.
-- It names no tenant (shared) and ships inactive; items that need MTO
-- activate it and reference this route in their procurement config.
--
-- The rule says: when stock is needed at a location, pull it from the supplier
-- virtual root using the incoming receipt operation type. The actual supplier
-- is resolved per-item / per-order.

-- Global MTO route (active = false — activated by items)
INSERT INTO inventory.routes (id, name, active, sequence, product_selectable, product_categ_selectable, warehouse_selectable, metadata)
VALUES
('2c72e32f-0000-0000-0000-000000000006', 'Make To Order', false, 10, true, false, false,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb)
ON CONFLICT (id) DO NOTHING;

-- MTO pull rule (shared across all companies)
-- This rule says: when stock is needed at a destination, pull from the supplier
-- virtual root. The picking_type_id will be filled in by the reference-data
-- consumer after they create their incoming operation type.
--
-- NOTE: The picking_type_id reference is intentionally set to a placeholder UUID.
-- Real deployments should update this to point to the incoming receipt operation
-- type that gets created when a company creates its first warehouse.
-- The location_dest_id is likewise scaffolding finalized at activation: it points
-- at the shared Customers root (…0002), the classic make-to-order pull shape
-- (supplier root → customer root). Rule matching INNER-JOINS locations on this
-- column and requires demand to fall under the destination's subtree, so a
-- deployment whose demand locations do not hang under the Customers root must
-- repoint this rule before activating the route — the seed alone does not make
-- MTO cover company-owned locations.
INSERT INTO inventory.route_rules (id, name, active, sequence, action, auto, procure_method, delay, location_src_id, location_dest_id, picking_type_id, route_id, warehouse_id, propagate_cancel, metadata)
VALUES
('2c72e32f-0000-0000-0000-000000000007', 'MTO: Pull from Suppliers', true, 20, 'pull', 'manual', 'make_to_order', 0, '2c72e32f-0000-0000-0000-000000000001', '2c72e32f-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000000', '2c72e32f-0000-0000-0000-000000000006', NULL, false,
 '{"created_at":null,"updated_at":null,"deleted_at":null,"created_by":null,"updated_by":null,"deleted_by":null}'::jsonb)
ON CONFLICT (id) DO NOTHING;
