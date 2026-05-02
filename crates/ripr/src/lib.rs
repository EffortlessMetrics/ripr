#![forbid(unsafe_code)]
//! `ripr` is a static RIPR mutation-exposure analyzer for Rust workspaces.
//!
//! It does not run mutants. It reads changed Rust code, creates mutation-shaped
//! probes, and estimates whether tests appear to reach, infect, propagate, and
//! reveal those changed behaviors through meaningful oracles.
//!
//! # Library entry points
//!
//! Most integrations should start with [`check_workspace`] to analyze a unified
//! diff and obtain structured findings.
//!
//! # Output interpretation
//!
//! `ripr` reports static exposure evidence, not dynamic mutation outcomes.
//! Integrations should treat [`ExposureClass`] values as conservative draft-mode
//! signals (`exposed`, `weakly_exposed`, `reachable_unrevealed`,
//! `no_static_path`, and `static_unknown`) that guide targeted test intent.
//! Downstream automation should avoid mapping these classes to definitive terms
//! like "killed" or "survived" without a real mutation-testing pass.
//!
//! - Use [`check_workspace`] for end-to-end analysis.
//! - Use [`explain_finding`] to retrieve focused evidence for one probe.
//! - Use [`collect_context`] to retrieve neighboring context around a probe.
//!
//! The CLI wraps these same APIs and renders the resulting model in human,
//! JSON, and annotation formats.

/// Static analysis pipeline: diff loading, syntax indexing, probe generation,
/// and finding classification.
pub mod analysis;
/// Public application orchestration and library-level use cases.
pub mod app;
/// Command-line adapter layer for the `ripr` binary.
pub mod cli;
/// Core domain model for probes, RIPR evidence, and exposure classes.
pub mod domain;
/// Experimental language-server sidecar adapter.
pub mod lsp;
/// Output renderers for human-readable, JSON, and annotation formats.
pub mod output;

/// Analyze a workspace diff using the default RIPR static pipeline.
pub use app::{CheckInput, CheckOutput, check_workspace, collect_context, explain_finding};
/// Domain model types exposed as part of the stable public contract.
pub use domain::{ExposureClass, Finding, Probe, ProbeFamily, RiprEvidence};
