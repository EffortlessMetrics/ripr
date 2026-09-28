//! Public repo badge projection (RIPR-SPEC-0066).
//!
//! The public README badge answers one question for a repo owner or visitor:
//!
//! ```text
//! Is this repo currently clean, actionable, limited, or stale —
//! and is the number I am looking at current and complete?
//! ```
//!
//! This module projects a repo-scoped public badge into exactly one of the
//! five closed user-facing states and never renders a degraded input as a
//! clean count. The projection fails closed with the precedence:
//!
//! ```text
//! unknown > stale > limited > any count
//! ```
//!
//! A degraded input never resolves toward the cleaner-looking state. The
//! function [`project_public_badge`] is pure: the wall clock and the source
//! report identity are caller-supplied so every state-mapping row and
//! reject-list entry is unit-testable.

use std::time::{SystemTime, UNIX_EPOCH};

use super::model::{BadgeStatus, BadgeSummary};
use super::summaries::badge_status_color;

/// Default maximum age, in seconds, before a public badge or its source
/// report is considered stale (RIPR-SPEC-0066 "the configured maximum age").
///
/// Fourteen days: long enough that a quiet repo does not flap to `stale`,
/// short enough that an abandoned endpoint stops claiming a current count.
/// This is the default for the configurable max-age knob.
pub(crate) const DEFAULT_BADGE_MAX_AGE_SECS: u64 = 14 * 24 * 60 * 60;

/// Basis tokens permitted on the public badge. Raw `finding_exposure` is a
/// legacy/internal basis and is never a permitted public basis.
const PUBLIC_BADGE_BASES: &[&str] = &["canonical_actionable_gap", "gap_decision_ledger"];

/// The Lane-1 completeness state of a full, current run.
const RUN_STATUS_FULL: &str = "full";

/// True when `run_status` names a `limited_*` Lane-1 completeness state.
fn is_limited_run_status(run_status: &str) -> bool {
    run_status.starts_with("limited_")
}

/// The closed public badge message vocabulary (RIPR-SPEC-0066). No other
/// state may ship on the public badge surface; a new state requires a spec
/// revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicBadgeState {
    /// A full, current repo-scoped run found zero unresolved canonical
    /// actionable gaps.
    ZeroActionable,
    /// A full, current repo-scoped run found `N > 0` unresolved canonical
    /// actionable gaps.
    Actionable(usize),
    /// The source report exists but its `run_status` is a `limited_*` state;
    /// counts are not safe to publish.
    Limited,
    /// The source report or committed endpoint exceeds the configured
    /// maximum age; the last count is no longer claimed as current.
    Stale,
    /// No consumable source report exists, or the only available basis is
    /// raw findings; no count is claimed.
    Unknown,
}

impl PublicBadgeState {
    /// The Shields `message` field for this state. The public badge renders
    /// as `{label}: {message}` (for example `ripr: 191 actionable`), so the
    /// message itself is label-agnostic and shared by `ripr` and `ripr+`.
    pub(crate) fn shields_message(self) -> String {
        match self {
            PublicBadgeState::ZeroActionable => "0 actionable".to_string(),
            PublicBadgeState::Actionable(count) => format!("{count} actionable"),
            PublicBadgeState::Limited => "limited".to_string(),
            PublicBadgeState::Stale => "stale".to_string(),
            PublicBadgeState::Unknown => "unknown".to_string(),
        }
    }

    /// Stable machine token for this state, emitted in the badge sidecar.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PublicBadgeState::ZeroActionable => "zero_actionable",
            PublicBadgeState::Actionable(_) => "actionable",
            PublicBadgeState::Limited => "limited",
            PublicBadgeState::Stale => "stale",
            PublicBadgeState::Unknown => "unknown",
        }
    }

    fn status_color(self) -> (BadgeStatus, &'static str) {
        match self {
            PublicBadgeState::ZeroActionable => badge_status_color(0, false),
            PublicBadgeState::Actionable(count) => badge_status_color(count, false),
            // Degraded states are warn, never pass and never a hard fail: the
            // badge is honest that no current count is being claimed.
            PublicBadgeState::Limited | PublicBadgeState::Stale | PublicBadgeState::Unknown => {
                (BadgeStatus::Warn, "lightgrey")
            }
        }
    }
}

