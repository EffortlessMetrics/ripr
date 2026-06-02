use crate::domain::{ExposureClass, Finding, LanguageId, ProbeFamily};

pub(crate) fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        python_preview_rank(a)
            .cmp(&python_preview_rank(b))
            .then(a.probe.location.file.cmp(&b.probe.location.file))
            .then(a.probe.location.line.cmp(&b.probe.location.line))
            .then(a.probe.family.as_str().cmp(b.probe.family.as_str()))
    });
}

fn python_preview_rank(finding: &Finding) -> u8 {
    if finding.language != Some(LanguageId::Python) {
        return 0;
    }

    if is_repairable_python_finding(finding) {
        let mut rank = 0;
        if !python_owner_is_public(finding) {
            rank += 4;
        }
        if !has_direct_python_test_relation(finding) {
            rank += 8;
        }
        if !has_python_verify_command(finding) {
            rank += 2;
        }
        if !is_core_python_repair_family(&finding.probe.family) {
            rank += 4;
        }
        return rank;
    }

    match finding.class {
        ExposureClass::Exposed => 40,
        ExposureClass::WeaklyExposed => 50,
        ExposureClass::ReachableUnrevealed => 55,
        ExposureClass::NoStaticPath => 60,
        ExposureClass::InfectionUnknown | ExposureClass::PropagationUnknown => 70,
        ExposureClass::StaticUnknown => 80,
    }
}

fn is_repairable_python_finding(finding: &Finding) -> bool {
    finding.class == ExposureClass::WeaklyExposed
        && finding.static_limit_kind.is_none()
        && finding.canonical_gap.is_some()
        && finding.recommended_next_step.is_some()
        && !finding.activation.missing_discriminators.is_empty()
}

fn python_owner_is_public(finding: &Finding) -> bool {
    let Some(owner) = &finding.probe.owner else {
        return false;
    };
    let owner_path = owner.0.rsplit("::").next().unwrap_or(owner.0.as_str());
    let owner_name = owner_path.rsplit('.').next().unwrap_or(owner_path);
    !owner_name.starts_with('_')
}

fn has_direct_python_test_relation(finding: &Finding) -> bool {
    finding.evidence.iter().any(|entry| {
        entry.starts_with("related_test_relation: syntactic_call")
            || entry.starts_with("related_test_relation: import_alias_call")
    })
}

fn has_python_verify_command(finding: &Finding) -> bool {
    finding
        .evidence
        .iter()
        .any(|entry| entry.starts_with("test_verify_command: "))
}

