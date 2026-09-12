//! `run_scheduler` — the module's ONE scheduled job (hand-authored, user-owned).
//!
//! Spec: docs/odoo/inventory/stock/stock-business-logic.md §7 (orderpoints + the daily
//! scheduler). The declaration of record is the `scheduled_jobs.run_scheduler` block in
//! `schema/hooks/index.hook.yaml`; this file is the handler it names. Its ADR-0020 posture:
//!
//! - **`posture: pull`** — a plain interval-driven scan; no domain event re-arms it.
//! - **`commit_policy: commit_per_batch`** — progress commits between batches AND between the
//!   three tasks, so a crash mid-run replays at most one batch next run. That is an
//!   at-least-once window (ADR-0017 vocabulary): consumers must be idempotent. Reordering is
//!   idempotent by predicate, not by hope — a claimed orderpoint is excluded while it has any
//!   open replenishment move, so a replay cannot double-order.
//! - **`pickup_lock: true`** — every intake claim reads `FOR UPDATE SKIP LOCKED`, so two
//!   concurrent scheduler replicas (or a manual overlap) take disjoint sets instead of
//!   double-firing (ADR-0020 §4, the MMB-4 class).
//!
//! The three ordered tasks:
//!
//! 1. **Reorder** (`_run_orderpoints`): claim `trigger='auto'` orderpoints, refresh the T11
//!    computes, and for every `qty_forecast < item_min_qty` mint the replenishment move of
//!    `qty_to_order` and publish `OrderpointTriggered` (after the batch commits).
//! 2. **Assign** (`_run_scheduler_assign`): claim confirmed/partially-available moves and run
//!    the move engine's assign verb on each — reserve what became available overnight.
//! 3. **Housekeep**: vacuum zero-quantity, fully-unreserved, count-free quants.
//!
//! **Plain sweep** (tenancy is composition-installed, ADR-0029): the handler never enumerates
//! tenants and never binds one itself — the composing host owns org binding, relaying the
//! ambient org scope onto the job's connections, and the decorator's fence bounds every claim
//! the sweep reads. Undecorated (module tests) the statements run plain.
//!
//! The move-lifecycle verbs arrive through the [`MovePipeline`] port (implemented by the
//! stock-move engine) — a job cannot be constructed against a silent no-op.

use std::sync::Arc;

use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::application::service::procurement_service::{
    MovePipeline, ProcurementService,
};
use crate::domain::entity::MoveState;
use crate::infrastructure::persistence::procurement_repository::ProcurementRepository;

/// Rows per commit (the `commit_per_batch` granularity).
pub const SCHEDULER_BATCH_SIZE: i64 = 50;

/// Hard stop on batches per task — a pathological tenant cannot wedge the job into an
/// unbounded loop; the next run resumes where this one stopped.
pub const MAX_BATCHES_PER_TASK: usize = 1_000;

/// One run's counters.
#[derive(Debug, Clone, Default)]
pub struct SchedulerReport {
    pub orderpoints_claimed: usize,
    pub orderpoints_triggered: usize,
    /// Reorder rungs that found `qty_forecast >= item_min_qty` (computes refreshed, no order).
    pub orderpoints_skipped: usize,
    pub replenishment_moves_minted: usize,
    pub moves_assign_attempted: usize,
    pub moves_assigned: usize,
    pub moves_partially_available: usize,
    pub assign_failures: usize,
    pub quants_vacuumed: u64,
}

/// Knobs the host may override; defaults match the declaration of record.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerBatching {
    pub batch_size: i64,
    pub max_batches: usize,
}

impl Default for SchedulerBatching {
    fn default() -> Self {
        Self { batch_size: SCHEDULER_BATCH_SIZE, max_batches: MAX_BATCHES_PER_TASK }
    }
}

/// Run the daily scheduler's one sweep. See the module docs for the posture declarations;
/// the caller passes the move engine's [`MovePipeline`] implementation.
pub async fn run_scheduler(
    pool: &PgPool,
    service: &ProcurementService,
    pipeline: Arc<dyn MovePipeline>,
) -> Result<SchedulerReport, sqlx::Error> {
    run_scheduler_with(pool, service, pipeline, SchedulerBatching::default()).await
}

