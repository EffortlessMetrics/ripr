use super::rust_index::{
    FunctionSummary, OracleFact, RustIndex, TestSummary, extract_identifier_tokens,
    extract_literals,
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
    let flow_sinks = local_flow_sinks(probe, owner_fn);
    let propagate = propagation_evidence(probe, &flow_sinks);
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
        flow_sinks,
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

fn propagation_evidence(probe: &Probe, flow_sinks: &[FlowSinkFact]) -> StageEvidence {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "No propagation model is available for this changed syntax",
        );
    }

    if let Some(sink) = flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
    {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!(
                "Changed behavior appears to influence {}: {}",
                sink.kind.label(),
                sink.text
            ),
        )
    } else {
        StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Propagation is not statically obvious from syntax-first analysis",
        )
    }
}

fn local_flow_sinks(probe: &Probe, owner_fn: Option<&FunctionSummary>) -> Vec<FlowSinkFact> {
    let owner = owner_fn.map(|function| function.id.clone());
    let mut sinks = match probe.family {
        ProbeFamily::StaticUnknown => vec![flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::ErrorPath => vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            if probe.expression.contains("Err(") {
                vec![flow_sink(
                    FlowSinkKind::ErrorVariant,
                    result_error_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else if probe.expression.starts_with("return ")
                || probe.expression.contains("Ok(")
                || probe.expression.contains("Some(")
            {
                vec![flow_sink(
                    FlowSinkKind::ReturnValue,
                    return_sink_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else {
                vec![flow_sink(
                    FlowSinkKind::CallEffect,
                    call_effect_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            }
        }
        ProbeFamily::FieldConstruction => vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::MatchArm => vec![match_arm_sink(probe, owner.clone())],
        ProbeFamily::ReturnValue => vec![return_value_sink(probe, owner_fn, owner.clone())],
        ProbeFamily::Predicate => predicate_flow_sinks(probe, owner_fn, owner.clone()),
    };

    sinks.sort_by(|a, b| {
        a.kind
            .as_str()
            .cmp(b.kind.as_str())
            .then(a.line.cmp(&b.line))
            .then(a.text.cmp(&b.text))
    });
    sinks.dedup_by(|a, b| a.kind == b.kind && a.line == b.line && a.text == b.text);
    sinks
}

fn predicate_flow_sinks(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> Vec<FlowSinkFact> {
    if let Some(error) = first_error_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&error.text),
            error.line,
            owner,
        )];
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        )];
    }
    if let Some(field) = first_field_construction(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&field.text),
            field.line,
            owner,
        )];
    }
    if let Some(branch) = next_branch_value(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            branch.text,
            branch.line,
            owner,
        )];
    }
    Vec::new()
}

fn return_value_sink(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    if probe.expression.contains("Err(") {
        return flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner,
        );
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        );
    }
    if !is_obvious_return_expression(&probe.expression) {
        return flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner,
        );
    }
    flow_sink(
        FlowSinkKind::ReturnValue,
        return_sink_text(&probe.expression),
        probe.location.line,
        owner,
    )
}

