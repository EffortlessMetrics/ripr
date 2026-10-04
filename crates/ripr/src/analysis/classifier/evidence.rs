use crate::analysis::classify::{
    OwnerPinSyntax, OwnerReturnPin, ProbeContext, PropagationWitnessV1, ReturnOracleAdmission,
    activation_evidence_with_value_facts, classify, confidence_score, contains_as_whole_word,
    current_path_witness, has_same_test_boundary_oracle_pairing, infection_evidence,
    local_flow_sinks, owner_may_be_reached_unseen, package_prefix,
    propagation_evidence_with_witness, reach_evidence, reveal_evidence_with_expression,
    same_test_pairing_missing_summary,
};
use crate::analysis::facts::{FunctionSummary, OracleFact, TestSummary};
use crate::domain::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod tuple_match;

pub(in crate::analysis) struct ClassifiedProbeEvidence {
    pub(in crate::analysis) ripr: RiprEvidence,
    pub(in crate::analysis) evidence: Vec<String>,
    pub(in crate::analysis) flow_sinks: Vec<FlowSinkFact>,
    /// Retained diagnostic witness outcome for the PR-A migration slice.  It
    /// is not projected into `Finding` or used to strengthen a stage yet.
    pub(in crate::analysis) propagation_witness: Option<PropagationWitnessDiagnostic>,
    pub(in crate::analysis) activation: ActivationEvidence,
    pub(in crate::analysis) related_tests: Vec<RelatedTest>,
    pub(in crate::analysis) related_tests_matched_total: usize,
    pub(in crate::analysis) reach: StageEvidence,
    pub(in crate::analysis) infect: StageEvidence,
    pub(in crate::analysis) propagate: StageEvidence,
    pub(in crate::analysis) observe: StageEvidence,
    pub(in crate::analysis) discriminate: StageEvidence,
    /// Reach is `No` for a resolved owner that nothing in the workspace
    /// names (see `owner_may_be_reached_unseen`), so no test can run the
    /// change. An owner with an unresolved caller chain keeps its
    /// shape-based class even when no related test was found.
    pub(in crate::analysis) reach_ruled_out: bool,
}

