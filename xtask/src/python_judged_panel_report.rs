//! Python judged PR panel report and adjudication (RIPR-SPEC-0092, #3555 PR C).
//!
//! `adjudicate` records a current independent judgment for one case under
//! `target/ripr/python-judged-panel/adjudications/<case_id>.json` without
//! touching the accepted panel: reviewer role and identity are required (flag
//! or `RIPR_PANEL_ADJUDICATOR`), at least one own evidence citation is
//! required, `must_not_claim` is echoed from the validated row, and RIPR's
//! replay candidate classification is stored only as a separately named
//! advisory reference — never as the adjudicator's ground truth. A case
//! counts as adjudicated only with judgments from at least two distinct
//! recorded roles and identities (independence is self-claimed, not
//! mechanically verified); one role stays
//! `pending_second_role`, role disagreement is `disputed`, and an agreeing
//! judgment with no direction-admitted error axis decided is `inconclusive`
//! — never a pass.
//!
//! `report` derives deterministic JSON + Markdown from the validated
//! inventory, the replay records, and the adjudication records. Every rate
//! carries its exact numerator, denominator, coverage boundary, denominator
//! case ids, and the as-of identity bound by those cases' own replay
//! records; no denominator means no
//! rate (the rate key is omitted, never a fake zero). The two-error lattice
//! stays separate — no combined quality score exists anywhere. Replay
//! mismatches are advisory divergence data and never enter a rate or a
//! threshold. Threshold evaluation runs only when an explicit
//! `--threshold-policy <file>` is supplied, emits
//! `pass`/`fail`/`not_evaluable` per threshold, echoes the policy's own
//! rationale and authority, and is non-authoritative by construction: the
//! report never selects a threshold from observed results, never promotes
//! support, and writes no tier claim. Both renderings come from one derived
//! Value; volatile record fields (recorded command line, temp workspace
//! paths, stderr detail) and adjudication timestamps are never echoed, so
//! independent replay runs over identical inputs render identical bytes
//! (`report_bytes_are_stable_across_independent_runs`).

mod adjudication;
mod cli;
mod judgment_semantics;
mod publish;
mod replay_records;
mod report;
mod threshold;
mod view;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use adjudication::adjudicate_case_at;
#[cfg(test)]
pub(crate) use cli::AdjudicationRequest;
pub(crate) use cli::{run_adjudicate, run_report};
pub(crate) use report::{RenderedReport, build_report_at};

// Re-imported at the facade so `tests.rs` keeps reaching the moved items
// through `super::` exactly as it did when this module was one file. The
// items now re-exported `pub(crate)` above (adjudicate_case_at,
// AdjudicationRequest, RenderedReport, build_report_at) are covered there.
#[cfg(test)]
use adjudication::{acquire_path_lock, acquire_record_lock};
#[cfg(test)]
use publish::write_report_generation;
#[cfg(test)]
use replay_records::{cite_replay_record, parse_replay_record_bytes};

const SPEC: &str = "RIPR-SPEC-0092";
const REPORT_SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "python_judged_panel_report";
const ADJUDICATION_SCHEMA_VERSION: &str = "0.1";
const ADJUDICATION_KIND: &str = "python_judged_panel_adjudication_record";
const POLICY_KIND: &str = "python_judged_panel_threshold_policy";
const AUTHORITY_BOUNDARY: &str = "review_advisory_only";
const REPORT_OUT_DIR: &str = "target/ripr/python-judged-panel";
const ADJUDICATIONS_DIR: &str = "target/ripr/python-judged-panel/adjudications";
/// Reviewer identity env fallback (identity is required, flag or env).
const REVIEWER_ENV: &str = "RIPR_PANEL_ADJUDICATOR";
const REPORT_RERUN: &str = "cargo xtask python-judged-panel report";
const ADJUDICATE_RERUN: &str = "cargo xtask python-judged-panel adjudicate";
const RECORD_KIND: &str = "python_judged_panel_replay_record";
const RECORD_SCHEMA_VERSION: &str = "0.1";
const OUTCOME_VOCABULARY: [&str; 6] = [
    "complete",
    "partial",
    "failed",
    "parse_failed",
    "timed_out",
    "not_run",
];
const MISMATCH_VOCABULARY: [&str; 4] = [
    "classification_mismatch",
    "expected_but_quiet",
    "prior_actual_mismatch",
    "prior_actual_quiet",
];

const NOTE_REPLAY_ADVISORY: &str = "Replay comparisons are advisory divergence data (RIPR-SPEC-0092 PR B); mismatch and comparison-unavailable counts are disclosed for context and never enter a rate, a denominator, or a threshold evaluation.";
const NOTE_NO_INHERITED_DENOMINATOR: &str = "The historical combined denominator n=7 is not inherited; every rate here states its actual achieved denominator.";
const NOTE_NO_COMBINED_SCORE: &str = "No single quality score is computed: false_actionable and false_exposed keep separate numerators, denominators, and rates.";
const NOTE_RELATION_BASIS: &str = "Coverage by relation basis is disclosed unavailable: the retained panel schema carries no typed relation-basis field, and none is invented here.";
const NOTE_STALE_DEFINITION: &str = "`stale` counts replay records whose evidence identity no longer binds the current inputs: a prior-actual stale note, a diff digest that no longer matches the retained fixture, a row kind that changed since the record was written, or an anchor that moved (or a record written before the anchor echo existed) — the `anchor_stale` count and each case's `anchor_stale_reason` name that last kind. `not_run` includes selected rows that have no replay record in the read directory.";
const THRESHOLD_AUTHORITY_NOTE: &str = "Non-authoritative by construction: the threshold candidate and its rationale come entirely from the supplied policy file; this report never selects a threshold from observed results, never promotes support, and writes no operator tier ruling.";
const FALSE_ACTIONABLE_BOUNDARY: &str = "adjudicated rows whose expected_direction admits false_actionable (should_stay_quiet, should_limit) with a decided label";
const FALSE_EXPOSED_BOUNDARY: &str = "adjudicated rows whose expected_direction admits false_exposed (should_gap, should_limit) with a decided label";
