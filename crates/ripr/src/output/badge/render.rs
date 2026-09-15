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
    match &summary.analysis_outcome {
        Some(outcome) => {
            out.push_str(&format!(
                "  \"analysis_complete\": {},\n",
                outcome.kind.is_complete()
            ));
            let rendered = serde_json::to_string(outcome).unwrap_or_else(|error| {
                format!(
                    "{{\"serialization_error\":\"{}\"}}",
                    json_escape(&error.to_string())
                )
            });
            out.push_str(&format!("  \"analysis_outcome\": {rendered},\n"));
        }
        None => {
            out.push_str("  \"analysis_complete\": null,\n");
            out.push_str("  \"analysis_outcome\": null,\n");
        }
    }

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
        out.push_str("],\n");
    } else {
        out.push('\n');
        for (index, warning) in summary.warnings.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&format!("    \"{}\"", json_escape(warning)));
        }
        out.push_str("\n  ],\n");
    }

    // Always emit `preview_skipped` as an array so consumers can detect when
    // a preview-language diff was not analyzed and the badge is not a clean
    // Rust-grade result. Non-empty only when at least one preview-language
    // adapter was detected but NOT enabled (v0.6 contract field).
    //
    // The typed diff outcome fields (v0.8) precede `preview_skipped`. The
    // `public_projection` object (v0.7) follows only on repo-scoped public
    // badges; when present `preview_skipped` keeps a trailing comma and the
    // projection closes the object.
    out.push_str("  \"preview_skipped\": [");
    if summary.preview_skipped.is_empty() {
        out.push(']');
    } else {
        out.push('\n');
        for (index, lang) in summary.preview_skipped.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&format!("    \"{}\"", json_escape(lang)));
        }
        out.push_str("\n  ]");
    }
    if let Some(projection) = &summary.projection {
        out.push_str(",\n");
        out.push_str(&render_public_projection(projection));
    }
    out.push_str("\n}\n");
    out
}

/// Renders the `public_projection` object (RIPR-SPEC-0066): the closed public
/// badge state plus the six required sidecar fields. Optional fields render
/// as JSON `null` when absent so the object shape is stable.
fn render_public_projection(
    projection: &super::public_projection::PublicBadgeProjection,
) -> String {
    let mut out = String::new();
    out.push_str("  \"public_projection\": {\n");
    out.push_str(&format!(
        "    \"state\": \"{}\",\n",
        projection.state.as_str()
    ));
    out.push_str(&format!(
        "    \"message\": \"{}\",\n",
        json_escape(&projection.shields_message)
    ));
    out.push_str(&format!(
        "    \"run_status\": \"{}\",\n",
        json_escape(&projection.run_status)
    ));
    out.push_str(&format!(
        "    \"generated_at\": {},\n",
        json_string_or_null(projection.generated_at.as_deref())
    ));
    out.push_str(&format!(
        "    \"actionable_count\": {},\n",
        projection
            .actionable_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "    \"limited_reason\": {},\n",
        json_string_or_null(projection.limited_reason.as_deref())
    ));
    out.push_str(&format!(
        "    \"stale_age_secs\": {},\n",
        projection
            .stale_age_secs
            .map(|secs| secs.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "    \"source_report\": {}\n",
        json_string_or_null(projection.source_report.as_deref())
    ));
    out.push_str("  }");
    out
}

/// Renders an optional string as a quoted JSON string or `null`.
fn json_string_or_null(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("\"{}\"", json_escape(text)),
        None => "null".to_string(),
    }
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
