//! Shared org-spine fixtures for the database-bound test suites (ADR-0028).
//!
//! Since the warehouse re-key, `inventory.warehouses.org_unit_id` carries a write-path kind
//! guard that rejects any id not pointing at a `company`/`branch` node in
//! `organization.org_units` — a minted uuid no longer passes, on any role (the guard is a
//! trigger; it fires for superusers too, RLS or not). Every suite that creates a warehouse
//! therefore needs REAL org nodes, which means the test database needs the organization
//! schema and a spine — the composed-tenant posture of ADR-0027, where every module schema
//! lives in the tenant's own database.
//!
//! This module provides that, self-healing, for suites connected as the schema owner:
//!
//! - [`ensure_org_spine`] applies the sibling `backbone-organization` up-migrations when
//!   `organization.org_units` does not exist yet (its DDL is idempotent — `IF NOT EXISTS`
//!   throughout), then seeds the four-node shape the fence probes read — root + company +
//!   branch under it + a second company — when that shape is absent. Seeding races between
//!   parallel test threads are serialized by an advisory lock.
//! - [`fresh_company`] inserts one new `company` node under the root and returns its id:
//!   the drop-in replacement for the `Uuid::new_v4()` each test used to mint, preserving
//!   the per-test company isolation the suites were written against.
//! - [`read_org_spine`] is the read-only variant for suites connected as a restricted
//!   role (they cannot seed): it returns `None` when the spine shape is absent so the
//!   caller can skip with a printed reason, exactly like `tests/org_fence_probes.rs`.
//!
//! The org migrations are read from the sibling checkout (`../backbone-organization`)
//! rather than re-declared here, so the test database always gets the REAL upstream DDL —
//! no drift between this helper and the organization module.

// Each suite includes this module in its own test crate and uses only part of the surface
// (owner suites use `fresh_company`, restricted-role suites use `read_org_spine`), so
// unused items here are expected per crate, not a smell.
#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

/// The four-node org-tree shape the fixtures read: one tenant root, a company with a
/// branch under it, and a second (sibling) company to prove isolation.
pub struct Spine {
    pub root: Uuid,
    pub company: Uuid,
    pub branch: Uuid,
    pub company_b: Uuid,
}

/// Advisory-lock key — one fixed constant so every connection seeding the spine
/// serializes on the same lock, whatever database it lands on.
const SPINE_LOCK_KEY: i64 = 0x494E_564F_5247_3131;

/// Read the spine shape without writing anything. `None` when the organization schema is
/// absent, has no spine, or no company-with-branch pair exists — callers connected as a
/// restricted role skip on `None` (they cannot seed).
pub async fn read_org_spine(pool: &PgPool) -> Option<Spine> {
    let root: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM organization.org_units WHERE kind::text = 'root' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    // A company that has a branch child.
    let pair: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT c.id, b.id FROM organization.org_units c \
         JOIN organization.org_units b ON b.parent_id = c.id AND b.kind::text = 'branch' \
         WHERE c.kind::text = 'company' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let (company, branch) = pair?;
    // Any OTHER company node — must differ from the first.
    let company_b: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM organization.org_units \
         WHERE kind::text = 'company' AND id <> $1 LIMIT 1",
    )
    .bind(company)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    Some(Spine { root: root?, company, branch, company_b: company_b? })
}

