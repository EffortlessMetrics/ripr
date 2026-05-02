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
//! # Typical integration flow
//!
//! 1. Build a [`CheckInput`] with repository root, target diff, and options.
//! 2. Call [`check_workspace`] to produce a [`CheckOutput`] report.
//! 3. For a specific probe id, call [`explain_finding`] to inspect evidence.
//! 4. Use [`collect_context`] when you need neighboring source context for UX.
//!
//! - Use [`check_workspace`] for end-to-end analysis.
//! - Use [`explain_finding`] to retrieve focused evidence for one probe.
//! - Use [`collect_context`] to retrieve neighboring context around a probe.
//!
//! The CLI wraps these same APIs and renders the resulting model in human,
//! JSON, and annotation formats.
//!
//! # Exposure language
//!
//! `ripr` reports static exposure estimates such as [`ExposureClass::Exposed`]
//! and [`ExposureClass::WeaklyExposed`]. Findings can also remain unknown when
//! static evidence is incomplete. These results are intended to guide targeted
//! test intent, not to claim runtime mutation outcomes.
//!
//! # Quick start
//!
//! ```no_run
//! use ripr::{CheckInput, check_workspace};
//! use std::path::PathBuf;
//!
//! let report = check_workspace(CheckInput {
//!     root: PathBuf::from("."),
//!     ..CheckInput::default()
//! })?;
//!
//! println!("findings: {}", report.findings.len());
//! # Ok::<(), String>(())
//! ```
//!

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
