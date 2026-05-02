use super::rust_index::{
    FunctionSummary, RustIndex, TestSummary, extract_identifier_tokens, extract_literals,
};
use crate::domain::*;
use std::path::Path;

pub fn classify_probe(probe: &Probe, index: &RustIndex) -> Finding {
    let owner_fn = probe.owner.as_ref().and_then(|owner| {
        index
            .functions
            .iter()
            .find(|function| &function.id == owner)
    });

    let related_tests = find_related_tests(probe, owner_fn, index);
    let reach = reach_evidence(&related_tests, owner_fn);
    let infect = infection_evidence(probe, &related_tests);
    let propagate = propagation_evidence(probe, owner_fn);
    let (observe, discriminate, related) = reveal_evidence(probe, &related_tests);

    let ripr = RiprEvidence {
        reach: reach.clone(),
        infect: infect.clone(),
        propagate: propagate.clone(),
        reveal: RevealEvidence {
            observe: observe.clone(),
            discriminate: discriminate.clone(),
        },
    };

    let class = classify(&reach, &infect, &propagate, &observe, &discriminate, probe);
    let confidence = confidence_score(&reach, &infect, &propagate, &observe, &discriminate, &class);
    let mut evidence = Vec::new();
    evidence.push(reach.summary.clone());
    if !infect.summary.is_empty() {
        evidence.push(infect.summary.clone());
    }
    if !propagate.summary.is_empty() {
        evidence.push(propagate.summary.clone());
    }
    if !observe.summary.is_empty() {
        evidence.push(observe.summary.clone());
    }
    if !discriminate.summary.is_empty() {
        evidence.push(discriminate.summary.clone());
    }
    evidence.sort();
    evidence.dedup();

    let missing = missing_evidence(probe, &class, &infect, &observe, &discriminate);
    let mut stop_reasons = stop_reasons(probe, owner_fn, &related_tests);
    ensure_unknown_stop_reason(&class, &mut stop_reasons);
    let recommended_next_step = recommended_next_step(probe, &class);

    Finding {
        id: probe.id.0.clone(),
        probe: probe.clone(),
        class,
        ripr,
        confidence,
        evidence,
        missing,
        stop_reasons,
        related_tests: related,
        recommended_next_step,
    }
}

fn ensure_unknown_stop_reason(class: &ExposureClass, stop_reasons: &mut Vec<StopReason>) {
    if class.requires_stop_reason()
        && stop_reasons.is_empty()
        && let Some(reason) = StopReason::for_unknown_class(class)
    {
        stop_reasons.push(reason);
    }
}

fn find_related_tests<'a>(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    index: &'a RustIndex,
) -> Vec<&'a TestSummary> {
    let mut related = Vec::new();
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let file_name = probe
        .location
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let package_prefix = owner_fn.and_then(|owner| package_prefix(&owner.file));

    for test in &index.tests {
        if let Some(prefix) = &package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || test.body.contains(owner_name));
        let mentions_tokens = probe_tokens
            .iter()
            .any(|token| token.len() > 3 && test.body.contains(token));
        let same_file_or_named = normalize_path(&test.file).contains(file_name)
            || test
                .name
                .to_ascii_lowercase()
                .contains(&owner_name.to_ascii_lowercase())
            || probe_tokens.iter().any(|token| {
                test.name
                    .to_ascii_lowercase()
                    .contains(&token.to_ascii_lowercase())
            });

        if calls_owner || mentions_tokens || same_file_or_named {
            related.push(test);
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
}

fn reach_evidence(
    related_tests: &[&TestSummary],
    owner_fn: Option<&FunctionSummary>,
) -> StageEvidence {
    if related_tests.is_empty() {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No static test path found for the changed owner",
        )
    } else {
        let target = owner_fn.map(|f| f.name.as_str()).unwrap_or("changed owner");
        let names = related_tests
            .iter()
            .take(3)
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!("Related tests appear to reach {target}: {names}"),
        )
    }
}