/// Make sure the test database carries the organization schema and the four-node spine,
/// then return it. Safe to call from every test in parallel: idempotent, and the seeding
/// path holds an advisory lock so concurrent callers queue instead of racing the partial
/// unique index on the root node.
///
/// Requires the privileges of the schema owner (it applies migrations and inserts org
/// nodes); restricted-role callers use [`read_org_spine`] instead.
pub async fn ensure_org_spine(pool: &PgPool) -> Spine {
    ensure_org_schema(pool).await
        .expect("apply the backbone-organization migrations on the test database");

    let mut tx = pool
        .begin()
        .await
        .expect("begin the spine-seeding transaction");
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SPINE_LOCK_KEY)
        .execute(&mut *tx)
        .await
        .expect("take the spine-seeding advisory lock");

    // Root — exactly one per database (partial unique index).
    let root: Uuid = match sqlx::query_scalar(
        "SELECT id FROM organization.org_units WHERE kind::text = 'root' LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .expect("read the spine root")
    {
        Some(id) => id,
        None => insert_node(&mut tx, "root", None, "TEST-ROOT", "Test tenant root").await,
    };

    // A company with a branch under it, plus a second company for isolation probes.
    let pair: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT c.id, b.id FROM organization.org_units c \
         JOIN organization.org_units b ON b.parent_id = c.id AND b.kind::text = 'branch' \
         WHERE c.kind::text = 'company' LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .expect("read the company+branch pair");
    let (company, branch) = match pair {
        Some((c, b)) => (c, b),
        None => {
            let company = insert_node(&mut tx, "company", Some(root), "TEST-CO", "Test company").await;
            let branch =
                insert_node(&mut tx, "branch", Some(company), "TEST-BR", "Test branch").await;
            (company, branch)
        }
    };
    let company_b: Uuid = match sqlx::query_scalar(
        "SELECT id FROM organization.org_units \
         WHERE kind::text = 'company' AND id <> $1 LIMIT 1",
    )
    .bind(company)
    .fetch_optional(&mut *tx)
    .await
    .expect("read the second company")
    {
        Some(id) => id,
        None => {
            insert_node(&mut tx, "company", Some(root), "TEST-CO2", "Second test company").await
        }
    };

    tx.commit().await.expect("commit the spine seed");
    Spine { root, company, branch, company_b }
}

/// Insert one fresh `company` node under the tenant root and return its id — the
/// replacement for the per-test `Uuid::new_v4()` company key, now that warehouse writes
/// must reference a real node. Each call is a distinct company, so tests keep the
/// per-company isolation they were written against.
pub async fn fresh_company(pool: &PgPool) -> Uuid {
    ensure_org_spine(pool).await;
    let root: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization.org_units WHERE kind::text = 'root' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("read the spine root");
    insert_node_pool(pool, "company", Some(root), &node_code("CO"), "Fresh test company").await
}

/// Apply the sibling `backbone-organization` up-migrations when `organization.org_units`
/// is missing. Does nothing when the table exists (the DDL is idempotent, but re-running
/// the whole chain buys nothing and costs a round of parse errors on re-created triggers).
async fn ensure_org_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT to_regclass('organization.org_units')::text",
    )
    .fetch_optional(pool)
    .await?;
    if exists.is_some() {
        return Ok(());
    }

    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../backbone-organization/migrations");
    let mut up_files: Vec<std::path::PathBuf> = std::fs::read_dir(&migrations_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().ends_with(".up.sql"))
                .unwrap_or(false)
        })
        .collect();
    up_files.sort();

    for path in up_files {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(pool).await?;
    }
    Ok(())
}

/// Insert an org node inside the seeding transaction and return its id.
///
/// The kind rides as an SQL literal, not a bound parameter: `kind` is a fixed vocabulary
/// (root/company/branch — asserted below, so there is no injection surface), and a literal
/// coerces to whatever the column's type is. Test databases carry both spellings — the
/// organization module's enum-typed `org_unit_kind`, and older seeded spines where `kind`
/// is plain text — and a bound text parameter casts to neither portably.
async fn insert_node(
    tx: &mut sqlx::PgConnection,
    kind: &str,
    parent: Option<Uuid>,
    code: &str,
    name: &str,
) -> Uuid {
    assert!(matches!(kind, "root" | "company" | "branch"), "fixed node vocabulary");
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO organization.org_units (id, kind, parent_id, code, name) \
         VALUES ($1, '{kind}', $2, $3, $4)"
    );
    sqlx::query(&sql)
        .bind(id)
        .bind(parent)
        .bind(code)
        .bind(name)
        .execute(&mut *tx)
        .await
        .unwrap_or_else(|e| panic!("insert the {kind} org node: {e}"));
    id
}

/// Insert an org node directly on the pool (no explicit transaction) and return its id.
/// See [`insert_node`] for why the kind is an inlined literal.
async fn insert_node_pool(
    pool: &PgPool,
    kind: &str,
    parent: Option<Uuid>,
    code: &str,
    name: &str,
) -> Uuid {
    assert!(matches!(kind, "root" | "company" | "branch"), "fixed node vocabulary");
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO organization.org_units (id, kind, parent_id, code, name) \
         VALUES ($1, '{kind}', $2, $3, $4)"
    );
    sqlx::query(&sql)
        .bind(id)
        .bind(parent)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("insert the {kind} org node: {e}"));
    id
}

/// A short unique node code — org_units has no uniqueness on code, but readable codes
/// make a stray fixture row identifiable in the database after a run.
fn node_code(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8])
}
