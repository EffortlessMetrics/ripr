//! Change classifier for the TypeScript preview adapter.

use super::*;

pub(crate) fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    all_tests: &[TypeScriptTest],
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related_candidates = related_test_candidates(owner, all_tests);
    let related = find_related_tests(owner, all_tests);
    let bun_array_buffer_facts = collect_related_bun_array_buffer_facts(&related_candidates);
    let bun_bridge_hints = collect_related_bun_bridge_hints(&bun_array_buffer_facts);
    let mock_paths = collect_related_mock_paths(owner, all_tests);
    let static_limit = static_limit_for_change(line_text, owner, &mock_paths);

    // Collect named TS-specific limitations (RIPR-SPEC-0085 §PR4 taxonomy).
    // These are ADDITIVE evidence lines; existing `static_limit_kind` is unchanged.
    let named_limitations_from_static: Vec<TypeScriptNamedLimitation> = static_limit
        .as_ref()
        .and_then(|limit| named_limitation_for_static_limit(limit, file, line))
        .into_iter()
        .collect();
    // Oracle-based limitations fire from oracle-eligible candidates even when
    // there is no static_limit. We always compute them; they are empty when there
    // are no oracle-eligible candidates or no qualifying assertions.
    let named_limitations_from_oracle =
        named_limitations_for_oracle_candidates(&related_candidates);
    let has_oracle_eligible_relation = related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle());
    let probe_shape = classify_probe_shape_detail(line_text);

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
    let mock_payload_oracle = related_mock_payload_oracle(&related);

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if related.is_empty()
    {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![no_static_path_missing(owner)],
        )
    } else if !has_oracle_eligible_relation {
        (
            ExposureClass::WeaklyExposed,
            StageState::Weak,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Only heuristic TypeScript test links were found for `{}`; verify the suggested test location or add a direct Jest/Vitest owner call with an exact-value assertion.",
                owner.name
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value or exact-error-variant discriminator.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![weak_oracle_missing_summary(
                &owner.name,
                &strongest_kind,
                &probe_shape.family,
                mock_payload_oracle.as_deref(),
            )],
        )
    };
    if let Some(limit) = &static_limit {
        missing.push(limit.missing.clone());
    }

    let flow_sink = typescript_flow_sink_for(&probe_shape, owner, line, line_text);
    let missing_discriminators = if matches!(class, ExposureClass::WeaklyExposed)
        && has_oracle_eligible_relation
        && static_limit.is_none()
    {
        typescript_missing_discriminators(&probe_shape, line, line_text, flow_sink.as_ref())
    } else {
        Vec::new()
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let family = probe_shape.family.clone();
    let delta = probe_shape.delta.clone();
    let expected_sinks = if probe_shape.specific {
        probes::expected_sinks(line_text, &family)
    } else {
        Vec::new()
    };
    let required_oracles = if probe_shape.specific {
        probes::required_oracles(line_text, &family)
    } else {
        Vec::new()
    };
    let owner_sym = owner.symbol_id();
    let ts_probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "typescript_preview",
        owner_sym.0.as_str(),
        &normalize_expression(line_text),
        1,
    );
    let probe = Probe {
        id: ts_probe_id,
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner_sym),
        family: family.clone(),
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks,
        required_oracles,
    };
    let actionability = typescript_actionability_for(
        &class,
        static_limit.as_ref(),
        has_oracle_eligible_relation,
        &missing_discriminators,
    );
    missing.push(actionability.missing_summary());

    let related_count = related.len();
    let reach_summary = format!(
        "{} related test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state.clone(), Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model propagation.",
    );
    let observe_summary = format!(
        "Strongest extracted oracle kind: `{}` (rank {})",
        strongest_kind.as_str(),
        strongest_strength
    );
    let observe = StageEvidence::new(observe_state, Confidence::Low, &observe_summary);
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank() {
        format!(
            "Related test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else if let Some(discriminator) = missing_discriminators.first() {
        format!(
            "TypeScript preview adapter found no strong discriminator; missing proof: `{}`.",
            discriminator.value
        )
    } else {
        "TypeScript preview adapter found no strong discriminator; use `toBe` / `toEqual` / `toStrictEqual` to escalate. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = if let Some(limit) = &static_limit {
        format!(
            "TypeScript preview advisory: static limit `{}`; {}; no actionable repair packet is emitted.",
            limit.kind.as_str(),
            limit.repair_route
        )
    } else {
        match &class {
        ExposureClass::Exposed => {
            "TypeScript preview advisory: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            no_static_path_recommendation(owner)
        }
        _ if !has_oracle_eligible_relation => {
            "TypeScript preview advisory: related-test proximity is heuristic only; add a direct owner call before treating this as an actionable repair target.".to_string()
        }
        _ if let Some(discriminator) = missing_discriminators.first() => {
            weak_oracle_recommendation(
                &strongest_kind,
                &discriminator.value,
                mock_payload_oracle.as_deref(),
            )
        }
        _ if owner.owner_kind == OwnerKind::ModuleFunction => {
            format!(
                "TypeScript preview advisory: related module-initializer observer reaches `{}` but no safe target shape is available; add an exact value assertion for the exported value and keep the finding advisory until repair-card fields are complete.",
                owner.name
            )
        }
        _ => {
            "TypeScript preview advisory: add a test that exercises the changed behavior with an exact-value assertion (`toBe` / `toEqual` / `toStrictEqual`); no actionable repair packet is emitted until the target shape is explicit.".to_string()
        }
        }
    };
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else {
        0.4
    };

    let mut evidence = vec![format!("owner: {}", owner.name)];
    if !probe_shape.specific {
        evidence.push("probe_fact: ambiguous_fallback".to_string());
    }
    for discriminator in &missing_discriminators {
        evidence.push(format!("missing_discriminator: {}", discriminator.value));
    }
    if let Some(oracle) = &mock_payload_oracle {
        evidence.push(format!("mock_payload_evidence: {oracle}"));
    }
    for fact in &bun_array_buffer_facts {
        evidence.push(fact.evidence_line());
    }
    for hint in &bun_bridge_hints {
        evidence.extend(hint.evidence_lines());
    }
    if let Some(limit) = &static_limit {
        evidence.extend(limit.evidence.iter().cloned());
    }
    // Emit additive named limitation evidence lines (RIPR-SPEC-0085 §PR4).
    // These lines are ADDITIVE — they do not change any existing field value.
    for named_limit in named_limitations_from_static
        .iter()
        .chain(named_limitations_from_oracle.iter())
    {
        evidence.extend(named_limit.evidence_lines());
    }
    evidence.extend(actionability.evidence(typescript_raw_evidence_ref(
        file,
        line,
        Some(owner),
        &probe.id.0,
    )));
    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.is_uncertain())
    {
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
        evidence.push(format!(
            "related_test_uncertain: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
    }
    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
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
        activation: ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(owner.owner_kind),
        static_limit_kind: static_limit.map(|limit| limit.kind),
    })
}