fn match_arm_sink(probe: &Probe, owner: Option<SymbolId>) -> FlowSinkFact {
    let arm_result = probe
        .expression
        .split_once("=>")
        .map(|(_, result)| result.trim().trim_end_matches(',').to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| probe.expression.clone());

    if arm_result.contains("Err(") {
        flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&arm_result),
            probe.location.line,
            owner,
        )
    } else {
        flow_sink(
            FlowSinkKind::MatchArm,
            arm_result,
            probe.location.line,
            owner,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalTextFact {
    line: usize,
    text: String,
}

fn first_error_return(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .find(|return_fact| return_fact.line >= probe_line && return_fact.text.contains("Err("))
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn nearest_return(owner_fn: Option<&FunctionSummary>, probe_line: usize) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .filter(|return_fact| return_fact.line >= probe_line)
            .min_by_key(|return_fact| return_fact.line - probe_line)
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn next_branch_value(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    let function = owner_fn?;
    let start_index = probe_line.saturating_sub(function.start_line);
    function
        .body
        .lines()
        .enumerate()
        .skip(start_index + 1)
        .find_map(|(offset, line)| {
            let text = line.trim().trim_end_matches(',').to_string();
            if !looks_like_branch_tail_expression(&text) {
                return None;
            }
            Some(LocalTextFact {
                line: function.start_line + offset,
                text,
            })
        })
}

fn first_field_construction(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .body
            .lines()
            .enumerate()
            .skip(probe_line.saturating_sub(function.start_line))
            .find_map(|(offset, line)| {
                let text = line.trim().trim_end_matches(',').to_string();
                if looks_like_field_assignment(&text) {
                    Some(LocalTextFact {
                        line: function.start_line + offset,
                        text,
                    })
                } else {
                    None
                }
            })
    })
}

fn flow_sink(
    kind: FlowSinkKind,
    text: impl Into<String>,
    line: usize,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    FlowSinkFact {
        kind,
        text: text.into(),
        line,
        owner,
    }
}

fn result_error_text(text: &str) -> String {
    if let Some(start) = text.find("Err(") {
        let error = text[start..]
            .trim()
            .trim_start_matches("return ")
            .trim_end_matches(';')
            .trim_end_matches(',')
            .to_string();
        return format!("Result::{error}");
    }
    return_sink_text(text)
}

fn return_sink_text(text: &str) -> String {
    text.trim()
        .trim_start_matches("return ")
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim()
        .to_string()
}

fn call_effect_text(text: &str) -> String {
    return_sink_text(text)
}

fn field_sink_text(text: &str) -> String {
    return_sink_text(text)
}

fn looks_like_field_assignment(text: &str) -> bool {
    let Some((field, _)) = text.split_once(':') else {
        return false;
    };
    if text.contains("::") {
        return false;
    }
    let field = field.trim();
    !field.is_empty()
        && field
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && field
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

fn looks_like_branch_tail_expression(text: &str) -> bool {
    if text.is_empty()
        || text == "{"
        || text == "}"
        || text.starts_with("else")
        || text.starts_with("//")
        || text.starts_with("let ")
        || text.ends_with(';')
    {
        return false;
    }
    if text.contains(" = ")
        || text.contains(" += ")
        || text.contains(" -= ")
        || text.contains(" *= ")
        || text.contains(" /= ")
    {
        return false;
    }
    is_obvious_return_expression(text)
}

fn is_obvious_return_expression(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("return ")
        || trimmed.starts_with("Ok(")
        || trimmed.starts_with("Some(")
        || trimmed.contains("Err(")
        || trimmed.contains('(')
        || trimmed.contains('"')
        || trimmed.chars().any(|ch| ch.is_ascii_digit())
        || [" + ", " - ", " * ", " / ", " % "]
            .iter()
            .any(|operator| trimmed.contains(operator))
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

    let analysis = analyze_related_assertions(probe, related_tests);
    let related = finalize_related_tests(analysis.related);
    let observe = build_observe_evidence(analysis.matched_any);
    let discriminate =
        build_discriminate_evidence(&analysis.strongest, &analysis.strongest_kind, &probe.family);

    (observe, discriminate, related)
}

struct RevealAssertionAnalysis {
    related: Vec<RelatedTest>,
    strongest: OracleStrength,
    strongest_kind: OracleKind,
    matched_any: bool,
}

fn analyze_related_assertions(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> RevealAssertionAnalysis {
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut strongest_kind = OracleKind::Unknown;
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
            if assertion_matches_probe(
                &probe_tokens,
                &probe.family,
                assertion,
                test.assertions.len(),
            ) {
                matched_any = true;
                let relative_strength = probe_relative_oracle_strength(&probe.family, assertion);
                if relative_strength.rank() > strongest.rank() {
                    strongest = relative_strength.clone();
                    strongest_kind = assertion.kind.clone();
                }
                related.push(RelatedTest {
                    name: test.name.clone(),
                    file: test.file.clone(),
                    line: test.start_line,
                    oracle: Some(assertion.text.clone()),
                    oracle_strength: relative_strength,
                });
            }
        }
    }

    RevealAssertionAnalysis {
        related,
        strongest,
        strongest_kind,
        matched_any,
    }
}

fn assertion_matches_probe(
    probe_tokens: &[String],
    family: &ProbeFamily,
    assertion: &OracleFact,
    assertion_count: usize,
) -> bool {
    let token_match = probe_tokens
        .iter()
        .any(|token| token.len() > 3 && assertion.text.contains(token));
    let family_match = oracle_matches_family(family, assertion);
    token_match || family_match || assertion_count == 1
}

fn finalize_related_tests(mut related: Vec<RelatedTest>) -> Vec<RelatedTest> {
    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.line.cmp(&b.line)));
    related.dedup_by(|a, b| a.name == b.name && a.oracle == b.oracle);
    related
}