impl ClassifiedProbeEvidence {
    pub(in crate::analysis) fn gather(context: &ProbeContext<'_>, reveal_expression: &str) -> Self {
        let test_summaries = context.related_test_summaries();
        let reach = reach_evidence(&context.related_tests, context.owner_fn, || {
            context
                .owner_fn
                .is_some_and(|owner| owner_may_be_reached_unseen(owner, context.index))
        });
        let flow_sinks = local_flow_sinks(context.probe, context.owner_fn);
        let propagation_witness = current_path_witness(context.probe, &flow_sinks)
            .map(PropagationWitnessDiagnostic::from_witness);
        let activation = activation_evidence_with_value_facts(
            context.probe,
            context.owner_fn,
            &test_summaries,
            &flow_sinks,
            context.helper_chain.as_ref(),
            context.index,
            context.workspace_complete,
            context.test_value_facts,
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
        // Both defeats below depend only on the test's file (and the probe's
        // constant owner callee), never on the individual test. A
        // high-traffic owner relates to thousands of tests spread over a
        // few files; without memoization every test re-masked and
        // re-scanned its whole file source (profiled: ~80% of a 60 s
        // `ripr check` on a 505-line diff of this repository). The package
        // defeat is memoized per probe because it also depends on the
        // owner's package; the import scan does not, so it uses the
        // run-scoped per-file memo on the context.
        // #4478: the owner-side half of the owner-return pin, established
        // once per probe; `None` keeps every assertion on the token rule.
        let fallback_pin_syntax = OwnerPinSyntax::default();
        let pin_syntax = context.owner_pin_syntax.unwrap_or(&fallback_pin_syntax);
        // Every diff-classifier consumer of a covered equality uses this same
        // decision. Keep the original TestSummary and assertion cardinality;
        // filtering a clone would manufacture singleton matching fallbacks.
        let assertion_admitted = |test: &TestSummary, assertion: &OracleFact| {
            pin_syntax.admits_equality_assertion(context.probe, test, assertion, context.index)
        };
        let owner_return_pin = context
            .owner_fn
            .and_then(|owner| OwnerReturnPin::establish(context.probe, owner, context.index));
        let package_defeats_by_file = FileDefeatMemo::default();
        let owner_locals = context
            .owner_fn
            .map(owner_local_binding_names)
            .unwrap_or_default();
        let (observe, discriminate, related_tests, matched_total) = reveal_evidence_with_expression(
            context.probe,
            reveal_expression,
            &context.related_tests,
            &owner_locals,
            // #3731 review (F11): the related test's file source is
            // reachable here, so the caller computes the same-name-import
            // defeat per test instead of restructuring the reveal inputs.
            &|test, callee| {
                context.index.files().get(&test.file).is_some_and(|facts| {
                    context.test_file_imports_foreign_callee_name(&test.file, &facts.source, callee)
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
                memoized_file_defeat(&package_defeats_by_file, &test.file, callee, || {
                    let Some(test_package) = package_prefix(&test.file) else {
                        return false;
                    };
                    let Some(owner_package) = owner_package.as_deref() else {
                        return false;
                    };
                    if test_package == owner_package {
                        return false;
                    }
                    context.index.functions().iter().any(|function| {
                        function.name == callee
                            && package_prefix(&function.file).as_deref()
                                == Some(test_package.as_str())
                    })
                })
            },
            &ReturnOracleAdmission {
                owner_return_pin: &|test, assertion| {
                    owner_return_pin.as_ref().is_some_and(|pin| {
                        pin.admits(
                            test,
                            assertion,
                            context.index,
                            &|file, name| {
                                context.index.files().get(file).is_some_and(|facts| {
                                    context.test_file_imports_foreign_callee_name(
                                        file,
                                        &facts.source,
                                        name,
                                    )
                                })
                            },
                            pin_syntax,
                        )
                    })
                },
                assertion_admitted: &assertion_admitted,
            },
        );

        let discriminate =
            tuple_match::discrimination(context, &observe, &discriminate).unwrap_or(discriminate);
        // #4828: a boundary-class probe may not read `exposed` by taking a
        // boundary input from one test and a discriminating oracle from
        // another. Infection and discrimination stay independently scored;
        // only the combined `exposed` path requires same-test pairing on
        // the owner call that sits on the boundary. Pairing reuses
        // activation's `==` facts so named constants and helper hops that
        // already infected stay paired when the same test holds the oracle.
        let discriminate = if matches!(context.probe.family, ProbeFamily::Predicate)
            && infect.state == StageState::Yes
            && discriminate.state == StageState::Yes
            && !has_same_test_boundary_oracle_pairing(
                context.probe,
                context.owner_fn,
                &test_summaries,
                &activation,
                &assertion_admitted,
            ) {
            StageEvidence::new(
                StageState::Weak,
                Confidence::Medium,
                same_test_pairing_missing_summary(),
            )
        } else {
            discriminate
        };
        // The missing-field fact is the authority on whether an assertion
        // observes the constructed field. A token that merely coincides with
        // the field value (`Box` in `downcast_ref::<Box<dyn E>>()`) must not
        // make the finding `exposed` while that fact says nothing observes it.
        let discriminate = if matches!(context.probe.family, ProbeFamily::FieldConstruction)
            && discriminate.state == StageState::Yes
            && activation.missing_discriminators.iter().any(|fact| {
                fact.flow_sink
                    .as_ref()
                    .is_some_and(|sink| sink.kind == FlowSinkKind::StructField)
            }) {
            StageEvidence::new(
                StageState::Weak,
                Confidence::Medium,
                "Discriminator unconfirmed: no field-value assertion observes the constructed field",
            )
        } else {
            discriminate
        };
        // Tests kept only as suggested locations never run the owner, so
        // they neither activate the change nor observe it.
        let reach_ruled_out = reach.state == StageState::No
            && context
                .owner_fn
                .is_some_and(|owner| !owner_may_be_reached_unseen(owner, context.index));
        let unreached = |stage: StageEvidence, verb: &str| {
            if reach_ruled_out && stage.state == StageState::Yes {
                unreached_stage(verb)
            } else {
                stage
            }
        };
        let infect = unreached(infect, "activate");
        let observe = unreached(observe, "observe");
        let discriminate = unreached(discriminate, "discriminate");

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
            related_tests_matched_total: matched_total,
            reach,
            infect,
            propagate,
            observe,
            discriminate,
            reach_ruled_out,
        }
    }

    pub(in crate::analysis) fn classify(&self, probe: &Probe) -> ExposureClass {
        // A changed line inside a function no test reaches has no static path
        // whatever its shape: "cannot classify, escalate" would send the
        // reader after mutation testing when the plain gap is a missing test.
        if self.reach_ruled_out {
            return ExposureClass::NoStaticPath;
        }
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

fn unreached_stage(stage: &str) -> StageEvidence {
    StageEvidence::new(
        StageState::No,
        Confidence::Medium,
        format!("No test reaches the changed owner, so no test can {stage} this change"),
    )
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

/// Names the owner binds with `let` that are not also named in its
/// signature. Such a binding exists only inside the owner, so a test's
/// same-named local can never be it. A `let` that rebinds a parameter
/// (`let cache = cache;`) keeps the parameter's name out of this list.
/// Parser-backed facts only; a lexically indexed owner yields none.
fn owner_local_binding_names(owner: &FunctionSummary) -> Vec<String> {
    let signature = owner
        .body
        .split_once('{')
        .map_or(owner.body.as_str(), |(head, _)| head);
    let mut names = owner
        .let_bindings
        .iter()
        .map(|binding| binding.name.clone())
        .filter(|name| !contains_as_whole_word(signature, name))
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

/// Per-file defeat results for one probe, keyed by test file then callee.
type FileDefeatMemo = RefCell<BTreeMap<PathBuf, BTreeMap<String, bool>>>;

/// Returns the cached defeat for `(file, callee)`, computing it once.
fn memoized_file_defeat(
    memo: &FileDefeatMemo,
    file: &Path,
    callee: &str,
    compute: impl FnOnce() -> bool,
) -> bool {
    if let Some(cached) = memo
        .borrow()
        .get(file)
        .and_then(|by_callee| by_callee.get(callee))
    {
        return *cached;
    }
    let defeats = compute();
    memo.borrow_mut()
        .entry(file.to_path_buf())
        .or_default()
        .insert(callee.to_string(), defeats);
    defeats
}

#[cfg(test)]
mod tests {
    use super::evidence_summaries;
    use super::owner_local_binding_names;
    use super::{ClassifiedProbeEvidence, ProbeContext, PropagationWitnessDiagnostic};
    use crate::analysis::classifier::finding::build_finding;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::LetBindingFact;
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
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            item: Default::default(),
            impl_context: Default::default(),
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
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            item: Default::default(),
            impl_context: Default::default(),
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

    /// The memo computes a defeat once per (file, callee) and keeps
    /// different files and callees apart, so a cached answer never leaks
    /// from one test file to another.
    #[test]
    fn file_defeat_memo_computes_once_per_file_and_callee() {
        use std::cell::Cell;
        use std::path::Path;

        let memo = super::FileDefeatMemo::default();
        let calls = Cell::new(0);
        let lookup = |file: &str, callee: &str, answer: bool| {
            super::memoized_file_defeat(&memo, Path::new(file), callee, || {
                calls.set(calls.get() + 1);
                answer
            })
        };

        assert!(lookup("tests/a.rs", "score", true));
        // Cached: the second compute would answer false but never runs.
        assert!(lookup("tests/a.rs", "score", false));
        assert!(!lookup("tests/b.rs", "score", false));
        assert!(!lookup("tests/a.rs", "total", false));
        assert_eq!(calls.get(), 3);
    }

    /// G1 falsifier: two packages each define `expect_response`; the test
    /// lives in package `beta` and calls bare `expect_response` while the
    /// probe's owner is package `alpha`'s — the bare binding is ambiguous
    /// across packages, so the observation stays unverified. Pre-fix this
    /// confirmed through the bare name.
    #[test]
    fn cross_package_same_name_function_defeats_owner_confirmation() {
        let index = RustIndex::from_owned(crate::analysis::facts::OwnedRustIndex {
            functions: vec![
                same_named_function("crates/alpha/src/lib.rs"),
                same_named_function("crates/beta/src/lib.rs"),
            ],
            ..Default::default()
        });
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
        let index = RustIndex::from_owned(crate::analysis::facts::OwnedRustIndex {
            functions: vec![
                same_named_function("crates/alpha/src/lib.rs"),
                same_named_function("crates/beta/src/lib.rs"),
            ],
            ..Default::default()
        });
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
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            item: Default::default(),
            impl_context: Default::default(),
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
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            item: Default::default(),
            impl_context: Default::default(),
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

    #[test]
    fn owner_local_bindings_exclude_names_the_signature_binds() {
        let binding = |name: &str| LetBindingFact {
            line: 1,
            name: name.to_string(),
        };
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::from_rows".to_string()),
            name: "from_rows".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 6,
            body: "fn from_rows(cache: &mut Cache, rows: Vec<u32>) -> Result<Table, String> {\n    let cache = cache;\n    let table = Table { rows };\n    table.validate()?;\n    Ok(table)\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: vec![binding("cache"), binding("table"), binding("table")],
            item: Default::default(),
            impl_context: Default::default(),
        };
        assert_eq!(owner_local_binding_names(&owner), vec!["table".to_string()]);
    }
}
