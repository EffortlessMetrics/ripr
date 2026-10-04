use std::path::PathBuf;

use crate::analysis::seam_classification::ClassifiedSeam;
use crate::analysis::seams::{
    ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
};
use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence, TestTargetEvidence};
use crate::domain::{
    ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
    LanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe,
    ProbeFamily, ProbeId, RelatedTest, RelationReason, RevealEvidence, RiprEvidence,
    SourceCurrentness, SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason,
    SymbolId,
};

use super::{
    AdapterInput, InputCurrentness, ParityDisposition, ParityReport, SubjectSet, TargetState,
    compare_pair, from_classified_seam, from_classified_seam_with_input, from_finding,
    from_finding_with_input, pair_by_portable_id,
};

fn stage(state: StageState) -> StageEvidence {
    StageEvidence::new(state, Confidence::Medium, "producer")
}

fn ripr(
    reach: StageState,
    infect: StageState,
    propagate: StageState,
    observe: StageState,
    discriminate: StageState,
) -> RiprEvidence {
    RiprEvidence {
        reach: stage(reach),
        infect: stage(infect),
        propagate: stage(propagate),
        reveal: RevealEvidence {
            observe: stage(observe),
            discriminate: stage(discriminate),
        },
    }
}

struct FindingSpec {
    id: &'static str,
    canonical_id: Option<&'static str>,
    owner: &'static str,
    family: ProbeFamily,
    expression: &'static str,
    class: ExposureClass,
    reach: StageState,
    infect: StageState,
    propagate: StageState,
    observe: StageState,
    discriminate: StageState,
    discriminator: &'static str,
    related: Vec<RelatedTest>,
    missing: Vec<&'static str>,
    expected_sink: &'static str,
    language: Option<LanguageId>,
    language_status: Option<LanguageStatus>,
    stop_reasons: Vec<StopReason>,
    static_limit_kind: Option<StaticLimitKind>,
    source_currentness: SourceCurrentness,
    line: usize,
}

impl Default for FindingSpec {
    fn default() -> Self {
        Self {
            id: "finding:pricing",
            canonical_id: Some("gap:pricing:boundary"),
            owner: "pricing::discounted_total",
            family: ProbeFamily::Predicate,
            expression: "amount >= threshold",
            class: ExposureClass::WeaklyExposed,
            reach: StageState::Yes,
            infect: StageState::Yes,
            propagate: StageState::Yes,
            observe: StageState::Yes,
            discriminate: StageState::Weak,
            discriminator: "amount >= threshold",
            related: Vec::new(),
            missing: vec!["amount >= threshold"],
            expected_sink: "return_value",
            language: Some(LanguageId::Rust),
            language_status: Some(LanguageStatus::Stable),
            stop_reasons: Vec::new(),
            static_limit_kind: None,
            source_currentness: SourceCurrentness::CandidateCurrent,
            line: 12,
        }
    }
}

impl FindingSpec {
    fn related(
        mut self,
        name: &str,
        kind: OracleKind,
        strength: OracleStrength,
        reason: Option<RelationReason>,
    ) -> Self {
        self.related.push(RelatedTest {
            name: name.to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 10,
            oracle: None,
            oracle_kind: kind,
            oracle_strength: strength,
            relation_confidence: reason.map(RelationReason::confidence),
            relation_reason: reason,
        });
        self
    }

    fn build(self) -> Finding {
        Finding {
            id: self.id.to_string(),
            canonical_gap: self.canonical_id.map(|id| FindingCanonicalGap {
                id: id.to_string(),
                language: "rust".to_string(),
                file: "src/pricing.rs".to_string(),
                owner: self.owner.to_string(),
                behavior_kind: self.family.as_str().to_string(),
                probe_kind: self.family.as_str().to_string(),
                normalized_discriminator: self.discriminator.to_string(),
            }),
            probe: Probe {
                id: ProbeId("probe:pricing".to_string()),
                location: SourceLocation::new("src/pricing.rs", self.line, 5),
                owner: Some(SymbolId(self.owner.to_string())),
                family: self.family,
                delta: DeltaKind::Control,
                before: Some(self.expression.to_string()),
                after: Some(self.expression.to_string()),
                expression: self.expression.to_string(),
                expected_sinks: vec![self.expected_sink.to_string()],
                required_oracles: vec![self.discriminator.to_string()],
            },
            class: self.class,
            ripr: ripr(
                self.reach,
                self.infect,
                self.propagate,
                self.observe,
                self.discriminate,
            ),
            confidence: 0.5,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: self
                    .missing
                    .iter()
                    .map(|value| MissingDiscriminatorFact {
                        value: (*value).to_string(),
                        reason: "fixture".to_string(),
                        flow_sink: None,
                    })
                    .collect(),
            },
            stop_reasons: self.stop_reasons,
            related_tests_matched_total: None,
            related_tests: self.related,
            recommended_next_step: None,
            language: self.language,
            language_status: self.language_status,
            owner_kind: None,
            static_limit_kind: self.static_limit_kind,
            changed_sink: Some(self.expected_sink.to_string()),
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: self.source_currentness,
        }
    }
}

