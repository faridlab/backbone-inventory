-- Restore the pre-org-node form of the R11 trigger: the warehouse's company read
-- straight off warehouses.company_id (the column the later re-key drops, so this down
-- only makes sense on a database where that re-key is also rolled back).

CREATE OR REPLACE FUNCTION inventory.route_rule_company_consistency() RETURNS trigger AS $$
DECLARE
    pt_company UUID;
    wh_company UUID;
    dest_usage TEXT;
BEGIN
    SELECT company_id INTO pt_company FROM inventory.operation_types
     WHERE id = NEW.picking_type_id;
    SELECT company_id INTO wh_company FROM inventory.warehouses
     WHERE id = NEW.warehouse_id;
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
