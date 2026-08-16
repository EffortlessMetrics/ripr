use super::{ExposureClass, Finding, MissingDiscriminatorFact, RelatedTest};

/// The bounded, producer-backed witness projected by passive editor diagnostics.
///
/// This type deliberately contains no renderer-derived assertion or target.  A
/// missing field is retained as a named limitation so a client can distinguish
/// unavailable producer evidence from an empty or guessed value.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticWitness {
    pub kind: String,
    pub probe_family: String,
    pub changed_expression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sink: Option<String>,
    pub missing_discriminators: Vec<MissingDiscriminatorFact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_site: Option<DiagnosticFixSite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_assertion: Option<String>,
    pub explain_command: String,
    pub confidence: DiagnosticConfidence,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<DiagnosticWitnessLimitation>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticFixSite {
    pub file: String,
    pub line: usize,
    pub test_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_oracle: Option<String>,
    pub oracle_kind: String,
    pub oracle_strength: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_location: Option<DiagnosticSourceLocation>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticSourceLocation {
    pub file: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticConfidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f32>,
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticWitnessLimitation {
    pub kind: String,
    pub detail: String,
}

impl DiagnosticWitness {
    /// Build a witness only for findings that represent an unresolved or
    /// weakly observed behavior.  A fully exposed finding is not relabeled as
    /// a gap merely because it has a probe.
    pub fn from_finding(finding: &Finding) -> Option<Self> {
        if matches!(finding.class, ExposureClass::Exposed)
            && finding.activation.missing_discriminators.is_empty()
        {
            return None;
        }

        let best_test = best_related_test(&finding.related_tests);
        // `observed_values` is finding-wide and does not identify the
        // selected related test.  Do not guess an assertion location by
        // matching text; the test's own line remains the only producer-owned
        // fix-site location available until the analyzer supplies oracle
        // identity.
        let oracle_location = None;
        let fix_site = best_test.map(|test| DiagnosticFixSite {
            file: display_path(&test.file),
            line: test.line,
            test_name: test.name.clone(),
            current_oracle: test
                .oracle
                .clone()
                .filter(|oracle| !oracle.trim().is_empty()),
            oracle_kind: test.oracle_kind.as_str().to_string(),
            oracle_strength: test.oracle_strength.as_str().to_string(),
            oracle_location: oracle_location.clone(),
        });

        let mut limitations = Vec::new();
        if finding.activation.missing_discriminators.is_empty() {
            limitations.push(limitation(
                "missing_discriminator_unavailable",
                "The producer did not resolve an exact missing discriminator.",
            ));
        }
        match best_test {
            None => limitations.push(limitation(
                "fix_site_unavailable",
                "The producer did not identify a related test target.",
            )),
            Some(test) if test.oracle.as_deref().is_none_or(str::is_empty) => {
                limitations.push(limitation(
                    "oracle_text_unavailable",
                    "The producer identified a test target but did not provide its oracle text.",
                ));
            }
            Some(_) if oracle_location.is_none() => limitations.push(limitation(
                "oracle_source_location_unavailable",
                "The producer did not provide an exact assertion source location.",
            )),
            Some(_) => {}
        }
        // No Finding field currently carries a canonical, symbol-resolved
        // assertion template.  Keep the limitation explicit until a producer
        // supplies one; do not synthesize it from the missing value or prose.
        limitations.push(limitation(
            "suggested_assertion_unavailable",
            "No producer-owned symbol-resolved assertion template is available.",
        ));

        Some(Self {
            kind: "static_discriminator_gap".to_string(),
            probe_family: finding.probe.family.as_str().to_string(),
            changed_expression: finding.probe.expression.clone(),
            before: finding.probe.before.clone(),
            after: finding.probe.after.clone(),
            expected_sink: finding
                .flow_sinks
                .first()
                .map(|sink| sink.kind.as_str().to_string())
                .or_else(|| finding.probe.expected_sinks.first().cloned()),
            missing_discriminators: finding.activation.missing_discriminators.clone(),
            fix_site,
            suggested_assertion: None,
            explain_command: format!("ripr explain --root . {}", finding.id),
            confidence: DiagnosticConfidence {
                value: finding.confidence.is_finite().then_some(finding.confidence),
                basis: "static_only".to_string(),
            },
            limitations,
        })
    }
}

fn best_related_test(related_tests: &[RelatedTest]) -> Option<&RelatedTest> {
    related_tests.iter().max_by(|left, right| {
        left.oracle_strength
            .rank()
            .cmp(&right.oracle_strength.rank())
            .then_with(|| {
                right
                    .relation_confidence
                    .map_or(3, |confidence| confidence.rank())
                    .cmp(
                        &left
                            .relation_confidence
                            .map_or(3, |confidence| confidence.rank()),
                    )
            })
    })
}

fn display_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn limitation(kind: &str, detail: &str) -> DiagnosticWitnessLimitation {
    DiagnosticWitnessLimitation {
        kind: kind.to_string(),
        detail: detail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
        OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, SymbolId, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn witness_preserves_exact_facts_and_named_missing_assertion_limitations() -> Result<(), String>
    {
        let mut finding = sample_finding();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:rust:pricing:error_path:error_variant:amount".to_string(),
            language: "rust".to_string(),
            file: "src/pricing.rs".to_string(),
            owner: "pricing::discount".to_string(),
            behavior_kind: "error_path".to_string(),
            probe_kind: "error_path".to_string(),
            normalized_discriminator: "PricingError::Boundary".to_string(),
        });
        finding.probe.family = ProbeFamily::ErrorPath;
        finding.probe.before = Some("Err(PricingError::Old)".to_string());
        finding.probe.after = Some("Err(PricingError::Boundary)".to_string());
        finding.probe.expected_sinks = vec!["error_variant".to_string()];
        finding.flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ErrorVariant,
            text: "PricingError::Boundary".to_string(),
            line: 88,
            owner: Some(SymbolId("pricing::discount".to_string())),
        }];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "PricingError::Boundary".to_string(),
            reason: "the broad error oracle does not distinguish the variant".to_string(),
            flow_sink: None,
        }];
        finding.activation.observed_values = vec![ValueFact {
            line: 12,
            text: "assert!(result.is_err())".to_string(),
            value: "result.is_err()".to_string(),
            context: ValueContext::AssertionArgument,
        }];
        finding.related_tests = vec![
            RelatedTest {
                name: "rejects_boundary".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 10,
                oracle: Some("assert!(result.is_err())".to_string()),
                oracle_kind: OracleKind::BroadError,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
            },
            RelatedTest {
                name: "rejects_boundary_in_other_fixture".to_string(),
                file: PathBuf::from("tests/other_pricing.rs"),
                line: 22,
                oracle: Some("assert!(result.is_err())".to_string()),
                oracle_kind: OracleKind::BroadError,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
            },
        ];

        let witness = DiagnosticWitness::from_finding(&finding)
            .ok_or_else(|| "expected weak finding witness".to_string())?;
        assert_eq!(witness.kind, "static_discriminator_gap");
        assert_eq!(witness.expected_sink.as_deref(), Some("error_variant"));
        assert_eq!(
            witness.missing_discriminators[0].value,
            "PricingError::Boundary"
        );
        assert_eq!(witness.fix_site.as_ref().map(|site| site.line), Some(22));
        assert!(
            witness
                .fix_site
                .as_ref()
                .is_some_and(|site| site.oracle_location.is_none())
        );
        assert!(
            witness
                .limitations
                .iter()
                .any(|item| item.kind == "oracle_source_location_unavailable")
        );
        assert!(witness.suggested_assertion.is_none());
        assert!(
            witness
                .limitations
                .iter()
                .any(|item| item.kind == "suggested_assertion_unavailable")
        );
        Ok(())
    }

    #[test]
    fn exposed_finding_without_missing_fact_does_not_become_a_gap_witness() {
        let finding = sample_finding();
        assert!(DiagnosticWitness::from_finding(&finding).is_none());
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:pricing:88:error_path".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:pricing:88:error_path".to_string()),
                location: SourceLocation::new("src/pricing.rs", 88, 1),
                owner: Some(SymbolId("pricing::discount".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "amount >= threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::Exposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "reachable"),
                infect: StageEvidence::new(StageState::Yes, Confidence::High, "infectable"),
                propagate: StageEvidence::new(StageState::Yes, Confidence::Medium, "propagatable"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Yes, Confidence::Medium, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::Yes,
                        Confidence::Medium,
                        "discriminated",
                    ),
                },
            },
            confidence: 0.75,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }
}