/// Caller-supplied inputs for [`project_public_badge`]. The clock and the
/// source identity travel as data so the projection stays pure.
#[derive(Clone, Debug)]
pub(crate) struct PublicBadgeInput {
    /// `BadgeBasis::as_str()` of the source.
    pub basis: &'static str,
    /// `BadgeScope::as_str()` of the source.
    pub scope: &'static str,
    /// Lane-1 completeness state of the source run (`full` or `limited_*`).
    pub run_status: String,
    /// Unresolved canonical actionable gap count from the source.
    pub actionable_count: usize,
    /// When the source artifact was generated (unix milliseconds), or `None`
    /// when no `generated_at` is available.
    pub generated_at_unix_ms: Option<u64>,
    /// Evaluation time (unix milliseconds).
    pub now_unix_ms: u64,
    /// Maximum age, in seconds, before the artifact is stale.
    pub max_age_secs: u64,
    /// Repo-relative path of the report the badge was projected from, or
    /// `None` when no consumable source exists.
    pub source_report: Option<String>,
    /// Named limited reason (`limitation_category` and `repair_route` when
    /// present) for a `limited_*` run; ignored for full runs.
    pub limited_reason: Option<String>,
}

/// The projected public badge: the rendered state plus the six required
/// sidecar fields (RIPR-SPEC-0066 "Required fields").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PublicBadgeProjection {
    pub state: PublicBadgeState,
    /// Shields `message` field (label-agnostic, e.g. `191 actionable`).
    pub shields_message: String,
    pub status: BadgeStatus,
    pub color: &'static str,
    /// Lane-1 completeness state of the source run.
    pub run_status: String,
    /// RFC3339 UTC timestamp the badge was generated, or `None`.
    pub generated_at: Option<String>,
    /// Unresolved canonical actionable gap count; `Some` only for the
    /// `0 actionable` / `N actionable` states, never alongside a degraded
    /// state.
    pub actionable_count: Option<usize>,
    /// The `limitation_category` (and repair route) explaining a `limited`
    /// state; `None` otherwise.
    pub limited_reason: Option<String>,
    /// Age of the artifact relative to its source at evaluation time, in
    /// seconds; `None` when `generated_at` is unknown.
    pub stale_age_secs: Option<u64>,
    /// Repo-relative path the badge was projected from, or `None`.
    pub source_report: Option<String>,
}

/// Projects a repo-scoped public badge into exactly one closed state with
/// fail-closed precedence (`unknown > stale > limited > count`). A degraded
/// input never resolves toward the cleaner-looking state, and a count is
/// never carried alongside a degraded state.
pub(crate) fn project_public_badge(input: &PublicBadgeInput) -> PublicBadgeProjection {
    let stale_age_secs = input
        .generated_at_unix_ms
        .map(|gen_ms| input.now_unix_ms.saturating_sub(gen_ms) / 1_000);

    let state = classify(input, stale_age_secs);
    let (status, color) = state.status_color();

    let actionable_count = match state {
        PublicBadgeState::ZeroActionable => Some(0),
        PublicBadgeState::Actionable(count) => Some(count),
        // Reject-list: never a count alongside limited/stale/unknown.
        PublicBadgeState::Limited | PublicBadgeState::Stale | PublicBadgeState::Unknown => None,
    };

    let limited_reason = if state == PublicBadgeState::Limited {
        input.limited_reason.clone()
    } else {
        None
    };

    PublicBadgeProjection {
        state,
        shields_message: state.shields_message(),
        status,
        color,
        run_status: input.run_status.clone(),
        generated_at: input.generated_at_unix_ms.map(format_rfc3339_from_unix_ms),
        actionable_count,
        limited_reason,
        stale_age_secs,
        source_report: input.source_report.clone(),
    }
}

/// State selection with fail-closed precedence. Each rejected condition
/// resolves to `unknown`, `limited`, or `stale` — never a silent green.
fn classify(input: &PublicBadgeInput, stale_age_secs: Option<u64>) -> PublicBadgeState {
    // `unknown` (highest precedence): no trustworthy basis for any count.
    if input.scope != "repo" {
        return PublicBadgeState::Unknown;
    }
    if !PUBLIC_BADGE_BASES.contains(&input.basis) {
        return PublicBadgeState::Unknown;
    }
    if input.source_report.is_none() {
        return PublicBadgeState::Unknown;
    }
    if input.generated_at_unix_ms.is_none() {
        return PublicBadgeState::Unknown;
    }

    // `stale`: the artifact is older than the configured maximum age.
    if let Some(age) = stale_age_secs
        && age > input.max_age_secs
    {
        return PublicBadgeState::Stale;
    }

    // `limited`: the run did not complete fully.
    if is_limited_run_status(&input.run_status) {
        return PublicBadgeState::Limited;
    }
    // Any unrecognized run_status fails closed rather than claiming a count.
    if input.run_status != RUN_STATUS_FULL {
        return PublicBadgeState::Unknown;
    }

    // Full, current, canonical-basis run: render the count.
    if input.actionable_count == 0 {
        PublicBadgeState::ZeroActionable
    } else {
        PublicBadgeState::Actionable(input.actionable_count)
    }
}