fn build_observe_evidence(matched_any: bool) -> StageEvidence {
    if matched_any {
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
    }
}

fn build_discriminate_evidence(
    strongest: &OracleStrength,
    strongest_kind: &OracleKind,
    family: &ProbeFamily,
) -> StageEvidence {
    match strongest {
        OracleStrength::Strong => StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::ExactErrorVariant => {
                    "Strong oracle found: exact error variant assertion"
                }
                OracleKind::WholeObjectEquality => {
                    "Strong oracle found: whole-object equality assertion"
                }
                _ => "Strong oracle found: exact value or pattern assertion",
            },
        ),
        OracleStrength::Medium => StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::Snapshot => {
                    "Medium oracle found: snapshot assertion observes the changed behavior"
                }
                OracleKind::MockExpectation => {
                    "Medium oracle found: mock or expectation observes the changed behavior"
                }
                _ => "Medium oracle found: property or partial structural assertion",
            },
        ),
        OracleStrength::Weak => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            match (strongest_kind, family) {
                (OracleKind::BroadError, ProbeFamily::ErrorPath) => {
                    "Only broad error oracle found; is_err() does not discriminate exact error variants"
                }
                (OracleKind::BroadError, _) => {
                    "Only broad error oracle found; it may not discriminate the changed behavior exactly"
                }
                (OracleKind::RelationalCheck, _) => {
                    "Only relational oracle found; it may not discriminate the changed value exactly"
                }
                _ => {
                    "Only weak oracle found, such as a broad relational assertion or non-empty check"
                }
            },
        ),
        OracleStrength::Smoke => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
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
    }
}

fn oracle_matches_family(family: &ProbeFamily, assertion: &OracleFact) -> bool {
    let text = assertion.text.as_str();
    match family {
        ProbeFamily::ErrorPath => {
            matches!(
                assertion.kind,
                OracleKind::ExactErrorVariant | OracleKind::BroadError
            ) || text.contains("Err")
                || text.contains("Error::")
        }
        ProbeFamily::SideEffect => {
            matches!(assertion.kind, OracleKind::MockExpectation)
                || text.contains("expect")
                || text.contains("mock")
                || text.contains("saved")
                || text.contains("published")
        }
        ProbeFamily::FieldConstruction => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            ) || text.contains('.')
        }
        ProbeFamily::Predicate => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::ExactErrorVariant
                    | OracleKind::Snapshot
            )
        }
        ProbeFamily::ReturnValue => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
                    | OracleKind::SmokeOnly
            )
        }
        ProbeFamily::CallDeletion => {
            matches!(
                assertion.kind,
                OracleKind::MockExpectation
                    | OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::SmokeOnly
            ) || text.contains("assert")
                || text.contains("expect")
        }
        ProbeFamily::MatchArm => {
            matches!(
                assertion.kind,
                OracleKind::ExactErrorVariant
                    | OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            )
        }
        ProbeFamily::StaticUnknown => false,
    }
}