fn is_core_python_repair_family(family: &ProbeFamily) -> bool {
    matches!(
        family,
        ProbeFamily::Predicate
            | ProbeFamily::ReturnValue
            | ProbeFamily::ErrorPath
            | ProbeFamily::FieldConstruction
            | ProbeFamily::SideEffect
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, FindingCanonicalGap, MissingDiscriminatorFact,
        OwnerKind, Probe, ProbeFamily, ProbeId, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, StaticLimitKind, SymbolId,
    };

    fn stage() -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Unknown, "sort test")
    }

    fn finding(id: &str, file: &str, line: usize, family: ProbeFamily) -> Finding {
        Finding {
            id: id.to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId(format!("probe-{id}")),
                location: SourceLocation::new(file, line, 1),
                owner: None,
                family,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "value > threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(),
                infect: stage(),
                propagate: stage(),
                reveal: RevealEvidence {
                    observe: stage(),
                    discriminate: stage(),
                },
            },
            confidence: 0.0,
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
        }
    }

    fn python_finding(
        id: &str,
        file: &str,
        line: usize,
        family: ProbeFamily,
        class: ExposureClass,
    ) -> Finding {
        let mut finding = finding(id, file, line, family.clone());
        finding.language = Some(LanguageId::Python);
        finding.owner_kind = Some(OwnerKind::Function);
        finding.class = class;
        finding.probe.owner = Some(SymbolId(format!("python:{file}::calculate_discount")));
        if finding.class == ExposureClass::WeaklyExposed {
            finding.canonical_gap = Some(FindingCanonicalGap {
                id: format!(
                    "gap:python:{file}:calculate_discount:predicate_boundary:predicate:amount>=threshold"
                ),
                language: "python".to_string(),
                file: file.to_string(),
                owner: "calculate_discount".to_string(),
                behavior_kind: "predicate_boundary".to_string(),
                probe_kind: family.as_str().to_string(),
                normalized_discriminator: "amount>=threshold".to_string(),
            });
            finding.recommended_next_step = Some(
                "Add an exact Python boundary assertion for `amount == threshold`.".to_string(),
            );
            finding
                .activation
                .missing_discriminators
                .push(MissingDiscriminatorFact {
                    value: "amount == threshold".to_string(),
                    reason: "predicate boundary equality is not asserted".to_string(),
                    flow_sink: None,
                });
        }
        finding
    }

    fn with_direct_relation(mut finding: Finding) -> Finding {
        finding
            .evidence
            .push("related_test_relation: syntactic_call (test_discount)".to_string());
        finding
    }

    fn with_verify_command(mut finding: Finding) -> Finding {
        finding.evidence.push(
            "test_verify_command: pytest tests/test_pricing.py::test_discount (test_discount)"
                .to_string(),
        );
        finding
    }

    fn with_private_owner(mut finding: Finding) -> Finding {
        finding.probe.owner = Some(SymbolId(
            "python:src/pricing.py::_calculate_discount".to_string(),
        ));
        finding
    }

    #[test]
    fn sort_findings_orders_by_file_line_then_probe_family() {
        let mut findings = vec![
            finding(
                "same-line-predicate",
                "src/b.rs",
                10,
                ProbeFamily::Predicate,
            ),
            finding("later-line", "src/a.rs", 20, ProbeFamily::Predicate),
            finding("same-line-error", "src/b.rs", 10, ProbeFamily::ErrorPath),
            finding("earlier-file", "src/a.rs", 5, ProbeFamily::SideEffect),
        ];

        sort_findings(&mut findings);

        let ids: Vec<&str> = findings.iter().map(|finding| finding.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "earlier-file",
                "later-line",
                "same-line-error",
                "same-line-predicate",
            ]
        );
    }

    #[test]
    fn sort_findings_prioritizes_repairable_python_preview_findings() {
        let actionable = with_verify_command(with_direct_relation(python_finding(
            "actionable-direct",
            "src/z_pricing.py",
            10,
            ProbeFamily::Predicate,
            ExposureClass::WeaklyExposed,
        )));
        let private_actionable =
            with_private_owner(with_verify_command(with_direct_relation(python_finding(
                "actionable-private",
                "src/a_private.py",
                1,
                ProbeFamily::Predicate,
                ExposureClass::WeaklyExposed,
            ))));
        let mut exposed = python_finding(
            "already-observed",
            "src/a_observed.py",
            1,
            ProbeFamily::ReturnValue,
            ExposureClass::Exposed,
        );
        exposed.recommended_next_step = None;
        exposed.activation.missing_discriminators.clear();
        let mut heuristic_only = python_finding(
            "heuristic-only",
            "src/a_heuristic.py",
            1,
            ProbeFamily::ReturnValue,
            ExposureClass::WeaklyExposed,
        );
        heuristic_only.recommended_next_step = None;
        heuristic_only.activation.missing_discriminators.clear();
        heuristic_only
            .evidence
            .push("related_test_relation: same_stem (test_price)".to_string());
        let mut static_limit = python_finding(
            "static-limit",
            "src/a_dynamic.py",
            1,
            ProbeFamily::StaticUnknown,
            ExposureClass::StaticUnknown,
        );
        static_limit.static_limit_kind = Some(StaticLimitKind::DynamicDispatch);
        static_limit.recommended_next_step = None;
        static_limit.activation.missing_discriminators.clear();

        let mut findings = vec![
            static_limit,
            heuristic_only,
            exposed,
            private_actionable,
            actionable,
        ];

        sort_findings(&mut findings);

        let ids: Vec<&str> = findings.iter().map(|finding| finding.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "actionable-direct",
                "actionable-private",
                "already-observed",
                "heuristic-only",
                "static-limit",
            ]
        );
    }
}
