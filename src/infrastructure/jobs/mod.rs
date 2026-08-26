//! Scheduled jobs (hand-authored, user-owned).
//!
//! The module's single scheduled job lives here: `run_scheduler` — the daily replenishment +
//! reservation sweep declared in `schema/hooks/index.hook.yaml` under `scheduled_jobs`.

pub mod run_scheduler;

pub use run_scheduler::{run_scheduler, run_scheduler_with, SchedulerBatching, SchedulerReport};
