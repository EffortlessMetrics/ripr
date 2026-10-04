use super::relations::{RelationStatus, RelationWitness, established_reason};
use super::{
    AnalysisPath, BehaviorEvidenceWitnessV1, DiscriminatorIdentity, ExpressionIdentity,
    InputCurrentness, OwnerIdentity, StageWitness, SubjectSet, TargetState,
    normalize_semantic_text,
};
use crate::analysis::canonical_gap::canonical_gap_identity;
use crate::analysis::seam_classification::ClassifiedSeam;
use crate::analysis::seams::RequiredDiscriminator;
use crate::analysis::test_grip_evidence::RelatedTestGrip;
use crate::domain::{
    Finding, LanguageId, LanguageStatus, ProbeFamily, RelatedTest, RelationReason,
    SourceCurrentness, StopReason,
};

/// Declared adapter input identity. Completeness is never inferred from
/// producer facts that do not carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdapterInput {
    pub(crate) subject_set: SubjectSet,
    pub(crate) currentness: InputCurrentness,
}

impl Default for AdapterInput {
    fn default() -> Self {
        Self {
            subject_set: SubjectSet::Unspecified,
            currentness: InputCurrentness::Unspecified,
        }
    }
}

impl AdapterInput {
    pub(crate) fn diff_current() -> Self {
        Self {
            subject_set: SubjectSet::DiffOnly,
            currentness: InputCurrentness::Current,
        }
    }

    pub(crate) fn workspace_complete_current() -> Self {
        Self {
            subject_set: SubjectSet::WorkspaceComplete,
            currentness: InputCurrentness::Current,
        }
    }
}

/// Project a diff finding. Stage meaning is copied, never recomputed.
pub(crate) fn from_finding(finding: &Finding) -> BehaviorEvidenceWitnessV1 {
    from_finding_with_input(finding, AdapterInput::diff_current())
}

/// Keep the finding adapter on the production classification path without
/// changing public Finding bytes or class.
pub(crate) fn retain_finding_projection(finding: &Finding) {
    let valid = from_finding(finding).digest_matches();
    debug_assert!(
        valid,
        "behavior evidence witness digest must match canonical bytes"
    );
    let _ = valid;
}

/// Keep the classified-seam adapter on the production inventory path without
/// changing public class or cache bytes.
pub(crate) fn retain_classified_seams(seams: &[ClassifiedSeam], input: AdapterInput) {
    use rayon::prelude::*;
    // Each projection is independent and its result is only asserted, so
    // the check runs on the rayon pool instead of serially per seam.
    seams.par_iter().for_each(|seam| {
        let witness = if input == AdapterInput::workspace_complete_current() {
            from_classified_seam(seam)
        } else {
            from_classified_seam_with_input(seam, input)
        };
        let valid = witness.digest_matches();
        debug_assert!(
            valid,
            "behavior evidence witness digest must match canonical bytes"
        );
        let _ = valid;
    });
}

pub(crate) fn repo_adapter_input(partial: bool) -> AdapterInput {
    if partial {
        AdapterInput {
            subject_set: SubjectSet::PartialIndex,
            currentness: InputCurrentness::Current,
        }
    } else {
        AdapterInput::workspace_complete_current()
    }
}