fn infection_evidence(probe: &Probe, related_tests: &[&TestSummary]) -> StageEvidence {
    match probe.family {
        ProbeFamily::Predicate => {
            let probe_literals = extract_literals(&probe.expression);
            let test_literals = related_tests
                .iter()
                .flat_map(|test| test.literals.iter().map(|literal| literal.value.clone()))
                .collect::<Vec<_>>();
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No tests were found, so activation/infection cannot be estimated",
                )
            } else if probe_literals.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Predicate changed, but no literal boundary was visible in the changed expression",
                )
            } else if probe_literals
                .iter()
                .any(|literal| test_literals.iter().any(|t| t == literal))
            {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    format!(
                        "Detected test input literal matching changed boundary: {}",
                        probe_literals.join(", ")
                    ),
                )
            } else if !test_literals.is_empty() {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    format!(
                        "Tests have literals [{}], but no detected value matches changed boundary [{}]",
                        test_literals.join(", "),
                        probe_literals.join(", ")
                    ),
                )
            } else {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Related tests use opaque fixtures; activation/infection is unknown",
                )
            }
        }
        ProbeFamily::StaticUnknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Unknown,
            "Changed syntax is not mapped to a high-confidence probe family",
        ),
        _ => {
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No reachable tests were found, so infection cannot be established",
                )
            } else {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Reachable tests can plausibly activate this changed behavior",
                )
            }
        }
    }
}

fn propagation_evidence(probe: &Probe, owner_fn: Option<&FunctionSummary>) -> StageEvidence {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "No propagation model is available for this changed syntax",
        );
    }
    let body = owner_fn.map(|f| f.body.as_str()).unwrap_or("");
    let expression = probe.expression.as_str();
    if matches!(probe.delta, DeltaKind::Effect) {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "Changed behavior reaches an effect boundary such as a call, write, publish, save, or send",
        )
    } else if expression.contains("return")
        || expression.contains("Ok(")
        || expression.contains("Err(")
        || body.contains("return")
    {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "Changed behavior can propagate through a return or Result boundary",
        )
    } else if expression.contains(':') || body.contains("{") && body.contains('}') {
        StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            "Changed behavior may propagate through a constructed value or field",
        )
    } else {
        StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Propagation is not statically obvious from syntax-first analysis",
        )
    }
}

fn reveal_evidence(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> (StageEvidence, StageEvidence, Vec<RelatedTest>) {
    if related_tests.is_empty() {
        return (
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No reachable test oracle found",
            ),
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No assertion can discriminate the changed behavior without a reachable test",
            ),
            Vec::new(),
        );
    }

    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut matched_any = false;

    for test in related_tests {
        if test.assertions.is_empty() {
            related.push(RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
                oracle: None,
                oracle_strength: OracleStrength::None,
            });
            continue;
        }
        for assertion in &test.assertions {
            let token_match = probe_tokens
                .iter()
                .any(|token| token.len() > 3 && assertion.text.contains(token));
            let family_match = oracle_matches_family(&probe.family, &assertion.text);
            if token_match || family_match || test.assertions.len() == 1 {
                matched_any = true;
                if assertion.strength.rank() > strongest.rank() {
                    strongest = assertion.strength.clone();
                }
                related.push(RelatedTest {
                    name: test.name.clone(),
                    file: test.file.clone(),
                    line: test.start_line,
                    oracle: Some(assertion.text.clone()),
                    oracle_strength: assertion.strength.clone(),
                });
            }
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.line.cmp(&b.line)));
    related.dedup_by(|a, b| a.name == b.name && a.oracle == b.oracle);

    let observe = if matched_any {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "A related test observes a value or effect near the changed behavior",
        )
    } else {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "Related tests were found, but no assertion appears to observe the changed value, error, field, or effect",
        )
    };

    let discriminate = match strongest {
        OracleStrength::Strong => StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "Strong oracle found: exact equality, exact variant, or pattern assertion",
        ),
        OracleStrength::Medium => StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            "Medium oracle found: snapshot, property, or partial structural assertion",
        ),
        OracleStrength::Weak | OracleStrength::Smoke => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            "Only weak or smoke oracle found, such as is_ok/is_err, unwrap/expect, broad relational assertion, or non-empty check",
        ),
        OracleStrength::None => StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No assertion found on related tests",
        ),
        OracleStrength::Unknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Assertions exist, but oracle strength is unknown",
        ),
    };

    (observe, discriminate, related)
}

