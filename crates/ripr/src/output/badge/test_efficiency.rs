use crate::app::CheckOutput;
use crate::output::suppressions::{SuppressionEntry, apply_test_efficiency_suppressions};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use super::model::{BADGE_REASON_KEYS, BadgeCounts, BadgeKind, BadgePolicy, BadgeSummary};
use super::summaries::{badge_status_color, ripr_badge_summary_with_suppressions};

/// One test-efficiency entry seen by the badge, retained so suppressions
/// can be applied per-`(test, path)` after the report is parsed and so
/// scope-aware aggregation can filter by relationship to the diff.
///
/// `class` is the per-test class string from the test-efficiency report
/// (e.g. `smoke_only`, `likely_vacuous`, `opaque`, `strong_discriminator`).
/// `reached_owners` is the per-test owner list — the same shape used by
/// the analyzer's `Finding.probe.owner.0` (`SymbolId.0`) — so a
/// diff-scope filter can intersect them with the changed/probed owner
/// set without an extra fact-extraction pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestEfficiencyBadgeEntry {
    pub test: String,
    pub path: String,
    pub has_intent: bool,
    pub class: String,
    pub reached_owners: Vec<String>,
}

/// Test-efficiency contribution to the `ripr+` badge. Built by parsing
/// `target/ripr/reports/test-efficiency.json`; the per-test ledger is
/// the source of truth because `declared_intent` exclusion is per-test
/// and cannot be derived from aggregate `class_counts` alone.
///
/// `entries` carries every parsed entry (actionable, intentional,
/// opaque, and visible-only) with its class and reached owners so that
/// diff-scope aggregation can filter and recount without re-parsing.
/// The aggregate counters (`unsuppressed_*`, `intentional_*`,
/// `unknowns_te`) are the **repo-wide** totals; diff-scope aggregation
/// recomputes its own counts from `entries`.
///
/// `actionable_entries` is the legacy projection preserved for the
/// suppression matcher: only actionable, non-intentional entries.
/// Repo-scope aggregation pairs it with the repo-wide totals; diff-scope
/// aggregation derives its own filtered view from `entries`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TestEfficiencyBadgeSummary {
    pub unsuppressed_test_efficiency_findings: usize,
    pub intentional_test_efficiency_findings: usize,
    pub unknowns_test_efficiency: usize,
    pub analyzed_tests: usize,
    pub reason_counts: BTreeMap<&'static str, usize>,
    /// Actionable, non-intentional entries — i.e., the candidate set for
    /// `ripr+` suppression matching under repo scope. Empty when no
    /// test-efficiency entry is actionable.
    pub actionable_entries: Vec<TestEfficiencyBadgeEntry>,
    /// Every parsed entry (actionable, intentional, opaque, and
    /// visible-only) with class and reached owners. Used by diff-scope
    /// aggregation to filter to tests related to the changed code.
    pub entries: Vec<TestEfficiencyBadgeEntry>,
}

/// The test-efficiency `class` strings that contribute to `ripr+` when not
/// covered by `declared_intent`. Mirrors the locked vocabulary in
/// `docs/BADGE_POLICY.md`. `strong_discriminator` and `useful_but_broad`
/// never count by default; `opaque` flows into `unknowns_test_efficiency`
/// rather than the headline.
const ACTIONABLE_TE_CLASSES: &[&str] = &[
    "likely_vacuous",
    "possibly_circular",
    "smoke_only",
    "duplicative",
];

const NON_ACTIONABLE_TE_CLASSES: &[&str] = &["strong_discriminator", "useful_but_broad"];

