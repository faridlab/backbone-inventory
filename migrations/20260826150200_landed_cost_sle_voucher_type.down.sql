-- PostgreSQL cannot remove a member from an enum type. Rolling this migration back is a no-op:
-- no code path mints 'landed_cost' SLE rows anymore once the landed-cost surface is removed,
-- and existing rows (if any) keep their label as historical data. Dropping and recreating the
-- type without the member would rewrite every historical ledger row — not worth it for a
-- vocabulary rollback.

SELECT 1;
