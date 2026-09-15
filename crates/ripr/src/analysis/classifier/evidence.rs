use crate::analysis::classify::{
    ProbeContext, PropagationWitnessV1, activation_evidence, classify, confidence_score,
    current_path_witness, file_imports_foreign_callee_name, infection_evidence, local_flow_sinks,
    package_prefix, propagation_evidence_with_witness, reach_evidence,
    reveal_evidence_with_expression,
};
use crate::domain::*;

pub(in crate::analysis) struct ClassifiedProbeEvidence {
    pub(in crate::analysis) ripr: RiprEvidence,
    pub(in crate::analysis) evidence: Vec<String>,
    pub(in crate::analysis) flow_sinks: Vec<FlowSinkFact>,
    /// Retained diagnostic witness outcome for the PR-A migration slice.  It
    /// is not projected into `Finding` or used to strengthen a stage yet.
    pub(in crate::analysis) propagation_witness: Option<PropagationWitnessDiagnostic>,
    pub(in crate::analysis) activation: ActivationEvidence,
    pub(in crate::analysis) related_tests: Vec<RelatedTest>,
    pub(in crate::analysis) reach: StageEvidence,
    pub(in crate::analysis) infect: StageEvidence,
    pub(in crate::analysis) propagate: StageEvidence,
    pub(in crate::analysis) observe: StageEvidence,
    pub(in crate::analysis) discriminate: StageEvidence,
}

impl ClassifiedProbeEvidence {
    pub(in crate::analysis) fn gather(context: &ProbeContext<'_>, reveal_expression: &str) -> Self {
        let test_summaries = context.related_test_summaries();
        let reach = reach_evidence(&context.related_tests, context.owner_fn);
        let flow_sinks = local_flow_sinks(context.probe, context.owner_fn);
        let propagation_witness = current_path_witness(context.probe, &flow_sinks)
            .map(PropagationWitnessDiagnostic::from_witness);
        let activation = activation_evidence(
            context.probe,
            context.owner_fn,
            &test_summaries,
            &flow_sinks,
            context.helper_chain.as_ref(),
            context.index,
            context.workspace_complete,
        );
        let infect = infection_evidence(context.probe, &test_summaries, &activation);
        let valid_witness = propagation_witness
            .as_ref()
            .and_then(|diagnostic| match diagnostic {
                PropagationWitnessDiagnostic::Valid(witness) => Some(witness),
                PropagationWitnessDiagnostic::InvalidDigest(_) => None,
            });
        let propagate =
            propagation_evidence_with_witness(context.probe, &flow_sinks, valid_witness);
        // #3731 review (G1): the changed owner's package scope, computed
        // once — the cross-package same-name defeat below compares each
        // related test's package against it.
        let owner_package = context
            .owner_fn
            .and_then(|owner| package_prefix(&owner.file));
        let (observe, discriminate, related_tests) = reveal_evidence_with_expression(
            context.probe,
            reveal_expression,
            &context.related_tests,
            // #3731 review (F11): the related test's file source is
            // reachable here, so the caller computes the same-name-import
            // defeat per test instead of restructuring the reveal inputs.
            &|test, callee| {
                context.index.files.get(&test.file).is_some_and(|facts| {
                    file_imports_foreign_callee_name(
                        &facts.source,
                        callee,
                        &context.index.package_names,
                    )
                })
            },
            // #3731 review (G1): the test's OWN package defining a
            // same-named function defeats the bare-scrutinee binding the
            // same way a foreign import does — the bare call in that test
            // may bind the local definition while the changed owner lives
            // in another package. Index-backed, not a new lexical scan:
            // package scopes come from the shared `package_prefix`
            // authority and the same-named definition from the workspace's
            // indexed functions. Both package scopes must resolve; an
            // unscopable side (single-crate relative paths, absolute
            // paths) keeps today's behavior.
            &|test, callee| {
                let Some(test_package) = package_prefix(&test.file) else {
                    return false;
                };
                let Some(owner_package) = owner_package.as_deref() else {
                    return false;
                };
                if test_package == owner_package {
                    return false;
                }
                context.index.functions.iter().any(|function| {
                    function.name == callee
                        && package_prefix(&function.file).as_deref() == Some(test_package.as_str())
                })
            },
        );

        let ripr = RiprEvidence {
            reach: reach.clone(),
            infect: infect.clone(),
            propagate: propagate.clone(),
            reveal: RevealEvidence {
                observe: observe.clone(),
                discriminate: discriminate.clone(),
            },
        };
        let evidence = evidence_summaries([&reach, &infect, &propagate, &observe, &discriminate]);

        Self {
            ripr,
            evidence,
            flow_sinks,
            propagation_witness,
            activation,
            related_tests,
            reach,
            infect,
            propagate,
            observe,
            discriminate,
        }
    }