/// Parses `target/ripr/reports/test-efficiency.json` into the
/// `ripr+`-shaped summary. Validates the schema_version, requires the
/// per-test ledger, and rejects unknown class strings so a class name
/// drift in the emitter surfaces as a parse error rather than a silent
/// undercount.
pub fn parse_test_efficiency_badge_summary(
    text: &str,
) -> Result<TestEfficiencyBadgeSummary, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("test-efficiency.json is not valid JSON: {err}"))?;

    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| "test-efficiency.json is missing `schema_version`".to_string())?;
    if schema_version != "0.1" {
        return Err(format!(
            "test-efficiency.json schema_version `{schema_version}` is not supported (expected `0.1`)"
        ));
    }

    let tests = value
        .get("tests")
        .and_then(Value::as_array)
        .ok_or_else(|| "test-efficiency.json is missing the `tests` array".to_string())?;

    let mut unsuppressed = 0usize;
    let mut intentional = 0usize;
    let mut unknowns_te = 0usize;
    let mut actionable_entries: Vec<TestEfficiencyBadgeEntry> = Vec::new();
    let mut all_entries: Vec<TestEfficiencyBadgeEntry> = Vec::new();

    for entry in tests {
        let class = entry
            .get("class")
            .and_then(Value::as_str)
            .ok_or_else(|| "test-efficiency entry is missing `class`".to_string())?;
        let has_intent = entry.get("declared_intent").is_some();

        let test_name = entry
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let path = entry
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let reached_owners: Vec<String> = entry
            .get("reached_owners")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        if ACTIONABLE_TE_CLASSES.contains(&class) {
            if has_intent {
                intentional += 1;
            } else {
                unsuppressed += 1;
                actionable_entries.push(TestEfficiencyBadgeEntry {
                    test: test_name.clone(),
                    path: path.clone(),
                    has_intent: false,
                    class: class.to_string(),
                    reached_owners: reached_owners.clone(),
                });
            }
        } else if class == "opaque" {
            unknowns_te += 1;
        } else if NON_ACTIONABLE_TE_CLASSES.contains(&class) {
            // strong_discriminator / useful_but_broad: visible only.
        } else {
            return Err(format!(
                "test-efficiency entry has unknown class `{class}`; recognized classes are {}",
                [
                    ACTIONABLE_TE_CLASSES,
                    NON_ACTIONABLE_TE_CLASSES,
                    &["opaque"],
                ]
                .concat()
                .join(", ")
            ));
        }

        all_entries.push(TestEfficiencyBadgeEntry {
            test: test_name,
            path,
            has_intent,
            class: class.to_string(),
            reached_owners,
        });
    }

    let analyzed_tests = value
        .get("metrics")
        .and_then(|m| m.get("tests_scanned"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "test-efficiency.json is missing `metrics.tests_scanned`".to_string())?
        as usize;

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }
    if let Some(counts) = value
        .get("metrics")
        .and_then(|m| m.get("reason_counts"))
        .and_then(Value::as_object)
    {
        for (key, value) in counts {
            if let Some(known) = BADGE_REASON_KEYS
                .iter()
                .find(|known| **known == key.as_str())
                && let Some(count) = value.as_u64()
            {
                reason_counts.insert(*known, count as usize);
            }
        }
    }

    Ok(TestEfficiencyBadgeSummary {
        unsuppressed_test_efficiency_findings: unsuppressed,
        intentional_test_efficiency_findings: intentional,
        unknowns_test_efficiency: unknowns_te,
        analyzed_tests,
        reason_counts,
        actionable_entries,
        entries: all_entries,
    })
}

/// The set of tests + owners considered "related to the diff" for
/// scope-aware `ripr+` aggregation. Built from `CheckOutput.findings`:
///
/// - `related_test_keys` contains both the bare test name and a
///   `<path>::<name>` qualified form so the filter can match either
///   shape from the test-efficiency report.
/// - `changed_owners` is the set of owner symbol strings extracted from
///   `Finding.probe.owner` — same shape as the test-efficiency JSON's
///   `reached_owners` field.
///
/// A test-efficiency entry is *related* to the diff if either:
/// 1. its bare or qualified name appears in `related_test_keys`, or
/// 2. its `reached_owners` intersect `changed_owners`.
#[derive(Clone, Debug, Default)]
pub struct DiffRelatedTests {
    pub related_test_keys: BTreeSet<String>,
    pub changed_owners: BTreeSet<String>,
}

impl DiffRelatedTests {
    pub fn from_check_output(output: &CheckOutput) -> Self {
        let mut related_test_keys = BTreeSet::new();
        let mut changed_owners = BTreeSet::new();
        for finding in &output.findings {
            if let Some(owner) = finding.probe.owner.as_ref() {
                changed_owners.insert(owner.0.clone());
            }
            for test in &finding.related_tests {
                let path = test.file.to_string_lossy().into_owned();
                related_test_keys.insert(test.name.clone());
                related_test_keys.insert(format!("{}::{}", path, test.name));
            }
        }
        Self {
            related_test_keys,
            changed_owners,
        }
    }

