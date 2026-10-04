//! Bounded JSON and Markdown projections for executed-control packets.
//!
//! Both projections derive from one [`ExecutedControlPacketV1`]. They do not
//! inspect live GitHub, enforce merge eligibility, or rewrite historical
//! execution as pass.

use crate::domain::{
    EXECUTED_CONTROL_PACKET_KIND, EXECUTED_CONTROL_SCHEMA_VERSION, ExecutedControlPacketV1,
    ObligationSatisfaction, ResultState,
};
use crate::output::json;

/// Canonical JSON for one packet. Obligation and result order is sorted first
/// so input/map order cannot change semantic identity. Invalid packets are
/// rejected; this projection does not emit an unvalidated pass claim.
pub(crate) fn render_packet_json(packet: &ExecutedControlPacketV1) -> Result<String, String> {
    let mut canonical = packet.clone();
    canonical.canonicalize();
    canonical.validate().map_err(|error| error.to_string())?;
    json::render_pretty_with_newline(&canonical, "executed-control packet")
}

/// Bounded human projection of the same packet object.
pub(crate) fn render_packet_markdown(packet: &ExecutedControlPacketV1) -> Result<String, String> {
    let mut canonical = packet.clone();
    canonical.canonicalize();
    let evaluation = canonical.validate().map_err(|error| error.to_string())?;
    Ok(render_validated_markdown(
        &canonical,
        &evaluation.satisfactions,
    ))
}

