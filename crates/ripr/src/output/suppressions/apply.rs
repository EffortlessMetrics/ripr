//! Suppression application against candidate finding ids and test
//! entries. Pure functions: no I/O, no clock — the caller supplies
//! `today` so tests can pass synthetic values.

use super::{SuppressionApplication, SuppressionEntry, SuppressionKind, is_expired};

/// Applies exposure-gap suppressions against a slice of finding ids that
/// the caller already considered "candidate exposure gaps" (i.e. the
/// classes counted by `ripr_badge_summary`). Unmatched selectors and
/// expired entries surface as warnings; expired entries are not applied
/// so they cannot silently keep the badge green.
pub fn apply_exposure_suppressions(
    candidate_finding_ids: &[String],
    suppressions: &[SuppressionEntry],
    today: &str,
) -> SuppressionApplication {
    let mut app = SuppressionApplication::default();
    let candidate_set: std::collections::BTreeSet<&str> =
        candidate_finding_ids.iter().map(String::as_str).collect();

    for entry in suppressions
        .iter()
        .filter(|e| e.kind == SuppressionKind::ExposureGap)
    {
        let Some(id) = &entry.finding_id else {
            continue;
        };
        if is_expired(entry.expires.as_deref(), today) {
            app.warnings.push(format!(
                "expired {} suppression for `{id}` (expired on {})",
                entry.kind.as_str(),
                entry.expires.as_deref().unwrap_or("unknown")
            ));
            continue;
        }
        if !candidate_set.contains(id.as_str()) {
            app.warnings.push(format!(
                "{} suppression for `{id}` did not match any current exposure-gap finding",
                entry.kind.as_str()
            ));
            continue;
        }
        app.suppressed_findings.insert(id.clone());
    }
    app
}

/// Applies test-efficiency suppressions against a slice of `(name, path)`
/// pairs from the test-efficiency report. Unmatched selectors and expired
/// entries surface as warnings; expired entries are not applied.
pub fn apply_test_efficiency_suppressions(
    candidate_entries: &[(String, String)],
    suppressions: &[SuppressionEntry],
    today: &str,
) -> SuppressionApplication {
    let mut app = SuppressionApplication::default();
    for entry in suppressions
        .iter()
        .filter(|e| e.kind == SuppressionKind::TestEfficiency)
    {
        let Some(test_name) = &entry.test else {
            continue;
        };
        let key_label = match &entry.path {
            Some(p) => format!("`{}` at `{}`", test_name, p),
            None => format!("`{}`", test_name),
        };
        if is_expired(entry.expires.as_deref(), today) {
            app.warnings.push(format!(
                "expired {} suppression for {key_label} (expired on {})",
                entry.kind.as_str(),
                entry.expires.as_deref().unwrap_or("unknown")
            ));
            continue;
        }
        let matches: Vec<&(String, String)> = candidate_entries
            .iter()
            .filter(|(name, path)| {
                name == test_name && entry.path.as_ref().map(|p| p == path).unwrap_or(true)
            })
            .collect();
        if matches.is_empty() {
            app.warnings.push(format!(
                "{} suppression for {key_label} did not match any test-efficiency entry",
                entry.kind.as_str()
            ));
            continue;
        }
        for (name, path) in matches {
            // Always store the actual entry path so name-only suppressions
            // matching multiple files dedupe per-(name, path) — i.e., the
            // count reflects distinct tests suppressed, not distinct
            // selectors.
            app.suppressed_tests
                .insert((name.clone(), Some(path.clone())));
        }
    }
    app
}