    fn includes(&self, entry: &TestEfficiencyBadgeEntry) -> bool {
        if self.related_test_keys.contains(&entry.test) {
            return true;
        }
        let qualified = format!("{}::{}", entry.path, entry.test);
        if self.related_test_keys.contains(&qualified) {
            return true;
        }
        entry
            .reached_owners
            .iter()
            .any(|owner| self.changed_owners.contains(owner))
    }
}

/// Aggregation scope for the `ripr+` test-efficiency contribution.
/// `cargo xtask test-efficiency-report` is repo-wide as a fact source;
/// badge aggregation must be scope-aware so a PR badge is not noisy
/// with unrelated whole-repo test-efficiency debt.
#[derive(Clone, Debug)]
pub enum TestEfficiencyAggregationScope<'a> {
    /// Repo-scoped aggregation: count every entry from the repo-wide
    /// ledger (current behavior, used by `repo-badge-plus-*` formats).
    Repo,
    /// Diff-scoped aggregation: filter to entries whose tests appear in
    /// the diff's related-tests set or whose `reached_owners` intersect
    /// the diff's changed/probed owners.
    Diff(&'a DiffRelatedTests),
}

/// Builds the `ripr+` badge summary from a `CheckOutput` plus a parsed
/// test-efficiency contribution and a slice of suppressions. Applies
/// `exposure_gap` suppressions to the exposure side and
/// `test_efficiency` suppressions to the actionable test-efficiency
/// entries; expired and unmatched selectors surface as `warnings`.
///
/// `scope` controls whether the test-efficiency contribution comes
/// from the repo-wide ledger (`Repo`) or is filtered to entries
/// related to the diff under analysis (`Diff`). The exposure side is
/// already scope-aware via the underlying `CheckOutput` (built by the
/// diff or repo analysis), so only the test-efficiency contribution
/// needs scope-awareness here.
pub fn ripr_plus_badge_summary_with_suppressions(
    output: &CheckOutput,
    test_efficiency: TestEfficiencyBadgeSummary,
    suppressions: &[SuppressionEntry],
    today: &str,
    policy: BadgePolicy,
    scope: TestEfficiencyAggregationScope<'_>,
) -> BadgeSummary {
    let exposure =
        ripr_badge_summary_with_suppressions(output, suppressions, today, policy.clone());
    ripr_plus_badge_summary_from_exposure(
        exposure,
        test_efficiency,
        suppressions,
        today,
        policy,
        scope,
    )
}

/// Builds repo-scoped `ripr+` from the canonical actionable repair basis.
///
/// Test-efficiency entries are parsed so the report remains validated and
/// `analyzed_tests` / reason counts stay visible, but raw test-efficiency debt
/// does not move the public headline until a later producer lifts those items
/// into the same repair / verify / receipt model as canonical gaps.
pub(crate) fn ripr_plus_canonical_actionable_gap_badge_summary(
    mut exposure: BadgeSummary,
    test_efficiency: TestEfficiencyBadgeSummary,
    policy: BadgePolicy,
) -> BadgeSummary {
    let counts = BadgeCounts {
        unsuppressed_exposure_gaps: exposure.counts.unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: exposure.counts.suppressed_exposure_gaps,
        suppressed_test_efficiency_findings: 0,
        unknowns: exposure.counts.unknowns,
        unknowns_test_efficiency: 0,
        analyzed_findings: exposure.counts.analyzed_findings,
        analyzed_seams: exposure.counts.analyzed_seams,
        analyzed_gap_records: exposure.counts.analyzed_gap_records,
        analyzed_tests: test_efficiency.analyzed_tests,
    };
    let unknown_contribution = if policy.include_unknowns {
        counts.unknowns + counts.unknowns_test_efficiency
    } else {
        0
    };
    let headline = counts.unsuppressed_exposure_gaps
        + counts.unsuppressed_test_efficiency_findings
        + unknown_contribution;
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    exposure.kind = BadgeKind::RiprPlus;
    exposure.message = headline.to_string();
    exposure.status = status;
    exposure.color = color;
    exposure.counts = counts;
    exposure.reason_counts = test_efficiency.reason_counts;
    exposure.policy = policy;
    exposure
}

