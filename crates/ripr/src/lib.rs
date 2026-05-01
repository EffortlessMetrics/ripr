#![forbid(unsafe_code)]
//! `ripr` is a static RIPR mutation-exposure analyzer for Rust workspaces.
//!
//! It does not run mutants. It reads changed Rust code, creates mutation-shaped
//! probes, and estimates whether tests appear to reach, infect, propagate, and
//! reveal those changed behaviors through meaningful oracles.

pub mod analysis;
pub mod app;
pub mod cli;
pub mod domain;
pub mod lsp;
pub mod output;

pub use app::{CheckInput, CheckOutput, check_workspace, collect_context, explain_finding};
pub use domain::{ExposureClass, Finding, Probe, ProbeFamily, RiprEvidence};