pub(crate) fn from_finding_with_input(
    finding: &Finding,
    input: AdapterInput,
) -> BehaviorEvidenceWitnessV1 {
    let mut limitations = Vec::new();
    let non_claims = vec![
        "adapter_does_not_recompute_stage_meaning".to_string(),
        "public_class_is_producer_projection".to_string(),
        "finding_has_no_test_target_authority".to_string(),
    ];
    push_input_limitations(input, &mut limitations);

    let portable_item_id = finding
        .canonical_gap
        .as_ref()
        .map(|gap| gap.id.clone())
        .unwrap_or_else(|| finding.id.clone());
    if portable_item_id.is_empty() {
        limitations.push("missing_portable_item_identity".to_string());
    }

    let language = finding
        .language
        .map(|language| language.as_str().to_string())
        .unwrap_or_else(|| {
            limitations.push("unrepresentable_language".to_string());
            "unknown".to_string()
        });
    let language_status = finding
        .language_status
        .map(|status| status.as_str().to_string());
    if language_status.as_deref() == Some(LanguageStatus::Preview.as_str()) {
        limitations.push("preview_language".to_string());
    }
    if matches!(
        finding.language,
        Some(
            LanguageId::TypeScript | LanguageId::JavaScript | LanguageId::Python | LanguageId::Perl
        )
    ) {
        limitations.push("cross_language_limit".to_string());
    }

    let family = family_from_probe(&finding.probe.family);
    if finding.probe.family == ProbeFamily::CallDeletion {
        limitations.push("no_canonical_crosswalk_to_call_presence".to_string());
    }
    if finding.probe.family == ProbeFamily::StaticUnknown {
        limitations.push("static_unknown_family".to_string());
    }

    let owner = owner_from_finding(finding, &mut limitations);
    let expression = ExpressionIdentity {
        normalized: normalize_semantic_text(&finding.probe.expression),
    };
    let required_discriminator = discriminator_from_finding(finding, &mut limitations);
    let expected_sink = sink_from_finding(finding, &mut limitations);

    let (candidate_relations, established_relations) =
        split_finding_relations(&finding.related_tests);
    let mut reach = StageWitness::from_producer("reach", &finding.ripr.reach);
    let mut activation = StageWitness::from_producer("activation", &finding.ripr.infect);
    let mut propagation = StageWitness::from_producer("propagation", &finding.ripr.propagate);
    let mut observation = StageWitness::from_producer("observation", &finding.ripr.reveal.observe);
    let mut discrimination =
        StageWitness::from_producer("discrimination", &finding.ripr.reveal.discriminate);

    attach_relation_facts(
        &mut reach,
        &established_relations,
        &candidate_relations,
        &mut limitations,
    );
    attach_activation_facts(
        &mut activation,
        &finding
            .activation
            .observed_values
            .iter()
            .map(|value| value.value.clone())
            .collect::<Vec<_>>(),
        &finding
            .activation
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.clone())
            .collect::<Vec<_>>(),
    );
    attach_flow_sinks(&mut propagation, finding);
    attach_observation_facts(&mut observation, finding);
    attach_discrimination_facts(
        &mut discrimination,
        &required_discriminator.identity,
        !finding.activation.missing_discriminators.is_empty(),
    );

    for reason in finding.effective_stop_reasons() {
        limitations.push(format!("stop_reason:{}", reason.as_str()));
        match reason {
            StopReason::ProcMacroOpaque | StopReason::MacroReachUnresolved => {
                limitations.push("macro_limit".to_string());
            }
            StopReason::FixtureOpaque => limitations.push("opaque_fixture".to_string()),
            StopReason::DynamicDispatchUnresolved => {
                limitations.push("dynamic_dispatch".to_string());
            }
            StopReason::AsyncBoundaryOpaque => {
                limitations.push("closure_or_async_limit".to_string());
            }
            _ => {}
        }
    }
    if let Some(kind) = finding.static_limit_kind {
        limitations.push(format!("static_limit:{}", kind.as_str()));
        match kind {
            crate::domain::StaticLimitKind::DynamicDispatch => {
                limitations.push("dynamic_dispatch".to_string());
            }
            crate::domain::StaticLimitKind::RustMacroReachUnresolved
            | crate::domain::StaticLimitKind::RustMacroWrappedTestCallUnresolved
            | crate::domain::StaticLimitKind::RustMacroWrappedAssertionUnresolved => {
                limitations.push("macro_limit".to_string());
            }
            crate::domain::StaticLimitKind::CrossLanguageOracleVisibilityUnresolved => {
                limitations.push("cross_language_limit".to_string());
            }
            _ => {}
        }
    }
    match finding.source_currentness {
        SourceCurrentness::CandidateCurrent => {}
        other => limitations.push(format!("source_currentness:{}", other.as_str())),
    }

    BehaviorEvidenceWitnessV1 {
        schema_version: super::SCHEMA_VERSION,
        generation: super::GENERATION,
        path: AnalysisPath::Diff,
        portable_item_id,
        subject_set: input.subject_set,
        currentness: input.currentness,
        language,
        language_status,
        family,
        owner,
        expression,
        required_discriminator,
        expected_sink,
        candidate_relations,
        established_relations,
        reach,
        activation,
        propagation,
        observation,
        discrimination,
        selected_target: TargetState::TypedAbsence {
            reason: "finding_has_no_test_target_authority".to_string(),
        },
        earliest_unresolved_edge: None,
        limitations,
        non_claims,
        public_class: finding.class.as_str().to_string(),
        semantic_digest: String::new(),
    }
    .finalize()
}

/// Project a classified repository seam. Stage meaning is copied, never recomputed.
pub(crate) fn from_classified_seam(entry: &ClassifiedSeam) -> BehaviorEvidenceWitnessV1 {
    from_classified_seam_with_input(entry, AdapterInput::workspace_complete_current())
}

