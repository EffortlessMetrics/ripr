use super::discriminators::python_missing_discriminators;
use super::no_behavior::{
    changed_default_overridden_params, format_param_name_list, is_annotation_only_def_change,
    is_annotation_only_var_change, is_python_no_behavior_line,
};
use super::probe_shape::{
    canonical_python_gap_for, classify_probe_shape, python_flow_sink_for,
    python_infection_evidence, python_propagation_evidence,
};
use super::related_tests::{
    find_related_tests, python_repair_placement, related_test_candidates, strongest_assertion,
    verify_command_for_test,
};
use super::sink_alignment::{SinkAlignment, classify_sink_alignment_with_old};
use super::static_limits::static_limit_for_change;
use super::{
    PythonOracleShape, PythonOwner, PythonTest, fingerprint_probe_id, normalize_expression,
    owner_for_changed_line, python_recommended_next_step, python_weak_missing_summary,
    stop_reason_for_python_static_limit,
};
use crate::domain::{
    Confidence, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use std::path::Path;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
) -> Option<Finding> {
    classify_change_with_old(file, line, line_text, None, owners, all_tests)
}

#[cfg(test)]
pub(super) fn classify_change_with_old(
    file: &Path,
    line: usize,
    line_text: &str,
    old_line_text: Option<&str>,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
) -> Option<Finding> {
    classify_change_with_context(
        file,
        line,
        line_text,
        old_line_text,
        owners,
        all_tests,
        PythonNoBehaviorContext::default(),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PythonNoBehaviorContext {
    pub(super) new_line_in_docstring: bool,
    pub(super) old_line_in_docstring: bool,
}

/// Classify a change from producer-owned owner, relation, and oracle facts.
/// Discriminator narration follows the final class, not oracle strength alone.
pub(super) fn classify_change_with_context(
    file: &Path,
    line: usize,
    line_text: &str,
    old_line_text: Option<&str>,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
    no_behavior: PythonNoBehaviorContext,
) -> Option<Finding> {
    // No-op / no-behavior-delta guard (#1279): a docstring-only, comment-only, or
    // blank-line change has no runtime behavior, so there is nothing for a test to
    // discriminate. Emit no probe — crediting `exposed` here would falsely imply the
    // tests notice a behavior change that does not exist. The change is a no-op only
    // when BOTH sides are non-behavioral: if real code was replaced by a docstring
    // (old line behavioral), keep analyzing rather than silently dropping it.
    let new_is_noop = no_behavior.new_line_in_docstring || is_python_no_behavior_line(line_text);
    let old_is_noop = old_line_text
        .is_none_or(|old| no_behavior.old_line_in_docstring || is_python_no_behavior_line(old));
    if new_is_noop && old_is_noop {
        return None;
    }
    // Annotation-only `def`-header guard (#1289): Python does not enforce type
    // annotations at runtime, so a `def` change that touches only parameter/return
    // annotations — leaving the callable's runtime signature (name, parameter names
    // and order, default VALUES, *args/**kwargs, async-ness) unchanged — has no
    // behavior delta. Emit no probe rather than crediting `exposed` from a test that
    // reaches the owner. Requires the paired old line; fails closed when anything
    // beyond an annotation differs (e.g. a default-value change, which IS behavioral).
    if let Some(old) = old_line_text
        && is_annotation_only_def_change(old, line_text)
    {
        return None;
    }
    let owner = owner_for_changed_line(file, line, owners)?;
    // Bare-variable annotation-only suppression at MODULE SCOPE only (#1289):
    // Python does not enforce type annotations at runtime at module scope, so a
    // module-scope annotated-variable change that touches ONLY the annotation
    // (identical target name and value) has no behavior delta — emit no probe.
    // Class-body annotation changes are deliberately NOT suppressed: `@dataclass`,
    // Pydantic `BaseModel`, and `attrs` make annotations runtime-meaningful
    // (they drive validation/coercion), and base-class tracking does not exist
    // yet, so the safe stance is to fail closed for every class body. Also fails
    // closed when anything beyond the annotation differs (value, target) or the
    // line is not a parseable annotated assignment.
    if owner.is_module_owner()
        && let Some(old) = old_line_text
        && is_annotation_only_var_change(old, line_text)
    {
        return None;
    }
    let related_candidates = related_test_candidates(owner, all_tests);
    let related = find_related_tests(owner, all_tests);
    let alignment =
        classify_sink_alignment_with_old(owner, line_text, old_line_text, &related, all_tests);
    let static_limit = static_limit_for_change(line_text, owner, &related_candidates);
    let (family, delta) = classify_probe_shape(line_text);
    let has_oracle_eligible_relation = related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle());
    let strongest_strength = related
        .iter()
        .map(|test| test.oracle_strength.rank())
        .max()
        .unwrap_or(0);
    let strongest_kind = related
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .map(|test| test.oracle_kind.clone())
        .unwrap_or(OracleKind::Unknown);
    // A raise / error-path change is discriminated only by an oracle that observes the
    // RAISED exception (`pytest.raises` / `assertRaises`). A strong normal-path value
    // oracle reaches the owner but never triggers the changed raise — e.g.
    // `raise ValueError` -> `KeyError` on an `if not text:` branch, with a test that
    // only calls `parse("42")` — so it does not discriminate the change (#1290 Class C).
    // Require an exception-observing oracle for an ErrorPath change before crediting
    // `exposed`; otherwise it falls through to the strong-but-orthogonal weak branch.
    let error_path_oracle_ok = !matches!(family, ProbeFamily::ErrorPath)
        || matches!(
            strongest_kind,
            OracleKind::ExactErrorVariant | OracleKind::BroadError
        );

    // A changed default VALUE is discriminated only by a call that OMITS the
    // parameter (and so reaches the default). If every strong related test binds
    // the changed parameter explicitly — `render("Sam", verbose=False)` for a
    // `verbose=True` default change — the changed default is never exercised, so a
    // strong observing oracle does not discriminate it (#1289 trap 45). Block
    // `exposed` in that case and name the parameter(s) to test by omission.
    let changed_default_override =
        changed_default_overridden_params(old_line_text, line_text, owner, &related_candidates);
    let changed_default_exercised_ok = changed_default_override.is_none();

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if static_limit
        .is_some()
    {
        (
            ExposureClass::StaticUnknown,
            if related.is_empty() {
                StageState::No
            } else if has_oracle_eligible_relation {
                StageState::Yes
            } else {
                StageState::Weak
            },
            if related.is_empty() {
                StageState::No
            } else if has_oracle_eligible_relation {
                StageState::Yes
            } else {
                StageState::Weak
            },
            if related.is_empty() {
                StageState::No
            } else {
                StageState::Unknown
            },
            Vec::new(),
        )
    } else if related.is_empty() {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![format!(
                "No Python test references {}; add a pytest or unittest test that calls the changed owner.",
                owner.missing_test_reference()
            )],
        )
    } else if !has_oracle_eligible_relation {
        (
            ExposureClass::WeaklyExposed,
            StageState::Weak,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Only heuristic Python test links were found for `{}`; verify the suggested test location or add a direct pytest or unittest call with an exact-value assertion.",
                owner.name
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank()
        && alignment.observes()
        && error_path_oracle_ok
        && changed_default_exercised_ok
    {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related Python test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value discriminator.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank()
        && alignment.observes()
        && error_path_oracle_ok
        && let Some(params) = &changed_default_override
    {
        // A strong oracle observes the owner's output, but every reaching call binds
        // the changed-default parameter(s), so the changed default is never
        // exercised. Fail closed to weakly_exposed and name the parameter(s) to test
        // by omission (#1289 trap 45).
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "A strong Python oracle reaches `{}`, but every related call passes {} explicitly, so the changed default value is never exercised; static evidence cannot confirm the changed default is discriminated. Add a test that calls `{}` without {} to exercise the changed default.",
                owner.name,
                format_param_name_list(params),
                owner.name,
                format_param_name_list(params),
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        // A strong oracle reaches the owner, but its assertion observes a value
        // other than the changed owner's output, so static evidence cannot
        // confirm the changed behavior is discriminated. Fail closed to
        // weakly_exposed rather than crediting reach-plus-strong-oracle as
        // discrimination (which would degrade `exposed` back into coverage).
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "A strong Python oracle reaches `{}`, but its assertion does not observe the changed owner's output; static evidence cannot confirm the changed behavior is discriminated. Add an assertion on the changed owner's output.",
                owner.name
            )],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![python_weak_missing_summary(owner, &family, &strongest_kind)],
        )
    };
    if let Some(limit) = &static_limit {
        missing.push(limit.missing.clone());
    }

    // Surface the sink alignment only where the classifier actually consulted a
    // strong oracle: the `exposed` branch and the strong-but-orthogonal
    // `weakly_exposed` branch. No-static-path, static-limit, heuristic-only, and
    // weak-oracle findings never computed owner alignment, so they read
    // `unknown` (the `changed_sink` of the changed line is still retained).
    let surfaced_alignment = if matches!(class, ExposureClass::Exposed)
        || (matches!(class, ExposureClass::WeaklyExposed)
            && static_limit.is_none()
            && has_oracle_eligible_relation
            && strongest_strength >= OracleStrength::Strong.rank())
    {
        alignment
    } else {
        SinkAlignment::unknown(alignment.changed_sink.clone())
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let canonical_gap = static_limit
        .is_none()
        .then(|| canonical_python_gap_for(file, owner, &family, line_text));
    let owner_id = owner.symbol_id();
    let probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "python_preview",
        owner_id.0.as_str(),
        &normalize_expression(line_text),
        1,
    );
    let probe = Probe {
        id: probe_id,
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner.symbol_id()),
        family: family.clone(),
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };

    let related_count = related.len();
    let reach_summary = if related_count == 0 {
        format!("0 related Python test(s) found for owner `{}`", owner.name)
    } else if has_oracle_eligible_relation {
        format!(
            "{} related Python test(s) found for owner `{}`",
            related_count, owner.name
        )
    } else {
        format!(
            "{} heuristic Python test link(s) found for owner `{}`; relation is uncertain",
            related_count, owner.name
        )
    };
    let reach = StageEvidence::new(reach_state, Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        if static_limit.is_some() {
            StageState::Unknown
        } else {
            StageState::Yes
        },
        Confidence::Low,
        if let Some(limit) = &static_limit {
            format!(
                "Static limit `{}` prevents a safe Python infection claim.",
                limit.kind.as_str()
            )
        } else {
            python_infection_evidence(&family, line_text).summary
        },
    );
    let propagate = python_propagation_evidence(&family, line_text, static_limit.as_ref());
    let flow_sink = static_limit
        .is_none()
        .then(|| python_flow_sink_for(&family, owner, line, line_text))
        .flatten();
    let missing_discriminators = if static_limit.is_none()
        && matches!(class, ExposureClass::WeaklyExposed)
        && has_oracle_eligible_relation
    {
        if let Some(params) = &changed_default_override {
            // The override downgrade names a specific, actionable missing
            // discriminator — "call the owner WITHOUT the changed-default
            // parameter(s)" — that the generic `python_missing_discriminators`
            // cannot derive for a `def` header (no comparison operator to read).
            // Populate it directly so the structured field (repair card,
            // recommended_next_step) carries the omission guidance, not just the
            // top-level `missing` string.
            vec![MissingDiscriminatorFact {
                value: format!(
                    "call `{}` without {}",
                    owner.name,
                    format_param_name_list(params)
                ),
                reason: format!(
                    "changed default parameter(s) {} at line {line} are always explicitly bound, so the default is never exercised",
                    format_param_name_list(params)
                ),
                flow_sink: flow_sink.clone(),
            }]
        } else {
            python_missing_discriminators(&family, line, line_text, owner, flow_sink.as_ref())
        }
    } else {
        Vec::new()
    };
    let observe = StageEvidence::new(
        observe_state,
        Confidence::Low,
        format!(
            "Strongest extracted Python oracle kind: `{}` (rank {})",
            strongest_kind.as_str(),
            strongest_strength
        ),
    );
    let discriminate_summary = if let Some(limit) = &static_limit {
        format!(
            "Static limit `{}` prevents a safe Python discriminator claim.",
            limit.kind.as_str()
        )
    } else if matches!(class, ExposureClass::Exposed) {
        format!(
            "Related Python test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        // Strength alone cannot override the classifier's alignment, error-path,
        // or default-activation decision. Keep the narrative at that verdict.
        format!(
            "Related Python test uses a `{}` oracle, but static evidence does not establish discrimination of the changed behavior.",
            strongest_kind.as_str()
        )
    } else {
        missing_discriminators
            .first()
            .map(|missing| {
                format!(
                    "Python preview adapter found no strong discriminator; missing proof: `{}`.",
                    missing.value
                )
            })
            .unwrap_or_else(|| {
                "Python preview adapter found no strong discriminator; typed repair guidance is unavailable for this shape.".to_string()
            })
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, discriminate_summary);

    let recommended = python_recommended_next_step(
        &class,
        &family,
        has_oracle_eligible_relation,
        &missing_discriminators,
    );
    let repair_placement = python_repair_placement(&class, &related_candidates);
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else if matches!(class, ExposureClass::StaticUnknown) {
        0.2
    } else {
        0.4
    };

    let mut evidence = vec![
        format!("owner: {}", owner.qualified_name),
        format!("owner_kind: {}", owner.kind_label()),
    ];
    if !owner.decorators.is_empty() {
        evidence.push(format!("owner_decorators: {}", owner.decorators.join(", ")));
    }
    if let Some(limit) = &static_limit {
        evidence.push(limit.evidence.clone());
    }
    for discriminator in &missing_discriminators {
        evidence.push(format!("missing_discriminator: {}", discriminator.value));
    }
    if let Some(placement) = &repair_placement {
        evidence.push(format!(
            "suggested_repair_action: {}",
            placement.repair_action
        ));
        evidence.push(format!(
            "suggested_test_file: {}",
            placement.suggested_test_file
        ));
        evidence.push(format!(
            "suggested_test_name: {}",
            placement.suggested_test_name
        ));
        if let Some(node_id) = &placement.suggested_test_node_id {
            evidence.push(format!("suggested_test_node_id: {node_id}"));
        }
        evidence.push(format!(
            "suggested_verify_command: {}",
            placement.verify_command
        ));
        evidence.push(format!(
            "suggested_verify_command_confidence: {}",
            placement.verify_command_confidence
        ));
        evidence.push(format!(
            "suggested_test_location_reason: {}",
            placement.location_reason
        ));
    }
    for candidate in related_candidates {
        let test = candidate.test;
        evidence.push(format!(
            "test_framework: {} ({})",
            test.framework, test.name
        ));
        if !test.fixtures.is_empty() {
            evidence.push(format!(
                "test_fixtures: {} ({})",
                test.fixtures.join(", "),
                test.name
            ));
        }
        if test.parametrized {
            evidence.push(format!("test_parametrized: pytest ({})", test.name));
        }
        if let Some(command) = verify_command_for_test(test) {
            evidence.push(format!("test_verify_command: {command} ({})", test.name));
        }
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            test.name
        ));
        if candidate.relation.is_uncertain() {
            evidence.push(format!(
                "related_test_uncertain: {} ({})",
                candidate.relation.as_str(),
                test.name
            ));
        }
        if candidate.relation.uses_oracle()
            && let Some(assertion) = strongest_assertion(&test.assertions)
        {
            evidence.push(format!(
                "test_oracle: {} {} ({})",
                assertion.oracle_kind.as_str(),
                assertion.oracle_strength.as_str(),
                test.name
            ));
            if assertion.oracle_shape != PythonOracleShape::ExactAssertion {
                evidence.push(format!(
                    "test_oracle_shape: {} ({})",
                    assertion.oracle_shape.as_str(),
                    test.name
                ));
            }
        } else if candidate.relation.uses_oracle() {
            evidence.push(format!("test_oracle_shape: reach_only ({})", test.name));
        }
    }

    // Resolved from the probe's own delta evidence (#3281) before the probe
    // moves into the finding: Python probes are seeded from head-side added
    // lines.
    let source_currentness = crate::domain::SourceCurrentness::from_probe_delta(
        probe.before.as_deref(),
        probe.after.as_deref(),
    );

    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap,
        probe,
        class,
        ripr: RiprEvidence {
            reach,
            infect,
            propagate,
            reveal: RevealEvidence {
                observe,
                discriminate,
            },
        },
        confidence: confidence_value,
        evidence,
        missing,
        flow_sinks: flow_sink.into_iter().collect(),
        activation: crate::domain::ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: static_limit
            .as_ref()
            .map(stop_reason_for_python_static_limit)
            .into_iter()
            .collect(),
        related_tests: related,
        recommended_next_step: recommended,
        language: Some(DomainLanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: owner.owner_kind,
        static_limit_kind: static_limit.map(|limit| limit.kind),
        changed_sink: surfaced_alignment.changed_sink,
        observed_sink: surfaced_alignment.observed_sink,
        oracle_alignment: Some(surfaced_alignment.oracle_alignment),
        alignment_reason: Some(surfaced_alignment.alignment_reason),
        // Resolved above, before the probe moved into the finding (#3281).
        source_currentness,
    })
}