fn ripr_plus_badge_summary_from_exposure(
    exposure: BadgeSummary,
    test_efficiency: TestEfficiencyBadgeSummary,
    suppressions: &[SuppressionEntry],
    today: &str,
    policy: BadgePolicy,
    scope: TestEfficiencyAggregationScope<'_>,
) -> BadgeSummary {
    // Decide which entries contribute to this scope's headline. For
    // repo scope, take the parser's pre-computed repo-wide totals and
    // the existing actionable list. For diff scope, recompute counts
    // from the filtered entry list — `related_test_keys` from
    // `Finding.related_tests` and `changed_owners` from
    // `Finding.probe.owner` are the only inputs.
    let (actionable_pairs, unsuppressed_te_before_suppression, intentional_te, unknowns_te) =
        match scope {
            TestEfficiencyAggregationScope::Repo => (
                test_efficiency
                    .actionable_entries
                    .iter()
                    .map(|entry| (entry.test.clone(), entry.path.clone()))
                    .collect::<Vec<_>>(),
                test_efficiency.unsuppressed_test_efficiency_findings,
                test_efficiency.intentional_test_efficiency_findings,
                test_efficiency.unknowns_test_efficiency,
            ),
            TestEfficiencyAggregationScope::Diff(filter) => {
                let mut pairs: Vec<(String, String)> = Vec::new();
                let mut unsuppressed_count = 0usize;
                let mut intentional_count = 0usize;
                let mut unknowns_count = 0usize;
                for entry in &test_efficiency.entries {
                    if !filter.includes(entry) {
                        continue;
                    }
                    if ACTIONABLE_TE_CLASSES.contains(&entry.class.as_str()) {
                        if entry.has_intent {
                            intentional_count += 1;
                        } else {
                            unsuppressed_count += 1;
                            pairs.push((entry.test.clone(), entry.path.clone()));
                        }
                    } else if entry.class == "opaque" {
                        unknowns_count += 1;
                    }
                    // strong_discriminator / useful_but_broad stay visible only.
                }
                (pairs, unsuppressed_count, intentional_count, unknowns_count)
            }
        };

    // Apply test-efficiency suppressions against the (scope-filtered)
    // candidate pairs. Suppressed entries shift from
    // `unsuppressed_test_efficiency_findings` to
    // `suppressed_test_efficiency_findings`. `intentional_*` is
    // unaffected — declared intent and suppressions are distinct.
    let te_application = apply_test_efficiency_suppressions(&actionable_pairs, suppressions, today);
    let suppressed_te = te_application.suppressed_tests.len();
    let unsuppressed_te = unsuppressed_te_before_suppression.saturating_sub(suppressed_te);

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps: exposure.counts.unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: unsuppressed_te,
        intentional_test_efficiency_findings: intentional_te,
        suppressed_exposure_gaps: exposure.counts.suppressed_exposure_gaps,
        suppressed_test_efficiency_findings: suppressed_te,
        unknowns: exposure.counts.unknowns,
        unknowns_test_efficiency: unknowns_te,
        analyzed_findings: exposure.counts.analyzed_findings,
        analyzed_seams: exposure.counts.analyzed_seams,
        analyzed_gap_records: exposure.counts.analyzed_gap_records,
        analyzed_tests: test_efficiency.analyzed_tests,
    };

    let unknown_contribution = if policy.include_unknowns {
        counts.unknowns + counts.unknowns_test_efficiency
    } else {
        0
    };
    let headline = counts.unsuppressed_exposure_gaps
        + counts.unsuppressed_test_efficiency_findings
        + unknown_contribution;
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    let mut warnings = exposure.warnings;
    warnings.extend(te_application.warnings);

    BadgeSummary {
        kind: BadgeKind::RiprPlus,
        scope: exposure.scope,
        basis: exposure.basis,
        message: headline.to_string(),
        status,
        color,
        counts,
        reason_counts: test_efficiency.reason_counts,
        policy,
        warnings,
    }
}

/// Convenience wrapper: builds the `ripr+` badge with no suppressions
/// and **repo** aggregation scope. Test-only — production calls
/// [`ripr_plus_badge_summary_with_suppressions`] directly via
/// [`crate::app::render_check`], which threads the right scope from
/// the requested `OutputFormat`.
#[cfg(test)]
pub fn ripr_plus_badge_summary(
    output: &CheckOutput,
    test_efficiency: TestEfficiencyBadgeSummary,
    policy: BadgePolicy,
) -> BadgeSummary {
    ripr_plus_badge_summary_with_suppressions(
        output,
        test_efficiency,
        &[],
        "",
        policy,
        TestEfficiencyAggregationScope::Repo,
    )
}
