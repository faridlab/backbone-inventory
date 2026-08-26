-- Enum values cannot be dropped from a PostgreSQL type. The down-migration is a no-op:
-- reverting the policy vocabulary requires recreating the type (and every dependant
-- column), which is intentionally out of scope for a rollback.
SELECT 1;