fn oracle_matches_family(family: &ProbeFamily, assertion: &str) -> bool {
    match family {
        ProbeFamily::ErrorPath => {
            assertion.contains("Err")
                || assertion.contains("is_err")
                || assertion.contains("Error::")
        }
        ProbeFamily::SideEffect => {
            assertion.contains("expect")
                || assertion.contains("mock")
                || assertion.contains("saved")
                || assertion.contains("published")
        }
        ProbeFamily::FieldConstruction => {
            assertion.contains('.') || assertion.contains("assert_eq!")
        }
        ProbeFamily::Predicate => {
            assertion.contains("assert_eq!")
                || assertion.contains("assert!")
                || assertion.contains("matches!")
        }
        ProbeFamily::ReturnValue => {
            assertion.contains("assert_eq!")
                || assertion.contains("assert!")
                || assertion.contains("is_ok")
        }
        ProbeFamily::CallDeletion => assertion.contains("assert") || assertion.contains("expect"),
        ProbeFamily::MatchArm => {
            assertion.contains("matches!")
                || assertion.contains("assert_matches!")
                || assertion.contains("assert_eq!")
        }
        ProbeFamily::StaticUnknown => false,
    }
}

fn classify(
    reach: &StageEvidence,
    infect: &StageEvidence,
    propagate: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    probe: &Probe,
) -> ExposureClass {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return ExposureClass::StaticUnknown;
    }
    if reach.state == StageState::No {
        return ExposureClass::NoStaticPath;
    }
    if infect.state == StageState::Unknown || infect.state == StageState::Opaque {
        return ExposureClass::InfectionUnknown;
    }
    if propagate.state == StageState::Unknown || propagate.state == StageState::Opaque {
        return ExposureClass::PropagationUnknown;
    }
    if observe.state == StageState::No {
        return ExposureClass::ReachableUnrevealed;
    }
    if discriminate.state == StageState::Yes
        && infect.state == StageState::Yes
        && propagate.state == StageState::Yes
    {
        ExposureClass::Exposed
    } else {
        ExposureClass::WeaklyExposed
    }
}

fn confidence_score(
    reach: &StageEvidence,
    infect: &StageEvidence,
    propagate: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    class: &ExposureClass,
) -> f32 {
    let states = [
        &reach.state,
        &infect.state,
        &propagate.state,
        &observe.state,
        &discriminate.state,
    ];
    let mut score = 0.0;
    for state in states {
        score += match state {
            StageState::Yes => 0.2,
            StageState::Weak => 0.12,
            StageState::Unknown => 0.07,
            StageState::Opaque => 0.05,
            StageState::No => 0.02,
            StageState::NotApplicable => 0.1,
        };
    }
    if matches!(
        class,
        ExposureClass::NoStaticPath | ExposureClass::ReachableUnrevealed
    ) {
        score = (score + 0.15_f32).min(0.95_f32);
    }
    (score * 100.0).round() / 100.0
}

fn missing_evidence(
    probe: &Probe,
    class: &ExposureClass,
    infect: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
) -> Vec<String> {
    let mut missing = Vec::new();
    match class {
        ExposureClass::Exposed => {}
        ExposureClass::NoStaticPath => {
            missing.push("No static test path reaches the changed owner".to_string())
        }
        ExposureClass::ReachableUnrevealed => missing.push(
            "No detected assertion observes the changed value, error, field, or effect".to_string(),
        ),
        ExposureClass::InfectionUnknown => missing.push(infect.summary.clone()),
        ExposureClass::PropagationUnknown => missing.push(
            "No clear propagation path from changed behavior to an observable sink".to_string(),
        ),
        ExposureClass::StaticUnknown => missing.push(
            "Syntax-first analysis cannot classify this change; use deep mode or real mutation"
                .to_string(),
        ),
        ExposureClass::WeaklyExposed => {}
    }
    if matches!(probe.family, ProbeFamily::Predicate) && infect.state != StageState::Yes {
        missing.push("No detected boundary input for the changed predicate".to_string());
    }
    if observe.state != StageState::Yes {
        missing.push("No relevant oracle was detected".to_string());
    }
    if discriminate.state != StageState::Yes {
        missing.push("No strong discriminator was detected".to_string());
    }
    missing.sort();
    missing.dedup();
    missing
}