    pub(in crate::analysis) fn classify(&self, probe: &Probe) -> ExposureClass {
        classify(
            &self.reach,
            &self.infect,
            &self.propagate,
            &self.observe,
            &self.discriminate,
            probe,
        )
    }

    pub(in crate::analysis) fn confidence(&self, class: &ExposureClass) -> f32 {
        confidence_score(
            &self.reach,
            &self.infect,
            &self.propagate,
            &self.observe,
            &self.discriminate,
            class,
        )
    }

    pub(in crate::analysis) fn propagation_witness(&self) -> Option<&PropagationWitnessDiagnostic> {
        self.propagation_witness.as_ref()
    }
}

pub(in crate::analysis) enum PropagationWitnessDiagnostic {
    Valid(PropagationWitnessV1),
    InvalidDigest(PropagationWitnessV1),
}

impl PropagationWitnessDiagnostic {
    fn from_witness(witness: PropagationWitnessV1) -> Self {
        if witness.digest_matches() {
            Self::Valid(witness)
        } else {
            Self::InvalidDigest(witness)
        }
    }

    pub(in crate::analysis) fn witness(&self) -> &PropagationWitnessV1 {
        match self {
            Self::Valid(witness) | Self::InvalidDigest(witness) => witness,
        }
    }

    pub(in crate::analysis) fn is_invalid(&self) -> bool {
        matches!(self, Self::InvalidDigest(_))
    }
}

fn evidence_summaries<'e>(stages: impl IntoIterator<Item = &'e StageEvidence>) -> Vec<String> {
    let mut summaries = stages
        .into_iter()
        .filter_map(|stage| (!stage.summary.is_empty()).then_some(stage.summary.clone()))
        .collect::<Vec<_>>();
    summaries.sort();
    summaries.dedup();
    summaries
}

#[cfg(test)]
mod tests {
    use super::evidence_summaries;
    use super::{ClassifiedProbeEvidence, ProbeContext, PropagationWitnessDiagnostic};
    use crate::analysis::classifier::finding::build_finding;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::{FunctionFact, FunctionSummary, ReturnFact, RustIndex};
    use crate::analysis::rust_index::{OracleFact, TestSummary, extract_identifier_tokens};
    use crate::domain::{
        Confidence, DeltaKind, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelationReason, SourceLocation, StageEvidence, StageState, SymbolId,
    };
    use std::path::PathBuf;

    // --- #3731 review (G1): the cross-package same-name defeat ---

    /// The guarded-match harness text whose bare scrutinee binds the owner
    /// by name — the confirmation the cross-package gate decides on.
    fn guarded_oracle() -> OracleFact {
        let text = "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }";
        OracleFact {
            line: 3,
            text: text.to_string(),
            kind: OracleKind::GuardedResultMatch,
            strength: OracleStrength::Strong,
            observed_tokens: extract_identifier_tokens(text),
            ok_value_observed: Some(true),
        }
    }

