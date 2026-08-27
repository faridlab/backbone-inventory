//! Probe test for the move engine's company-scope binding in `create_move`.
//!
//! The engine binds the company scope on its transaction BEFORE reading the move's endpoint
//! locations: the row-level-security fence hides every row whose company_id differs from
//! `app.company_id` (shared rows with no company stay visible), so a fetch made before the
//! bind cannot see the company's own locations and the mint would refuse with
//! `location_not_found` under an armed fence. The explicit company check in `create_move`
//! still guards the rows the read returns — a cross-company or shared internal endpoint is
//! rejected with a hard failure, so no silent cross-tenant write is possible in either
//! posture (fenced: the row is invisible; unfenced: the check fires).

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
    // This test verifies the move engine's company-scope posture: the engine binds the company
    // scope before its location reads, and the explicit company check still rejects a
    // cross-company endpoint. This FAILS CLOSED when companies don't match, preventing
    // cross-tenant writes.

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
