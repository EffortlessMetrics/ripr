#![forbid(unsafe_code)]
//! `ripr` is a static RIPR mutation-exposure analyzer for Rust workspaces.
//!
//! It does not run mutants. It reads changed Rust code, creates mutation-shaped
//! probes, and estimates whether tests appear to reach, infect, propagate, and
//! reveal those changed behaviors through meaningful oracles.
//!
//! # Quick start
//!
//! ```no_run
//! use ripr::{CheckInput, check_workspace};
//!
//! # fn run() -> anyhow::Result<()> {
//! let input = CheckInput::from_diff_path("crates/ripr/examples/sample/example.diff")?;
//! let output = check_workspace(&input)?;
//! println!("findings: {}", output.findings.len());
//! # Ok(())
//! # }
//! ```

/// Diff loading, syntax indexing, probe generation, and static classification.
pub mod analysis;
/// Application use-cases and public API orchestration.
pub mod app;
/// Command-line adapter and argument parsing.
pub mod cli;
/// Core RIPR concepts, findings, and exposure vocabulary.
pub mod domain;
/// Experimental language-server sidecar adapter.
pub mod lsp;
/// Human, JSON, and GitHub annotation rendering.
pub mod output;

/// Input contract for [`check_workspace`].
pub use app::{CheckInput, CheckOutput, check_workspace, collect_context, explain_finding};
/// Public domain model re-exports for downstream integrations.
pub use domain::{ExposureClass, Finding, Probe, ProbeFamily, RiprEvidence};
