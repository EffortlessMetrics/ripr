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
//! The CLI wraps these same APIs and renders the resulting model in human,
//! JSON, and annotation formats.
//!
//! # Output and compatibility
//!
//! [`CheckOutput`] and the domain re-exports in this crate are the intended
//! integration surface for editor tooling, CI automation, and custom reporting.
//! Prefer consuming these typed values over parsing CLI output so integrations
//! remain resilient as human-readable formatting evolves.
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

// Shared internal outcome vocabulary for parser and output children under
// #2827. The public Rust API remains unchanged while internal projections
// deliberately carry the contract.
mod analysis_outcome;
mod atomic_file;
// Staged RepairAttempt edit-cage contract. #3163 connects the repository
// baseline/delta producer before any public receipt projection consumes it.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged internal contract; #3163 connects the RepairAttempt delta producer before receipt projection"
    )
)]
mod edit_cage;
// Shared internal repair-guidance availability vocabulary for the agent packet
// children under #2830. The public Rust API remains unchanged until those
// consumers adopt and deliberately expose the contract.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged internal contract; #2657 connects the first producer before public projection"
    )
)]
mod repair_guidance;

#[cfg(not(feature = "lang-rust"))]
compile_error!(
    "ripr requires the `lang-rust` Cargo feature; build rust-only binaries with `--no-default-features --features lang-rust`."
);

// Kept public for compatibility; prefer the crate-root re-exports for new
// integrations.
pub(crate) mod agent;
#[doc(hidden)]
pub mod analysis;
pub(crate) mod git;
// Kept public for compatibility; prefer the crate-root re-exports for new
// integrations.
#[doc(hidden)]
pub mod app;
// Kept public for compatibility with existing embedders.
#[doc(hidden)]
pub mod cli;
mod command_spec_digest;
pub mod config;
// Kept public for compatibility; prefer the crate-root domain type re-exports
// for new integrations.
#[doc(hidden)]
pub mod domain;
// Kept public for compatibility with experimental editor integrations.
#[doc(hidden)]
pub mod lsp;
// Protocol process entry point selected by the packaged binary before the
// human-oriented CLI dispatcher. The product status producer remains private.
#[doc(hidden)]
pub mod mcp;
// Kept public for compatibility with existing render integrations.
#[doc(hidden)]
pub mod output;
/// Exact-snapshot, read-only provider DTOs for external proof orchestrators.
pub mod provider_contract;
pub mod review_input;
mod workspace_status;
pub use analysis::LanguageRun;
pub use analysis::LanguageRunStatus;
pub use analysis::PartialDiffScope;
pub use analysis::PartialDiffStopReason;
pub use analysis::PreviewLanguageAdvisory;
/// Analyze a workspace diff using the default RIPR static pipeline.
pub use app::{
    CheckInput, CheckOutput, check_workspace, collect_context, explain_finding,
    explain_finding_with_input, reject_pr_evidence_error_packet, render_check,
};
/// Field types of the public entrypoint types (#2112): every public field
/// on `CheckInput` / `CheckOutput` is nameable from the crate root, so a
/// consumer never needs the `#[doc(hidden)]` modules to annotate,
/// pattern-match, or re-render a result.
pub use app::{Mode, OutputFormat};
/// Domain model types exposed as part of the stable public contract.
pub use domain::{
    ExposureClass, Finding, Probe, ProbeFamily, RiprEvidence, TestEvidenceEntry,
    TestEvidenceSummary,
};
/// Immutable Git candidate subject family (#3237 / #3276): construction
/// and validation types for naming an exact base/candidate tree pair as
/// the analysis input.
pub use domain::{
    GitCandidateBase, GitCandidateDiffSemantics, GitCandidateSubject, GitCandidateSubjectError,
    GitHashFormat, GitObjectId, GitTreeish,
};
pub use domain::{LanguageFileCount, SourceCurrentness, Summary};
pub use output::suppressions::CheckSuppressionOutcome;
pub use output::suppressions::SuppressedCheckFinding;
pub use provider_contract::{
    RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION, RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION,
    RIPR_PROVIDER_CAPABILITY_SCHEMA_VERSION, RIPR_PROVIDER_CLAIM_BOUNDARY,
    RIPR_REQUIRED_EXCLUDED_CLAIMS, RiprAnalysisReceiptV1, RiprAnalysisRequestV1,
    RiprEvidenceSubjectV1, RiprProviderCapabilityDescriptorV1, RiprProviderCapabilitySetV1,
    RiprProviderCapabilityV1, RiprProviderContractErrorCodeV1, RiprProviderContractErrorV1,
    RiprProviderDiagnosticV1, RiprProviderEvidenceEntryV1, RiprProviderEvidenceSummaryV1,
    RiprProviderNativeStatusV1, RiprProviderResultClassV1, RiprRepositorySnapshotV1,
    RiprSourceViewV1,
};

// #2610: global verbose flag. Set by the binary entry point before dispatch.
use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub(crate) fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}