/// Computes the public badge projection (RIPR-SPEC-0066) for a repo-scoped
/// public badge `summary` and attaches it, using the current wall clock as
/// the freshness anchor.
///
/// A freshly rendered badge is, by construction, a `full` and current run:
/// the projection therefore resolves to a `0 actionable` / `N actionable`
/// state for canonical/ledger-basis repo summaries. A summary whose basis or
/// scope is not public fails closed to `unknown` rather than mislabelling a
/// count. The Shields-facing `message`, `status`, and `color` are overwritten
/// from the projection so the public surface speaks the closed vocabulary.
///
/// `source_report` names the artifact the count was projected from. Diff-first
/// limited runs and age-based staleness over committed endpoints arrive when
/// the xtask pipeline drives this projection with real `run_status` and
/// `generated_at` inputs; the pure [`project_public_badge`] already implements
/// and tests those states.
pub(crate) fn attach_public_projection(summary: &mut BadgeSummary, source_report: &str) {
    // Only repo-scoped public-basis badges carry a projection. Diff-scoped and
    // internal badges are left byte-identical so their native JSON and Shields
    // message do not change.
    if summary.scope.as_str() != "repo" || !PUBLIC_BADGE_BASES.contains(&summary.basis.as_str()) {
        return;
    }
    let now = now_unix_ms();
    let input = PublicBadgeInput {
        basis: summary.basis.as_str(),
        scope: summary.scope.as_str(),
        run_status: RUN_STATUS_FULL.to_string(),
        actionable_count: summary.counts.unsuppressed_exposure_gaps,
        generated_at_unix_ms: Some(now),
        now_unix_ms: now,
        max_age_secs: DEFAULT_BADGE_MAX_AGE_SECS,
        source_report: Some(source_report.to_string()),
        limited_reason: None,
    };
    let projection = project_public_badge(&input);
    summary.message = projection.shields_message.clone();
    summary.status = projection.status;
    summary.color = projection.color;
    summary.projection = Some(projection);
}

/// Current wall-clock time in unix milliseconds. Boundary helper; the pure
/// projection takes the value as data.
pub(crate) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let ms = d.as_millis();
            u64::try_from(ms).unwrap_or(u64::MAX)
        })
        .unwrap_or(0)
}

