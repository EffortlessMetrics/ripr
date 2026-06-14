use super::super::rust_index::{OracleFact, TestSummary, extract_identifier_tokens};
use crate::domain::*;

pub(in crate::analysis) fn reveal_evidence(
    probe: &Probe,
    related_tests: &[(&TestSummary, RelationReason)],
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
    let discriminate = build_discriminate_evidence(
        &analysis.strongest,
        &analysis.strongest_kind,
        &probe.family,
        analysis.observation_unverified,
    );

    (observe, discriminate, related)
}

struct RevealAssertionAnalysis {
    related: Vec<RelatedTest>,
    strongest: OracleStrength,
    strongest_kind: OracleKind,
    matched_any: bool,
    /// True when this probe's family requires a `token_match` to confirm that
    /// an assertion actually references the specific changed sub-expression, and
    /// no such match has fired yet.
    ///
    /// Applies to: `MatchArm`, `ReturnValue`, `FieldConstruction`, `SideEffect`,
    /// `CallDeletion`. For each of these families, the broad `family_match` or
    /// `assertion_count == 1` matcher alone is insufficient: it tells us an
    /// oracle of the right shape exists in a reachable test, but cannot confirm
    /// it observes *this particular* changed expression (vs. a sibling, an
    /// unrelated field, or a different call site).
    ///
    /// Confirmation differs by family:
    /// - **Value** families (MatchArm, ReturnValue, FieldConstruction): the only
    ///   static signal of specificity is a `token_match` — an assertion whose
    ///   text contains an identifier token from the probe expression.
    /// - **Effect** families (SideEffect, CallDeletion): the canonical observer
    ///   is a mock/expectation/snapshot that kind-matches the seam without
    ///   sharing a token, so a genuine effect observer (`effect_observer_confirms`)
    ///   confirms in addition to `token_match`.
    ///
    /// Cleared as soon as a confirming assertion fires.
    observation_unverified: bool,
}

/// Returns true for families where an assertion must specifically reference the
/// changed sub-expression to confirm observation. For **value** families
/// (MatchArm, ReturnValue, FieldConstruction) the only static confirmation
/// signal is a `token_match`. For **effect** families (SideEffect, CallDeletion)
/// the legitimate observer is often a mock/expectation that **kind-matches the
/// seam** without sharing any probe token; for those, a seam-kind match also
/// confirms observation (see `effect_observer_confirms`).
fn needs_token_confirmation(family: &ProbeFamily) -> bool {
    matches!(
        family,
        ProbeFamily::MatchArm
            | ProbeFamily::ReturnValue
            | ProbeFamily::FieldConstruction
            | ProbeFamily::SideEffect
            | ProbeFamily::CallDeletion
    )
}

/// Returns true for the **effect** families (SideEffect, CallDeletion) whose
/// changed behavior is a side effect or outbound call. For these, the canonical
/// observer is a mock/expectation that kind-matches the seam rather than a
/// value assertion that names a token from the changed expression. A genuine
/// effect observer therefore confirms observation even without a `token_match`.
fn is_effect_family(family: &ProbeFamily) -> bool {
    matches!(family, ProbeFamily::SideEffect | ProbeFamily::CallDeletion)
}

/// Returns true when `assertion` is a genuine **effect observer** that
/// kind-matches an effect seam: a mock/expectation, a snapshot, or a
/// whole-object equality capturing the resulting state. This is intentionally
/// narrower than `oracle_matches_family` for effect families — it excludes the
/// broad `text.contains("assert")` / `text.contains("expect")` substring
/// matches, so a plain non-observing assertion (e.g. `assert!(result)`) does
/// **not** clear `observation_unverified`. Only a real expectation/snapshot
/// observer does.
fn effect_observer_confirms(assertion: &OracleFact) -> bool {
    matches!(
        assertion.kind,
        OracleKind::MockExpectation | OracleKind::Snapshot | OracleKind::WholeObjectEquality
    )
}

