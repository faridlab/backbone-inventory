//! Org-fence probes for warehouses (ADR-0028) — the pilot table re-keyed from `company_id` to
//! `org_unit_id`.
//!
//! These probes MUST run against a fenced database that carries BOTH the inventory migrations
//! (including the warehouse org-unit re-key: entitlement-union fence + write-path kind guard)
//! and the organization module's schema with the org spine populated — `org_units` root +
//! company nodes (+ at least one branch under a company, and a second company to prove
//! isolation). Exactly the posture a composed tenant database has (ADR-0027: every module
//! schema lives in the tenant's own database). The behavior suites run as the migration owner
//! (superuser), which BYPASSES row-level security — these probes connect as a restricted,
//! non-superuser role so the fence is armed for real.
//!
//! What is proven here, end to end through the real application stack:
//!
//! - the ORM org scope resolver (`backbone_orm::resolve_org_scope`) against the REAL spine:
//!   a branch-acting session resolves to its own subtree + the root node, with the legacy
//!   company bridge resolved from the acting node's company ancestry;
//! - the REAL `InventoryWriteService::create_warehouse` (hand-written write path) inside
//!   `with_org_request_scope`: writes under the acting company and its branch land; a write
//!   under the ROOT node or an unknown id is rejected by the kind guard;
//! - the REAL generated `WarehouseService::list` (generic CRUD read path, unchanged generated
//!   code) inside the same request scope: a company session sees its whole subtree's
//!   warehouses, a branch session only its own node's, and a sister company sees none.
//!
//! Gated on `INVENTORY_ORG_FENCE_DSN`: a DSN for the restricted role (e.g.
//! `postgresql://<role>:<pw>@127.0.0.1:5433/<db>`). The role needs USAGE on the inventory and
//! organization schemas, SELECT/INSERT/UPDATE/DELETE on the inventory tables it writes, and
//! SELECT on `organization.org_units`. Skips with a printed reason when the DSN is absent or
//! the database carries no org spine, so an unfenced dev database does not fail the run.

use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use backbone_inventory::application::service::inventory_write_service::{
    InventoryWriteService, NewWarehouse,
};
use backbone_inventory::application::service::WarehouseService;
use backbone_inventory::domain::entity::Warehouse;
use backbone_inventory::infrastructure::persistence::WarehouseRepository;

use backbone_orm::{resolve_org_scope, with_org_request_scope};

fn dsn() -> Option<String> {
    std::env::var("INVENTORY_ORG_FENCE_DSN").ok()
}

