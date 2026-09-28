//! Test-only shared helpers for the `ripr` unit-test suite.
//!
//! This module is compiled for `cargo test` targets only and never ships in
//! the library surface. Integration tests under `tests/` cannot see
//! `pub(crate)` items; their harness-local mirror lives in
//! `tests/common/fixture_git.rs` and keeps the same contract.

pub(crate) mod cwd_placeholder;
pub(crate) mod fixture_git;
pub(crate) mod fixture_workspace;
pub(crate) mod rebless;
