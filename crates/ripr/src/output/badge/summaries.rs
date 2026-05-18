#[cfg(test)]
use crate::analysis::ClassifiedSeam;
use crate::analysis::SeamGripClassCounts;
use crate::analysis::seams::SeamGripClass;
use crate::app::CheckOutput;
use crate::config::{ConfigSeverity, RiprConfig};
use crate::domain::ExposureClass;
use crate::output::gap_decision_ledger;
use crate::output::suppressions::{SuppressionEntry, apply_exposure_suppressions};
use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    BADGE_REASON_KEYS, BadgeBasis, BadgeCounts, BadgeKind, BadgePolicy, BadgeScope, BadgeStatus,
    BadgeSummary,
};

/// Builds the `ripr` badge summary from a `CheckOutput`, applying any
/// `kind = "exposure_gap"` suppressions whose `finding_id` matches a
/// currently-counted exposure gap. Expired and unmatched suppressions
/// surface as `warnings` so silently-stale debt cannot keep the badge
/// green. `today` is the ISO date used for expiry comparison.
pub fn ripr_badge_summary_with_suppressions(
    output: &CheckOutput,
    suppressions: &[SuppressionEntry],
    today: &str,
    policy: BadgePolicy,
) -> BadgeSummary {
    let mut candidate_ids: Vec<String> = Vec::new();
    let mut unknowns = 0usize;
    let mut unique_tests: BTreeSet<(String, String, usize)> = BTreeSet::new();

    for finding in &output.findings {
        match finding.class {
            ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::NoStaticPath => {
                candidate_ids.push(finding.id.clone());
            }
            ExposureClass::InfectionUnknown
            | ExposureClass::PropagationUnknown
            | ExposureClass::StaticUnknown => {
                unknowns += 1;
            }
            ExposureClass::Exposed => {}
        }
        for test in &finding.related_tests {
            unique_tests.insert((
                test.file.to_string_lossy().into_owned(),
                test.name.clone(),
                test.line,
            ));
        }
    }

    let suppression_app = apply_exposure_suppressions(&candidate_ids, suppressions, today);
    let suppressed = suppression_app.suppressed_findings.len();
    let unsuppressed_exposure_gaps = candidate_ids.len().saturating_sub(suppressed);

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: suppressed,
        suppressed_test_efficiency_findings: 0,
        unknowns,
        unknowns_test_efficiency: 0,
        analyzed_findings: output.findings.len(),
        analyzed_seams: 0,
        analyzed_gap_records: 0,
        analyzed_tests: unique_tests.len(),
    };

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }

    let headline = counts.unsuppressed_exposure_gaps
        + if policy.include_unknowns {
            counts.unknowns
        } else {
            0
        };
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    BadgeSummary {
        kind: BadgeKind::Ripr,
        scope: BadgeScope::Diff,
        basis: BadgeBasis::FindingExposure,
        message: headline.to_string(),
        status,
        color,
        counts,
        reason_counts,
        policy,
        warnings: suppression_app.warnings,
    }
}

/// Convenience wrapper: builds the `ripr` badge with no suppressions.
/// Equivalent to calling [`ripr_badge_summary_with_suppressions`] with
/// an empty slice. Test-only since production callers always go through
/// [`crate::app::render_check`] which threads the loaded suppressions.
#[cfg(test)]
pub fn ripr_badge_summary(output: &CheckOutput, policy: BadgePolicy) -> BadgeSummary {
    ripr_badge_summary_with_suppressions(output, &[], "", policy)
}

