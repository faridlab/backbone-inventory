-- R11 rule-company consistency: resolve the warehouse's company through its org-tree node.
--
-- The warehouses re-key to org_unit_id (ADR-0028) dropped warehouses.company_id, which
-- this trigger still read — every route_rules write then failed with
-- "column company_id does not exist" inside the trigger. The warehouse's company is now
-- the nearest company-kind node at or above its org_unit_id (a company node is itself; a
-- branch node walks up to its company). The org spine seeded companies as nodes
-- preserving their ids, so that node id is the same value every legacy company_id
-- reference carries — the R11 comparisons keep their meaning, only the read changes.

CREATE OR REPLACE FUNCTION inventory.route_rule_company_consistency() RETURNS trigger AS $$
DECLARE
    pt_company UUID;
    wh_company UUID;
    dest_usage TEXT;
BEGIN
    SELECT company_id INTO pt_company FROM inventory.operation_types
     WHERE id = NEW.picking_type_id;
    SELECT cw.id INTO wh_company
      FROM inventory.warehouses w
      JOIN LATERAL (
          WITH RECURSIVE up AS (
              SELECT u.id, u.parent_id, u.kind::text AS kind
              FROM organization.org_units u
              WHERE u.id = w.org_unit_id
              UNION ALL
              SELECT u.id, u.parent_id, u.kind::text AS kind
              FROM organization.org_units u
              JOIN up ON u.id = up.parent_id
          )
          SELECT id FROM up WHERE kind = 'company' LIMIT 1
      ) cw ON true
     WHERE w.id = NEW.warehouse_id;
    SELECT usage::text INTO dest_usage FROM inventory.locations
     WHERE id = NEW.location_dest_id;

    IF NEW.company_id IS NOT NULL THEN
        IF pt_company IS NOT NULL AND pt_company <> NEW.company_id THEN
            RAISE EXCEPTION 'rule_company_mismatch: rule company % differs from operation type company %', NEW.company_id, pt_company;
        END IF;
        IF wh_company IS NOT NULL AND wh_company <> NEW.company_id THEN
            RAISE EXCEPTION 'rule_company_mismatch: rule company % differs from warehouse company %', NEW.company_id, wh_company;
        END IF;
    END IF;

    IF pt_company IS NOT NULL AND wh_company IS NOT NULL AND pt_company <> wh_company THEN
        RAISE EXCEPTION 'rule_company_mismatch: operation type company % differs from warehouse company %', pt_company, wh_company;
    END IF;

    IF dest_usage = 'view' THEN
        RAISE EXCEPTION 'rule_dest_is_view: rule destination % is a view location', NEW.location_dest_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
