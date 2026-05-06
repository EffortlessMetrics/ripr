use super::super::rust_index::{FunctionSummary, TestSummary};
use crate::domain::*;

pub(in crate::analysis) fn ensure_unknown_stop_reason(
    class: &ExposureClass,
    stop_reasons: &mut Vec<StopReason>,
) {
    if class.requires_stop_reason()
        && stop_reasons.is_empty()
        && let Some(reason) = StopReason::for_unknown_class(class)
    {
        stop_reasons.push(reason);
    }
}

pub(in crate::analysis) fn classify(
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

pub(in crate::analysis) fn confidence_score(
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

pub(in crate::analysis) fn missing_evidence(
    probe: &Probe,
    class: &ExposureClass,
    infect: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    activation: &ActivationEvidence,
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
    if matches!(probe.family, ProbeFamily::Predicate)
        && infect.state != StageState::Yes
        && !activation
            .missing_discriminators
            .iter()
            .any(|fact| fact.value.contains("=="))
    {
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
    missing.extend(
        activation
            .missing_discriminators
            .iter()
            .map(|fact| format!("Missing discriminator value: {}", fact.value)),
    );
    missing.sort();
    missing.dedup();
    missing
}

pub(in crate::analysis) fn stop_reasons(
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

pub(in crate::analysis) fn recommended_next_step(
    probe: &Probe,
    class: &ExposureClass,
) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_reachable_but_unobserved_probe_to_reachable_unrevealed() {
        let class = classify(
            &stage(StageState::Yes),
            &stage(StageState::Yes),
            &stage(StageState::Yes),
            &stage(StageState::No),
            &stage(StageState::Yes),
            &probe(ProbeFamily::ReturnValue, "value + 1"),
        );

        assert_eq!(class, ExposureClass::ReachableUnrevealed);
    }

    #[test]
    fn confidence_score_handles_opaque_no_and_not_applicable_stage_states() {
        let score = confidence_score(
            &stage(StageState::Opaque),
            &stage(StageState::No),
            &stage(StageState::NotApplicable),
            &stage(StageState::Yes),
            &stage(StageState::Weak),
            &ExposureClass::NoStaticPath,
        );

        assert!(
            (score - 0.64).abs() < f32::EPSILON,
            "confidence score should equal 0.64 (got {score})"
        );
    }

    #[test]
    fn missing_evidence_reports_reachable_unrevealed_gap() {
        let probe = probe(ProbeFamily::ReturnValue, "value + 1");
        let missing = missing_evidence(
            &probe,
            &ExposureClass::ReachableUnrevealed,
            &stage(StageState::Yes),
            &stage(StageState::No),
            &stage(StageState::Yes),
            &ActivationEvidence::default(),
        );

        assert!(
            missing.contains(
                &"No detected assertion observes the changed value, error, field, or effect"
                    .to_string()
            )
        );
    }

    #[test]
    fn recommended_next_step_covers_side_effect_and_default_weak_guidance() {
        let side_effect = recommended_next_step(
            &probe(ProbeFamily::SideEffect, "client.send(value)"),
            &ExposureClass::WeaklyExposed,
        );
        assert_eq!(
            side_effect.as_deref(),
            Some(
                "Add a mock expectation, event receiver assertion, persisted-state check, or metric assertion for the changed effect."
            )
        );

        let match_arm = recommended_next_step(
            &probe(ProbeFamily::MatchArm, "None => 0"),
            &ExposureClass::WeaklyExposed,
        );
        assert_eq!(
            match_arm.as_deref(),
            Some("Strengthen the related assertion so it discriminates the changed behavior.")
        );
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "stage")
    }

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: None,
            family,
            delta: DeltaKind::Value,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }
}