fn probe_relative_oracle_strength(family: &ProbeFamily, assertion: &OracleFact) -> OracleStrength {
    match family {
        ProbeFamily::ErrorPath => match assertion.kind {
            OracleKind::ExactErrorVariant => OracleStrength::Strong,
            OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            _ => assertion.strength.clone(),
        },
        ProbeFamily::ReturnValue
        | ProbeFamily::Predicate
        | ProbeFamily::FieldConstruction
        | ProbeFamily::MatchArm => match assertion.kind {
            OracleKind::ExactValue
            | OracleKind::ExactErrorVariant
            | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::Snapshot | OracleKind::MockExpectation => OracleStrength::Medium,
            OracleKind::RelationalCheck | OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => match assertion.kind {
            OracleKind::MockExpectation => OracleStrength::Medium,
            OracleKind::ExactValue | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::RelationalCheck | OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::ExactErrorVariant => OracleStrength::Medium,
            OracleKind::Snapshot => OracleStrength::Medium,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::StaticUnknown => OracleStrength::Unknown,
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
        if matches!(probe.family, ProbeFamily::ErrorPath) {
            missing.push("No exact error variant discriminator was detected".to_string());
        } else {
            missing.push("No strong discriminator was detected".to_string());
        }
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
    use crate::analysis::rust_index::{CallFact, LiteralFact, OracleFact, ReturnFact};
    use std::path::PathBuf;

    #[test]
    fn given_owner_symbol_when_resolving_owner_then_matches_full_identity() {
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
    fn given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers() {
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
    fn given_non_workspace_paths_when_extracting_package_prefix_then_returns_none() {
        assert_eq!(package_prefix(Path::new("src/lib.rs")), None);
        assert_eq!(package_prefix(Path::new("tests/basic.rs")), None);
        assert_eq!(package_prefix(Path::new("README.md")), None);
    }

    #[test]
    fn given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form() {
        let normalized = normalize_path(Path::new("./crates\\ripr\\src\\lib.rs"));
        assert_eq!(normalized, "crates/ripr/src/lib.rs");
    }

    #[test]
    fn given_infection_unknown_probe_when_classified_then_stop_reason_is_present() {
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
    fn given_propagation_unknown_probe_when_classified_then_stop_reason_is_present() {
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
    fn given_static_unknown_probe_when_classified_then_stop_reason_is_present() {
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
    fn given_exact_error_variant_assertion_when_error_path_probe_changes_then_oracle_is_strong() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_exact",
                "score(\"\")",
                oracle_fact(
                    "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Yes);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Strong oracle found: exact error variant assertion"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
    }

    #[test]
    fn given_broad_is_err_assertion_when_error_variant_changes_then_oracle_is_weak() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only broad error oracle found; is_err() does not discriminate exact error variants"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Weak
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_unwrap_only_test_when_return_value_probe_changes_then_oracle_is_smoke() {
        let unwrap_only = format!("score(1).{}();", "unwrap");
        let index = RustIndex {
            functions: vec![FunctionSummary {
                body: "pub fn score(input: i32) -> Result<i32, Error> { Ok(input) }".to_string(),
                ..function("src/lib.rs", "score")
            }],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_smoke",
                "score(1)",
                oracle_fact(&unwrap_only, OracleKind::SmokeOnly, OracleStrength::Smoke),
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
            after: Some("return Ok(input + 1)".to_string()),
            expression: "return Ok(input + 1)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Smoke
        );
    }

    #[test]
    fn given_broad_error_assertion_when_non_error_probe_changes_then_gap_stays_generic() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_call_is_broad",
                "score(1)",
                oracle_fact(
                    "assert!(score(1).is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call_deletion".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("client.send(input)".to_string()),
            expression: "client.send(input)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only broad error oracle found; it may not discriminate the changed behavior exactly"
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No strong discriminator was detected" })
        );
        assert!(
            !finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_value_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "amount - 10");
        assert_eq!(finding.flow_sinks[0].line, 3);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Changed behavior appears to influence returned value: amount - 10"
        );
    }

    #[test]
    fn given_changed_error_variant_when_result_err_is_returned_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Err(AuthError::RevokedToken);".to_string()),
            expression: "return Err(AuthError::RevokedToken);".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_side_effect_call_when_effect_method_is_called_then_flow_sink_is_call_effect() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_publishes",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:side_effect".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::SideEffect,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("events.publish(score)".to_string()),
            expression: "events.publish(score)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
        assert_eq!(finding.flow_sinks[0].text, "events.publish(score)");
    }

    #[test]
    fn given_changed_field_construction_when_field_is_assigned_then_flow_sink_is_struct_field() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_builds_field",
                "score(1)",
                "assert_eq!(score(1).total, 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:field_construction".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::FieldConstruction,
            delta: DeltaKind::Value,
            before: None,
            after: Some("total: computed_total".to_string()),
            expression: "total: computed_total".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: computed_total");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_value_then_flow_sink_is_match_arm_return() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_matches",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Some(value) => value + 1,".to_string()),
            expression: "Some(value) => value + 1,".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::MatchArm);
        assert_eq!(finding.flow_sinks[0].text, "value + 1");
    }

    #[test]
    fn given_changed_return_binding_when_function_returns_ok_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: "pub fn score(input: i32) -> Result<i32, Error> { let value = input + 1; Ok(value) }"
                .to_string(),
            returns: vec![ReturnFact {
                line: 1,
                text: "Ok(value)".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:1:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("let value = input + 1".to_string()),
            expression: "let value = input + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(value)");
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_error_then_flow_sink_is_error_variant() {
        let function = FunctionSummary {
            body: r#"pub fn authenticate(token: &str) -> Result<User, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(User)
}"#
            .to_string(),
            start_line: 1,
            end_line: 6,
            returns: vec![
                ReturnFact {
                    line: 3,
                    text: "return Err(AuthError::RevokedToken);".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "Ok(User)".to_string(),
                },
            ],
            ..function("src/lib.rs", "authenticate")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "empty_token_is_rejected",
                "authenticate(\"\")",
                oracle_fact(
                    "assert!(authenticate(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("token.is_empty()".to_string()),
            expression: "token.is_empty()".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_constructs_field_then_flow_sink_is_struct_field() {
        let function = FunctionSummary {
            body: r#"pub fn quote(amount: i32) -> Quote {
    if amount > 0 {
        Quote {
            total: amount - 10,
        }
    } else {
        Quote {
            total: amount,
        }
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 11,
            ..function("src/lib.rs", "quote")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/quote.rs",
                "positive_quote_has_total",
                "quote(100)",
                "assert_eq!(quote(100).total, 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::quote".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount > 0".to_string()),
            expression: "amount > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: amount - 10");
    }

    #[test]
    fn given_changed_predicate_when_return_contains_colon_in_string_then_flow_sink_is_return_value()
    {
        let function = FunctionSummary {
            body: r#"pub fn message(code: i32) -> String {
    if code > 0 {
        format!("error:{code}")
    } else {
        "ok".to_string()
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "message")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/message.rs",
                "message_returns_error_code",
                "message(1)",
                "assert_eq!(message(1), \"error:1\");",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::message".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("code > 0".to_string()),
            expression: "code > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "format!(\"error:{code}\")");
    }

    #[test]
    fn given_changed_predicate_when_next_line_is_assignment_then_flow_sink_stays_unknown() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        let discounted = amount - 10;
        discounted
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 8,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(finding.flow_sinks.is_empty());
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_return_after_early_return_when_no_downstream_return_exists_then_sink_is_unknown()
     {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32) -> i32 {
    if amount < 0 {
        return 0;
    }
    let adjusted = amount;
    adjusted
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            returns: vec![ReturnFact {
                line: 3,
                text: "return 0;".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_positive",
                "score(1)",
                "assert_eq!(score(1), 1);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:5:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 5, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("adjusted".to_string()),
            expression: "adjusted".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_call_deletion_when_result_ok_is_returned_then_flow_sink_is_return_value() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("Ok(total)".to_string()),
            expression: "Ok(total)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(total)");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_error_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "authenticate")],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "revoked_token_is_exact",
                "authenticate(\"\")",
                oracle_fact(
                    "assert_matches!(authenticate(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("None => Err(AuthError::RevokedToken),".to_string()),
            expression: "None => Err(AuthError::RevokedToken),".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_opaque_return_expression_when_no_sink_is_obvious_then_propagation_is_unknown()
    {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
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

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Propagation is not statically obvious from syntax-first analysis"
        );
    }

    #[test]
    fn given_probe_family_and_exposure_class_when_recommending_next_step_then_guidance_matches() {
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
    fn given_macro_like_expression_when_collecting_stop_reasons_then_ignores_inequality_tokens() {
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
    fn given_duplicate_stop_reasons_when_collecting_then_results_are_deduplicated_and_sorted() {
        let probe = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "async move { spawn(task).await; trace!(task); }",
        );

        let labels = stop_reason_labels(&probe);
        assert_eq!(labels, vec!["async_boundary_opaque", "proc_macro_opaque"]);
    }

    #[test]
    fn stop_reasons_include_fixture_and_missing_owner_signals() {
        let probe = probe(
            ProbeFamily::CallDeletion,
            DeltaKind::Effect,
            "client.send(input)",
        );
        let fixture_test = test(
            "tests/service.rs",
            "service_uses_fixture",
            "score(1)",
            "let fixture = build_fixture(); assert_eq!(score(1), 2);",
        );

        let reasons = stop_reasons(&probe, None, &[&fixture_test]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();

        assert_eq!(labels, vec!["fixture_opaque", "no_changed_rust_line"]);
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
        test_with_oracle(
            file,
            name,
            call,
            oracle_fact(assertion, OracleKind::ExactValue, OracleStrength::Strong),
        )
    }

    fn test_with_oracle(file: &str, name: &str, call: &str, oracle: OracleFact) -> TestSummary {
        let body = format!("{call};\n{}", oracle.text.as_str());
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body,
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: call.to_string(),
            }],
            assertions: vec![oracle],
            literals: vec![LiteralFact {
                line: 1,
                value: "1".to_string(),
            }],
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: assertion.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(assertion),
        }
    }
}