/// For a `MatchArm` probe expression, extract only the "variant" tokens —
/// the identifier segments that appear immediately after a `::` separator.
/// These are the arm-specific tokens that can confirm an assertion targets
/// this arm rather than a sibling sharing the same enum qualifier.
///
/// Example: `"Mode::Frozen => -1,"` → `["Frozen"]`.
/// Example: `"Status::Active | Status::Idle => 0,"` → `["Active", "Idle"]`.
/// Example: `"None => 0,"` → `[]` (no `::` in expression).
fn match_arm_variant_tokens(expression: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let bytes = expression.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 1 < len {
        if bytes[i] == b':' && bytes[i + 1] == b':' {
            // skip "::"
            i += 2;
            // collect the identifier that follows
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if i > start {
                let variant = &expression[start..i];
                if extract_identifier_tokens(variant).contains(&variant.to_string()) {
                    variants.push(variant.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    variants
}

fn analyze_related_assertions(
    probe: &Probe,
    related_tests: &[(&TestSummary, RelationReason)],
) -> RevealAssertionAnalysis {
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    // For MatchArm: collect variant-only tokens (post-`::`) for the specificity
    // check. Qualifier tokens (e.g. the type name before `::`) are excluded so
    // that a sibling-arm assertion sharing the qualifier cannot spuriously
    // confirm observation of this arm.
    let match_arm_variants = if matches!(probe.family, ProbeFamily::MatchArm) {
        match_arm_variant_tokens(&probe.expression)
    } else {
        Vec::new()
    };
    let confirm_required = needs_token_confirmation(&probe.family);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut strongest_kind = OracleKind::Unknown;
    let mut matched_any = false;
    // For families that need token confirmation: start pessimistic and clear
    // once a token_match fires.
    let mut observation_unverified = false;

    for (test, reason) in related_tests {
        let relation_reason = Some(*reason);
        let relation_confidence = Some(reason.confidence());
        if test.assertions.is_empty() {
            related.push(RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
                relation_reason,
                relation_confidence,
            });
            continue;
        }
        for assertion in &test.assertions {
            let (matched, has_token_match) = assertion_matches_probe_detail(
                &probe_tokens,
                &match_arm_variants,
                &probe.family,
                assertion,
                test.assertions.len(),
            );
            if matched {
                if confirm_required {
                    // Observation is confirmed when the assertion specifically
                    // references the changed sub-expression. For value families
                    // (MatchArm/ReturnValue/FieldConstruction) the only static
                    // signal is a `token_match`. For effect families
                    // (SideEffect/CallDeletion) the canonical observer is a
                    // mock/expectation/snapshot that kind-matches the seam
                    // without sharing a token, so a genuine effect observer also
                    // confirms. This prevents a real mock from being wrongly
                    // flagged `observation_unverified`, while a plain
                    // non-observing assertion (no token, no effect observer)
                    // stays unverified.
                    let observation_confirmed = has_token_match
                        || (is_effect_family(&probe.family) && effect_observer_confirms(assertion));
                    if !matched_any {
                        // First matching assertion: observation is unverified
                        // unless confirmed.
                        observation_unverified = !observation_confirmed;
                    } else if observation_confirmed {
                        // A later confirmed assertion clears the unverified flag.
                        observation_unverified = false;
                    }
                }
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
                    oracle_kind: assertion.kind.clone(),
                    oracle_strength: relative_strength,
                    relation_reason,
                    relation_confidence,
                });
            }
        }
    }

    RevealAssertionAnalysis {
        related,
        strongest,
        strongest_kind,
        matched_any,
        observation_unverified,
    }
}

/// Returns `(matched, has_token_match)`.
///
/// `matched` is true when the assertion should be associated with this probe
/// (via token text, family kind, or single-assertion escape hatch).
/// `has_token_match` is true when the assertion text contains an identifier
/// token from the probe expression that is specific enough to confirm this
/// particular sub-expression is being observed.
///
/// For `MatchArm` probes, `has_token_match` uses only the **variant** tokens
/// (`match_arm_variants`, the identifiers immediately after `::` in the probe
/// expression). This prevents a sibling-arm assertion like `Mode::Warm` from
/// clearing `observation_unverified` for a probe on `Mode::Frozen`, because
/// the shared qualifier token `Mode` is excluded from the confirmation set.
/// When the expression contains no `::` (e.g. `None => 0,`), the variant
/// token list is empty and `has_token_match` is always false, reflecting that
/// the probe has no arm-specific identifier.
fn assertion_matches_probe_detail(
    probe_tokens: &[String],
    match_arm_variants: &[String],
    family: &ProbeFamily,
    assertion: &OracleFact,
    assertion_count: usize,
) -> (bool, bool) {
    let token_match = probe_tokens
        .iter()
        .any(|token| token.len() > 3 && assertion.text.contains(token));
    // For MatchArm probes, restrict the confirmation check to variant-only
    // tokens (post-`::`). The qualifier ("Mode" in "Mode::Frozen") is shared
    // across all arms and therefore cannot confirm this specific arm.
    let has_token_match = if matches!(family, ProbeFamily::MatchArm) {
        match_arm_variants
            .iter()
            .any(|v| v.len() > 3 && assertion.text.contains(v.as_str()))
    } else {
        token_match
    };
    let family_match = oracle_matches_family(family, assertion);
    let matched = token_match || family_match || assertion_count == 1;
    (matched, has_token_match)
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
    observation_unverified: bool,
) -> StageEvidence {
    // For families that require token confirmation (MatchArm, ReturnValue,
    // FieldConstruction, SideEffect, CallDeletion), a family_match or
    // assertion_count==1 alone cannot confirm that an assertion observes *this*
    // specific changed sub-expression. Without a token_match, downgrade to Weak
    // so classify() emits weakly_exposed (observation_unverified). A probe
    // with a token_match stays exposed.
    if observation_unverified {
        return StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            "Discriminator unconfirmed: no assertion text references this probe's changed expression (observation_unverified)",
        );
    }
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
            ) || text.contains("Error::")
                || text.contains("Err")
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
        ProbeFamily::ReturnValue => [
            OracleKind::ExactValue,
            OracleKind::WholeObjectEquality,
            OracleKind::RelationalCheck,
            OracleKind::Snapshot,
            OracleKind::SmokeOnly,
        ]
        .contains(&assertion.kind),
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
        ProbeFamily::MatchArm => [
            OracleKind::ExactErrorVariant,
            OracleKind::ExactValue,
            OracleKind::RelationalCheck,
            OracleKind::Snapshot,
        ]
        .contains(&assertion.kind),
        ProbeFamily::StaticUnknown => false,
    }
}