/// Builds the repo-scoped `ripr` badge summary from classified seams.
///
/// This is the seam-native badge path used by public repo badges. It counts
/// configured-visible headline-eligible seam classes as unresolved gaps,
/// keeps opaque seams in the `unknowns` bucket, and omits classes configured
/// as `off` from both the headline and visible count buckets.
#[cfg(test)]
pub(crate) fn ripr_seam_badge_summary(
    classified: &[ClassifiedSeam],
    config: &RiprConfig,
    policy: BadgePolicy,
) -> BadgeSummary {
    let mut counts = SeamGripClassCounts::new(classified.len());
    for entry in classified {
        counts.increment(entry.class);
    }
    ripr_seam_badge_summary_from_counts(&counts, config, policy)
}

/// Builds the repo-scoped `ripr` badge summary from compact seam grip
/// class counts.
pub(crate) fn ripr_seam_badge_summary_from_counts(
    class_counts: &SeamGripClassCounts,
    config: &RiprConfig,
    policy: BadgePolicy,
) -> BadgeSummary {
    let mut unresolved = 0usize;
    let mut suppressed = 0usize;
    let mut unknowns = 0usize;

    for class in SeamGripClass::ALL {
        let count = class_counts.count_for(class);
        if count == 0 || config.severity().for_seam(class) == ConfigSeverity::Off {
            continue;
        }
        if class.is_headline_eligible() {
            unresolved += count;
        } else if class == SeamGripClass::Suppressed {
            suppressed += count;
        } else if class == SeamGripClass::Opaque {
            unknowns += count;
        }
    }

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps: unresolved,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: suppressed,
        suppressed_test_efficiency_findings: 0,
        unknowns,
        unknowns_test_efficiency: 0,
        analyzed_findings: 0,
        analyzed_seams: class_counts.analyzed_seams(),
        analyzed_gap_records: 0,
        analyzed_tests: 0,
    };

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }

    let headline = counts.unsuppressed_exposure_gaps
        + if policy.include_unknowns {
            counts.unknowns
        } else {
            0
        };
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    BadgeSummary {
        kind: BadgeKind::Ripr,
        scope: BadgeScope::Repo,
        basis: BadgeBasis::SeamNative,
        message: headline.to_string(),
        status,
        color,
        counts,
        reason_counts,
        policy,
        warnings: Vec::new(),
    }
}

/// Builds a repo-scoped badge summary from explicit GapRecord projection
/// targets. This path is opt-in: normal repo badges keep using seam-native
/// counts unless a caller supplies a gap decision ledger.
pub(crate) fn repo_gap_ledger_badge_summary_from_json(
    text: &str,
    kind: BadgeKind,
    policy: BadgePolicy,
) -> Result<BadgeSummary, String> {
    let records = gap_decision_ledger::parse_gap_records_json(text)?;
    let projection = match kind {
        BadgeKind::Ripr => "ripr_zero_count",
        BadgeKind::RiprPlus => "ripr_plus_count",
    };
    let target_count = records
        .iter()
        .filter(|record| gap_decision_ledger::projection_eligible(record, projection))
        .count();
    let (status, color) = badge_status_color(target_count, policy.fail_on_nonzero);

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps: target_count,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: 0,
        suppressed_test_efficiency_findings: 0,
        unknowns: 0,
        unknowns_test_efficiency: 0,
        analyzed_findings: 0,
        analyzed_seams: 0,
        analyzed_gap_records: records.len(),
        analyzed_tests: 0,
    };

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }

    Ok(BadgeSummary {
        kind,
        scope: BadgeScope::Repo,
        basis: BadgeBasis::GapDecisionLedger,
        message: target_count.to_string(),
        status,
        color,
        counts,
        reason_counts,
        policy,
        warnings: Vec::new(),
    })
}

pub(super) fn badge_status_color(
    count: usize,
    fail_on_nonzero: bool,
) -> (BadgeStatus, &'static str) {
    if fail_on_nonzero && count > 0 {
        return (BadgeStatus::Fail, "red");
    }
    match count {
        0 => (BadgeStatus::Pass, "brightgreen"),
        1..=3 => (BadgeStatus::Warn, "yellow"),
        _ => (BadgeStatus::Warn, "orange"),
    }
}
