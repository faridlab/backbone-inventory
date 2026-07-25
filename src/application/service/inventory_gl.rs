//! Outbound GL-posting port (hand-authored, user-owned) — re-export of the shared contract.
//!
//! The GL-posting wire types (`AccountingPostEnvelope`, `GlPostLine`, `GlPostAck`, `GlPostRejected`)
//! and the `GlPostSink` port now live in the shared `backbone-gl-posting` crate (backbone-framework
//! v2.7.5) — the single source for all producers (phase 2). This file re-exports them under inventory's
//! existing paths so inventory's write service, tests, and `application::service::*` resolve unchanged.
//! Inventory is the supply-chain emitter (Purchase Receipt `Dr Inventory · Cr GR/IR`; Delivery Note
//! `Dr COGS · Cr Inventory`; Stock Reconciliation the value diff); the ACL maps the envelope into
//! accounting's `PostingRequest`. Zero normal Cargo edge into backbone-accounting.

pub use backbone_gl_posting::{
    AccountingPostEnvelope, GlPostAck, GlPostLine, GlPostRejected, GlPostSink,
};
