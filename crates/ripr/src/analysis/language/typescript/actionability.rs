//! Actionability analysis for the TypeScript preview adapter.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptActionability {
    pub(crate) gap_state: &'static str,
    pub(crate) category: &'static str,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) missing_fields: Vec<&'static str>,
    pub(crate) evidence_needed: &'static str,
}

impl TypeScriptActionability {
    pub(crate) fn evidence(&self, raw_evidence_ref: String) -> Vec<String> {
        let mut evidence = vec![
            format!("gap_state: {}", self.gap_state),
            format!("actionability_category: {}", self.category),
            format!("why_not_actionable: {}", self.why_not_actionable),
            format!("repair_route: {}", self.repair_route),
        ];
        if !self.missing_fields.is_empty() {
            evidence.push(format!(
                "missing_actionability_fields: {}",
                self.missing_fields.join(", ")
            ));
        }
        evidence.push(format!(
            "evidence_needed_to_promote: {}",
            self.evidence_needed
        ));
        evidence.push(raw_evidence_ref);
        evidence
    }

    pub(crate) fn missing_summary(&self) -> String {
        format!(
            "TypeScript preview actionability `{}` / `{}`: {}. Repair route: {}",
            self.gap_state, self.category, self.why_not_actionable, self.repair_route
        )
    }
}

pub(crate) fn typescript_actionability_for(
    class: &ExposureClass,
    static_limit: Option<&TypeScriptStaticLimit>,
    has_oracle_eligible_relation: bool,
    missing_discriminators: &[MissingDiscriminatorFact],
) -> TypeScriptActionability {
    if let Some(limit) = static_limit {
        return TypeScriptActionability {
            gap_state: "static_limitation",
            category: limit.kind.as_str(),
            why_not_actionable: format!(
                "static limit `{}` prevents bounded TypeScript repair guidance",
                limit.kind.as_str()
            ),
            repair_route: normalize_repair_route(&limit.repair_route),
            missing_fields: Vec::new(),
            evidence_needed: "resolve the named static limit and re-run TypeScript preview evidence extraction",
        };
    }

    if matches!(class, ExposureClass::Exposed) {
        return TypeScriptActionability {
            gap_state: "already_observed",
            category: "strong_oracle_observed",
            why_not_actionable:
                "related Jest/Vitest evidence already has a strong exact oracle; no repair packet should be emitted"
                    .to_string(),
            repair_route:
                "keep the finding advisory preview and verify the existing assertion still targets the changed behavior"
                    .to_string(),
            missing_fields: Vec::new(),
            evidence_needed:
                "none for a repair packet; retain strong related-test evidence as non-actionable context",
        };
    }

    if matches!(class, ExposureClass::NoStaticPath) {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "missing_context",
            why_not_actionable:
                "no trusted related Jest/Vitest test or observer is available for a bounded TypeScript repair route"
                    .to_string(),
            repair_route:
                "add trusted related-test matching for this owner shape before emitting a repair packet"
                    .to_string(),
            missing_fields: vec![
                "related_test_or_observer",
                "target_test_shape",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "trusted related test or observer, target shape, verify command, receipt command, and edit boundaries",
        };
    }

    if !has_oracle_eligible_relation {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "ambiguous_related_test",
            why_not_actionable:
                "related-test link is heuristic-only and cannot safely borrow extracted assertions as proof"
                    .to_string(),
            repair_route:
                "add a direct owner-call, import-aware, or receiver-aware relation before repair packet projection"
                    .to_string(),
            missing_fields: vec![
                "related_test_or_observer",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "trusted token-aware relation plus complete verify, receipt, and edit-boundary fields",
        };
    }

    if missing_discriminators.is_empty() {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "missing_target_shape",
            why_not_actionable:
                "TypeScript preview found related test evidence but cannot name a safe target discriminator or observer shape"
                    .to_string(),
            repair_route:
                "add probe-specific discriminator extraction for this expression before repair packet projection"
                    .to_string(),
            missing_fields: vec![
                "repair_kind",
                "target_test_shape",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "safe probe discriminator, repair kind, target shape, verify command, receipt command, and edit boundaries",
        };
    }

    TypeScriptActionability {
        gap_state: "advisory",
        category: "incomplete_repair_packet",
        why_not_actionable:
            "TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract"
                .to_string(),
        repair_route:
            "project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available"
                .to_string(),
        missing_fields: vec![
            "canonical_gap_id",
            "repair_kind",
            "target_test_shape",
            "related_test_or_observer",
            "verify_command",
            "receipt_command",
            "must_not_change",
            "allowed_edit_surface",
            "raw_evidence_refs",
        ],
        evidence_needed:
            "canonical gap identity, repair kind, target test shape, related observer, verify command, receipt command, raw evidence refs, and edit constraints",
    }
}

pub(crate) fn normalize_repair_route(route: &str) -> String {
    route
        .strip_prefix("Repair route: ")
        .unwrap_or(route)
        .trim_end_matches('.')
        .to_string()
}

pub(crate) fn typescript_raw_evidence_ref(
    file: &Path,
    line: usize,
    owner: Option<&TypeScriptOwner>,
    source_id: &str,
) -> String {
    let mut parts = vec![
        format!("file={}", normalized_path(file)),
        format!("line={line}"),
        "kind=typescript_preview_probe".to_string(),
        format!("source_id={source_id}"),
    ];
    if let Some(owner) = owner {
        parts.push(format!("owner={}", owner.name));
    }
    format!("raw_evidence_ref: {}", parts.join(";"))
}
