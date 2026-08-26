-- R11 rule-company consistency + R13-rule destination-not-a-view — the DB backstop half of
-- the `enforcement: both` pair (ADR-0015). The service pre-check lives in
-- src/application/service/procurement_service.rs (check_rule_consistency); this trigger is
-- what a raw-SQL writer (a job, a seeder, a psql session) cannot skip.
--
-- R11: whenever the rule's company, its operation type's company, and its warehouse's company
-- are set, they must all be one company. A NULL rule company is the shared master-data
-- posture (ADR-0014 shared_blank) and agrees with anything — but the operation type and the
-- warehouse must still agree with EACH OTHER when both are set (a rule cannot bridge two
-- companies).
--
-- R13-rule: a rule's destination must never be a view location.

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

DROP TRIGGER IF EXISTS route_rule_company_consistency ON inventory.route_rules;
CREATE TRIGGER route_rule_company_consistency
    BEFORE INSERT OR UPDATE OF company_id, picking_type_id, warehouse_id, location_dest_id
    ON inventory.route_rules
    FOR EACH ROW EXECUTE FUNCTION inventory.route_rule_company_consistency();