struct SeamSpec {
    owner: &'static str,
    kind: SeamKind,
    expression: &'static str,
    discriminator: RequiredDiscriminator,
    sink: ExpectedSink,
    class: SeamGripClass,
    reach: StageState,
    activate: StageState,
    propagate: StageState,
    observe: StageState,
    discriminate: StageState,
    related: Vec<RelatedTestGrip>,
    missing: Vec<&'static str>,
    byte_offset: usize,
    line: usize,
}

impl Default for SeamSpec {
    fn default() -> Self {
        Self {
            owner: "pricing::discounted_total",
            kind: SeamKind::PredicateBoundary,
            expression: "amount >= threshold",
            discriminator: RequiredDiscriminator::BoundaryValue {
                description: "amount >= threshold".to_string(),
            },
            sink: ExpectedSink::ReturnValue,
            class: SeamGripClass::WeaklyGripped,
            reach: StageState::Yes,
            activate: StageState::Yes,
            propagate: StageState::Yes,
            observe: StageState::Yes,
            discriminate: StageState::Weak,
            related: Vec::new(),
            missing: vec!["amount >= threshold"],
            byte_offset: 42,
            line: 12,
        }
    }
}

impl SeamSpec {
    fn related(
        mut self,
        name: &str,
        kind: OracleKind,
        strength: OracleStrength,
        reason: RelationReason,
        with_target: bool,
    ) -> Self {
        self.related.push(RelatedTestGrip {
            test_name: name.to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 10,
            test_target: if with_target {
                Some(TestTargetEvidence::fixture(
                    name,
                    std::path::Path::new("tests/pricing.rs"),
                    10,
                ))
            } else {
                None
            },
            oracle_kind: kind,
            oracle_strength: strength,
            evidence_summary: "fixture".to_string(),
            relation_reason: reason,
            relation_confidence: reason.confidence(),
        });
        self
    }

    fn build(self) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            self.owner,
            self.kind,
            self.byte_offset,
            self.line,
            self.expression,
            self.discriminator,
            self.sink,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: self.related,
                reach: stage(self.reach),
                activate: stage(self.activate),
                propagate: stage(self.propagate),
                observe: stage(self.observe),
                discriminate: stage(self.discriminate),
                observed_values: Vec::new(),
                missing_discriminators: self
                    .missing
                    .iter()
                    .map(|value| MissingDiscriminatorFact {
                        value: (*value).to_string(),
                        reason: "fixture".to_string(),
                        flow_sink: None,
                    })
                    .collect(),
                new_test_target: None,
            },
            class: self.class,
        }
    }
}

fn boundary_related_finding() -> Finding {
    FindingSpec::default()
        .related(
            "below_threshold_has_no_discount",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            Some(RelationReason::DirectOwnerCall),
        )
        .build()
}

fn boundary_related_seam() -> ClassifiedSeam {
    SeamSpec::default()
        .related(
            "below_threshold_has_no_discount",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            RelationReason::DirectOwnerCall,
            true,
        )
        .build()
}

fn assert_json_ok(report: &ParityReport) -> String {
    let result = report.to_json();
    assert!(
        result.is_ok(),
        "parity JSON must serialize: {:?}",
        result.as_ref().err()
    );
    result.unwrap_or_default()
}

#[test]
fn adapters_copy_producer_classes_and_do_not_invent_targets_on_findings() {
    let finding = boundary_related_finding();
    let seam = boundary_related_seam();
    let diff = from_finding(&finding);
    let repo = from_classified_seam(&seam);

    assert_eq!(diff.public_class, finding.class.as_str());
    assert_eq!(repo.public_class, seam.class.as_str());
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(seam.class, SeamGripClass::WeaklyGripped);
    assert!(matches!(
        diff.selected_target,
        TargetState::TypedAbsence { .. }
    ));
    assert!(matches!(
        repo.selected_target,
        TargetState::SelectedExisting { .. }
    ));
    assert!(diff.digest_matches());
    assert!(repo.digest_matches());
}