pub(crate) fn no_static_path_missing(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => format!(
            "No trusted TypeScript method receiver relation for `{}`. Direct `new ClassName(...)` receiver calls are supported, but factories, dependency injection, mocked modules, prototype aliases, and dynamic property access stay ambiguous in preview.",
            owner.name
        ),
        OwnerKind::ClassMethod => format!(
            "No trusted TypeScript class-method relation for `{}`. Direct same-file or imported `Class.method(...)` calls are supported, but local shadows, mocked modules, namespace chains, dynamic member access, and missing class-name context stay ambiguous in preview.",
            owner.name
        ),
        OwnerKind::ModuleFunction => format!(
            "No trusted TypeScript module-initializer observer for `{}`. Direct `expect(IMPORTED_CONST)...` and `expect(namespace.EXPORT)...` observers are supported, but helper-derived values, shadowed aliases, dynamic initialization, and non-expect references stay advisory in preview.",
            owner.name
        ),
        _ => format!(
            "No test references `{}(` — add a test that calls the changed owner.",
            owner.name
        ),
    }
}

pub(crate) fn no_static_path_recommendation(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => {
            "TypeScript preview advisory: method receiver relation is missing context; use a direct `new ClassName(...)` receiver observer when safe, and keep factories, dependency injection, mocked modules, prototype aliases, and dynamic property access advisory.".to_string()
        }
        OwnerKind::ClassMethod => {
            "TypeScript preview advisory: class-method relation is missing context; use a direct same-file or imported `Class.method(...)` observer when safe, and keep local shadows, mocked modules, namespace chains, dynamic member access, and missing class-name context advisory.".to_string()
        }
        OwnerKind::ModuleFunction => {
            "TypeScript preview advisory: module initializer observer is missing context; add a direct `expect(IMPORTED_CONST).toBe(...)` or `expect(namespace.EXPORT).toEqual(...)` observer when safe, and keep helper-derived or dynamic initialization evidence advisory.".to_string()
        }
        _ => {
            "TypeScript preview advisory: no test references the changed owner; add a test that calls the owner and asserts the changed behavior with `toBe` / `toEqual` before any repair packet is emitted.".to_string()
        }
    }
}