pub(crate) fn from_classified_seam_with_input(
    entry: &ClassifiedSeam,
    input: AdapterInput,
) -> BehaviorEvidenceWitnessV1 {
    let mut limitations = Vec::new();
    let mut non_claims = vec![
        "adapter_does_not_recompute_stage_meaning".to_string(),
        "public_class_is_producer_projection".to_string(),
        "adapter_does_not_strengthen_candidate_relations".to_string(),
    ];
    push_input_limitations(input, &mut limitations);

    let portable_item_id = canonical_gap_identity(entry)
        .map(|identity| identity.id)
        .unwrap_or_else(|| entry.seam.id().as_str().to_string());
    if portable_item_id.is_empty() {
        limitations.push("missing_portable_item_identity".to_string());
    }

    let (candidate_relations, established_relations) =
        split_repo_relations(&entry.evidence.related_tests);
    let mut reach = StageWitness::from_producer("reach", &entry.evidence.reach);
    let mut activation = StageWitness::from_producer("activation", &entry.evidence.activate);
    let propagation = StageWitness::from_producer("propagation", &entry.evidence.propagate);
    let observation = StageWitness::from_producer("observation", &entry.evidence.observe);
    let mut discrimination =
        StageWitness::from_producer("discrimination", &entry.evidence.discriminate);

    attach_relation_facts(
        &mut reach,
        &established_relations,
        &candidate_relations,
        &mut limitations,
    );
    attach_activation_facts(
        &mut activation,
        &entry
            .evidence
            .observed_values
            .iter()
            .map(|value| value.value.clone())
            .collect::<Vec<_>>(),
        &entry
            .evidence
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.clone())
            .collect::<Vec<_>>(),
    );
    let required_discriminator = discriminator_from_seam(entry.seam.required_discriminator());
    attach_discrimination_facts(
        &mut discrimination,
        &required_discriminator.identity,
        !entry.evidence.missing_discriminators.is_empty(),
    );

    let selected_target = repo_target(&entry.evidence.related_tests, &mut non_claims);

    BehaviorEvidenceWitnessV1 {
        schema_version: super::SCHEMA_VERSION,
        generation: super::GENERATION,
        path: AnalysisPath::Repo,
        portable_item_id,
        subject_set: input.subject_set,
        currentness: input.currentness,
        language: LanguageId::Rust.as_str().to_string(),
        language_status: Some(LanguageStatus::Stable.as_str().to_string()),
        family: entry.seam.kind().as_str().to_string(),
        owner: OwnerIdentity {
            identity: entry.seam.owner().to_string(),
            source: "seam_owner".to_string(),
        },
        expression: ExpressionIdentity {
            normalized: normalize_semantic_text(entry.seam.expression()),
        },
        required_discriminator,
        expected_sink: entry.seam.expected_sink().as_str().to_string(),
        candidate_relations,
        established_relations,
        reach,
        activation,
        propagation,
        observation,
        discrimination,
        selected_target,
        earliest_unresolved_edge: None,
        limitations,
        non_claims,
        public_class: entry.class.as_str().to_string(),
        semantic_digest: String::new(),
    }
    .finalize()
}

fn push_input_limitations(input: AdapterInput, limitations: &mut Vec<String>) {
    match input.subject_set {
        SubjectSet::Unspecified => limitations.push("unspecified_input_completeness".to_string()),
        other => limitations.push(other.as_str().to_string()),
    }
    match input.currentness {
        InputCurrentness::Current => {}
        InputCurrentness::Unspecified => limitations.push("unspecified_currentness".to_string()),
        other => limitations.push(other.as_str().to_string()),
    }
}

fn family_from_probe(family: &ProbeFamily) -> String {
    match family {
        ProbeFamily::Predicate => "predicate_boundary".to_string(),
        ProbeFamily::ReturnValue => "return_value".to_string(),
        ProbeFamily::ErrorPath => "error_variant".to_string(),
        ProbeFamily::FieldConstruction => "field_construction".to_string(),
        ProbeFamily::SideEffect => "side_effect".to_string(),
        ProbeFamily::MatchArm => "match_arm".to_string(),
        ProbeFamily::CallDeletion => "call_deletion".to_string(),
        ProbeFamily::StaticUnknown => "static_unknown".to_string(),
    }
}

fn owner_from_finding(finding: &Finding, limitations: &mut Vec<String>) -> OwnerIdentity {
    if let Some(owner) = finding.probe.owner.as_ref() {
        OwnerIdentity {
            identity: owner.0.clone(),
            source: "probe_owner".to_string(),
        }
    } else if let Some(gap) = finding.canonical_gap.as_ref() {
        OwnerIdentity {
            identity: gap.owner.clone(),
            source: "canonical_gap_owner".to_string(),
        }
    } else {
        limitations.push("unresolved_owner".to_string());
        OwnerIdentity {
            identity: String::new(),
            source: "unresolved".to_string(),
        }
    }
}

