use crate::output::json::escape as json_escape;

use super::model::{BADGE_REASON_KEYS, BADGE_SCHEMA_VERSION, BadgeSummary};

/// Renders the native badge JSON (snake_case, full counts/reasons/policy).
pub fn render_native_json(summary: &BadgeSummary) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{BADGE_SCHEMA_VERSION}\",\n"
    ));
    out.push_str(&format!("  \"kind\": \"{}\",\n", summary.kind.as_str()));
    out.push_str(&format!("  \"scope\": \"{}\",\n", summary.scope.as_str()));
    out.push_str(&format!("  \"basis\": \"{}\",\n", summary.basis.as_str()));
    out.push_str(&format!(
        "  \"label\": \"{}\",\n",
        json_escape(summary.kind.label())
    ));
    out.push_str(&format!(
        "  \"message\": \"{}\",\n",
        json_escape(&summary.message)
    ));
    out.push_str(&format!("  \"status\": \"{}\",\n", summary.status.as_str()));
    out.push_str(&format!("  \"color\": \"{}\",\n", summary.color));

    let counts = &summary.counts;
    out.push_str("  \"counts\": {\n");
    out.push_str(&format!(
        "    \"unsuppressed_exposure_gaps\": {},\n",
        counts.unsuppressed_exposure_gaps
    ));
    out.push_str(&format!(
        "    \"unsuppressed_test_efficiency_findings\": {},\n",
        counts.unsuppressed_test_efficiency_findings
    ));
    out.push_str(&format!(
        "    \"intentional_test_efficiency_findings\": {},\n",
        counts.intentional_test_efficiency_findings
    ));
    out.push_str(&format!(
        "    \"suppressed_exposure_gaps\": {},\n",
        counts.suppressed_exposure_gaps
    ));
    out.push_str(&format!(
        "    \"suppressed_test_efficiency_findings\": {},\n",
        counts.suppressed_test_efficiency_findings
    ));
    out.push_str(&format!("    \"unknowns\": {},\n", counts.unknowns));
    out.push_str(&format!(
        "    \"unknowns_test_efficiency\": {},\n",
        counts.unknowns_test_efficiency
    ));
    out.push_str(&format!(
        "    \"analyzed_findings\": {},\n",
        counts.analyzed_findings
    ));
    out.push_str(&format!(
        "    \"analyzed_seams\": {},\n",
        counts.analyzed_seams
    ));
    out.push_str(&format!(
        "    \"analyzed_gap_records\": {},\n",
        counts.analyzed_gap_records
    ));
    out.push_str(&format!(
        "    \"analyzed_tests\": {}\n",
        counts.analyzed_tests
    ));
    out.push_str("  },\n");

    out.push_str("  \"reason_counts\": {");
    if summary.reason_counts.is_empty() {
        out.push_str("},\n");
    } else {
        out.push('\n');
        // Render in the canonical order the badge reserves, not BTreeMap
        // alpha order, so consumers see the policy-aligned sequence.
        let mut wrote_any = false;
        for key in BADGE_REASON_KEYS {
            if let Some(count) = summary.reason_counts.get(*key) {
                if wrote_any {
                    out.push_str(",\n");
                }
                out.push_str(&format!("    \"{}\": {}", json_escape(key), count));
                wrote_any = true;
            }
        }
        out.push_str("\n  },\n");
    }

    let policy = &summary.policy;
    out.push_str("  \"policy\": {\n");
    out.push_str(&format!(
        "    \"include_unknowns\": {},\n",
        policy.include_unknowns
    ));
    out.push_str(&format!(
        "    \"fail_on_nonzero\": {},\n",
        policy.fail_on_nonzero
    ));
    out.push_str(&format!(
        "    \"test_intent_path\": \"{}\",\n",
        json_escape(&policy.test_intent_path)
    ));
    out.push_str(&format!(
        "    \"suppressions_path\": \"{}\"\n",
        json_escape(&policy.suppressions_path)
    ));
    out.push_str("  },\n");

    // Always emit `warnings` as an array (possibly empty) so consumers
    // can rely on a stable shape. Currently used for expired
    // suppressions and unmatched suppression selectors.
    out.push_str("  \"warnings\": [");
    if summary.warnings.is_empty() {
        out.push_str("]\n}\n");
    } else {
        out.push('\n');
        for (index, warning) in summary.warnings.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&format!("    \"{}\"", json_escape(warning)));
        }
        out.push_str("\n  ]\n}\n");
    }
    out
}

/// Renders the Shields-compatible projection: exactly four top-level
/// fields (`schemaVersion`, `label`, `message`, `color`).
pub fn render_shields_json(summary: &BadgeSummary) -> String {
    format!(
        "{{\n  \"schemaVersion\": 1,\n  \"label\": \"{}\",\n  \"message\": \"{}\",\n  \"color\": \"{}\"\n}}\n",
        json_escape(summary.kind.label()),
        json_escape(&summary.message),
        summary.color
    )
}