/// Formats a unix-millisecond timestamp as an RFC3339 UTC string
/// (`YYYY-MM-DDTHH:MM:SSZ`). Self-contained civil-date arithmetic (Howard
/// Hinnant), good for any in-range timestamp.
fn format_rfc3339_from_unix_ms(unix_ms: u64) -> String {
    let secs = unix_ms / 1_000;
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    let (year, month, day) = days_to_civil_date(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Converts days since 1970-01-01 to a civil `(year, month, day)` using the
/// Howard Hinnant algorithm. `d` (1..=31) and `m` (1..=12) are positive by
/// construction; the checked conversions keep the function lint-clean without
/// an allow attribute.
fn days_to_civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::badge::{BadgeBasis, BadgeScope};

    const NOW_MS: u64 = 1_700_000_000 * 1_000;

    /// A full, current, repo-scoped canonical-basis input with `count`
    /// actionable gaps. Each reject-list / state test mutates one field away
    /// from this clean baseline.
    fn full_canonical(count: usize) -> PublicBadgeInput {
        PublicBadgeInput {
            basis: BadgeBasis::CanonicalActionableGap.as_str(),
            scope: BadgeScope::Repo.as_str(),
            run_status: "full".to_string(),
            actionable_count: count,
            generated_at_unix_ms: Some(NOW_MS),
            now_unix_ms: NOW_MS,
            max_age_secs: DEFAULT_BADGE_MAX_AGE_SECS,
            source_report: Some("target/ripr/reports/repo-ripr-badge.json".to_string()),
            limited_reason: None,
        }
    }

    // --- State mapping rows (RIPR-SPEC-0066 "State mapping rules") ---

    #[test]
    fn state_tokens_are_stable() {
        // The sidecar `state` token is a public contract; pin every arm.
        assert_eq!(PublicBadgeState::ZeroActionable.as_str(), "zero_actionable");
        assert_eq!(PublicBadgeState::Actionable(3).as_str(), "actionable");
        assert_eq!(PublicBadgeState::Limited.as_str(), "limited");
        assert_eq!(PublicBadgeState::Stale.as_str(), "stale");
        assert_eq!(PublicBadgeState::Unknown.as_str(), "unknown");
    }

    #[test]
    fn state_messages_are_the_closed_vocabulary() {
        assert_eq!(
            PublicBadgeState::ZeroActionable.shields_message(),
            "0 actionable"
        );
        assert_eq!(
            PublicBadgeState::Actionable(7).shields_message(),
            "7 actionable"
        );
        assert_eq!(PublicBadgeState::Limited.shields_message(), "limited");
        assert_eq!(PublicBadgeState::Stale.shields_message(), "stale");
        assert_eq!(PublicBadgeState::Unknown.shields_message(), "unknown");
    }

    #[test]
    fn full_current_zero_gaps_is_zero_actionable() {
        let projection = project_public_badge(&full_canonical(0));
        assert_eq!(projection.state, PublicBadgeState::ZeroActionable);
        assert_eq!(projection.shields_message, "0 actionable");
        assert_eq!(projection.status, BadgeStatus::Pass);
        assert_eq!(projection.color, "brightgreen");
        assert_eq!(projection.actionable_count, Some(0));
        assert_eq!(projection.limited_reason, None);
        assert!(projection.generated_at.is_some());
        assert_eq!(projection.stale_age_secs, Some(0));
    }

    #[test]
    fn full_current_n_gaps_is_n_actionable() {
        let projection = project_public_badge(&full_canonical(191));
        assert_eq!(projection.state, PublicBadgeState::Actionable(191));
        assert_eq!(projection.shields_message, "191 actionable");
        assert_eq!(projection.status, BadgeStatus::Warn);
        assert_eq!(projection.actionable_count, Some(191));
    }

    #[test]
    fn gap_decision_ledger_basis_is_a_valid_count_basis() {
        let mut input = full_canonical(3);
        input.basis = BadgeBasis::GapDecisionLedger.as_str();
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Actionable(3));
    }

    #[test]
    fn limited_run_status_is_limited_with_named_reason() {
        let mut input = full_canonical(5);
        input.run_status = "limited_timeout".to_string();
        input.limited_reason = Some("lane1_repo_exposure_timeout".to_string());
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Limited);
        assert_eq!(projection.shields_message, "limited");
        // Reject-list: never a count alongside a limited state.
        assert_eq!(projection.actionable_count, None);
        assert_eq!(
            projection.limited_reason.as_deref(),
            Some("lane1_repo_exposure_timeout")
        );
    }

    #[test]
    fn over_age_artifact_is_stale() {
        let mut input = full_canonical(7);
        input.generated_at_unix_ms = Some(NOW_MS - (DEFAULT_BADGE_MAX_AGE_SECS * 1_000) - 1_000);
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Stale);
        assert_eq!(projection.shields_message, "stale");
        // Reject-list: the previous count is not re-claimed.
        assert_eq!(projection.actionable_count, None);
    }

    // --- Reject list (RIPR-SPEC-0066 "Verifier reject-list") ---

    #[test]
    fn raw_finding_basis_is_unknown_never_a_count() {
        let mut input = full_canonical(42);
        input.basis = BadgeBasis::FindingExposure.as_str();
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Unknown);
        assert_eq!(projection.shields_message, "unknown");
        assert_eq!(projection.actionable_count, None);
    }

    #[test]
    fn diff_scope_is_unknown_on_public_badge() {
        let mut input = full_canonical(0);
        input.scope = BadgeScope::Diff.as_str();
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Unknown);
        // The always-zero no-diff artifact must never read as 0 actionable.
        assert_eq!(projection.actionable_count, None);
    }

    #[test]
    fn missing_source_report_is_unknown() {
        let mut input = full_canonical(2);
        input.source_report = None;
        assert_eq!(
            project_public_badge(&input).state,
            PublicBadgeState::Unknown
        );
    }

    #[test]
    fn missing_generated_at_is_unknown() {
        let mut input = full_canonical(2);
        input.generated_at_unix_ms = None;
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Unknown);
        assert_eq!(projection.generated_at, None);
        assert_eq!(projection.stale_age_secs, None);
    }

    #[test]
    fn unrecognized_run_status_fails_closed_to_unknown() {
        let mut input = full_canonical(2);
        input.run_status = "sampled".to_string();
        assert_eq!(
            project_public_badge(&input).state,
            PublicBadgeState::Unknown
        );
    }

    // --- Precedence: unknown > stale > limited > count ---

    #[test]
    fn stale_takes_precedence_over_limited() {
        let mut input = full_canonical(9);
        input.run_status = "limited_timeout".to_string();
        input.generated_at_unix_ms = Some(NOW_MS - (DEFAULT_BADGE_MAX_AGE_SECS * 1_000) - 1_000);
        assert_eq!(project_public_badge(&input).state, PublicBadgeState::Stale);
    }

    #[test]
    fn unknown_takes_precedence_over_stale_and_limited() {
        let mut input = full_canonical(9);
        input.basis = BadgeBasis::FindingExposure.as_str();
        input.run_status = "limited_timeout".to_string();
        input.generated_at_unix_ms = Some(NOW_MS - (DEFAULT_BADGE_MAX_AGE_SECS * 1_000) - 1_000);
        assert_eq!(
            project_public_badge(&input).state,
            PublicBadgeState::Unknown
        );
    }

    // --- Max-age boundary ---

    #[test]
    fn age_exactly_at_max_is_not_stale() {
        let mut input = full_canonical(1);
        input.generated_at_unix_ms = Some(NOW_MS - (DEFAULT_BADGE_MAX_AGE_SECS * 1_000));
        let projection = project_public_badge(&input);
        assert_eq!(projection.state, PublicBadgeState::Actionable(1));
        assert_eq!(projection.stale_age_secs, Some(DEFAULT_BADGE_MAX_AGE_SECS));
    }

    #[test]
    fn age_one_second_over_max_is_stale() {
        let mut input = full_canonical(1);
        input.generated_at_unix_ms = Some(NOW_MS - (DEFAULT_BADGE_MAX_AGE_SECS * 1_000) - 1_000);
        assert_eq!(project_public_badge(&input).state, PublicBadgeState::Stale);
    }

    #[test]
    fn rfc3339_formats_unix_ms_as_utc() {
        // 1_700_000_000 seconds since the epoch is 2023-11-14T22:13:20Z.
        assert_eq!(
            format_rfc3339_from_unix_ms(1_700_000_000 * 1_000),
            "2023-11-14T22:13:20Z"
        );
        assert_eq!(format_rfc3339_from_unix_ms(0), "1970-01-01T00:00:00Z");
    }

    // --- attach_public_projection wiring ---

    #[test]
    fn attach_projects_repo_canonical_summary() {
        use crate::analysis::ClassifiedSeam;
        let classified: Vec<ClassifiedSeam> = Vec::new();
        let mut summary = crate::output::badge::ripr_canonical_actionable_gap_badge_summary(
            &classified,
            crate::output::badge::BadgePolicy::default(),
        );
        attach_public_projection(&mut summary, "target/ripr/reports/repo-ripr-badge.json");
        assert_eq!(summary.message, "0 actionable");
        assert_eq!(
            summary.projection.as_ref().map(|p| p.run_status.as_str()),
            Some("full")
        );
        assert_eq!(
            summary.projection.as_ref().map(|p| p.state),
            Some(PublicBadgeState::ZeroActionable)
        );
    }

    #[test]
    fn attach_is_no_op_for_diff_badges() {
        use crate::analysis::ClassifiedSeam;
        // A diff-scoped badge must be left unprojected so its native JSON and
        // Shields message stay byte-identical.
        let classified: Vec<ClassifiedSeam> = Vec::new();
        let mut summary = crate::output::badge::ripr_canonical_actionable_gap_badge_summary(
            &classified,
            crate::output::badge::BadgePolicy::default(),
        );
        summary.scope = BadgeScope::Diff;
        let before = summary.message.clone();
        attach_public_projection(&mut summary, "source");
        assert!(summary.projection.is_none());
        assert_eq!(summary.message, before);
    }
}