fn discriminator_from_finding(
    finding: &Finding,
    limitations: &mut Vec<String>,
) -> DiscriminatorIdentity {
    let family = family_from_probe(&finding.probe.family);
    if let Some(gap) = finding
        .canonical_gap
        .as_ref()
        .filter(|gap| !gap.normalized_discriminator.trim().is_empty())
    {
        return DiscriminatorIdentity {
            identity: prefix_discriminator(&family, &gap.normalized_discriminator),
            source: "canonical_gap".to_string(),
        };
    }
    if let Some(first) = finding.activation.missing_discriminators.first() {
        return DiscriminatorIdentity {
            identity: prefix_discriminator(&family, &first.value),
            source: "missing_discriminator_fact".to_string(),
        };
    }
    if let Some(first) = finding.probe.required_oracles.first() {
        return DiscriminatorIdentity {
            identity: prefix_discriminator(&family, first),
            source: "required_oracle".to_string(),
        };
    }
    limitations.push("unrepresentable_discriminator".to_string());
    DiscriminatorIdentity {
        identity: String::new(),
        source: "unrepresentable".to_string(),
    }
}

fn prefix_discriminator(family: &str, raw: &str) -> String {
    let normalized = normalize_semantic_text(raw);
    if normalized.is_empty() || normalized.contains(':') {
        return normalized;
    }
    let prefix = match family {
        "predicate_boundary" => "boundary_value",
        "error_variant" => "error_variant",
        "return_value" => "return_value",
        "field_construction" => "field_value",
        "side_effect" => "effect",
        "match_arm" => "match_arm_taken",
        "call_presence" | "call_deletion" => "call_site",
        _ => return normalized,
    };
    format!("{prefix}:{normalized}")
}

fn discriminator_from_seam(required: &RequiredDiscriminator) -> DiscriminatorIdentity {
    let identity = match required {
        RequiredDiscriminator::BoundaryValue { description } => {
            format!("boundary_value:{}", normalize_semantic_text(description))
        }
        RequiredDiscriminator::ErrorVariant { variant } => {
            format!("error_variant:{}", normalize_semantic_text(variant))
        }
        RequiredDiscriminator::ReturnValue { description } => {
            format!("return_value:{}", normalize_semantic_text(description))
        }
        RequiredDiscriminator::FieldValue { field } => {
            format!("field_value:{}", normalize_semantic_text(field))
        }
        RequiredDiscriminator::Effect { sink } => {
            format!("effect:{}", normalize_semantic_text(sink))
        }
        RequiredDiscriminator::MatchArmTaken { arm } => {
            format!("match_arm_taken:{}", normalize_semantic_text(arm))
        }
        RequiredDiscriminator::CallSite { target } => {
            format!("call_site:{}", normalize_semantic_text(target))
        }
    };
    DiscriminatorIdentity {
        identity,
        source: "seam_required_discriminator".to_string(),
    }
}

fn sink_from_finding(finding: &Finding, limitations: &mut Vec<String>) -> String {
    if let Some(sink) = finding
        .changed_sink
        .as_ref()
        .filter(|sink| !sink.trim().is_empty())
    {
        return normalize_semantic_text(sink);
    }
    if let Some(sink) = finding.probe.expected_sinks.first() {
        return normalize_semantic_text(sink);
    }
    if let Some(sink) = finding.flow_sinks.first() {
        return sink.kind.as_str().to_string();
    }
    limitations.push("unrepresentable_sink".to_string());
    String::new()
}

fn split_finding_relations(
    related: &[RelatedTest],
) -> (Vec<RelationWitness>, Vec<RelationWitness>) {
    let mut candidate = Vec::new();
    let mut established = Vec::new();
    for test in related {
        let Some(reason) = test.relation_reason else {
            candidate.push(RelationWitness {
                relation_reason: "unknown".to_string(),
                relation_confidence: "unknown".to_string(),
                oracle_kind: test.oracle_kind.as_str().to_string(),
                oracle_strength: test.oracle_strength.as_str().to_string(),
                status: RelationStatus::Candidate,
                has_test_target: false,
            });
            continue;
        };
        let witness = relation_from_reason(
            reason,
            test.oracle_kind.as_str(),
            test.oracle_strength.as_str(),
            false,
        );
        if witness.status == RelationStatus::Established {
            established.push(witness);
        } else {
            candidate.push(witness);
        }
    }
    (candidate, established)
}

