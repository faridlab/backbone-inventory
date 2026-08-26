-- Drop the R11 + R13-rule DB backstop on route rules.
DROP TRIGGER IF EXISTS route_rule_company_consistency ON inventory.route_rules;
DROP FUNCTION IF EXISTS inventory.route_rule_company_consistency();