/// The tunable entry (tests shrink the batch); same flow as [`run_scheduler`].
pub async fn run_scheduler_with(
    pool: &PgPool,
    service: &ProcurementService,
    pipeline: Arc<dyn MovePipeline>,
    batching: SchedulerBatching,
) -> Result<SchedulerReport, sqlx::Error> {
    let mut report = SchedulerReport::default();

    // Task 1: reorder (commit per batch; events after each batch's commit).
    //
    // The seen-set is the loop's real terminator: an orderpoint that skips (forecast at or
    // above its minimum) never mints a move, so the open-move exclusion never removes it
    // from the claim set — without this guard every batch would re-claim it up to
    // `max_batches`. A batch whose claims are ALL repeats cannot produce anything new, so
    // the run stops there.
    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for _ in 0..batching.max_batches {
        let mut tx = pool.begin().await?;
        let claimed =
            ProcurementRepository::claim_orderpoints(&mut *tx, batching.batch_size).await?;
        if claimed.is_empty() || claimed.iter().all(|op| seen.contains(&op.id)) {
            tx.rollback().await?;
            break;
        }
        seen.extend(claimed.iter().map(|op| op.id));
        report.orderpoints_claimed += claimed.len();
        let mut fired = Vec::new();
        for op in &claimed {
            match service.reorder_one(&mut tx, op).await {
                Ok(Some(outcome)) => {
                    report.orderpoints_triggered += 1;
                    report.replenishment_moves_minted += 1;
                    fired.push(outcome);
                }
                Ok(None) => report.orderpoints_skipped += 1,
                // A rule-less or compute-broken orderpoint must not starve the rest of the
                // batch: log, count, let the NEXT run retry it once its configuration is
                // repaired. The batch commits without it.
                Err(e) => warn!(
                    target: "inventory.scheduler",
                    orderpoint_id = %op.id, error = %e,
                    "orderpoint reorder rung failed; skipped this run"
                ),
            }
        }
        tx.commit().await?;
        // Events strictly after the commit — never ahead of the durable record.
        for outcome in &fired {
            service.emit_orderpoint_triggered(outcome);
        }
    }

    // Task 2: assign sweep (claim FOR UPDATE SKIP LOCKED, per-move verb, commit per batch).
    // Same seen-set guard: a pipeline verb that lands a non-reserving state (a move left
    // `waiting` behind a chain) leaves the move claimable, and only the guard stops the
    // loop from re-reading that page up to `max_batches`.
    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for _ in 0..batching.max_batches {
        let mut tx = pool.begin().await?;
        let moves =
            ProcurementRepository::claim_assignable_moves(&mut *tx, batching.batch_size).await?;
        if moves.is_empty() || moves.iter().all(|m| seen.contains(&m.id)) {
            tx.rollback().await?;
            break;
        }
        seen.extend(moves.iter().map(|m| m.id));
        report.moves_assign_attempted += moves.len();
        for m in &moves {
            match service.assign_one(&mut tx, pipeline.as_ref(), m.id).await {
                Ok(MoveState::Assigned) => report.moves_assigned += 1,
                Ok(MoveState::PartiallyAvailable) => report.moves_partially_available += 1,
                Ok(other) => {
                    // The verb landed somewhere the sweep does not count as progress
                    // (e.g. waiting after a chain release) — not a failure, but visible.
                    warn!(
                        target: "inventory.scheduler",
                        move_id = %m.id, state = %other,
                        "assign verb returned a non-reserving state"
                    );
                }
                Err(e) => {
                    report.assign_failures += 1;
                    warn!(
                        target: "inventory.scheduler",
                        move_id = %m.id, error = %e,
                        "assign verb failed; move stays for the next run"
                    );
                }
            }
        }
        tx.commit().await?;
    }

    // Task 3: quant vacuum (single batch per commit until drained).
    for _ in 0..batching.max_batches {
        let mut tx = pool.begin().await?;
        let removed = service.housekeep_quants(&mut tx, batching.batch_size).await?;
        tx.commit().await?;
        report.quants_vacuumed += removed;
        if removed < batching.batch_size as u64 {
            break;
        }
    }

    Ok(report)
}