    fn harness_in(file: &str) -> TestSummary {
        TestSummary {
            name: "guards_the_result".to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 9,
            body: "match expect_response(&input, \"ready\") { .. }".to_string(),
            calls: Vec::new(),
            assertions: vec![guarded_oracle()],
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn same_named_function(file: &str) -> FunctionFact {
        FunctionFact {
            id: SymbolId(format!("{file}::expect_response")),
            name: "expect_response".to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body: "fn expect_response() -> Result<u32, ParseError> { Ok(1) }".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    /// The changed owner lives in package `alpha`; its guarded harness
    /// shares no changed-line token with the probe, so the reveal-side
    /// confirmation rides solely on the producer-owned bare binding.
    fn owner_harness_context(index: &RustIndex, test: TestSummary) -> ClassifiedProbeEvidence {
        let probe = Probe {
            id: ProbeId("probe:alpha:expect_response".to_string()),
            location: SourceLocation::new("crates/alpha/src/lib.rs", 10, 2),
            owner: Some(SymbolId(
                "crates/alpha/src/lib.rs::expect_response".to_string(),
            )),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("amount".to_string()),
            after: Some("amount + 1".to_string()),
            // No token of this expression appears in the guarded oracle
            // text, so only the owner binding can confirm.
            expression: "if trimmed != Some(expected_id.trim()).as_str() {".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };
        let owner = FunctionSummary {
            id: SymbolId("crates/alpha/src/lib.rs::expect_response".to_string()),
            name: "expect_response".to_string(),
            file: PathBuf::from("crates/alpha/src/lib.rs"),
            start_line: 1,
            end_line: 20,
            body: "fn expect_response() -> Result<u32, ParseError> { Ok(1) }".to_string(),
            calls: Vec::new(),
            returns: vec![ReturnFact {
                line: 14,
                text: "Ok(amount)".to_string(),
            }],
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let context = ProbeContext::new(
            &probe,
            Some(&owner),
            vec![(&test, RelationReason::DirectOwnerCall)],
            false,
            index,
            true,
        );
        ClassifiedProbeEvidence::gather(
            &context,
            "if trimmed != Some(expected_id.trim()).as_str() {",
        )
    }

    /// G1 falsifier: two packages each define `expect_response`; the test
    /// lives in package `beta` and calls bare `expect_response` while the
    /// probe's owner is package `alpha`'s — the bare binding is ambiguous
    /// across packages, so the observation stays unverified. Pre-fix this
    /// confirmed through the bare name.
    #[test]
    fn cross_package_same_name_function_defeats_owner_confirmation() {
        let index = RustIndex {
            functions: vec![
                same_named_function("crates/alpha/src/lib.rs"),
                same_named_function("crates/beta/src/lib.rs"),
            ],
            ..RustIndex::default()
        };
        let evidence = owner_harness_context(&index, harness_in("crates/beta/tests/protocol.rs"));
        assert_eq!(
            evidence.discriminate.state,
            StageState::Weak,
            "a same-named function in the test's own package must defeat the \
             bare binding: {:#?}",
            evidence.discriminate
        );
        assert!(
            evidence
                .discriminate
                .summary
                .contains("observation_unverified"),
            "the ambiguous binding leaves observation unverified: {:#?}",
            evidence.discriminate
        );
    }

    /// G1 positive control: the same harness in the owner's OWN package
    /// stays confirmed, and an unscopable test path (single-crate
    /// relative form) keeps today's behavior.
    #[test]
    fn same_package_harness_and_unscopable_paths_stay_confirmed() {
        let index = RustIndex {
            functions: vec![
                same_named_function("crates/alpha/src/lib.rs"),
                same_named_function("crates/beta/src/lib.rs"),
            ],
            ..RustIndex::default()
        };
        let own_package =
            owner_harness_context(&index, harness_in("crates/alpha/tests/protocol.rs"));
        assert_eq!(
            own_package.discriminate.state,
            StageState::Yes,
            "owner and test in the same package keep the confirmation: {:#?}",
            own_package.discriminate
        );
        let unscopable = owner_harness_context(&index, harness_in("tests/protocol.rs"));
        assert_eq!(
            unscopable.discriminate.state,
            StageState::Yes,
            "a test path without a package scope cannot establish the \
             cross-package ambiguity: {:#?}",
            unscopable.discriminate
        );
    }

    #[test]
    fn evidence_summaries_drop_empty_and_deduplicate_in_sorted_order() {
        let stages = [
            StageEvidence::new(StageState::Yes, Confidence::High, "z evidence"),
            StageEvidence::new(StageState::No, Confidence::Low, ""),
            StageEvidence::new(StageState::Weak, Confidence::Medium, "a evidence"),
            StageEvidence::new(StageState::Yes, Confidence::High, "z evidence"),
        ];

        assert_eq!(
            evidence_summaries(stages.iter()),
            vec!["a evidence".to_string(), "z evidence".to_string()]
        );
    }

    #[test]
    fn gather_retains_digest_valid_propagation_witness_for_internal_consumer() -> Result<(), String>
    {
        let probe = Probe {
            id: ProbeId("probe:fixture:1".to_string()),
            location: SourceLocation::new("src/lib.rs", 10, 2),
            owner: Some(SymbolId("owner:calculate".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("amount".to_string()),
            after: Some("amount + 1".to_string()),
            expression: "amount".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };
        let owner = FunctionSummary {
            id: SymbolId("owner:calculate".to_string()),
            name: "calculate".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 20,
            body: "fn calculate(amount: i32) -> Result<i32, Error> { Ok(amount) }".to_string(),
            calls: Vec::new(),
            returns: vec![ReturnFact {
                line: 14,
                text: "Ok(amount)".to_string(),
            }],
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let index = RustIndex::default();
        let context = ProbeContext::new(&probe, Some(&owner), Vec::new(), false, &index, true);
        let evidence = ClassifiedProbeEvidence::gather(&context, "amount");
        let Some(diagnostic) = evidence.propagation_witness() else {
            return Err("gather discarded the producer witness".to_string());
        };
        if diagnostic.is_invalid() || !diagnostic.witness().digest_matches() {
            return Err("gather retained a stale witness digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn corrupt_digest_is_retained_as_rejected_diagnostic_outcome() -> Result<(), String> {
        let probe = Probe {
            id: ProbeId("probe:fixture:corrupt".to_string()),
            location: SourceLocation::new("src/lib.rs", 10, 2),
            owner: Some(SymbolId("owner:calculate".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("amount".to_string()),
            after: Some("amount + 1".to_string()),
            expression: "amount".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };
        let owner = FunctionSummary {
            id: SymbolId("owner:calculate".to_string()),
            name: "calculate".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 20,
            body: "fn calculate(amount: i32) -> Result<i32, Error> { Ok(amount) }".to_string(),
            calls: Vec::new(),
            returns: vec![ReturnFact {
                line: 14,
                text: "Ok(amount)".to_string(),
            }],
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let index = RustIndex::default();
        let context = ProbeContext::new(&probe, Some(&owner), Vec::new(), false, &index, true);
        let mut evidence = ClassifiedProbeEvidence::gather(&context, "amount");
        let Some(PropagationWitnessDiagnostic::Valid(witness)) =
            evidence.propagation_witness.as_mut()
        else {
            return Err("expected a valid witness before corruption".to_string());
        };
        witness.semantic_digest = "sha256:corrupt".to_string();
        let finding = build_finding(
            &context,
            crate::domain::ExposureClass::PropagationUnknown,
            evidence,
        );
        if !finding
            .evidence
            .iter()
            .any(|line| line == "propagation witness digest invalid; diagnostic witness withheld")
        {
            return Err("corrupt witness diagnostic was not emitted".to_string());
        }
        Ok(())
    }
}