fn probe_relative_oracle_strength(family: &ProbeFamily, assertion: &OracleFact) -> OracleStrength {
    match family {
        ProbeFamily::ErrorPath => match assertion.kind {
            OracleKind::ExactErrorVariant => OracleStrength::Strong,
            OracleKind::BroadError => assertion.strength.clone(),
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
            OracleKind::Snapshot
            | OracleKind::MockExpectation
            | OracleKind::RelationalCheck
            | OracleKind::BroadError => assertion.strength.clone(),
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => match assertion.kind {
            OracleKind::MockExpectation => assertion.strength.clone(),
            OracleKind::ExactValue | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::RelationalCheck | OracleKind::BroadError => assertion.strength.clone(),
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::ExactErrorVariant => OracleStrength::Medium,
            OracleKind::Snapshot => assertion.strength.clone(),
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::StaticUnknown => OracleStrength::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reveal_evidence_keeps_assertionless_related_test_without_observe_signal() {
        let probe = probe(ProbeFamily::ReturnValue, "score");
        let test = test_with_assertions("score_returns_value", Vec::new());
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "score_returns_value");
        assert_eq!(related[0].oracle, None);
    }

    #[test]
    fn reveal_evidence_records_matching_assertions_and_sorts_related_tests() {
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
        );
        let late = test_with_assertions(
            "z_error_path",
            vec![oracle(
                "assert!(score(\"\").is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let early = test_with_assertions(
            "a_error_path",
            vec![oracle(
                "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, related) = reveal_evidence(
            &probe,
            &[
                (&late, RelationReason::DirectOwnerCall),
                (&early, RelationReason::DirectOwnerCall),
            ],
        );

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Yes);
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].name, "a_error_path");
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
        assert_eq!(related[1].name, "z_error_path");
    }

    #[test]
    fn reveal_evidence_ignores_unmatched_assertions() {
        let probe = probe(ProbeFamily::StaticUnknown, "opaque_changed_expr");
        let test = test_with_assertions(
            "opaque_behavior",
            vec![
                oracle(
                    "assert_eq!(unrelated, 3);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
                oracle(
                    "assert!(other_value);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
            ],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::WeakTokenSubstring)]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert!(related.is_empty());
    }

    #[test]
    fn assertion_matching_accepts_token_family_and_single_assertion_fallbacks() {
        let token_assertion = oracle(
            "assert_eq!(score, 3);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["score".to_string()],
            &[],
            &ProbeFamily::StaticUnknown,
            &token_assertion,
            2,
        );
        assert!(matched, "token match must fire");
        assert!(has_token, "token match must set has_token_match");

        let family_assertion = oracle(
            "assert!(result.is_err());",
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["err".to_string()],
            &[],
            &ProbeFamily::ErrorPath,
            &family_assertion,
            2,
        );
        assert!(matched, "family match must fire");
        assert!(!has_token, "family-only match must not set has_token_match");

        let fallback_assertion = oracle(
            "assert!(ran);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["run".to_string()],
            &[],
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            1,
        );
        assert!(matched, "single-assertion fallback must fire");
        assert!(
            !has_token,
            "escape-hatch-only match must not set has_token_match"
        );

        let (matched, _) = assertion_matches_probe_detail(
            &["run".to_string()],
            &[],
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            2,
        );
        assert!(!matched, "fallback must not fire for assertion_count > 1");
    }

    #[test]
    fn discriminate_evidence_names_strength_and_oracle_kind() {
        let cases = [
            (
                OracleStrength::Strong,
                OracleKind::ExactErrorVariant,
                ProbeFamily::ErrorPath,
                StageState::Yes,
                "Strong oracle found: exact error variant assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::WholeObjectEquality,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: whole-object equality assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: exact value or pattern assertion",
            ),
            (
                OracleStrength::Medium,
                OracleKind::Snapshot,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: snapshot assertion observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::MockExpectation,
                ProbeFamily::SideEffect,
                StageState::Weak,
                "Medium oracle found: mock or expectation observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: property or partial structural assertion",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ErrorPath,
                StageState::Weak,
                "Only broad error oracle found; is_err() does not discriminate exact error variants",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only broad error oracle found; it may not discriminate the changed behavior exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::RelationalCheck,
                ProbeFamily::Predicate,
                StageState::Weak,
                "Only relational oracle found; it may not discriminate the changed value exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only weak oracle found, such as a broad relational assertion or non-empty check",
            ),
            (
                OracleStrength::Smoke,
                OracleKind::SmokeOnly,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
            ),
            (
                OracleStrength::None,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::No,
                "No assertion found on related tests",
            ),
            (
                OracleStrength::Unknown,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::Unknown,
                "Assertions exist, but oracle strength is unknown",
            ),
        ];

        for (strength, kind, family, state, summary) in cases {
            let evidence = build_discriminate_evidence(&strength, &kind, &family, false);
            assert_eq!(evidence.state, state);
            assert_eq!(evidence.summary, summary);
        }
    }

    #[test]
    fn oracle_family_matching_covers_family_specific_shapes() {
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_matches!(result, Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(matches!(result, Err(_)));",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_eq!(kind, Error::Denied);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.saved);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.published);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_eq!(item.id, 3);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_debug_snapshot!(item);",
                OracleKind::Snapshot,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::Predicate,
            &oracle(
                "assert!(value >= 3);",
                OracleKind::RelationalCheck,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "assert_eq!(score(), 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "score().unwrap();",
                OracleKind::SmokeOnly,
                OracleStrength::Smoke
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "assert!(sent);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "expect_send_called();",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_matches!(kind, Ready);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_eq!(kind, Ready);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(!oracle_matches_family(
            &ProbeFamily::StaticUnknown,
            &oracle(
                "assert_eq!(value, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
    }

    #[test]
    fn probe_relative_oracle_strength_preserves_family_overrides() {
        let cases = [
            (
                ProbeFamily::ErrorPath,
                oracle("exact", OracleKind::ExactErrorVariant, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("broad", OracleKind::BroadError, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::ReturnValue,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::Predicate,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::FieldConstruction,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::MatchArm,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("mock", OracleKind::MockExpectation, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact",
                    OracleKind::WholeObjectEquality,
                    OracleStrength::Weak,
                ),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("rel", OracleKind::RelationalCheck, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact_error",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::StaticUnknown,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
        ];

        for (family, assertion, expected) in cases {
            assert_eq!(
                probe_relative_oracle_strength(&family, &assertion),
                expected
            );
        }
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

    fn test_with_assertions(name: &str, assertions: Vec<OracleFact>) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 1,
            end_line: 3,
            body: "score();".to_string(),
            calls: Vec::new(),
            assertions,
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn oracle(text: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: text.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(text),
        }
    }

    // --- RIPR-SPEC-0093 arm-blind downgrade ---

    /// A MatchArm probe whose expression has no extractable tokens (e.g. `None`
    /// is filtered) and whose single related test has an ExactValue oracle for a
    /// DIFFERENT arm must emit weakly_exposed with observation_unverified.
    #[test]
    fn match_arm_probe_without_token_match_downgrades_discriminate_to_weak() {
        // probe expression "None => 0," — None is filtered, 0 is not alpha
        let probe = probe(ProbeFamily::MatchArm, "None => 0,");
        let test = test_with_assertions(
            "some_arm_returns_incremented_value",
            vec![oracle(
                "assert_eq!(reason(Some(5)), 6);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes, "observe must still fire");
        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "discriminate must be downgraded to Weak (observation_unverified)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// A MatchArm probe whose expression DOES have a token that appears in the
    /// assertion text (token_match) must stay exposed (StageState::Yes).
    #[test]
    fn match_arm_probe_with_token_match_keeps_discriminate_yes() {
        // probe expression "Status::Idle => 0," — Idle and Status are extractable
        let probe = probe(ProbeFamily::MatchArm, "Status::Idle => 0,");
        let test = test_with_assertions(
            "idle_arm_returns_zero",
            vec![oracle(
                "assert_eq!(classify(Status::Idle), 0);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "token_match on Idle must keep discriminate Yes (no over-correction)"
        );
    }

    /// A ReturnValue probe whose single related test has a family-matching assertion
    /// but NO token referencing the changed expression must downgrade to Weak.
    /// This inverts the former bug-locking test
    /// `non_match_arm_probe_family_match_only_keeps_discriminate_yes`.
    #[test]
    fn return_value_family_match_only_without_token_downgrades_discriminate_to_weak() {
        // "value + 1" — tokens: ["value"] (len 5). Assertion "assert_eq!(compute(), 42);"
        // has no occurrence of "value", so has_token_match=false.
        let probe = probe(ProbeFamily::ReturnValue, "value + 1");
        let test = test_with_assertions(
            "returns_incremented",
            vec![oracle(
                "assert_eq!(compute(), 42);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ReturnValue probe with no token_match must downgrade to Weak (observation_unverified)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// A ReturnValue probe whose assertion text CONTAINS a token from the probe
    /// expression must stay exposed (StageState::Yes) — no over-correction.
    #[test]
    fn return_value_with_token_match_keeps_discriminate_yes() {
        // "score + 1" — tokens: ["score"] (len 5). Assertion contains "score".
        let probe = probe(ProbeFamily::ReturnValue, "score + 1");
        let test = test_with_assertions(
            "score_incremented",
            vec![oracle(
                "assert_eq!(score(), 6);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "ReturnValue probe with token_match must stay Yes (no over-correction)"
        );
    }

    /// A FieldConstruction probe whose single assertion has a dot (family_match)
    /// but no token referencing the specific changed field must downgrade to Weak.
    #[test]
    fn field_construction_family_match_only_without_token_downgrades_to_weak() {
        // "priority: 3" — tokens: ["priority"] (len 8). Assertion "assert_eq!(item.id, 3);"
        // does not contain "priority".
        let probe = probe(ProbeFamily::FieldConstruction, "priority: 3");
        let test = test_with_assertions(
            "item_has_id",
            vec![oracle(
                "assert_eq!(item.id, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "FieldConstruction probe with no token_match must downgrade to Weak"
        );
    }

    /// A FieldConstruction probe whose assertion references the exact changed field
    /// must stay exposed (StageState::Yes).
    #[test]
    fn field_construction_with_token_match_keeps_discriminate_yes() {
        // "priority: 3" — tokens: ["priority"]. Assertion "assert_eq!(item.priority, 3);"
        // contains "priority".
        let probe = probe(ProbeFamily::FieldConstruction, "priority: 3");
        let test = test_with_assertions(
            "item_has_priority",
            vec![oracle(
                "assert_eq!(item.priority, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "FieldConstruction probe with token_match on priority must stay Yes"
        );
    }

    /// A SideEffect probe whose single PLAIN assertion has neither a token
    /// referencing the changed effect NOR an effect-observer kind (no mock,
    /// snapshot, or whole-object) must emit observation_unverified. This is the
    /// genuinely-blind case: the assertion only fired via the single-assertion
    /// escape hatch.
    #[test]
    fn side_effect_plain_assertion_without_token_or_observer_emits_observation_unverified() {
        // "send_notification(user_id)" — tokens: ["send", "notification", "user"].
        // Assertion "assert!(ran);" contains none of those, and is not a mock,
        // snapshot, or whole-object observer.
        let probe = probe(ProbeFamily::SideEffect, "send_notification(user_id)");
        let test = test_with_assertions(
            "ran",
            vec![oracle(
                "assert!(ran);",
                OracleKind::Unknown,
                OracleStrength::Unknown,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "SideEffect probe with no token and no effect observer must downgrade to Weak"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "plain non-observing assertion must emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// REGRESSION LOCK (#1216 second-pass): a SideEffect probe whose single
    /// matched assertion is a genuine MOCK EXPECTATION that kind-matches the seam
    /// but shares NO token with the probe expression must NOT emit
    /// observation_unverified. The mock observes the effect; downgrading it to
    /// observation_unverified would be a false weakening. (It may still be Weak
    /// via the Medium-strength path, but never via observation_unverified.)
    #[test]
    fn side_effect_mock_observer_without_token_clears_observation_unverified() {
        // "send_notification(user_id)" — tokens: ["send", "notification", "user"].
        // Assertion "mock.verify();" shares no token but is a MockExpectation,
        // i.e. a genuine effect observer.
        let probe = probe(ProbeFamily::SideEffect, "send_notification(user_id)");
        let test = test_with_assertions(
            "notification_checked",
            vec![oracle(
                "mock.verify();",
                OracleKind::MockExpectation,
                OracleStrength::Medium,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "a genuine mock observer must clear observation_unverified even without a token: got `{}`",
            discriminate.summary
        );
    }

    /// REGRESSION LOCK (#1216 second-pass): a CallDeletion probe whose single
    /// matched assertion is a whole-object equality (a genuine effect observer)
    /// sharing no token must NOT emit observation_unverified. Whole-object
    /// equality captures the resulting persisted state, so it observes the
    /// effect even without naming the changed call token.
    #[test]
    fn call_deletion_whole_object_observer_without_token_clears_observation_unverified() {
        // "persist_audit(record)" — tokens: ["persist", "audit", "record"].
        // Assertion "assert_eq!(store, expected);" shares no token but is a
        // WholeObjectEquality effect observer (Strong).
        let probe = probe(ProbeFamily::CallDeletion, "persist_audit(record)");
        let test = test_with_assertions(
            "store_matches_expected",
            vec![oracle(
                "assert_eq!(store, expected);",
                OracleKind::WholeObjectEquality,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "a whole-object effect observer must clear observation_unverified even without a token: got `{}`",
            discriminate.summary
        );
    }

    /// A VALUE family (ReturnValue) must NOT treat a mock/whole-object as an
    /// observation confirmation — only a token_match confirms value families.
    /// This guards against the effect-family relaxation leaking into value
    /// families (the point of #1200/#1216: an ExactValue/whole-object oracle
    /// does not kind-match a value seam's specific sub-expression).
    #[test]
    fn return_value_whole_object_without_token_still_emits_observation_unverified() {
        // "base * SCALE" — tokens: ["base", "SCALE"]. Assertion
        // "assert_eq!(result, expected);" is WholeObjectEquality but shares no
        // token; for a VALUE family this must NOT clear observation_unverified.
        let probe = probe(ProbeFamily::ReturnValue, "base * SCALE");
        let test = test_with_assertions(
            "result_matches_expected",
            vec![oracle(
                "assert_eq!(result, expected);",
                OracleKind::WholeObjectEquality,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ReturnValue (value family) with no token must stay observation_unverified"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "value family must not be cleared by an effect-observer kind: got `{}`",
            discriminate.summary
        );
    }

    /// A SideEffect probe whose assertion references a specific token from the
    /// changed expression must not be downgraded by observation_unverified.
    #[test]
    fn side_effect_with_token_match_keeps_discriminate_not_unverified() {
        // "emit_payment_event(tx)" — tokens: ["emit_payment_event", "tx"] but
        // only "emit_payment_event" (len > 3, well, len 17) would match.
        // Assertion "mock.expect_emit_payment_event();" contains "emit_payment_event".
        let probe = probe(ProbeFamily::SideEffect, "emit_payment_event(tx)");
        let test = test_with_assertions(
            "payment_event_emitted",
            vec![oracle(
                "mock.expect_emit_payment_event();",
                OracleKind::MockExpectation,
                OracleStrength::Medium,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        // MockExpectation yields OracleStrength::Medium → StageState::Weak via
        // the existing Medium oracle path, NOT via observation_unverified.
        // The key invariant: observation_unverified must NOT fire when there IS
        // a token_match — so the summary must not contain "observation_unverified".
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "token-matched SideEffect must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// A CallDeletion probe whose single assertion fires family_match via
    /// `text.contains("assert")` but contains no token from the changed call
    /// expression must downgrade to Weak.
    #[test]
    fn call_deletion_family_match_only_without_token_downgrades_to_weak() {
        // "log_audit_event(record)" — tokens: ["audit", "event", "record"] (all len > 3).
        // Assertion "assert!(result.is_ok());" does not contain any of those.
        let probe = probe(ProbeFamily::CallDeletion, "log_audit_event(record)");
        let test = test_with_assertions(
            "result_ok",
            vec![oracle(
                "assert!(result.is_ok());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "CallDeletion probe with no token_match must downgrade to Weak"
        );
    }

    /// A CallDeletion probe whose assertion text contains the full call token
    /// must not be downgraded by observation_unverified.
    #[test]
    fn call_deletion_with_token_match_does_not_emit_observation_unverified() {
        // "log_audit_event(record)" — tokens: ["log_audit_event", "record"].
        // Assertion "assert!(log_audit_event_was_called);" contains
        // "log_audit_event" (the full call token).
        let probe = probe(ProbeFamily::CallDeletion, "log_audit_event(record)");
        let test = test_with_assertions(
            "audit_event_logged",
            vec![oracle(
                "assert!(log_audit_event_was_called);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "token-matched CallDeletion must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// MatchArm: type-blind token match — `Mode::Warm` assertion must NOT clear
    /// observation_unverified for a `Mode::Frozen` probe (Part B regression lock).
    #[test]
    fn match_arm_sibling_qualifier_does_not_clear_observation_unverified() {
        // probe expression "Mode::Frozen => -1," — tokens: ["Mode", "Frozen"].
        // Assertion "assert_eq!(classify(Mode::Warm), 1);" contains "Mode" (len 4)
        // but NOT "Frozen" (the variant token). With variant-scoped token_match,
        // "Mode" alone must not clear observation_unverified.
        let probe = probe(ProbeFamily::MatchArm, "Mode::Frozen => -1,");
        let test = test_with_assertions(
            "warm_arm_returns_one",
            vec![oracle(
                "assert_eq!(classify(Mode::Warm), 1);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "Mode::Warm assertion must not confirm Mode::Frozen arm (sibling-qualifier hole)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// MatchArm: assertion containing the specific VARIANT token confirms the arm.
    #[test]
    fn match_arm_variant_token_match_keeps_discriminate_yes() {
        // probe expression "Mode::Frozen => -1," — variant "Frozen" appears in assertion.
        let probe = probe(ProbeFamily::MatchArm, "Mode::Frozen => -1,");
        let test = test_with_assertions(
            "frozen_arm_returns_minus_one",
            vec![oracle(
                "assert_eq!(classify(Mode::Frozen), -1);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "Frozen variant in assertion must confirm observation (no over-correction)"
        );
    }

    // --- Predicate and ErrorPath must NOT be affected ---

    /// Predicate probes do not require token confirmation and must not be
    /// affected by the observation_unverified logic.
    #[test]
    fn predicate_probe_family_match_only_keeps_discriminate_yes() {
        let probe = probe(ProbeFamily::Predicate, "x > 0");
        let test = test_with_assertions(
            "check_positive",
            vec![oracle(
                "assert!(value >= 3);",
                OracleKind::RelationalCheck,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        // RelationalCheck → OracleStrength::Weak → StageState::Weak, but NOT
        // via observation_unverified.
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "Predicate probe must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// ErrorPath probes do not require token confirmation and must not be
    /// affected by the observation_unverified logic.
    #[test]
    fn error_path_probe_family_match_only_keeps_discriminate_yes() {
        let probe = probe(ProbeFamily::ErrorPath, "Err(AuthError::RevokedToken)");
        let test = test_with_assertions(
            "revoked_token_fails",
            vec![oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "ErrorPath probe must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }
}
