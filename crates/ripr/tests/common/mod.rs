//! Harness-local shared helpers for `ripr` integration tests (#3742).
//!
//! Each integration-test target compiles alone, so shared helpers live here
//! and are wired per-target with `#[path = "common/mod.rs"] mod common;`.

pub mod fixture_git;