#[test]
fn equality_predicate_before_and_after_the_exact_boundary_test() {
    let before_diff = from_finding(&boundary_related_finding());
    let before_repo = from_classified_seam(&boundary_related_seam());
    let before = compare_pair("equality-before", &before_diff, &before_repo);
    assert_eq!(before.disposition, ParityDisposition::Equal);
    assert_eq!(before.discrimination.diff, "weak");
    assert_eq!(before.discrimination.repo, "weak");

    let after_finding = FindingSpec {
        class: ExposureClass::Exposed,
        discriminate: StageState::Yes,
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "below_threshold_has_no_discount",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let after_seam = SeamSpec {
        class: SeamGripClass::StronglyGripped,
        discriminate: StageState::Yes,
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "below_threshold_has_no_discount",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let after = compare_pair(
        "equality-after",
        &from_finding(&after_finding),
        &from_classified_seam(&after_seam),
    );
    assert_eq!(after.disposition, ParityDisposition::Equal);
    assert_eq!(after.discrimination.diff, "yes");

    let crossed = compare_pair(
        "equality-crossed",
        &before_diff,
        &from_classified_seam(&after_seam),
    );
    assert_ne!(crossed.disposition, ParityDisposition::Equal);
}

#[test]
fn exact_versus_broad_error_oracle() {
    let exact = FindingSpec {
        family: ProbeFamily::ErrorPath,
        expression: "Err(Error::Overflow)",
        discriminator: "Overflow",
        expected_sink: "error_channel",
        missing: Vec::new(),
        discriminate: StageState::Yes,
        class: ExposureClass::Exposed,
        ..FindingSpec::default()
    }
    .related(
        "overflow_is_exact",
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let broad = SeamSpec {
        kind: SeamKind::ErrorVariant,
        expression: "Err(Error::Overflow)",
        discriminator: RequiredDiscriminator::ErrorVariant {
            variant: "Overflow".to_string(),
        },
        sink: ExpectedSink::ErrorChannel,
        discriminate: StageState::Weak,
        class: SeamGripClass::WeaklyGripped,
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "any_err_passes",
        OracleKind::BroadError,
        OracleStrength::Weak,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let row = compare_pair(
        "exact-vs-broad-error",
        &from_finding(&exact),
        &from_classified_seam(&broad),
    );
    assert_eq!(row.disposition, ParityDisposition::Contradiction);
}

#[test]
fn exact_field_versus_sibling_field_assertion() {
    let exact = FindingSpec {
        family: ProbeFamily::FieldConstruction,
        expression: "total: amount",
        discriminator: "total",
        expected_sink: "output_field",
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "pins_total",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let sibling = SeamSpec {
        kind: SeamKind::FieldConstruction,
        expression: "total: amount",
        discriminator: RequiredDiscriminator::FieldValue {
            field: "tax".to_string(),
        },
        sink: ExpectedSink::OutputField,
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "pins_tax",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let row = compare_pair(
        "field-vs-sibling",
        &from_finding(&exact),
        &from_classified_seam(&sibling),
    );
    assert_eq!(row.disposition, ParityDisposition::Contradiction);
    assert_ne!(
        from_finding(&exact).required_discriminator.identity,
        from_classified_seam(&sibling)
            .required_discriminator
            .identity
    );
}

#[test]
fn direct_return_versus_unrelated_strong_assertion() {
    let direct = FindingSpec {
        family: ProbeFamily::ReturnValue,
        expression: "amount",
        discriminator: "amount",
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "returns_amount",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let unrelated = SeamSpec {
        kind: SeamKind::ReturnValue,
        expression: "amount",
        discriminator: RequiredDiscriminator::ReturnValue {
            description: "amount".to_string(),
        },
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "unrelated_strong",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        RelationReason::WeakTokenSubstring,
        false,
    )
    .build();
    let row = compare_pair(
        "return-vs-unrelated",
        &from_finding(&direct),
        &from_classified_seam(&unrelated),
    );
    assert_eq!(row.disposition, ParityDisposition::Contradiction);
    assert!(
        from_classified_seam(&unrelated)
            .established_relations
            .is_empty()
    );
    assert!(
        !from_classified_seam(&unrelated)
            .reach
            .established_facts
            .iter()
            .any(|fact| fact.contains("weak_token_substring"))
    );
}

#[test]
fn side_effect_with_and_without_aligned_observer() {
    let observed = FindingSpec {
        family: ProbeFamily::SideEffect,
        expression: "log(amount)",
        discriminator: "log",
        expected_sink: "side_effect",
        observe: StageState::Yes,
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "observes_log",
        OracleKind::MockExpectation,
        OracleStrength::Medium,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let unobserved = SeamSpec {
        kind: SeamKind::SideEffect,
        expression: "log(amount)",
        discriminator: RequiredDiscriminator::Effect {
            sink: "log".to_string(),
        },
        sink: ExpectedSink::SideEffect,
        observe: StageState::No,
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "calls_owner",
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let row = compare_pair(
        "side-effect-observer",
        &from_finding(&observed),
        &from_classified_seam(&unobserved),
    );
    assert_eq!(row.disposition, ParityDisposition::Contradiction);
    assert_eq!(row.observation.diff, "yes");
    assert_eq!(row.observation.repo, "no");
}

#[test]
fn call_presence_direct_caller_and_unresolved_propagation() {
    let direct = FindingSpec {
        family: ProbeFamily::CallDeletion,
        expression: "notify(user)",
        discriminator: "notify",
        expected_sink: "side_effect",
        propagate: StageState::Yes,
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "calls_notify",
        OracleKind::MockExpectation,
        OracleStrength::Medium,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let unresolved = SeamSpec {
        kind: SeamKind::CallPresence,
        expression: "notify(user)",
        discriminator: RequiredDiscriminator::CallSite {
            target: "notify".to_string(),
        },
        sink: ExpectedSink::SideEffect,
        propagate: StageState::Unknown,
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "calls_notify",
        OracleKind::MockExpectation,
        OracleStrength::Medium,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let row = compare_pair(
        "call-presence-propagation",
        &from_finding(&direct),
        &from_classified_seam(&unresolved),
    );
    assert_ne!(row.disposition, ParityDisposition::Equal);
    assert_eq!(row.propagation.diff, "yes");
    assert_eq!(row.propagation.repo, "unknown");
}

#[test]
fn match_arm_sibling_variant_control() {
    let arm_ok = FindingSpec {
        family: ProbeFamily::MatchArm,
        expression: "Status::Ready",
        discriminator: "Ready",
        expected_sink: "return_value",
        missing: Vec::new(),
        ..FindingSpec::default()
    }
    .related(
        "ready_arm",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    let arm_pending = SeamSpec {
        kind: SeamKind::MatchArm,
        expression: "Status::Pending",
        discriminator: RequiredDiscriminator::MatchArmTaken {
            arm: "Pending".to_string(),
        },
        missing: Vec::new(),
        ..SeamSpec::default()
    }
    .related(
        "pending_arm",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        RelationReason::DirectOwnerCall,
        true,
    )
    .build();
    let row = compare_pair(
        "match-arm-sibling",
        &from_finding(&arm_ok),
        &from_classified_seam(&arm_pending),
    );
    assert_eq!(row.disposition, ParityDisposition::Contradiction);
}

#[test]
fn owner_relations_and_wrong_owner_token_collision_do_not_join() {
    let direct = from_finding(
        &FindingSpec::default()
            .related(
                "same_name",
                OracleKind::ExactValue,
                OracleStrength::Strong,
                Some(RelationReason::DirectOwnerCall),
            )
            .build(),
    );
    let helper = from_classified_seam(
        &SeamSpec::default()
            .related(
                "same_name",
                OracleKind::ExactValue,
                OracleStrength::Strong,
                RelationReason::HelperOwnerCall,
                true,
            )
            .build(),
    );
    let affinity = from_classified_seam(
        &SeamSpec::default()
            .related(
                "same_name",
                OracleKind::ExactValue,
                OracleStrength::Strong,
                RelationReason::ImportPathAffinity,
                false,
            )
            .build(),
    );
    assert_eq!(
        direct.established_relations[0].relation_reason,
        "direct_owner_call"
    );
    assert_eq!(
        helper.established_relations[0].relation_reason,
        "helper_owner_call"
    );
    assert!(affinity.established_relations.is_empty());
    assert_eq!(
        affinity.candidate_relations[0].relation_reason,
        "import_path_affinity"
    );

    let mut other_owner = FindingSpec {
        canonical_id: Some("gap:other:boundary"),
        owner: "other::unrelated",
        ..FindingSpec::default()
    }
    .related(
        "same_name",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::WeakTokenSubstring),
    )
    .build();
    other_owner.probe.owner = Some(SymbolId("other::unrelated".to_string()));
    let collision = from_finding(&other_owner);
    let joined = pair_by_portable_id(
        std::slice::from_ref(&direct),
        std::slice::from_ref(&collision),
    );
    assert!(
        joined
            .rows
            .iter()
            .all(|row| row.disposition == ParityDisposition::NotComparable),
        "same test/owner token must not join distinct portable identities: {:?}",
        joined.rows
    );
}

#[test]
fn no_test_macro_closure_dispatch_opaque_fixture_and_cross_language_limits() {
    let limited = FindingSpec {
        related: Vec::new(),
        reach: StageState::Unknown,
        class: ExposureClass::NoStaticPath,
        stop_reasons: vec![
            StopReason::ProcMacroOpaque,
            StopReason::FixtureOpaque,
            StopReason::DynamicDispatchUnresolved,
            StopReason::AsyncBoundaryOpaque,
        ],
        static_limit_kind: Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
        language: Some(LanguageId::TypeScript),
        language_status: Some(LanguageStatus::Preview),
        ..FindingSpec::default()
    }
    .build();
    let repo = SeamSpec {
        related: Vec::new(),
        reach: StageState::Unknown,
        class: SeamGripClass::Ungripped,
        ..SeamSpec::default()
    }
    .build();
    let diff = from_finding(&limited);
    for token in [
        "macro_limit",
        "opaque_fixture",
        "dynamic_dispatch",
        "closure_or_async_limit",
        "cross_language_limit",
        "preview_language",
    ] {
        assert!(
            diff.limitations.iter().any(|item| item == token),
            "missing limitation {token}: {:?}",
            diff.limitations
        );
    }
    let row = compare_pair("limits", &diff, &from_classified_seam(&repo));
    assert_eq!(row.disposition, ParityDisposition::ExplainedScopeDifference);
}

#[test]
fn partial_index_versus_workspace_complete_relation_state() {
    let complete = SeamSpec::default()
        .related(
            "direct",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            RelationReason::DirectOwnerCall,
            true,
        )
        .related(
            "helper",
            OracleKind::ExactValue,
            OracleStrength::Medium,
            RelationReason::HelperOwnerCall,
            true,
        )
        .build();
    let partial = SeamSpec::default()
        .related(
            "direct",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            RelationReason::DirectOwnerCall,
            true,
        )
        .build();
    let complete_w = from_classified_seam_with_input(
        &complete,
        AdapterInput {
            subject_set: SubjectSet::WorkspaceComplete,
            currentness: InputCurrentness::Current,
        },
    );
    let partial_w = from_classified_seam_with_input(
        &partial,
        AdapterInput {
            subject_set: SubjectSet::PartialIndex,
            currentness: InputCurrentness::Current,
        },
    );
    let row = compare_pair("partial-vs-complete", &partial_w, &complete_w);
    assert_eq!(row.disposition, ParityDisposition::ExplainedScopeDifference);
    assert!(row.diff_scope.contains("partial_index"));
    assert!(row.repo_scope.contains("workspace_complete"));
}

#[test]
fn equivalent_roots_and_harmless_line_movement_keep_digest() {
    let first = FindingSpec {
        line: 12,
        ..FindingSpec::default()
    }
    .related(
        "a",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .related(
        "b",
        OracleKind::BroadError,
        OracleStrength::Weak,
        Some(RelationReason::SameTestFile),
    )
    .build();
    let mut second = FindingSpec {
        line: 80,
        ..FindingSpec::default()
    }
    .related(
        "renamed_b",
        OracleKind::BroadError,
        OracleStrength::Weak,
        Some(RelationReason::SameTestFile),
    )
    .related(
        "renamed_a",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::DirectOwnerCall),
    )
    .build();
    second.related_tests[0].file = PathBuf::from("./tests\\pricing.rs");
    second.related_tests[1].file = PathBuf::from("tests/pricing.rs");
    let left = from_finding(&first);
    let right = from_finding(&second);
    assert_eq!(left.semantic_digest, right.semantic_digest);
    assert_eq!(
        compare_pair(
            "line-movement",
            &left,
            &from_classified_seam(&boundary_related_seam())
        )
        .disposition,
        compare_pair(
            "line-movement-2",
            &right,
            &from_classified_seam(&boundary_related_seam())
        )
        .disposition
    );
}

#[test]
fn stale_or_wrong_input_on_one_path_is_an_explained_scope_difference() {
    let current = from_classified_seam(&boundary_related_seam());
    let stale = from_finding_with_input(
        &boundary_related_finding(),
        AdapterInput {
            subject_set: SubjectSet::DiffOnly,
            currentness: InputCurrentness::Stale,
        },
    );
    let wrong = from_finding_with_input(
        &boundary_related_finding(),
        AdapterInput {
            subject_set: SubjectSet::DiffOnly,
            currentness: InputCurrentness::Wrong,
        },
    );
    assert_eq!(
        compare_pair("stale", &stale, &current).disposition,
        ParityDisposition::ExplainedScopeDifference
    );
    assert_eq!(
        compare_pair("wrong", &wrong, &current).disposition,
        ParityDisposition::ExplainedScopeDifference
    );
}

#[test]
fn reordering_source_facts_is_byte_stable() {
    let finding = FindingSpec::default()
        .related(
            "z_last",
            OracleKind::BroadError,
            OracleStrength::Weak,
            Some(RelationReason::SameTestFile),
        )
        .related(
            "a_first",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            Some(RelationReason::DirectOwnerCall),
        )
        .build();
    let mut reordered = finding.clone();
    reordered.related_tests.reverse();
    reordered.activation.missing_discriminators.insert(
        0,
        MissingDiscriminatorFact {
            value: "amount >= threshold".to_string(),
            reason: "duplicate".to_string(),
            flow_sink: None,
        },
    );
    let first = from_finding(&finding);
    let second = from_finding(&reordered);
    assert_eq!(first.semantic_digest, second.semantic_digest);
    let report_a = ParityReport::from_rows(vec![compare_pair(
        "stable",
        &first,
        &from_classified_seam(&boundary_related_seam()),
    )]);
    let report_b = ParityReport::from_rows(vec![compare_pair(
        "stable",
        &second,
        &from_classified_seam(&boundary_related_seam()),
    )]);
    assert_eq!(assert_json_ok(&report_a), assert_json_ok(&report_b));
    assert_eq!(report_a.to_markdown(), report_b.to_markdown());
}

#[test]
fn candidate_only_relation_cannot_appear_as_established_reach() {
    let finding = FindingSpec {
        reach: StageState::Yes,
        ..FindingSpec::default()
    }
    .related(
        "token_hit",
        OracleKind::ExactValue,
        OracleStrength::Strong,
        Some(RelationReason::WeakTokenSubstring),
    )
    .build();
    let witness = from_finding(&finding);
    assert!(witness.established_relations.is_empty());
    assert_eq!(witness.reach.state, "yes");
    assert!(witness.reach.established_facts.is_empty());
    assert!(
        witness
            .reach
            .candidate_facts
            .iter()
            .any(|fact| fact.contains("weak_token_substring"))
    );
    assert!(
        witness
            .limitations
            .iter()
            .any(|item| item == "producer_reach_without_established_relation")
    );
}

#[test]
fn favorable_stage_plus_absent_stage_cannot_collapse_to_equality() {
    let present = from_finding(&boundary_related_finding());
    let mut absent = present.clone();
    absent.observation = super::StageWitness::absent("observation");
    absent.semantic_digest = String::new();
    absent = absent.finalize();
    let row = compare_pair("favorable-vs-absent", &present, &absent);
    assert_ne!(row.disposition, ParityDisposition::Equal);
}

#[test]
fn removing_identity_or_digest_makes_the_row_not_comparable() {
    let diff = from_finding(&boundary_related_finding());
    let repo = from_classified_seam(&boundary_related_seam());
    let mut missing_id = diff.clone();
    missing_id.portable_item_id.clear();
    missing_id.semantic_digest = missing_id.compute_semantic_digest();
    assert_eq!(
        compare_pair("missing-id", &missing_id, &repo).disposition,
        ParityDisposition::NotComparable
    );
    let mut missing_digest = repo.clone();
    missing_digest.semantic_digest.clear();
    assert_eq!(
        compare_pair("missing-digest", &diff, &missing_digest).disposition,
        ParityDisposition::NotComparable
    );
}

#[test]
fn changing_selected_target_state_breaks_the_stale_digest() {
    let mut repo = from_classified_seam(&boundary_related_seam());
    assert!(matches!(
        repo.selected_target,
        super::TargetState::SelectedExisting { .. }
    ));
    let unchanged_digest = repo.semantic_digest.clone();

    repo.selected_target = super::TargetState::SelectedExisting {
        identities: vec!["crates/ripr/src/lib.rs::pricing::score".to_string()],
    };
    // A digest computed before the target-state change must stop matching.
    assert!(!repo.digest_matches());

    // Selected identities are part of the hashed bytes, not silent state.
    repo.semantic_digest = repo.compute_semantic_digest();
    assert!(repo.digest_matches());
    assert_ne!(repo.semantic_digest, unchanged_digest);

    // The typed-absence reason is hashed the same way on the diff path.
    let mut diff = from_finding(&boundary_related_finding());
    assert!(matches!(
        diff.selected_target,
        super::TargetState::TypedAbsence { .. }
    ));
    let diff_digest = diff.semantic_digest.clone();
    if let super::TargetState::TypedAbsence { reason } = &mut diff.selected_target {
        reason.push_str("-changed");
    }
    assert!(!diff.digest_matches());
    diff.semantic_digest = diff.compute_semantic_digest();
    assert_ne!(diff.semantic_digest, diff_digest);
}

#[test]
fn json_and_markdown_derive_from_the_same_normalized_dto() {
    let row = compare_pair(
        "dto",
        &from_finding(&boundary_related_finding()),
        &from_classified_seam(&boundary_related_seam()),
    );
    let report = ParityReport::from_rows(vec![row.clone()]);
    let json = assert_json_ok(&report);
    assert!(
        json.contains("\"disposition\":\"equal\"")
            || json.contains(&format!("\"disposition\":\"{}\"", row.disposition.as_str()))
    );
    let markdown = report.to_markdown();
    assert!(markdown.contains(row.case_id.as_str()));
    assert!(markdown.contains(row.disposition.as_str()));
    assert!(
        !markdown.contains("diff_semantic_digest"),
        "markdown is the summary table; stage and digest fields live on the shared JSON DTO"
    );
    for field in [
        "\"reach\"",
        "\"activation\"",
        "\"propagation\"",
        "\"observation\"",
        "\"discrimination\"",
        "\"candidate_relation_identities\"",
        "\"established_relation_identities\"",
        "\"limitations\"",
        "\"diff_semantic_digest\"",
        "\"repo_semantic_digest\"",
    ] {
        assert!(json.contains(field), "json missing {field}");
    }
}

#[test]
fn unspecified_repo_completeness_is_a_typed_limitation_not_optimistic_parity() {
    let witness =
        from_classified_seam_with_input(&boundary_related_seam(), AdapterInput::default());
    assert!(
        witness
            .limitations
            .iter()
            .any(|item| item == "unspecified_input_completeness")
    );
}

#[test]
fn production_retain_does_not_mutate_producer_classes_or_relations() {
    let finding = boundary_related_finding();
    let seam = boundary_related_seam();
    let class = finding.class.as_str().to_string();
    let seam_class = seam.class.as_str().to_string();
    let related = finding.related_tests.len();
    super::retain_finding_projection(&finding);
    super::retain_classified_seams(
        std::slice::from_ref(&seam),
        AdapterInput::workspace_complete_current(),
    );
    super::retain_classified_seams(std::slice::from_ref(&seam), super::repo_adapter_input(true));
    assert_eq!(finding.class.as_str(), class);
    assert_eq!(seam.class.as_str(), seam_class);
    assert_eq!(finding.related_tests.len(), related);
    assert!(from_finding(&finding).digest_matches());
    assert!(from_classified_seam(&seam).digest_matches());
}

#[test]
fn unfilled_finding_language_is_a_typed_limitation_not_silent_rust() {
    let filled = from_finding(&boundary_related_finding());
    assert_eq!(filled.language, "rust");
    assert!(
        !filled
            .limitations
            .iter()
            .any(|item| item == "unrepresentable_language")
    );

    let unfilled = FindingSpec {
        language: None,
        ..FindingSpec::default()
    }
    .build();
    let witness = from_finding(&unfilled);
    assert_eq!(witness.language, "unknown");
    assert!(
        witness
            .limitations
            .iter()
            .any(|item| item == "unrepresentable_language")
    );
}

#[test]
fn absent_finding_canonical_gap_does_not_join_seam_gap_identity() {
    let finding = FindingSpec {
        canonical_id: None,
        ..FindingSpec::default()
    }
    .build();
    let diff = from_finding(&finding);
    let repo = from_classified_seam(&boundary_related_seam());
    assert_eq!(diff.portable_item_id, finding.id);
    assert_ne!(diff.portable_item_id, repo.portable_item_id);
    let report = pair_by_portable_id(std::slice::from_ref(&diff), std::slice::from_ref(&repo));
    assert!(
        !report.rows.is_empty(),
        "unmatched identities must still emit rows"
    );
    assert!(
        report
            .rows
            .iter()
            .all(|row| row.disposition == ParityDisposition::NotComparable)
    );
}

#[test]
fn relation_identity_is_reason_and_oracle_not_test_name() {
    let first = FindingSpec::default()
        .related(
            "rejects_zero",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            Some(RelationReason::DirectOwnerCall),
        )
        .build();
    let second = FindingSpec::default()
        .related(
            "accepts_positive",
            OracleKind::ExactValue,
            OracleStrength::Strong,
            Some(RelationReason::DirectOwnerCall),
        )
        .build();
    let left = from_finding(&first);
    let right = from_finding(&second);
    assert_eq!(
        left.established_relations
            .iter()
            .map(super::RelationWitness::semantic_identity)
            .collect::<Vec<_>>(),
        right
            .established_relations
            .iter()
            .map(super::RelationWitness::semantic_identity)
            .collect::<Vec<_>>()
    );
    assert_eq!(left.semantic_digest, right.semantic_digest);
}

#[test]
fn corrupting_a_stage_source_identity_makes_the_row_not_comparable() {
    let present = from_finding(
        &FindingSpec::default()
            .related(
                "rejects_zero",
                OracleKind::ExactValue,
                OracleStrength::Strong,
                Some(RelationReason::DirectOwnerCall),
            )
            .build(),
    );
    assert!(!present.reach.source_identities.is_empty());
    let mut corrupted = present.clone();
    corrupted.reach.source_identities.pop();
    assert!(!corrupted.digest_matches());
    assert_eq!(
        compare_pair("source-identity", &present, &corrupted).disposition,
        ParityDisposition::NotComparable
    );
}

#[test]
fn same_path_public_class_drift_is_a_contradiction() {
    let weakly = from_finding(&FindingSpec::default().build());
    let exposed = from_finding(
        &FindingSpec {
            class: ExposureClass::Exposed,
            ..FindingSpec::default()
        }
        .build(),
    );
    assert_eq!(weakly.path, exposed.path);
    assert_ne!(weakly.public_class, exposed.public_class);
    assert_eq!(
        compare_pair("class-drift", &weakly, &exposed).disposition,
        ParityDisposition::Contradiction
    );
}

#[test]
fn scope_tokens_cannot_explain_an_identity_mismatch() {
    let mut partial = from_finding_with_input(
        &FindingSpec::default().build(),
        AdapterInput {
            subject_set: SubjectSet::PartialIndex,
            currentness: InputCurrentness::Current,
        },
    );
    let complete = from_classified_seam(&boundary_related_seam());
    partial.required_discriminator.identity = "other::discriminator".to_string();
    partial = partial.finalize();
    assert_eq!(
        compare_pair("identity-vs-scope", &partial, &complete).disposition,
        ParityDisposition::Contradiction
    );
}

#[test]
fn unmatched_repo_witness_stays_in_repo_columns() {
    let repo = from_classified_seam(&boundary_related_seam());
    let report = pair_by_portable_id(&[], std::slice::from_ref(&repo));
    assert_eq!(report.rows.len(), 1);
    let row = &report.rows[0];
    assert_eq!(row.disposition, ParityDisposition::NotComparable);
    assert!(row.public_class_diff.is_empty());
    assert_eq!(row.public_class_repo, repo.public_class);
    assert!(row.diff_semantic_digest.is_empty());
    assert_eq!(row.repo_semantic_digest, repo.semantic_digest);
    assert_eq!(row.diff_target, "absent");
    assert_eq!(row.repo_target, repo.selected_target.kind());
}
