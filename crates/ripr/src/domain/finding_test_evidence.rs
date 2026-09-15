//! Producer-backed `TestEvidenceSummary` projection for diff findings.
//!
//! This is a bounded convergence seam for #1658/#3160. It gives the
//! diff path a portable summary while preserving the authority boundary that
//! a `Finding` does not carry repo-seam `TestTargetEvidence`.

use super::{Finding, TestEvidenceEntry, TestEvidenceSummary};

impl Finding {
    /// Project this finding's related-test and discriminator facts into the
    /// shared portable test-evidence summary.
    ///
    /// The canonical gap ID is preferred when available. Related-test paths
    /// are normalized for transport, and no test target is invented from a
    /// name/path/line tuple: `has_test_target` remains false until a producer
    /// with target authority supplies that identity.
    pub fn test_evidence_summary(&self) -> TestEvidenceSummary {
        let seam_id = self
            .canonical_gap
            .as_ref()
            .map(|gap| gap.id.clone())
            .unwrap_or_else(|| self.id.clone());
        let entries = self
            .related_tests
            .iter()
            .map(|test| TestEvidenceEntry {
                test_name: test.name.clone(),
                file: normalize_path(&test.file.to_string_lossy()),
                line: test.line,
                oracle_kind: test.oracle_kind.as_str().to_string(),
                oracle_strength: test.oracle_strength.as_str().to_string(),
                relation_reason: test
                    .relation_reason
                    .as_ref()
                    .map_or("unknown", |reason| reason.as_str())
                    .to_string(),
                // A diff Finding has no producer-owned TestTargetEvidence.
                // Preserve that absence instead of inferring authority from a
                // path/name/line tuple.
                has_test_target: false,
            })
            .collect::<Vec<_>>();
        let missing = self
            .activation
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.clone())
            .collect::<Vec<_>>();

        TestEvidenceSummary::from_parts(seam_id, entries, &missing)
    }
}

fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized.trim_start_matches("./").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, FindingCanonicalGap,
        MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RelationReason, RevealEvidence, RiprEvidence, SourceCurrentness,
        SourceLocation, StageEvidence, StageState,
    };
    use std::path::PathBuf;

    fn stage() -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Unknown, "fixture")
    }

    fn ripr_evidence() -> RiprEvidence {
        RiprEvidence {
            reach: stage(),
            infect: stage(),
            propagate: stage(),
            reveal: RevealEvidence {
                observe: stage(),
                discriminate: stage(),
            },
        }
    }

    fn related_test(
        name: &str,
        file: &str,
        line: usize,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
        relation_reason: Option<RelationReason>,
    ) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: PathBuf::from(file),
            line,
            oracle: None,
            oracle_kind,
            oracle_strength,
            relation_confidence: relation_reason.map(RelationReason::confidence),
            relation_reason,
        }
    }

    fn finding(
        canonical_gap_id: Option<&str>,
        related_tests: Vec<RelatedTest>,
        missing_discriminators: &[&str],
    ) -> Finding {
        Finding {
            id: "finding:raw".to_string(),
            canonical_gap: canonical_gap_id.map(|id| FindingCanonicalGap {
                id: id.to_string(),
                language: "rust".to_string(),
                file: "src/pricing.rs".to_string(),
                owner: "pricing::discounted_total".to_string(),
                behavior_kind: "predicate_boundary".to_string(),
                probe_kind: "predicate".to_string(),
                normalized_discriminator: "amount == threshold".to_string(),
            }),
            probe: Probe {
                id: ProbeId("probe:pricing".to_string()),
                location: SourceLocation::new("src/pricing.rs", 12, 5),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("amount > threshold".to_string()),
                after: Some("amount >= threshold".to_string()),
                expression: "amount >= threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: ripr_evidence(),
            confidence: 0.5,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: missing_discriminators
                    .iter()
                    .map(|value| MissingDiscriminatorFact {
                        value: (*value).to_string(),
                        reason: "fixture".to_string(),
                        flow_sink: None,
                    })
                    .collect(),
            },
            stop_reasons: Vec::new(),
            related_tests,
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: SourceCurrentness::CandidateCurrent,
        }
    }

    #[test]
    fn finding_projection_prefers_canonical_identity_and_does_not_invent_targets() {
        let finding = finding(
            Some("gap:pricing:boundary"),
            vec![
                related_test(
                    "broad_case",
                    "./tests\\pricing.rs",
                    9,
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                    None,
                ),
                related_test(
                    "exact_case",
                    "tests\\pricing.rs",
                    12,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                    Some(RelationReason::DirectOwnerCall),
                ),
            ],
            &[" amount   == threshold ", "amount == threshold"],
        );

        let summary = finding.test_evidence_summary();

        assert_eq!(summary.seam_id, "gap:pricing:boundary");
        assert_eq!(summary.strongest_oracle, "strong");
        assert_eq!(summary.missing_discriminator_count, 1);
        assert_eq!(summary.related_tests.len(), 2);
        assert_eq!(summary.related_tests[0].test_name, "exact_case");
        assert_eq!(summary.related_tests[0].file, "tests/pricing.rs");
        assert_eq!(
            summary.related_tests[0].relation_reason,
            "direct_owner_call"
        );
        assert_eq!(summary.related_tests[1].relation_reason, "unknown");
        assert!(
            summary
                .related_tests
                .iter()
                .all(|entry| !entry.has_test_target),
            "diff findings must not manufacture producer-owned test targets"
        );
        assert!(
            summary.fingerprint.ends_with("missing:amount == threshold"),
            "normalized missing-discriminator identity must enter movement"
        );
    }

    #[test]
    fn finding_projection_falls_back_to_raw_identity_without_a_canonical_gap() {
        let summary = finding(None, Vec::new(), &[]).test_evidence_summary();

        assert_eq!(summary.seam_id, "finding:raw");
        assert!(summary.related_tests.is_empty());
        assert_eq!(summary.missing_discriminator_count, 0);
        assert_eq!(summary.strongest_oracle, "none");
        assert_eq!(summary.fingerprint, "fp:");
    }

    #[test]
    fn finding_projection_fingerprint_ignores_names_paths_lines_and_input_order() {
        let first = finding(
            None,
            vec![
                related_test(
                    "exact_a",
                    "tests/a.rs",
                    10,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                    Some(RelationReason::DirectOwnerCall),
                ),
                related_test(
                    "broad_a",
                    "tests/b.rs",
                    20,
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                    Some(RelationReason::SameTestFile),
                ),
            ],
            &[],
        )
        .test_evidence_summary();
        let second = finding(
            None,
            vec![
                related_test(
                    "renamed_broad",
                    "./different\\b.rs",
                    200,
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                    Some(RelationReason::SameTestFile),
                ),
                related_test(
                    "renamed_exact",
                    "different/a.rs",
                    100,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                    Some(RelationReason::DirectOwnerCall),
                ),
            ],
            &[],
        )
        .test_evidence_summary();

        assert_eq!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn transport_path_normalization_is_separator_and_prefix_only() {
        assert_eq!(normalize_path("tests\\pricing.rs"), "tests/pricing.rs");
        assert_eq!(normalize_path("././tests/pricing.rs"), "tests/pricing.rs");
        assert_eq!(normalize_path("../tests/pricing.rs"), "../tests/pricing.rs");
    }
}