fn uq(p: &str) -> String {
    format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

/// The org-tree fixture the probes need: a company with a branch under it, plus a second
/// (sibling) company. Read from the database's real spine; `None` when the tree does not have
/// that shape (no spine, or no company-with-branch) — the probe skips then.
struct TreeFixture {
    root: Uuid,
    company_a: Uuid,
    branch_a: Uuid,
    company_b: Uuid,
}

async fn fixture(pool: &PgPool) -> Option<TreeFixture> {
    let root: Option<Uuid> =
        sqlx::query_scalar("SELECT organization.org_unit_root()").fetch_optional(pool).await.ok().flatten();
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
    let (company_a, branch_a) = pair?;
    // Any OTHER company node (sibling or not) — must differ from company_a.
    let company_b: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization.org_units WHERE kind::text = 'company' AND id <> $1 LIMIT 1")
            .bind(company_a)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    Some(TreeFixture { root: root?, company_a, branch_a, company_b: company_b? })
}

#[tokio::test]
async fn resolver_shapes_branch_scope_from_the_real_spine() {
    let Some(dsn) = dsn() else {
        eprintln!("skipping: set INVENTORY_ORG_FENCE_DSN to a restricted-role DSN on a fenced, org-spined database");
        return;
    };
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let Some(fx) = fixture(&pool).await else {
        eprintln!("skipping: database has no org spine with a company+branch+second-company shape");
        return;
    };

    let mut conn = pool.acquire().await.unwrap();
    let scope = resolve_org_scope(&mut conn, fx.branch_a, &[])
        .await
        .expect("resolve branch scope against the real spine");
    // subtree() descends only: branch scope = {branch, root}; the parent company is not
    // auto-included — it needs entitlement.
    let mut got = scope.scope_unit_ids().to_vec();
    got.sort_unstable();
    let mut want = vec![fx.branch_a, fx.root];
    want.sort_unstable();
    assert_eq!(got, want);
    assert_eq!(scope.acting_unit_id(), fx.branch_a);
    assert_eq!(scope.legacy_company_id(), Some(fx.company_a));

    // Company scope: subtree includes the branch.
    let scope_a = resolve_org_scope(&mut conn, fx.company_a, &[]).await.unwrap();
    assert!(scope_a.scope_unit_ids().contains(&fx.branch_a));
    assert!(scope_a.scope_unit_ids().contains(&fx.root));
}

#[tokio::test]
async fn warehouse_write_and_read_through_the_real_services_under_the_fence() {
    let Some(dsn) = dsn() else {
        eprintln!("skipping: set INVENTORY_ORG_FENCE_DSN to a restricted-role DSN on a fenced, org-spined database");
        return;
    };
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let Some(fx) = fixture(&pool).await else {
        eprintln!("skipping: database has no org spine with a company+branch+second-company shape");
        return;
    };

    // Company-A session: write through the hand-written service, read through the generated
    // CRUD service — both inside one request scope on the restricted role's pool.
    let mut conn = pool.acquire().await.unwrap();
    let scope_a = resolve_org_scope(&mut conn, fx.company_a, &[]).await.unwrap();
    let writer = InventoryWriteService::new(pool.clone());
    let reader = WarehouseService::with_repository(Arc::new(WarehouseRepository::new(pool.clone())));

    let (code_a, code_branch) = (uq("WH-CO"), uq("WH-BR"));
    with_org_request_scope(&pool, scope_a.clone(), async {
        let id_a = writer
            .create_warehouse(NewWarehouse {
                org_unit_id: fx.company_a,
                code: code_a.clone(),
                name: code_a.clone(),
                warehouse_type: None,
                parent_warehouse_id: None,
                is_group: false,
            })
            .await
            .expect("write under the acting company node");
        let id_branch = writer
            .create_warehouse(NewWarehouse {
                org_unit_id: fx.branch_a,
                code: code_branch.clone(),
                name: code_branch.clone(),
                warehouse_type: None,
                parent_warehouse_id: None,
                is_group: false,
            })
            .await
            .expect("write under a branch inside the session's subtree");
        let _ = (id_a, id_branch);

        // The generated read path (unchanged generic CRUD) is fenced by the same session.
        let (items, _total): (Vec<Warehouse>, u64) = reader.list(1, 100, HashMap::new()).await.expect("list warehouses");
        let codes: Vec<&str> = items.iter().map(|w| w.code.as_str()).collect();
        assert!(codes.contains(&code_a.as_str()), "company session must see its own warehouse");
        assert!(codes.contains(&code_branch.as_str()), "company session must see its branch's warehouse");
    })
    .await
    .unwrap();

    // Branch session: subtree = {branch, root} — sees the branch warehouse, not the company one.
    let scope_branch = resolve_org_scope(&mut conn, fx.branch_a, &[]).await.unwrap();
    with_org_request_scope(&pool, scope_branch, async {
        let (items, _total): (Vec<Warehouse>, u64) = reader.list(1, 100, HashMap::new()).await.expect("list warehouses");
        let codes: Vec<&str> = items.iter().map(|w| w.code.as_str()).collect();
        assert!(codes.contains(&code_branch.as_str()), "branch session must see its own warehouse");
        assert!(!codes.contains(&code_a.as_str()), "branch session must NOT see the company-hung warehouse");
    })
    .await
    .unwrap();

    // Sister company: sees neither.
    let scope_b = resolve_org_scope(&mut conn, fx.company_b, &[]).await.unwrap();
    with_org_request_scope(&pool, scope_b, async {
        let (items, _total): (Vec<Warehouse>, u64) = reader.list(1, 100, HashMap::new()).await.expect("list warehouses");
        let codes: Vec<String> = items.iter().map(|w| w.code.clone()).collect();
        assert!(!codes.contains(&code_a), "sister company must not see company A's warehouses");
        assert!(!codes.contains(&code_branch), "sister company must not see the branch warehouse");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn kind_guard_rejects_root_and_unknown_nodes_on_the_write_path() {
    let Some(dsn) = dsn() else {
        eprintln!("skipping: set INVENTORY_ORG_FENCE_DSN to a restricted-role DSN on a fenced, org-spined database");
        return;
    };
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let Some(fx) = fixture(&pool).await else {
        eprintln!("skipping: database has no org spine with a company+branch+second-company shape");
        return;
    };

    let mut conn = pool.acquire().await.unwrap();
    let scope = resolve_org_scope(&mut conn, fx.company_a, &[]).await.unwrap();
    let writer = InventoryWriteService::new(pool.clone());

    // Root is in the session's scope (shared rows) — the fence would allow the write, but the
    // kind guard must refuse: warehouses hang off company/branch nodes, never the root.
    let root_write = with_org_request_scope(&pool, scope.clone(), async {
        writer
            .create_warehouse(NewWarehouse {
                org_unit_id: fx.root,
                code: uq("WH-ROOT"),
                name: uq("Root"),
                warehouse_type: None,
                parent_warehouse_id: None,
                is_group: false,
            })
            .await
    })
    .await
    .unwrap();
    assert!(root_write.is_err(), "a warehouse under the ROOT node must be rejected by the kind guard");

    // Unknown uuid: rejected by the kind guard (not by the fence — it is out of scope, but the
    // guard fires first with the clearer error).
    let ghost = Uuid::new_v4();
    let ghost_write = with_org_request_scope(&pool, scope, async {
        writer
            .create_warehouse(NewWarehouse {
                org_unit_id: ghost,
                code: uq("WH-GHOST"),
                name: uq("Ghost"),
                warehouse_type: None,
                parent_warehouse_id: None,
                is_group: false,
            })
            .await
    })
    .await
    .unwrap();
    assert!(ghost_write.is_err(), "a warehouse under an unknown node must be rejected by the kind guard");
}