fn split_repo_relations(
    related: &[RelatedTestGrip],
) -> (Vec<RelationWitness>, Vec<RelationWitness>) {
    let mut candidate = Vec::new();
    let mut established = Vec::new();
    for test in related {
        let witness = relation_from_reason(
            test.relation_reason,
            test.oracle_kind.as_str(),
            test.oracle_strength.as_str(),
            test.test_target.is_some(),
        );
        if witness.status == RelationStatus::Established {
            established.push(witness);
        } else {
            candidate.push(witness);
        }
    }
    (candidate, established)
}

/// Portable relation identity is the producer reason plus oracle facts.
/// Test names, files, and checkout spelling are locators, not digest
/// members; renaming a related test must not change equality.
fn relation_from_reason(
    reason: RelationReason,
    oracle_kind: &str,
    oracle_strength: &str,
    has_test_target: bool,
) -> RelationWitness {
    let established = established_reason(reason);
    RelationWitness {
        relation_reason: reason.as_str().to_string(),
        relation_confidence: reason.confidence().as_str().to_string(),
        oracle_kind: oracle_kind.to_string(),
        oracle_strength: oracle_strength.to_string(),
        status: if established {
            RelationStatus::Established
        } else {
            RelationStatus::Candidate
        },
        has_test_target,
    }
}

fn attach_relation_facts(
    reach: &mut StageWitness,
    established: &[RelationWitness],
    candidate: &[RelationWitness],
    limitations: &mut Vec<String>,
) {
    for relation in established {
        let identity = relation.semantic_identity();
        reach.push_source(identity.clone());
        reach.established_facts.push(identity);
    }
    for relation in candidate {
        let identity = relation.semantic_identity();
        reach.push_source(identity.clone());
        reach.candidate_facts.push(identity);
    }
    if reach.state == "yes" && reach.established_facts.is_empty() {
        limitations.push("producer_reach_without_established_relation".to_string());
        reach
            .limitations
            .push("producer_reach_without_established_relation".to_string());
    }
}

fn attach_activation_facts(
    activation: &mut StageWitness,
    observed_values: &[String],
    missing: &[String],
) {
    for value in observed_values {
        let identity = format!("observed:{}", normalize_semantic_text(value));
        activation.push_source(identity.clone());
        activation.established_facts.push(identity);
    }
    for value in missing {
        let identity = format!("missing:{}", normalize_semantic_text(value));
        activation.push_source(identity.clone());
        activation.candidate_facts.push(identity);
    }
}

fn attach_flow_sinks(propagation: &mut StageWitness, finding: &Finding) {
    for sink in &finding.flow_sinks {
        let identity = format!(
            "flow_sink:{}:{}",
            sink.kind.as_str(),
            normalize_semantic_text(&sink.text)
        );
        propagation.push_source(identity.clone());
        if sink.kind == crate::domain::FlowSinkKind::Unknown {
            propagation.candidate_facts.push(identity);
        } else {
            propagation.established_facts.push(identity);
        }
    }
}

fn attach_observation_facts(observation: &mut StageWitness, finding: &Finding) {
    if let Some(alignment) = finding.oracle_alignment.as_ref() {
        let identity = format!("oracle_alignment:{alignment}");
        observation.push_source(identity.clone());
        if alignment == "direct" {
            observation.established_facts.push(identity);
        } else {
            observation.candidate_facts.push(identity);
        }
    }
    if let Some(sink) = finding.observed_sink.as_ref() {
        observation
            .established_facts
            .push(format!("observed_sink:{}", normalize_semantic_text(sink)));
    }
}

fn attach_discrimination_facts(
    discrimination: &mut StageWitness,
    discriminator: &str,
    missing: bool,
) {
    if discriminator.is_empty() {
        return;
    }
    let identity = format!("discriminator:{discriminator}");
    discrimination.push_source(identity.clone());
    if missing {
        discrimination.candidate_facts.push(identity);
    } else if discrimination.state == "yes" {
        discrimination.established_facts.push(identity);
    } else {
        discrimination.candidate_facts.push(identity);
    }
}

fn repo_target(related: &[RelatedTestGrip], non_claims: &mut Vec<String>) -> TargetState {
    let mut identities: Vec<String> = related
        .iter()
        .filter_map(|test| {
            test.test_target
                .as_ref()
                .map(|target| target.symbol_id().0.clone())
        })
        .collect();
    identities.sort();
    identities.dedup();
    if identities.is_empty() {
        non_claims.push("no_producer_test_target".to_string());
        TargetState::TypedAbsence {
            reason: "no_producer_test_target".to_string(),
        }
    } else {
        TargetState::SelectedExisting { identities }
    }
}