fn render_validated_markdown(
    packet: &ExecutedControlPacketV1,
    satisfactions: &[ObligationSatisfaction],
) -> String {
    let mut lines = Vec::new();
    lines.push("# Executed-control packet".to_string());
    lines.push(String::new());
    lines.push(format!(
        "Schema: `{EXECUTED_CONTROL_PACKET_KIND}` version `{EXECUTED_CONTROL_SCHEMA_VERSION}`"
    ));
    lines.push(format!("Source: `{}`", packet.source_identity));
    lines.push(String::new());
    lines.push(
        "This packet distinguishes executed discriminating controls from ordinary positive tests and review prose. It does not inspect live GitHub, enforce merge eligibility, or rewrite historical execution as pass.".to_string(),
    );
    lines.push(String::new());
    lines.push("## Obligations".to_string());
    lines.push(String::new());
    if packet.obligations.is_empty() {
        lines.push("No obligations declared.".to_string());
    } else {
        for obligation in &packet.obligations {
            let substitute = match &obligation.permitted_substitute {
                None => "none".to_string(),
                Some(value) => format!("`{}` via `{}`", value.substitute_id, value.instrument_id),
            };
            lines.push(format!(
                "- `{}` ({}; {}; expected `{}`; subject `{}` on `{}`; substitute {substitute})",
                obligation.obligation_id,
                obligation.requiredness.as_str(),
                obligation.control_class.as_str(),
                obligation.expected_discriminating_outcome.as_str(),
                obligation
                    .required_execution_subject
                    .command_or_instrument_id,
                obligation.required_execution_subject.required_head
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Results".to_string());
    lines.push(String::new());
    if packet.results.is_empty() {
        lines.push("No results recorded.".to_string());
    } else {
        for result in &packet.results {
            let artifact = match &result.artifact {
                None => "no retained artifact".to_string(),
                Some(value) => format!("artifact `{}` `{}`", value.logical_id, value.digest),
            };
            let limitation = result
                .limitation
                .as_deref()
                .map(|text| format!("; limitation: {text}"))
                .unwrap_or_default();
            lines.push(format!(
                "- `{}`: state=`{}`; evidence=`{}`; observed=`{}`; head=`{}`; command=`{}`; {artifact}{limitation}",
                result.obligation_id,
                result.state.as_str(),
                result.offered_evidence_kind.as_str(),
                result.observed_outcome.as_str(),
                result.head,
                result.command_or_instrument_id
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Satisfaction".to_string());
    lines.push(String::new());
    lines.push(
        "`passed` requires an executed discriminating control that exercised the named wrong implementation. Ordinary positive tests, review prose, and structural-discrimination arguments cannot satisfy an obligation. `not_run`, `not_proven`, `substituted`, and `instrument_failure` stay explicit; only a substitute declared on the obligation can satisfy in place of a pass.".to_string(),
    );
    lines.push(String::new());
    for row in satisfactions {
        let state = row.state.map(ResultState::as_str).unwrap_or("absent");
        let verdict = if row.satisfies {
            "satisfies"
        } else {
            "does_not_satisfy"
        };
        lines.push(format!(
            "- `{}` ({}) {verdict}; recorded state=`{state}`",
            row.obligation_id,
            row.requiredness.as_str()
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::executed_control::tests::{
        HEAD_AFTER, HEAD_BEFORE, HEAD_OTHER, passing_result as domain_passing, sample_obligation,
    };
    use crate::domain::executed_control::{
        ControlClass, DiscriminatingOutcome, EXECUTED_CONTROL_OBLIGATION_KIND,
        EXECUTED_CONTROL_RESULT_KIND, EvidenceForm, ExecutedControlObligationV1,
        ExecutedControlPacketV1, ExecutedControlResultV1, ExecutionSubject, Invalidator,
        ObservedOutcome, OfferedEvidenceKind, PermittedSubstitute, Requiredness, ResultState,
    };
    use crate::output::test_support::{read_file, repo_root};
    use serde_json::{Value, json};

    fn packet(
        obligations: Vec<ExecutedControlObligationV1>,
        results: Vec<ExecutedControlResultV1>,
    ) -> ExecutedControlPacketV1 {
        ExecutedControlPacketV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_PACKET_KIND.to_string(),
            source_identity: "EffortlessMetrics/ripr-swarm".to_string(),
            obligations,
            results,
        }
    }

    fn passing_result(obligation: &ExecutedControlObligationV1) -> ExecutedControlResultV1 {
        domain_passing(obligation)
    }

    fn failed_before(obligation: &ExecutedControlObligationV1) -> ExecutedControlResultV1 {
        ExecutedControlResultV1 {
            head: HEAD_BEFORE.to_string(),
            observed_outcome: ObservedOutcome::RejectedWrongImplementation,
            state: ResultState::Failed,
            obligation_digest: None,
            ..passing_result(obligation)
        }
    }

    struct CorpusCase {
        id: &'static str,
        description: &'static str,
        invalid: bool,
        expected_failure: Option<&'static str>,
        packet: ExecutedControlPacketV1,
    }

    fn corpus_cases() -> Vec<CorpusCase> {
        let obligation = sample_obligation();
        let mut ordinary = passing_result(&obligation);
        ordinary.offered_evidence_kind = OfferedEvidenceKind::OrdinaryPositiveTest;
        ordinary.observed_outcome = ObservedOutcome::OrdinaryPositiveTestsPassed;

        let mut prose = passing_result(&obligation);
        prose.offered_evidence_kind = OfferedEvidenceKind::ReviewProse;
        prose.observed_outcome = ObservedOutcome::ReviewArgumentOnly;
        prose.artifact = None;

        let mut other_head = passing_result(&obligation);
        other_head.head = HEAD_OTHER.to_string();

        let mut unexercised = passing_result(&obligation);
        unexercised.observed_outcome = ObservedOutcome::CommandSucceededWithoutExercisingSubject;

        let mut with_substitute = obligation.clone();
        with_substitute
            .acceptable_evidence_forms
            .push(EvidenceForm::DeclaredSubstitute);
        with_substitute.permitted_substitute = Some(PermittedSubstitute {
            substitute_id: "hosted-eager-variant".to_string(),
            instrument_id: "hosted-mutation-runner".to_string(),
            evidence_form: EvidenceForm::DeclaredSubstitute,
        });
        let substitute_result = ExecutedControlResultV1 {
            command_or_instrument_id: "hosted-mutation-runner".to_string(),
            offered_evidence_kind: OfferedEvidenceKind::DeclaredSubstitute,
            observed_outcome: ObservedOutcome::InstrumentUnavailable,
            state: ResultState::Substituted,
            substitute_id: Some("hosted-eager-variant".to_string()),
            obligation_digest: Some(with_substitute.semantic_digest()),
            limitation: Some(
                "local eager-variant instrument unavailable; declared hosted substitute ran"
                    .to_string(),
            ),
            ..passing_result(&with_substitute)
        };

        let instrument_failure = ExecutedControlResultV1 {
            offered_evidence_kind: OfferedEvidenceKind::ExecutedDiscriminatingControl,
            observed_outcome: ObservedOutcome::InstrumentUnavailable,
            state: ResultState::InstrumentFailure,
            artifact: None,
            obligation_digest: None,
            limitation: Some("eager-variant instrument was not available".to_string()),
            ..passing_result(&obligation)
        };

        let mut unknown = passing_result(&obligation);
        unknown.obligation_id = "claim:unknown".to_string();

        let mut foreign_source = passing_result(&obligation);
        foreign_source.source_identity = "other/repo".to_string();

        let mut only_substitute = obligation.clone();
        only_substitute.acceptable_evidence_forms = vec![EvidenceForm::DeclaredSubstitute];
        only_substitute.permitted_substitute = Some(PermittedSubstitute {
            substitute_id: "hosted-eager-variant".to_string(),
            instrument_id: "hosted-mutation-runner".to_string(),
            evidence_form: EvidenceForm::DeclaredSubstitute,
        });
        let disallowed_form = passing_result(&only_substitute);

        let unexecuted_substitute = ExecutedControlResultV1 {
            command_or_instrument_id: "hosted-mutation-runner".to_string(),
            offered_evidence_kind: OfferedEvidenceKind::DeclaredSubstitute,
            observed_outcome: ObservedOutcome::NotExecuted,
            state: ResultState::Substituted,
            substitute_id: Some("hosted-eager-variant".to_string()),
            obligation_digest: Some(with_substitute.semantic_digest()),
            ..passing_result(&with_substitute)
        };

        let mut same_head_fail = passing_result(&obligation);
        same_head_fail.state = ResultState::Failed;
        same_head_fail.observed_outcome = ObservedOutcome::RejectedWrongImplementation;
        same_head_fail.obligation_digest = None;

        let mut other_obligation = obligation.clone();
        other_obligation.obligation_id = "claim:example:other".to_string();

        vec![
            CorpusCase {
                id: "fail_before_pass_after",
                description: "Executed removal control fails before and passes after the repair.",
                invalid: false,
                expected_failure: None,
                packet: packet(
                    vec![obligation.clone()],
                    vec![failed_before(&obligation), passing_result(&obligation)],
                ),
            },
            CorpusCase {
                id: "ordinary_positive_test",
                description: "Ordinary positive test incorrectly offered as the control.",
                invalid: true,
                expected_failure: Some("ordinary positive test"),
                packet: packet(vec![obligation.clone()], vec![ordinary]),
            },
            CorpusCase {
                id: "prose_without_artifact",
                description: "Control claimed in prose with no retained artifact.",
                invalid: true,
                expected_failure: Some("review prose"),
                packet: packet(vec![obligation.clone()], vec![prose]),
            },
            CorpusCase {
                id: "other_head",
                description: "Control run on another head.",
                invalid: true,
                expected_failure: Some("binds head"),
                packet: packet(vec![obligation.clone()], vec![other_head]),
            },
            CorpusCase {
                id: "command_succeeded_without_exercising_subject",
                description: "Command succeeded but did not exercise the named wrong implementation.",
                invalid: true,
                expected_failure: Some("without exercising"),
                packet: packet(vec![obligation.clone()], vec![unexercised]),
            },
            CorpusCase {
                id: "declared_substitute",
                description: "Unavailable instrument with an explicitly accepted substitute.",
                invalid: false,
                expected_failure: None,
                packet: packet(vec![with_substitute.clone()], vec![substitute_result]),
            },
            CorpusCase {
                id: "instrument_failure_without_substitute",
                description: "Unavailable instrument without an accepted substitute.",
                invalid: false,
                expected_failure: None,
                packet: packet(vec![obligation.clone()], vec![instrument_failure]),
            },
            CorpusCase {
                id: "duplicate_obligation_ids",
                description: "Duplicated obligation IDs.",
                invalid: true,
                expected_failure: Some("duplicated obligation_id"),
                packet: packet(
                    vec![obligation.clone(), obligation.clone()],
                    vec![passing_result(&obligation)],
                ),
            },
            CorpusCase {
                id: "unknown_obligation",
                description: "Result for an unknown obligation.",
                invalid: true,
                expected_failure: Some("unknown obligation_id"),
                packet: packet(vec![obligation.clone()], vec![unknown]),
            },
            CorpusCase {
                id: "foreign_source",
                description: "Result whose source_identity is not the packet source.",
                invalid: true,
                expected_failure: Some("source identity"),
                packet: packet(vec![obligation.clone()], vec![foreign_source]),
            },
            CorpusCase {
                id: "unexecuted_substitute",
                description: "Named substitute recorded as not_executed.",
                invalid: true,
                expected_failure: Some("unexecuted substitute"),
                packet: packet(vec![with_substitute.clone()], vec![unexecuted_substitute]),
            },
            CorpusCase {
                id: "disallowed_evidence_form",
                description: "Passed retained-artifact result against an obligation that accepts only declared_substitute.",
                invalid: true,
                expected_failure: Some("acceptable evidence form"),
                packet: packet(vec![only_substitute], vec![disallowed_form]),
            },
            CorpusCase {
                id: "same_head_pass_and_fail",
                description: "Passed and failed results for the same head and command.",
                invalid: true,
                expected_failure: Some("conflicting passed and failed"),
                packet: packet(
                    vec![obligation.clone()],
                    vec![same_head_fail, passing_result(&obligation)],
                ),
            },
            CorpusCase {
                id: "input_order_independent",
                description: "Deterministic output independent of input/map order.",
                invalid: false,
                expected_failure: None,
                packet: packet(
                    vec![other_obligation.clone(), obligation.clone()],
                    vec![
                        passing_result(&other_obligation),
                        passing_result(&obligation),
                    ],
                ),
            },
            CorpusCase {
                id: "issue_3858_not_proven",
                description: "Issue #3858 / PR #4063 execution is not established and must not be rewritten as passed.",
                invalid: false,
                expected_failure: None,
                packet: issue_3858_packet(),
            },
        ]
    }

    fn issue_3858_packet() -> ExecutedControlPacketV1 {
        let obligation = ExecutedControlObligationV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_OBLIGATION_KIND.to_string(),
            obligation_id: "issue:3858:eager-file-count-removal-control".to_string(),
            owning_claim: "issue:3858".to_string(),
            control_class: ControlClass::RemovedGuard,
            intended_wrong_implementation:
                "eager parse / removed lazy file-count guard (admit_file_count)".to_string(),
            required_execution_subject: ExecutionSubject {
                command_or_instrument_id: "cargo test -- removed_guard_or_eager_file_count_control"
                    .to_string(),
                named_wrong_implementation: "eager_admit_file_count".to_string(),
                required_head: HEAD_AFTER.to_string(),
            },
            expected_discriminating_outcome: DiscriminatingOutcome::FailsBeforePassesAfter,
            acceptable_evidence_forms: vec![
                EvidenceForm::RetainedArtifact,
                EvidenceForm::BoundedLogCommitment,
            ],
            permitted_substitute: None,
            requiredness: Requiredness::Required,
            invalidators: vec![
                Invalidator::SourceHeadMoved,
                Invalidator::ControlContractChanged,
                Invalidator::ArtifactMissing,
                Invalidator::CommandIdentityChanged,
            ],
        };
        let result = ExecutedControlResultV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_RESULT_KIND.to_string(),
            obligation_id: obligation.obligation_id.clone(),
            source_identity: "EffortlessMetrics/ripr-swarm".to_string(),
            candidate_id: Some("pr:4063".to_string()),
            head: HEAD_AFTER.to_string(),
            command_or_instrument_id: obligation
                .required_execution_subject
                .command_or_instrument_id
                .clone(),
            artifact: None,
            observed_outcome: ObservedOutcome::NotExecuted,
            offered_evidence_kind: OfferedEvidenceKind::ReviewProse,
            state: ResultState::NotProven,
            limitation: Some(
                "Issue #3858 / PR #4063 recorded no retained eager or removed-guard execution artifact. Execution is not_proven and must not be rewritten as passed.".to_string(),
            ),
            substitute_id: None,
            obligation_digest: None,
        };
        packet(vec![obligation], vec![result])
    }

    #[test]
    fn json_and_human_projections_agree_on_satisfaction_and_are_deterministic() -> Result<(), String>
    {
        let obligation = sample_obligation();
        let mut first = packet(
            vec![obligation.clone()],
            vec![failed_before(&obligation), passing_result(&obligation)],
        );
        let mut second = first.clone();
        second.results.reverse();
        first.canonicalize();
        second.canonicalize();
        let json_left = render_packet_json(&first)?;
        let json_right = render_packet_json(&second)?;
        assert_eq!(json_left, json_right);
        let human_left = render_packet_markdown(&first)?;
        let human_right = render_packet_markdown(&second)?;
        assert_eq!(human_left, human_right);
        assert!(human_left.contains("state=`passed`"));
        assert!(human_left.contains("state=`failed`"));
        assert!(human_left.contains("satisfies"));
        assert!(!human_left.contains("killed")); // ripr-allow: static-language: test guard verifying projection does not emit forbidden mutation-testing term
        assert!(!human_left.contains("survived")); // ripr-allow: static-language: test guard verifying projection does not emit forbidden mutation-testing term
        assert!(!human_left.contains("adequate")); // ripr-allow: static-language: test guard verifying projection does not emit forbidden mutation-testing term
        Ok(())
    }

    #[test]
    fn json_and_markdown_projections_reject_the_same_invalid_packet() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut ordinary = passing_result(&obligation);
        ordinary.offered_evidence_kind = OfferedEvidenceKind::OrdinaryPositiveTest;
        ordinary.observed_outcome = ObservedOutcome::OrdinaryPositiveTestsPassed;
        let invalid = packet(vec![obligation], vec![ordinary]);
        let json_error = match render_packet_json(&invalid) {
            Err(error) => error,
            Ok(_) => return Err("json should reject the invalid packet".to_string()),
        };
        let markdown_error = match render_packet_markdown(&invalid) {
            Err(error) => error,
            Ok(_) => return Err("markdown should reject the invalid packet".to_string()),
        };
        assert!(json_error.contains("ordinary positive test"));
        assert!(markdown_error.contains("ordinary positive test"));
        Ok(())
    }

    #[test]
    fn human_projection_keeps_issue_3858_not_proven() -> Result<(), String> {
        let packet = issue_3858_packet();
        let human = render_packet_markdown(&packet)?;
        assert!(human.contains("issue:3858:eager-file-count-removal-control"));
        assert!(human.contains("state=`not_proven`"));
        assert!(human.contains("does_not_satisfy"));
        assert!(!human.contains("state=`passed`"));
        assert!(human.contains("must not be rewritten as passed"));
        let json = render_packet_json(&packet)?;
        assert!(json.contains("\"state\": \"not_proven\""));
        assert!(!json.contains("\"state\": \"passed\""));
        Ok(())
    }

    fn corpus_document() -> Result<Value, String> {
        let mut cases = Vec::new();
        for case in corpus_cases() {
            let mut packet = case.packet.clone();
            packet.canonicalize();
            let packet_value = serde_json::to_value(&packet)
                .map_err(|error| format!("serialize {}: {error}", case.id))?;
            let mut object = serde_json::Map::new();
            object.insert("id".to_string(), json!(case.id));
            object.insert("description".to_string(), json!(case.description));
            if case.invalid {
                object.insert("invalid".to_string(), json!(true));
            }
            if let Some(expected) = case.expected_failure {
                object.insert("expected_failure".to_string(), json!(expected));
            }
            object.insert("packet".to_string(), packet_value);
            cases.push(Value::Object(object));
        }
        Ok(json!({
            "schema_version": "1",
            "kind": "executed_control_corpus",
            "cases": cases
        }))
    }

    #[test]
    fn corpus_covers_required_negative_and_documentation_cases() -> Result<(), String> {
        for case in corpus_cases() {
            match case.packet.validate() {
                Ok(_) if case.invalid => {
                    return Err(format!("{} advertised invalid but validated", case.id));
                }
                Ok(evaluation) => {
                    if case.id.contains("3858") {
                        assert!(evaluation.satisfactions.iter().all(|row| !row.satisfies));
                        assert_eq!(
                            evaluation.satisfactions[0].state,
                            Some(ResultState::NotProven)
                        );
                    }
                    if case.id == "fail_before_pass_after" || case.id == "declared_substitute" {
                        assert!(evaluation.satisfactions.iter().all(|row| row.satisfies));
                    }
                    if case.id == "instrument_failure_without_substitute" {
                        assert!(!evaluation.satisfactions[0].satisfies);
                    }
                }
                Err(error) if case.invalid => {
                    if let Some(expected) = case.expected_failure {
                        let rendered = error.to_string();
                        assert!(
                            rendered.contains(expected),
                            "{} expected {expected}, got {error}",
                            case.id
                        );
                    }
                }
                Err(error) => return Err(format!("{} unexpectedly invalid: {error}", case.id)),
            }
        }
        let root = repo_root()?;
        let corpus_path = root.join("fixtures/executed-control-contract/corpus.json");
        let text = read_file(&corpus_path)?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| format!("parse corpus: {error}"))?;
        let expected = corpus_document()?;
        assert_eq!(
            value, expected,
            "committed corpus drifted from constructors"
        );
        Ok(())
    }

    #[test]
    fn goldens_match_human_and_json_for_documentation_and_pass_packets() -> Result<(), String> {
        let root = repo_root()?;
        let expected_dir = root.join("fixtures/executed-control-contract/expected");
        let pass = {
            let obligation = sample_obligation();
            packet(
                vec![obligation.clone()],
                vec![failed_before(&obligation), passing_result(&obligation)],
            )
        };
        let not_proven = issue_3858_packet();
        assert_eq!(
            render_packet_json(&pass)?,
            read_file(&expected_dir.join("valid-pass-after-repair.json"))?
        );
        assert_eq!(
            render_packet_markdown(&pass)?,
            read_file(&expected_dir.join("valid-pass-after-repair.md"))?
        );
        assert_eq!(
            render_packet_json(&not_proven)?,
            read_file(&expected_dir.join("issue-3858-not_proven.json"))?
        );
        assert_eq!(
            render_packet_markdown(&not_proven)?,
            read_file(&expected_dir.join("issue-3858-not_proven.md"))?
        );
        Ok(())
    }

    #[test]
    #[ignore = "writes committed fixtures; run explicitly when regenerating"]
    fn write_executed_control_contract_fixtures() -> Result<(), String> {
        let root = repo_root()?;
        let fixture_dir = root.join("fixtures/executed-control-contract");
        let expected_dir = fixture_dir.join("expected");
        std::fs::create_dir_all(&expected_dir)
            .map_err(|error| format!("create expected dir: {error}"))?;
        let corpus = serde_json::to_string_pretty(&corpus_document()?)
            .map_err(|error| format!("serialize corpus: {error}"))?;
        std::fs::write(fixture_dir.join("corpus.json"), format!("{corpus}\n"))
            .map_err(|error| format!("write corpus: {error}"))?;
        let pass = {
            let obligation = sample_obligation();
            packet(
                vec![obligation.clone()],
                vec![failed_before(&obligation), passing_result(&obligation)],
            )
        };
        let not_proven = issue_3858_packet();
        std::fs::write(
            expected_dir.join("valid-pass-after-repair.json"),
            render_packet_json(&pass)?,
        )
        .map_err(|error| format!("write pass json: {error}"))?;
        std::fs::write(
            expected_dir.join("valid-pass-after-repair.md"),
            render_packet_markdown(&pass)?,
        )
        .map_err(|error| format!("write pass md: {error}"))?;
        std::fs::write(
            expected_dir.join("issue-3858-not_proven.json"),
            render_packet_json(&not_proven)?,
        )
        .map_err(|error| format!("write 3858 json: {error}"))?;
        std::fs::write(
            expected_dir.join("issue-3858-not_proven.md"),
            render_packet_markdown(&not_proven)?,
        )
        .map_err(|error| format!("write 3858 md: {error}"))?;
        Ok(())
    }
}
