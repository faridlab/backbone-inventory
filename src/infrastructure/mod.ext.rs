// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The module's scheduled jobs. `jobs/` is declared `user_owned` in metaphor.codegen.yaml
// (hand-written; the scheduler declaration of record is schema/hooks/index.hook.yaml).
pub mod jobs;