fn stop_reasons(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
) -> Vec<StopReason> {
    let mut reasons = Vec::new();
    if owner_fn.is_none() {
        reasons.push(StopReason::NoChangedRustLine);
    }
    if related_tests.iter().any(|test| {
        test.body.contains("fixture") || test.body.contains("builder") || test.body.contains("arb_")
    }) {
        reasons.push(StopReason::FixtureOpaque);
    }
    if probe.expression.contains("async")
        || probe.expression.contains("spawn")
        || probe.expression.contains("await")
    {
        reasons.push(StopReason::AsyncBoundaryOpaque);
    }
    if contains_macro_invocation(&probe.expression) {
        reasons.push(StopReason::ProcMacroOpaque);
    }
    reasons.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    reasons.dedup_by(|a, b| a.as_str() == b.as_str());
    reasons
}

fn contains_macro_invocation(expression: &str) -> bool {
    for (idx, ch) in expression.char_indices() {
        if ch != '!' || expression[idx + 1..].starts_with('=') {
            continue;
        }
        let before_bang = expression[..idx].trim_end();
        if before_bang
            .chars()
            .last()
            .is_some_and(|ch| ch == '_' || ch == ')' || ch.is_ascii_alphanumeric())
        {
            return true;
        }
    }
    false
}

fn recommended_next_step(probe: &Probe, class: &ExposureClass) -> Option<String> {
    match class {
        ExposureClass::Exposed => None,
        ExposureClass::WeaklyExposed => Some(match probe.family {
            ProbeFamily::Predicate => "Add boundary tests for below, equal, and above the changed threshold with exact assertions.".to_string(),
            ProbeFamily::ErrorPath => "Assert the exact error variant or payload instead of only is_err().".to_string(),
            ProbeFamily::SideEffect => "Add a mock expectation, event receiver assertion, persisted-state check, or metric assertion for the changed effect.".to_string(),
            ProbeFamily::ReturnValue => "Replace broad assertions with exact equality or a property that constrains the changed returned value.".to_string(),
            _ => "Strengthen the related assertion so it discriminates the changed behavior.".to_string(),
        }),
        ExposureClass::ReachableUnrevealed => Some("Add a meaningful assertion that observes the changed value, branch, error, field, event, or side effect.".to_string()),
        ExposureClass::NoStaticPath => Some("Add or identify a test path that reaches the changed owner, or run ready-mode mutation to confirm coverage.".to_string()),
        ExposureClass::InfectionUnknown => Some("Add a targeted boundary or negative-path test, or teach ripr about the fixture/builder in ripr.toml.".to_string()),
        ExposureClass::PropagationUnknown | ExposureClass::StaticUnknown => Some("Escalate to real mutation testing or deep static analysis for this probe.".to_string()),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn package_prefix(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if prefix.is_empty() {
                return None;
            }
            return Some(format!("{prefix}/"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{CallFact, LiteralFact, OracleFact};
    use std::path::PathBuf;

    #[test]
    fn resolves_owner_by_full_symbol_identity() {
        let crate_b_fn = function("crates/crate_b/src/lib.rs", "score");
        let crate_a_fn = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![crate_b_fn, crate_a_fn],
            tests: vec![
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                    "assert_eq!(score(2), 3);",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                    "assert_eq!(score(1), 2);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:crate_a:score".to_string()),
            location: SourceLocation::new("crates/crate_a/src/lib.rs", 2, 1),
            owner: Some(SymbolId("crates/crate_a/src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("score + 1".to_string()),
            expression: "score + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].name, "crate_a_score_test");
    }

    #[test]
    fn package_prefix_handles_workspace_crates_and_nested_markers() {
        assert_eq!(
            package_prefix(Path::new("crates/foo/src/support/src/lib.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/foo/tests/support/tests/cases.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("vendor/foo/src/support/src/lib.rs")).as_deref(),
            Some("vendor/foo/src/support/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/ripr/examples/sample/src/lib.rs")).as_deref(),
            Some("crates/ripr/examples/sample/")
        );
    }

    #[test]
    fn infection_unknown_findings_include_stop_reason() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "price")],
            tests: vec![test(
                "tests/pricing.rs",
                "price_test",
                "price(1)",
                "assert_eq!(price(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::price".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::InfectionUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::InfectionEvidenceUnknown.as_str() })
        );
    }

    #[test]
    fn propagation_unknown_findings_include_stop_reason() {
        let function = FunctionSummary {
            body: "value".to_string(),
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("value".to_string()),
            expression: "value".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::PropagationUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding.stop_reasons.iter().any(|reason| {
                reason.as_str() == StopReason::PropagationEvidenceUnknown.as_str()
            })
        );
    }

    #[test]
    fn static_unknown_findings_include_stop_reason() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:static_unknown".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::StaticUnknown,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some("score!(1)".to_string()),
            expression: "score".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::StaticProbeUnknown.as_str() })
        );
    }

    #[test]
    fn recommended_next_step_matches_probe_family_and_exposure_class() {
        let predicate_probe = probe(ProbeFamily::Predicate, DeltaKind::Control, "value > 10");
        let return_value_probe = probe(ProbeFamily::ReturnValue, DeltaKind::Value, "value + 1");

        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::Exposed),
            None
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::WeaklyExposed).as_deref(),
            Some(
                "Add boundary tests for below, equal, and above the changed threshold with exact assertions."
            )
        );
        assert_eq!(
            recommended_next_step(&return_value_probe, &ExposureClass::WeaklyExposed).as_deref(),
            Some(
                "Replace broad assertions with exact equality or a property that constrains the changed returned value."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::ReachableUnrevealed).as_deref(),
            Some(
                "Add a meaningful assertion that observes the changed value, branch, error, field, event, or side effect."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::NoStaticPath).as_deref(),
            Some(
                "Add or identify a test path that reaches the changed owner, or run ready-mode mutation to confirm coverage."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::InfectionUnknown).as_deref(),
            Some(
                "Add a targeted boundary or negative-path test, or teach ripr about the fixture/builder in ripr.toml."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::StaticUnknown).as_deref(),
            Some("Escalate to real mutation testing or deep static analysis for this probe.")
        );
    }

    #[test]
    fn stop_reasons_detect_macro_bang_without_treating_inequality_as_macro() {
        let inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold",
        );
        let unary_not = probe(ProbeFamily::StaticUnknown, DeltaKind::Unknown, "!enabled");
        let macro_with_inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold && trace!(value)",
        );

        assert_eq!(stop_reason_labels(&inequality), Vec::<&str>::new());
        assert_eq!(stop_reason_labels(&unary_not), Vec::<&str>::new());
        assert_eq!(
            stop_reason_labels(&macro_with_inequality),
            vec!["proc_macro_opaque"]
        );
    }

    #[test]
    fn stop_reasons_are_deduplicated_and_sorted() {
        let probe = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "async move { spawn(task).await; trace!(task); }",
        );

        let labels = stop_reason_labels(&probe);
        assert_eq!(labels, vec!["async_boundary_opaque", "proc_macro_opaque"]);
    }

    fn stop_reason_labels(probe: &Probe) -> Vec<&str> {
        let owner = function("crates/ripr/src/lib.rs", "dummy");
        let reasons = stop_reasons(probe, Some(&owner), &[]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();
        labels
    }

    fn probe(family: ProbeFamily, delta: DeltaKind, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("crates/ripr/src/lib.rs", 1, 1),
            owner: None,
            family,
            delta,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    fn function(file: &str, name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: format!("pub fn {name}(input: i32) -> i32 {{ input }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            is_test: false,
        }
    }

    fn test(file: &str, name: &str, call: &str, assertion: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body: format!("{call};\n{assertion}"),
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: call.to_string(),
            }],
            assertions: vec![OracleFact {
                line: 2,
                text: assertion.to_string(),
                strength: OracleStrength::Strong,
                observed_tokens: extract_identifier_tokens(assertion),
            }],
            literals: vec![LiteralFact {
                line: 1,
                value: "1".to_string(),
            }],
        }
    }
}
