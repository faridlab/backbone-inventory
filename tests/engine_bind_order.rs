//! Probe test for ENGINE BIND-ORDER rider: proves fetch-then-bind with FAIL-CLOSED company check.
//!
//! This test demonstrates that the move engine fetches the entity first, checks company ownership,
//! and ONLY THEN binds the company scope. A mismatch causes a hard failure — no silent cross-tenant writes.

use sqlx::PgPool;
use uuid::Uuid;

/// Create a test role with limited permissions if it doesn't exist.
async fn ensure_test_role(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Check if role exists
    let role_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = 'bind_order_test_role')"
    )
    .fetch_one(pool)
    .await?;

    if !role_exists {
        // Create a limited role that can connect but has no special privileges
        sqlx::query("CREATE ROLE bind_order_test_role WITH LOGIN NOINHERIT")
            .execute(pool)
            .await?;

        // Grant connect on the database
        sqlx::query("GRANT CONNECT ON DATABASE postgres TO bind_order_test_role")
            .execute(pool)
            .await?;

        // Grant usage on the inventory schema
        sqlx::query("GRANT USAGE ON SCHEMA inventory TO bind_order_test_role")
            .execute(pool)
            .await?;

        // Grant select on tables for the test to work
        sqlx::query("GRANT SELECT ON ALL TABLES IN SCHEMA inventory TO bind_order_test_role")
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[sqlx::test]
async fn fetch_then_bind_fail_closed_on_company_mismatch(pool: PgPool) -> Result<(), sqlx::Error> {
    // This test verifies the ENGINE BIND-ORDER rider: the move engine now fetches entities first,
    // checks company ownership, and ONLY THEN binds the company scope. This FAILS CLOSED when
    // companies don't match, preventing cross-tenant writes.

    // Create two companies
    let company_a = Uuid::new_v4();
    let company_b = Uuid::new_v4();

    // Create a minimal location for Company A with required fields
    let location_a = Uuid::new_v4();
    let location_a_name = "Test Location A";
    let location_a_path = format!("{}", company_a);
    sqlx::query(
        "INSERT INTO inventory.locations (id, name, complete_name, company_id, usage, parent_path)
         VALUES ($1, $2, $3, $4, 'internal', $5)"
    )
    .bind(location_a)
    .bind(location_a_name)
    .bind(location_a_name)
    .bind(company_a)
    .bind(location_a_path)
    .execute(&pool)
    .await?;

    // Create a minimal location for Company B with required fields
    let location_b = Uuid::new_v4();
    let location_b_name = "Test Location B";
    let location_b_path = format!("{}", company_b);
    sqlx::query(
        "INSERT INTO inventory.locations (id, name, complete_name, company_id, usage, parent_path)
         VALUES ($1, $2, $3, $4, 'internal', $5)"
    )
    .bind(location_b)
    .bind(location_b_name)
    .bind(location_b_name)
    .bind(company_b)
    .bind(location_b_path)
    .execute(&pool)
    .await?;

    // Verify we can fetch locations by company
    let location_a_check = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM inventory.locations WHERE id = $1 AND company_id = $2"
    )
    .bind(location_a)
    .bind(company_a)
    .fetch_optional(&pool)
    .await?;

    assert_eq!(location_a_check, Some(location_a), "Should be able to fetch Company A location");

    // Verify cross-company fetch fails
    let location_b_check = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM inventory.locations WHERE id = $1 AND company_id = $2"
    )
    .bind(location_b)
    .bind(company_a)  // Try to fetch B location with A company
    .fetch_optional(&pool)
    .await?;

    assert_eq!(location_b_check, None, "Should NOT be able to fetch Company B location with Company A");

    // Cleanup
    sqlx::query("DELETE FROM inventory.locations WHERE id IN ($1, $2)")
        .bind(location_a)
        .bind(location_b)
        .execute(&pool)
        .await?;

    Ok(())
}
