//! Test-grip evidence per RIPR-SPEC-0005, v1.
//!
//! For each `RepoSeam`, build per-stage evidence (reach / activate /
//! propagate / observe / discriminate) using the existing `RustIndex`
//! facts. This is **not** classification: the output is a per-stage
//! evidence record, not a `SeamGripClass`. The classification PR
//! (`analysis/repo-ripr-classification-v1`) consumes these records.
//!
//! Determinism: `evidence_for_seams` sorts by `seam_id`. Within each
//! evidence record, `related_tests` are deduped and ranked by relation
//! confidence, relation reason, oracle strength, activation overlap,
//! then stable file/name/line tie-breakers.

mod related_tests;

pub(crate) use related_tests::CompactGripContext;
use related_tests::{
    CompactTest, assertion_target_tokens, call_text_contains_named_call, find_owner_function,
    find_related_tests_compact, find_related_tests_with_context, required_discriminator_text,
    sort_related_tests_for_seam, strip_comments_and_strings,
    test_assertion_mentions_any_target_token,
};

use super::facts::CallFact;
use super::rust_index::{
    self, FunctionSummary, OracleFact, RustIndex, TestSummary, extract_call_facts,
    extract_identifier_tokens,
};
use super::seams::{ExpectedSink, RepoSeam, SeamId, SeamKind};
use crate::analysis::cancellation;
use crate::domain::{
    Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence, StageState,
    SymbolId, ValueContext, ValueFact,
};
// Re-export so callers that import from this module continue to compile.
pub(crate) use crate::domain::{RelationConfidence, RelationReason};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Per-seam test-grip evidence record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TestGripEvidence {
    pub(crate) seam_id: SeamId,
    pub(crate) related_tests: Vec<RelatedTestGrip>,
    pub(crate) reach: StageEvidence,
    pub(crate) activate: StageEvidence,
    pub(crate) propagate: StageEvidence,
    pub(crate) observe: StageEvidence,
    pub(crate) discriminate: StageEvidence,
    pub(crate) observed_values: Vec<ValueFact>,
    pub(crate) missing_discriminators: Vec<MissingDiscriminatorFact>,
}

const COMPACT_RELATED_TEST_LIMIT: usize = 12;
const LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";
const EVIDENCE_PROGRESS_CHUNK: usize = 500;
const HELPER_OWNER_CALL_GRAPH_MAX_HOPS: usize = 3;

/// Per-related-test grip facts attached to a `TestGripEvidence`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RelatedTestGrip {
    pub(crate) test_name: String,
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) test_target: Option<TestTargetEvidence>,
    pub(crate) oracle_kind: OracleKind,
    pub(crate) oracle_strength: OracleStrength,
    pub(crate) evidence_summary: String,
    pub(crate) relation_reason: RelationReason,
    pub(crate) relation_confidence: RelationConfidence,
}

/// Producer-owned identity for an existing Rust test target.
///
/// This is populated from the Rust index's test function facts. Renderers may
/// display it, but may not reconstruct it from a path, name, or line tuple.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestTargetEvidence {
    symbol_id: SymbolId,
    file: PathBuf,
    line: usize,
    test_kind: TestKind,
    relation: RelationReason,
    provenance: TestTargetProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestKind {
    InlineUnit,
    Integration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestTargetProvenance {
    RustIndexFunction,
    #[cfg(test)]
    FixtureOnly,
}

impl TestTargetEvidence {
    pub(crate) fn from_index(
        symbol_id: SymbolId,
        file: PathBuf,
        line: usize,
        test_kind: TestKind,
        relation: RelationReason,
    ) -> Self {
        Self {
            symbol_id,
            file,
            line,
            test_kind,
            relation,
            provenance: TestTargetProvenance::RustIndexFunction,
        }
    }

    pub(crate) fn symbol_id(&self) -> &SymbolId {
        &self.symbol_id
    }
}

#[cfg(test)]
impl TestTargetEvidence {
    pub(crate) fn fixture(name: &str, file: &std::path::Path, line: usize) -> Self {
        Self {
            symbol_id: SymbolId(format!("fixture:test:{}:{}:{}", file.display(), line, name)),
            file: file.to_path_buf(),
            line,
            test_kind: if file
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/tests/")
                || file
                    .to_string_lossy()
                    .replace('\\', "/")
                    .starts_with("tests/")
            {
                TestKind::Integration
            } else {
                TestKind::InlineUnit
            },
            relation: RelationReason::DirectOwnerCall,
            provenance: TestTargetProvenance::FixtureOnly,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OracleSemantics {
    pub(crate) observes: String,
    pub(crate) missing: String,
    pub(crate) upgrade_suggestion: Option<String>,
}

/// Build evidence records for a slice of seams. Output is sorted by
/// `seam_id` so two runs over the same input produce identical bytes.
///
/// A cancellation checkpoint can stop this non-fallible helper early, so a
/// caller that runs under a cancellation context must checkpoint immediately
/// after this function returns before using or publishing the vector.
pub(crate) fn evidence_for_seams(seams: &[RepoSeam], index: &RustIndex) -> Vec<TestGripEvidence> {
    let context_started = Instant::now();
    trace_latency_phase(
        "evidence_context",
        &format!("start_seams_{}", seams.len()),
        Duration::ZERO,
    );
    let context = CompactGripContext::new(index);
    trace_latency_phase(
        "evidence_context",
        &format!("tests_{}_seams_{}", context.tests.len(), seams.len()),
        context_started.elapsed(),
    );

    let evidence_started = Instant::now();
    let mut out: Vec<TestGripEvidence> = Vec::with_capacity(seams.len());
    for (index, seam) in seams.iter().enumerate() {
        if cancellation::checkpoint().is_err() {
            break;
        }
        out.push(evidence_for_seam_with_context(seam, &context));
        let processed = index + 1;
        if processed % EVIDENCE_PROGRESS_CHUNK == 0 || processed == seams.len() {
            trace_latency_phase(
                "evidence_for_seams_progress",
                &format!("processed_{processed}_of_{}", seams.len()),
                evidence_started.elapsed(),
            );
        }
    }
    out.sort_by(|a, b| a.seam_id.as_str().cmp(b.seam_id.as_str()));
    out
}

/// Build evidence for a single seam.
#[cfg(test)]
pub(crate) fn evidence_for_seam(seam: &RepoSeam, index: &RustIndex) -> TestGripEvidence {
    let context = CompactGripContext::new(index);
    evidence_for_seam_with_context(seam, &context)
}

fn evidence_for_seam_with_context(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
) -> TestGripEvidence {
    let mut related_with_reason = find_related_tests_with_context(seam, context);
    sort_related_tests_for_seam(seam, context, &mut related_with_reason);
    let related_indexed: Vec<&CompactTest<'_>> = related_with_reason
        .iter()
        .map(|(indexed, _reason)| *indexed)
        .collect();
    let owner_fn = find_owner_function(seam, context.index);

    let related: Vec<&TestSummary> = related_indexed.iter().map(|indexed| indexed.test).collect();

    let reach = reach_evidence(seam, &related);
    let (activate, observed_values, missing_discriminators) =
        activate_evidence(seam, &related_indexed, context.index, owner_fn);
    let propagate = propagate_evidence(seam, &related);
    let observe = observe_evidence(&related);
    let discriminate = discriminate_evidence(seam, &related);

    let related_tests: Vec<RelatedTestGrip> = related_with_reason
        .iter()
        .map(|(indexed, reason)| related_test_grip(seam, indexed.test, *reason, context.index))
        .collect();

    TestGripEvidence {
        seam_id: seam.id().clone(),
        related_tests,
        reach,
        activate,
        propagate,
        observe,
        discriminate,
        observed_values,
        missing_discriminators,
    }
}

fn trace_latency_phase(phase: &str, status: &str, duration: Duration) {
    if std::env::var_os(LATENCY_TRACE_ENV).is_some() {
        eprintln!("{}", latency_trace_line(phase, status, duration));
    }
}

fn latency_trace_line(phase: &str, status: &str, duration: Duration) -> String {
    format!(
        "ripr_repo_exposure_latency phase={phase} status={status} duration_ms={}",
        duration.as_millis()
    )
}

/// Build compact evidence for a single seam. The returned
/// `TestGripEvidence` preserves the stage states used by classification,
/// but intentionally omits related-test detail and observed-value
/// payloads because repo badges only need per-class counts.
pub(crate) fn compact_evidence_for_seam(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
) -> TestGripEvidence {
    let related_indexed = find_related_tests_compact(seam, context);
    let related: Vec<&TestSummary> = related_indexed.iter().map(|indexed| indexed.test).collect();
    let owner_fn = find_owner_function(seam, context.index);

    let reach = reach_evidence(seam, &related);
    let (activate, missing_discriminators) =
        compact_activate_evidence(seam, &related_indexed, context.index, owner_fn);
    let propagate = propagate_evidence(seam, &related);
    let observe = observe_evidence(&related);
    let discriminate = discriminate_evidence(seam, &related);

    TestGripEvidence {
        seam_id: seam.id().clone(),
        related_tests: Vec::new(),
        reach,
        activate,
        propagate,
        observe,
        discriminate,
        observed_values: Vec::new(),
        missing_discriminators,
    }
}

fn reach_evidence(seam: &RepoSeam, related: &[&TestSummary]) -> StageEvidence {
    if related.is_empty() {
        return StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            format!(
                "No static test path found for seam owner `{}`",
                seam.owner()
            ),
        );
    }
    let names: Vec<&str> = related.iter().take(3).map(|t| t.name.as_str()).collect();
    StageEvidence::new(
        StageState::Yes,
        Confidence::Medium,
        format!(
            "Related tests appear to reach `{}`: {}",
            seam.owner(),
            names.join(", ")
        ),
    )
}

/// Activation evidence.
///
/// Returns `(stage, observed_values, missing_discriminators)`. The
/// observed values come from the seam's owner-call argument lists
/// across all related tests. The missing-discriminator set is the
/// per-kind required value or shape minus what we observed.
fn activate_evidence(
    seam: &RepoSeam,
    related: &[&CompactTest<'_>],
    index: &RustIndex,
    owner_fn: Option<&FunctionSummary>,
) -> (StageEvidence, Vec<ValueFact>, Vec<MissingDiscriminatorFact>) {
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let mut observed: Vec<ValueFact> = Vec::new();
    let observed_argument_selection =
        (!owner_name.is_empty()).then(|| observed_argument_selection(seam, index, owner_name));

    if !owner_name.is_empty() {
        for indexed in related {
            observed.extend(observed_value_facts_for_test(
                seam, indexed, index, owner_name,
            ));
        }
    }
    sort_value_facts(&mut observed);

    let field_assignment_value_unresolved = observed.is_empty()
        && !owner_name.is_empty()
        && related.iter().any(|indexed| {
            field_assignment_value_unresolved_for_test(seam, indexed, index, owner_name)
        });

    let boundary_equality_observed = owner_fn.is_some_and(|owner_fn| {
        seam.kind() == SeamKind::PredicateBoundary
            && related
                .iter()
                .any(|indexed| boundary_equality_overlap_score(seam, indexed, index, owner_fn) > 0)
    });
    let boundary_activation_operands_unresolved =
        observed_argument_selection
            .as_ref()
            .is_some_and(|selection| {
                matches!(
                    selection,
                    ObservedArgumentSelection::UnresolvedBoundaryOperands
                ) || (selection.requires_projection()
                    && observed.is_empty()
                    && !boundary_equality_observed)
            });
    let missing = missing_discriminators_for(
        seam,
        &observed,
        boundary_activation_operands_unresolved,
        boundary_equality_observed,
    );
    let direct_value_insensitive_owner_call = !owner_name.is_empty()
        && !requires_concrete_activation_values(seam)
        && related
            .iter()
            .any(|indexed| has_direct_owner_call(indexed, owner_name));
    let target_affinity_tokens =
        (!requires_concrete_activation_values(seam)).then(|| assertion_target_tokens(seam));
    let helper_value_insensitive_owner_call = !owner_name.is_empty()
        && !requires_concrete_activation_values(seam)
        && related.iter().any(|indexed| {
            has_owner_call_via_one_hop_helper(indexed, owner_name)
                || has_owner_call_via_target_affinity(
                    indexed,
                    owner_name,
                    target_affinity_tokens.as_ref(),
                )
        });
    let ambiguous_constructor_field_owner = !owner_name.is_empty()
        && related
            .iter()
            .any(|indexed| has_ambiguous_constructor_field_owner(indexed, seam, owner_name));

    let state = if related.is_empty() {
        StageState::No
    } else if ambiguous_constructor_field_owner {
        StageState::Unknown
    } else if !observed.is_empty()
        || direct_value_insensitive_owner_call
        || helper_value_insensitive_owner_call
    {
        StageState::Yes
    } else {
        // Reach exists but no concrete value seen — most often a helper
        // call that hides the activation, or an integration test.
        StageState::Unknown
    };
    let stage = StageEvidence::new(
        state,
        if !observed.is_empty()
            || direct_value_insensitive_owner_call
            || helper_value_insensitive_owner_call
        {
            Confidence::Medium
        } else {
            Confidence::Low
        },
        if ambiguous_constructor_field_owner {
            format!(
                "constructor_field_owner_ambiguous: exact field observer found, but same-crate caller linkage to owner `{owner_name}` is ambiguous"
            )
        } else if !observed.is_empty() {
            format!(
                "Observed {} concrete activation value(s) for seam `{}`",
                observed.len(),
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else if direct_value_insensitive_owner_call {
            format!(
                "Observed direct owner call for value-insensitive seam `{}`",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else if helper_value_insensitive_owner_call {
            format!(
                "Observed helper owner call for value-insensitive seam `{}`",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else if field_assignment_value_unresolved {
            format!(
                "Field assignment value is unresolved for seam `{}`; direct field writes only support unconditional source-ordered assignments of literals and same-file literal constants with bounded +/- integer offsets and no intervening mutable borrow",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else if boundary_activation_operands_unresolved && !related.is_empty() {
            boundary_activation_operands_unresolved_summary(seam, index, owner_name)
        } else if requires_concrete_activation_values(seam) {
            format!(
                "No concrete activation values observed for seam `{}`",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else {
            format!(
                "No direct owner call observed for value-insensitive seam `{}`",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        },
    );
    (stage, observed, missing)
}

fn requires_concrete_activation_values(seam: &RepoSeam) -> bool {
    seam.kind() == SeamKind::PredicateBoundary && comparison_operands(seam.expression()).is_some()
}

enum ObservedArgumentSelection {
    AllArguments,
    ArgumentOperands(Vec<ObservedArgumentOperand>),
    UnresolvedBoundaryOperands,
}

impl ObservedArgumentSelection {
    fn requires_projection(&self) -> bool {
        matches!(
            self,
            ObservedArgumentSelection::ArgumentOperands(operands)
                if operands
                    .iter()
                    .any(|operand| operand.projection.is_some())
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ObservedArgumentOperand {
    index: usize,
    projection: Option<String>,
}

fn observed_value_facts_for_test(
    seam: &RepoSeam,
    indexed: &CompactTest<'_>,
    index: &RustIndex,
    owner_name: &str,
) -> Vec<ValueFact> {
    let mut observed: Vec<ValueFact> = Vec::new();
    let observed_argument_selection = observed_argument_selection(seam, index, owner_name);
    if matches!(
        observed_argument_selection,
        ObservedArgumentSelection::UnresolvedBoundaryOperands
    ) {
        return observed;
    }
    // Per-test resolution facts (let bindings, rstest cases, table
    // rows, same-file consts) are built lazily and then reused across
    // all owner calls in this test. Per `analysis/value-extraction-v2`.
    let value_facts = indexed
        .value_facts
        .get_or_init(|| super::value_resolution::ValueEnvFacts::build(indexed.test, index));
    let env = super::value_resolution::ValueEnv::new(seam, value_facts);
    for call in &indexed.test.calls {
        if call.name != owner_name {
            continue;
        }
        let Some(args) = call_arguments(&call.text, owner_name) else {
            continue;
        };
        for (arg_index, arg) in args.into_iter().enumerate() {
            let argument_operand = match &observed_argument_selection {
                ObservedArgumentSelection::ArgumentOperands(operands) => operands
                    .iter()
                    .find(|operand| operand.index == arg_index)
                    .cloned(),
                ObservedArgumentSelection::AllArguments => Some(ObservedArgumentOperand {
                    index: arg_index,
                    projection: None,
                }),
                ObservedArgumentSelection::UnresolvedBoundaryOperands => continue,
            };
            let Some(argument_operand) = argument_operand else {
                continue;
            };
            let arg = projected_argument_expression(&arg, argument_operand.projection.as_deref());
            let mut emitted = false;
            // Direct literal first (matches pre-v2 behavior).
            for value in scalar_values(&arg) {
                observed.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context: ValueContext::FunctionArgument,
                });
                emitted = true;
            }
            if emitted {
                continue;
            }
            // value-extraction-v2: try to resolve the arg through the
            // priority chain (let / rstest case / table row /
            // same-file const / Some/Ok/Err).
            for (value, context) in env.resolve_at_call(&arg, call.line, &call.name, &call.text) {
                observed.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context,
                });
            }
        }
    }
    // Builder-method values (e.g.,
    // `Quote::new().amount(100).threshold(100)`) - collected
    // separately because they don't fit the per-arg shape. These only
    // count when method names align with seam tokens; the env enforces
    // that filter.
    observed.extend(env.builder_facts());
    observed
}

fn field_assignment_value_unresolved_for_test(
    seam: &RepoSeam,
    indexed: &CompactTest<'_>,
    index: &RustIndex,
    owner_name: &str,
) -> bool {
    let selection = observed_argument_selection(seam, index, owner_name);
    let ObservedArgumentSelection::ArgumentOperands(operands) = selection else {
        return false;
    };
    let value_facts = indexed
        .value_facts
        .get_or_init(|| super::value_resolution::ValueEnvFacts::build(indexed.test, index));
    let env = super::value_resolution::ValueEnv::new(seam, value_facts);
    indexed.test.calls.iter().any(|call| {
        if call.name != owner_name {
            return false;
        }
        let Some(args) = call_arguments(&call.text, owner_name) else {
            return false;
        };
        operands.iter().any(|operand| {
            let Some(arg) = args.get(operand.index) else {
                return false;
            };
            let arg = projected_argument_expression(arg, operand.projection.as_deref());
            env.field_assignment_value_unresolved_at_call(&arg, call.line, &call.name, &call.text)
        })
    })
}

fn has_direct_owner_call(indexed: &CompactTest<'_>, owner_name: &str) -> bool {
    indexed.test.calls.iter().any(|call| {
        call.name == owner_name && call_text_contains_named_call(&call.text, owner_name)
    })
}

fn has_owner_call_via_one_hop_helper(indexed: &CompactTest<'_>, owner_name: &str) -> bool {
    indexed.helper_owner_call_names.contains(owner_name)
}

fn has_owner_call_via_target_affinity(
    indexed: &CompactTest<'_>,
    owner_name: &str,
    target_tokens: Option<&BTreeSet<String>>,
) -> bool {
    indexed
        .target_affinity_owner_call_names
        .contains(owner_name)
        && target_tokens
            .is_some_and(|tokens| test_assertion_mentions_any_target_token(indexed, tokens))
}

fn has_ambiguous_constructor_field_owner(
    indexed: &CompactTest<'_>,
    seam: &RepoSeam,
    owner_name: &str,
) -> bool {
    seam.kind() == SeamKind::FieldConstruction
        && indexed
            .ambiguous_target_affinity_owner_call_names
            .contains(owner_name)
        && indexed
            .test
            .assertions
            .iter()
            .any(|oracle| field_construction_oracle_matches_seam_field(seam, &oracle.text))
}

fn observed_argument_selection(
    seam: &RepoSeam,
    index: &RustIndex,
    owner_name: &str,
) -> ObservedArgumentSelection {
    if seam.kind() != SeamKind::PredicateBoundary {
        return ObservedArgumentSelection::AllArguments;
    }
    let Some(owner_fn) = find_owner_function(seam, index) else {
        return ObservedArgumentSelection::AllArguments;
    };
    if owner_fn.name != owner_name {
        return ObservedArgumentSelection::AllArguments;
    }
    let Some((left, right)) = comparison_operands(seam.expression()) else {
        return ObservedArgumentSelection::AllArguments;
    };
    let parameters = function_parameters(owner_fn);
    if let Some(left_operand) = boundary_operand_argument(owner_fn, &parameters, &left) {
        return ObservedArgumentSelection::ArgumentOperands(vec![left_operand]);
    }
    if let Some(right_operand) = boundary_operand_argument(owner_fn, &parameters, &right)
        && !scalar_values(&left).is_empty()
    {
        return ObservedArgumentSelection::ArgumentOperands(vec![right_operand]);
    }
    if !scalar_values(&left).is_empty()
        && let Some(right_operand) = boundary_operand_argument(owner_fn, &parameters, &right)
    {
        return ObservedArgumentSelection::ArgumentOperands(vec![right_operand]);
    }
    ObservedArgumentSelection::UnresolvedBoundaryOperands
}

fn boundary_activation_operands_unresolved_summary(
    seam: &RepoSeam,
    index: &RustIndex,
    owner_name: &str,
) -> String {
    let expression = seam
        .expression()
        .lines()
        .next()
        .unwrap_or(seam.expression());
    if boundary_activation_operands_are_iterator_derived(seam, index, owner_name) {
        format!(
            "Boundary activation operand is iterator-derived for seam `{expression}`; add analyzer support for iterator boundary operand resolution before emitting an actionable repair packet"
        )
    } else if boundary_activation_operands_are_closure_derived(seam, index, owner_name) {
        format!(
            "Boundary activation operand is closure-derived for seam `{expression}`; add analyzer support for closure boundary operand resolution before emitting an actionable repair packet"
        )
    } else {
        format!(
            "Boundary activation operands are local or computed for seam `{expression}`; add analyzer support for local/computed boundary operand resolution before emitting an actionable repair packet"
        )
    }
}

fn boundary_activation_operands_are_iterator_derived(
    seam: &RepoSeam,
    index: &RustIndex,
    owner_name: &str,
) -> bool {
    if seam.kind() != SeamKind::PredicateBoundary {
        return false;
    }
    let Some(owner_fn) = find_owner_function(seam, index) else {
        return false;
    };
    if owner_fn.name != owner_name {
        return false;
    }
    let Some((left, right)) = comparison_operands(seam.expression()) else {
        return false;
    };
    boundary_operand_is_iterator_derived(owner_fn, &left)
        || boundary_operand_is_iterator_derived(owner_fn, &right)
}

fn boundary_activation_operands_are_closure_derived(
    seam: &RepoSeam,
    index: &RustIndex,
    owner_name: &str,
) -> bool {
    if seam.kind() != SeamKind::PredicateBoundary {
        return false;
    }
    let Some(owner_fn) = find_owner_function(seam, index) else {
        return false;
    };
    if owner_fn.name != owner_name {
        return false;
    }
    let Some((left, right)) = comparison_operands(seam.expression()) else {
        return false;
    };
    boundary_operand_is_closure_derived(owner_fn, &left)
        || boundary_operand_is_closure_derived(owner_fn, &right)
}

fn boundary_operand_is_iterator_derived(owner_fn: &FunctionSummary, operand: &str) -> bool {
    let operand = operand.trim();
    if !is_boundary_operand_identifier(operand) {
        return false;
    }
    owner_fn
        .body
        .lines()
        .any(|line| loop_binds_operand_from_iterator(line, operand))
}

fn boundary_operand_is_closure_derived(owner_fn: &FunctionSummary, operand: &str) -> bool {
    let operand_root = boundary_operand_root_identifier(operand);
    if operand_root.is_empty() {
        return false;
    }
    owner_fn
        .body
        .lines()
        .any(|line| closure_binds_operand_root(line, &operand_root))
}

fn boundary_operand_root_identifier(operand: &str) -> String {
    let operand = operand.trim().trim_start_matches('&').trim();
    operand
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect()
}

fn closure_binds_operand_root(line: &str, operand_root: &str) -> bool {
    let code = strip_comments_and_strings(line);
    let mut rest = code.as_str();
    while let Some(start) = rest.find('|') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('|') else {
            return false;
        };
        let params = &after_start[..end];
        if closure_params_contain_operand(params, operand_root) {
            return true;
        }
        rest = &after_start[end + 1..];
    }
    false
}

fn closure_params_contain_operand(params: &str, operand_root: &str) -> bool {
    params
        .split(',')
        .filter_map(|param| {
            param
                .trim()
                .trim_start_matches("mut ")
                .split_once(':')
                .map(|(name, _)| name.trim())
                .or_else(|| Some(param.trim().trim_start_matches("mut ").trim()))
        })
        .any(|param| param == operand_root)
}

fn loop_binds_operand_from_iterator(line: &str, operand: &str) -> bool {
    let Some(for_index) = line.find("for ") else {
        return false;
    };
    let rest = &line[for_index + "for ".len()..];
    let Some((binding, source)) = rest.split_once(" in ") else {
        return false;
    };
    boundary_loop_source_is_iterator(source)
        && loop_binding_contains_boundary_operand(binding, operand)
}

fn boundary_loop_source_is_iterator(source: &str) -> bool {
    let source = source.split('{').next().unwrap_or(source);
    source.contains(".iter()")
        || source.contains(".iter_mut()")
        || source.contains(".into_iter()")
        || source.contains(".enumerate()")
        || source.contains(".keys()")
        || source.contains(".values()")
}

fn loop_binding_contains_boundary_operand(binding: &str, operand: &str) -> bool {
    binding
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == operand)
}

fn is_boundary_operand_identifier(operand: &str) -> bool {
    let mut chars = operand.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn boundary_operand_argument(
    owner_fn: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<ObservedArgumentOperand> {
    parameters
        .iter()
        .position(|parameter| parameter == operand)
        .map(|index| ObservedArgumentOperand {
            index,
            projection: None,
        })
        .or_else(|| {
            boundary_local_operand_parameter_index(owner_fn, parameters, operand).map(|index| {
                ObservedArgumentOperand {
                    index,
                    projection: None,
                }
            })
        })
        .or_else(|| boundary_parameter_field_projection(parameters, operand))
}

fn boundary_parameter_field_projection(
    parameters: &[String],
    operand: &str,
) -> Option<ObservedArgumentOperand> {
    let operand = operand.trim().trim_start_matches('&').trim();
    for (index, parameter) in parameters.iter().enumerate() {
        let Some(rest) = operand.strip_prefix(parameter) else {
            continue;
        };
        let Some(field) = rest.strip_prefix('.') else {
            continue;
        };
        if is_boundary_operand_identifier(field) {
            return Some(ObservedArgumentOperand {
                index,
                projection: Some(field.to_string()),
            });
        }
    }
    None
}

fn projected_argument_expression(arg: &str, projection: Option<&str>) -> String {
    let arg = arg.trim();
    let Some(field) = projection else {
        return arg.to_string();
    };
    if is_boundary_operand_identifier(arg) {
        format!("{arg}.{field}")
    } else if let Some(borrowed) = arg.strip_prefix('&').map(str::trim)
        && is_boundary_operand_identifier(borrowed)
    {
        format!("&{borrowed}.{field}")
    } else {
        arg.to_string()
    }
}

fn boundary_local_operand_parameter_index(
    owner_fn: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<usize> {
    if operand.is_empty() {
        return None;
    }
    for (index, parameter) in parameters.iter().enumerate() {
        if body_contains_wrapped_local_alias(&owner_fn.body, "Some", operand, parameter)
            || body_contains_wrapped_local_alias(&owner_fn.body, "Ok", operand, parameter)
            || body_contains_direct_local_alias(&owner_fn.body, operand, parameter)
        {
            return Some(index);
        }
    }
    None
}

fn body_contains_wrapped_local_alias(
    body: &str,
    wrapper: &str,
    operand: &str,
    parameter: &str,
) -> bool {
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        let prefix = format!("if let {wrapper}({operand}) = ");
        line.strip_prefix(&prefix)
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    }) || (body_contains_match_parameter(body, parameter)
        && body_contains_wrapper_pattern(body, wrapper, operand))
}

fn body_contains_match_parameter(body: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        if is_comment_line(line) {
            return false;
        }
        line.find("match ")
            .map(|index| &line[index + "match ".len()..])
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    })
}

fn body_contains_wrapper_pattern(body: &str, wrapper: &str, operand: &str) -> bool {
    let pattern = format!("{wrapper}({operand})");
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        !is_comment_line(line) && line.contains(&pattern)
    })
}

fn code_line_before_comment(line: &str) -> &str {
    let line = line.trim();
    let line = line.split_once("//").map_or(line, |(code, _comment)| code);
    line.split_once("/*")
        .map_or(line, |(code, _comment)| code)
        .trim()
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

fn starts_with_identifier_token(text: &str, token: &str) -> bool {
    let text = text.trim_start();
    let end = text
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    end > 0 && &text[..end] == token
}

fn body_contains_direct_local_alias(body: &str, operand: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim().trim_end_matches(';').trim();
        let Some(binding) = line.strip_prefix("let ") else {
            return false;
        };
        let Some((left, right)) = binding.split_once('=') else {
            return false;
        };
        let local_name = left.split_once(':').map(|(name, _)| name).unwrap_or(left);
        local_name.trim() == operand && right.trim() == parameter
    })
}

fn activation_overlap_score(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
    indexed: &CompactTest<'_>,
) -> usize {
    let Some(owner_fn) = find_owner_function(seam, context.index) else {
        return 0;
    };
    let owner_name = owner_fn.name.as_str();
    if owner_name.is_empty() {
        return 0;
    }

    let mut score = boundary_equality_overlap_score(seam, indexed, context.index, owner_fn);
    let required_text = required_discriminator_text(seam);
    score += observed_value_facts_for_test(seam, indexed, context.index, owner_name)
        .iter()
        .filter(|fact| observed_value_matches_required_discriminator(&fact.value, required_text))
        .count();
    score
}

fn observed_value_matches_required_discriminator(value: &str, required_text: &str) -> bool {
    let value = value.trim();
    let required_text = required_text.trim();
    !value.is_empty()
        && !required_text.is_empty()
        && (value == required_text
            || value.contains(required_text)
            || required_text.contains(value))
}

fn boundary_equality_overlap_score(
    seam: &RepoSeam,
    indexed: &CompactTest<'_>,
    index: &RustIndex,
    owner_fn: &FunctionSummary,
) -> usize {
    if seam.kind() != SeamKind::PredicateBoundary {
        return 0;
    }
    let Some((left, right)) = comparison_operands(seam.expression()) else {
        return 0;
    };
    let parameters = function_parameters(owner_fn);
    let Some(left_operand) = boundary_operand_argument(owner_fn, &parameters, &left) else {
        return 0;
    };
    let Some(right_operand) = boundary_operand_argument(owner_fn, &parameters, &right) else {
        return 0;
    };

    let mut score = 0;
    for call in &indexed.test.calls {
        if call.name != owner_fn.name {
            continue;
        }
        let Some(args) = call_arguments(&call.text, &owner_fn.name) else {
            continue;
        };
        let Some(left_arg) = args.get(left_operand.index) else {
            continue;
        };
        let Some(right_arg) = args.get(right_operand.index) else {
            continue;
        };
        let left_arg = projected_argument_expression(left_arg, left_operand.projection.as_deref());
        let right_arg =
            projected_argument_expression(right_arg, right_operand.projection.as_deref());
        if arguments_overlap_at_boundary(seam, indexed, index, left_arg, right_arg, call) {
            score += 1;
        }
    }
    score
}

fn arguments_overlap_at_boundary(
    seam: &RepoSeam,
    indexed: &CompactTest<'_>,
    index: &RustIndex,
    left_arg: String,
    right_arg: String,
    call: &CallFact,
) -> bool {
    if left_arg.trim() == right_arg.trim() && !left_arg.trim().is_empty() {
        return true;
    }
    let left_values = resolved_argument_values(seam, indexed, index, &left_arg, call);
    let right_values = resolved_argument_values(seam, indexed, index, &right_arg, call);
    left_values.iter().any(|left| {
        let left = comparable_value(left);
        right_values
            .iter()
            .any(|right| left == comparable_value(right))
    })
}

fn resolved_argument_values(
    seam: &RepoSeam,
    indexed: &CompactTest<'_>,
    index: &RustIndex,
    arg: &str,
    call: &CallFact,
) -> Vec<String> {
    let values = scalar_values(arg);
    if !values.is_empty() {
        return values;
    }
    let value_facts = indexed
        .value_facts
        .get_or_init(|| super::value_resolution::ValueEnvFacts::build(indexed.test, index));
    let env = super::value_resolution::ValueEnv::new(seam, value_facts);
    env.resolve_at_call(arg, call.line, &call.name, &call.text)
        .into_iter()
        .map(|(value, _context)| value)
        .collect()
}

fn compact_activate_evidence(
    seam: &RepoSeam,
    related: &[&CompactTest<'_>],
    index: &RustIndex,
    owner_fn: Option<&FunctionSummary>,
) -> (StageEvidence, Vec<MissingDiscriminatorFact>) {
    if seam.kind() == SeamKind::PredicateBoundary {
        let (stage, _observed, missing) = activate_evidence(seam, related, index, owner_fn);
        return (stage, missing);
    }

    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let target_affinity_tokens =
        (!requires_concrete_activation_values(seam)).then(|| assertion_target_tokens(seam));
    let direct_owner_call = !owner_name.is_empty()
        && related.iter().any(|indexed| {
            indexed.call_names.contains(owner_name)
                || indexed.helper_owner_call_names.contains(owner_name)
                || has_owner_call_via_target_affinity(
                    indexed,
                    owner_name,
                    target_affinity_tokens.as_ref(),
                )
        });
    let ambiguous_constructor_field_owner = !owner_name.is_empty()
        && related
            .iter()
            .any(|indexed| has_ambiguous_constructor_field_owner(indexed, seam, owner_name));
    let state = if related.is_empty() {
        StageState::No
    } else if ambiguous_constructor_field_owner {
        StageState::Unknown
    } else if direct_owner_call {
        StageState::Yes
    } else {
        StageState::Unknown
    };
    let summary = if ambiguous_constructor_field_owner {
        format!(
            "constructor_field_owner_ambiguous: exact field observer found, but same-crate caller linkage to owner `{owner_name}` is ambiguous"
        )
    } else {
        format!(
            "Compact activation evidence for seam `{}` is `{}`",
            seam.expression()
                .lines()
                .next()
                .unwrap_or(seam.expression()),
            state.as_str()
        )
    };
    let stage = StageEvidence::new(
        state.clone(),
        if direct_owner_call && !ambiguous_constructor_field_owner {
            Confidence::Medium
        } else {
            Confidence::Low
        },
        summary,
    );
    (stage, Vec::new())
}

fn missing_discriminators_for(
    seam: &RepoSeam,
    observed: &[ValueFact],
    boundary_activation_operands_unresolved: bool,
    boundary_equality_observed: bool,
) -> Vec<MissingDiscriminatorFact> {
    match seam.kind() {
        SeamKind::PredicateBoundary => {
            if boundary_activation_operands_unresolved || boundary_equality_observed {
                return Vec::new();
            }
            // Without a value model we cannot prove the boundary value is
            // tested. Surface a hypothesis if the predicate uses a
            // strict-or-equal operator and at least one observed value is
            // strictly above or below.
            let expression = seam.expression();
            if !boundary_predicate_uses_equal_op(expression) {
                return Vec::new();
            }
            let boundary_token = boundary_rhs_token(expression);
            if boundary_token.is_empty() {
                return Vec::new();
            }
            let any_observed = !observed.is_empty();
            if !any_observed {
                return vec![MissingDiscriminatorFact {
                    value: format!("{boundary_token} (boundary value)"),
                    reason: "no observed activation values for boundary predicate".to_string(),
                    flow_sink: None,
                }];
            }
            // We do not yet know the literal value of `boundary_token`,
            // so we can only flag that the equality boundary is not
            // explicitly named in the observed value set.
            //
            // Use exact equality rather than `contains` to avoid false
            // matches like `boundary_token = "10"` matching observed
            // value `"100"`. Observed values are literal scalars produced
            // by `scalar_values`, so byte-for-byte equality is the right
            // contract here.
            let equality_seen = observed
                .iter()
                .any(|v| v.value.as_str() == boundary_token.as_str());
            if equality_seen {
                Vec::new()
            } else {
                vec![MissingDiscriminatorFact {
                    value: format!("{boundary_token} (equality boundary)"),
                    reason:
                        "observed values do not include the equality-boundary case for this predicate"
                            .to_string(),
                    flow_sink: None,
                }]
            }
        }
        SeamKind::ErrorVariant => Vec::new(),
        SeamKind::ReturnValue
        | SeamKind::FieldConstruction
        | SeamKind::SideEffect
        | SeamKind::MatchArm
        | SeamKind::CallPresence => Vec::new(),
    }
}

fn boundary_predicate_uses_equal_op(expression: &str) -> bool {
    expression.contains(" >= ")
        || expression.contains(" <= ")
        || expression.contains(" == ")
        || expression.contains(" != ")
}

/// Best-effort right-hand-side identifier for a boundary predicate.
/// Returns empty if we cannot pick one out heuristically.
fn boundary_rhs_token(expression: &str) -> String {
    for op in [" >= ", " <= ", " == ", " != ", " > ", " < "] {
        if let Some(idx) = expression.find(op) {
            let rhs = expression[idx + op.len()..].trim();
            // Take up to the first non-identifier char.
            let token: String = rhs
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !token.is_empty() {
                return token;
            }
        }
    }
    String::new()
}

fn function_parameters(function: &FunctionSummary) -> Vec<String> {
    let signature = function
        .body
        .lines()
        .next()
        .unwrap_or(function.body.as_str());
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let after_open = &signature[open + 1..];
    let Some(close) = after_open.find(')') else {
        return Vec::new();
    };
    split_top_level_commas(&after_open[..close])
        .into_iter()
        .filter_map(|argument| {
            argument
                .split_once(':')
                .map(|(name, _type)| name.trim().to_string())
        })
        .filter(|name| !name.is_empty() && name != "self" && name != "&self" && name != "mut self")
        .collect()
}

fn comparison_operands(expression: &str) -> Option<(String, String)> {
    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = expression.split_once(operator) {
            let left = clean_operand(left);
            let right = clean_operand(right);
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

fn clean_operand(operand: &str) -> String {
    let cleaned = operand
        .trim()
        .trim_start_matches("if ")
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim();
    cleaned
        .split_once('{')
        .map(|(before, _after)| before.trim())
        .unwrap_or(cleaned)
        .to_string()
}

fn comparable_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|ch| *ch != '_')
        .collect()
}

fn propagate_evidence(seam: &RepoSeam, related: &[&TestSummary]) -> StageEvidence {
    if related.is_empty() {
        return StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No related tests; cannot infer propagation",
        );
    }
    // Static heuristic: if any related test contains an oracle that
    // matches the expected sink class (e.g., return value -> assert_eq!),
    // call it Yes. Otherwise Unknown.
    let any_oracle = related.iter().any(|t| !t.assertions.is_empty());
    let any_matching_sink = related
        .iter()
        .any(|t| oracles_match_sink(&t.assertions, seam.expected_sink()));
    let state = match (any_oracle, any_matching_sink) {
        (true, true) => StageState::Yes,
        (true, false) => StageState::Unknown,
        (false, _) => StageState::Unknown,
    };
    let summary = format!(
        "Static propagation to `{}` sink is {}",
        seam.expected_sink().as_str(),
        state.as_str()
    );
    StageEvidence::new(state, Confidence::Low, summary)
}

fn oracles_match_sink(oracles: &[OracleFact], sink: ExpectedSink) -> bool {
    oracles.iter().any(|oracle| match sink {
        ExpectedSink::ReturnValue | ExpectedSink::OutputField => matches!(
            oracle.kind,
            OracleKind::ExactValue
                | OracleKind::WholeObjectEquality
                | OracleKind::Snapshot
                | OracleKind::RelationalCheck
        ),
        ExpectedSink::ErrorChannel => matches!(
            oracle.kind,
            OracleKind::ExactErrorVariant | OracleKind::BroadError
        ),
        ExpectedSink::SideEffect => matches!(oracle.kind, OracleKind::MockExpectation),
    })
}

fn observe_evidence(related: &[&TestSummary]) -> StageEvidence {
    if related.is_empty() {
        return StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No related tests; nothing observes the seam",
        );
    }
    let any_oracle = related.iter().any(|t| !t.assertions.is_empty());
    let any_smoke_only = related.iter().all(|t| {
        !t.assertions.is_empty() && t.assertions.iter().all(|o| o.kind == OracleKind::SmokeOnly)
    });
    let state = if !any_oracle {
        StageState::No
    } else if any_smoke_only {
        StageState::Weak
    } else {
        StageState::Yes
    };
    let summary = format!("Observation evidence is `{}`", state.as_str());
    StageEvidence::new(state, Confidence::Medium, summary)
}

fn discriminate_evidence(seam: &RepoSeam, related: &[&TestSummary]) -> StageEvidence {
    if related.is_empty() {
        return StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No related tests; oracle cannot discriminate",
        );
    }
    let mut best = OracleStrength::None;
    let mut best_matching = OracleStrength::None;
    for test in related {
        for oracle in &test.assertions {
            if oracle.strength.rank() > best.rank() {
                best = oracle.strength.clone();
            }
            // RIPR-SPEC-0106 (Part B): for ErrorVariant seams, oracle_kind_matches_seam
            // is necessary but not sufficient — the oracle must also structurally pin
            // the seam's specific variant. oracle_discriminates_seam checks both.
            if oracle_discriminates_seam(seam, oracle)
                && oracle.strength.rank() > best_matching.rank()
            {
                best_matching = oracle.strength.clone();
            }
        }
    }
    let state = if seam.kind() == SeamKind::FieldConstruction {
        match (&best_matching, &best) {
            (OracleStrength::Strong | OracleStrength::Medium, _) => StageState::Yes,
            (OracleStrength::Weak | OracleStrength::Smoke, _) => StageState::Weak,
            (OracleStrength::Unknown, _) => StageState::Unknown,
            (OracleStrength::None, OracleStrength::None) => StageState::No,
            (OracleStrength::None, OracleStrength::Unknown) => StageState::Unknown,
            (OracleStrength::None, _) => StageState::Weak,
        }
    } else {
        match (best_matching != OracleStrength::None, &best) {
            (_, OracleStrength::None) => StageState::No,
            (_, OracleStrength::Unknown) => StageState::Unknown,
            (_, OracleStrength::Weak | OracleStrength::Smoke) => StageState::Weak,
            (true, OracleStrength::Strong | OracleStrength::Medium) => StageState::Yes,
            (false, OracleStrength::Strong | OracleStrength::Medium) => StageState::Weak,
        }
    };
    let summary = format!(
        "Strongest oracle for seam kind `{}` is `{}` (kind-match {})",
        seam.kind().as_str(),
        best.as_str(),
        best_matching != OracleStrength::None
    );
    StageEvidence::new(state, Confidence::Medium, summary)
}

/// Returns true when `oracle` is a discriminating match for `seam`.
///
/// For most non-ErrorVariant seams this is identical to
/// `oracle_kind_matches_seam` (the existing kind-category check).
/// `FieldConstruction` additionally requires the oracle to name the exact
/// constructed field through a member access or record field/pattern. A strong
/// assertion on a sibling field is not evidence for this field seam.
///
/// For `ErrorVariant` seams (RIPR-SPEC-0106, Part B) the oracle must also
/// structurally pin the seam's specific variant:
/// - The oracle kind must be `ExactErrorVariant`.
/// - The oracle text must contain the seam's variant token
///   (from `RequiredDiscriminator::ErrorVariant { variant }`).
/// - If the seam variant cannot be parsed from the discriminator, or the
///   oracle text does not contain it, the oracle is NOT credited (fail-closed).
///
/// This is the over-credit guard: a test that pins `MyError::Negative` does
/// NOT discriminate a `MyError::TooLarge` seam.
fn oracle_discriminates_seam(seam: &RepoSeam, oracle: &super::facts::OracleFact) -> bool {
    if !oracle_kind_matches_seam(seam, &oracle.kind) {
        return false;
    }
    if seam.kind() == SeamKind::FieldConstruction {
        return field_construction_oracle_matches_seam_field(seam, &oracle.text);
    }
    if seam.kind() != SeamKind::ErrorVariant {
        return true;
    }
    // ErrorVariant seam: require variant-level structural match.
    error_variant_oracle_matches_seam_variant(seam, &oracle.text)
}

fn field_construction_oracle_matches_seam_field(seam: &RepoSeam, oracle_text: &str) -> bool {
    use crate::analysis::seams::RequiredDiscriminator;

    let RequiredDiscriminator::FieldValue { field } = seam.required_discriminator() else {
        return false;
    };
    let Some(field_name) = record_field_name(field) else {
        return false;
    };
    let oracle = strip_comments_and_strings(oracle_text);
    code_contains_member_field(&oracle, field_name)
        || code_contains_record_field(&oracle, field_name)
}

fn record_field_name(field_expression: &str) -> Option<&str> {
    let candidate = field_expression
        .split_once(':')
        .map_or(field_expression, |(field, _value)| field)
        .trim();
    let candidate = candidate.strip_prefix("r#").unwrap_or(candidate);
    (!candidate.is_empty()
        && candidate
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && candidate
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic()))
    .then_some(candidate)
}

fn code_contains_member_field(code: &str, field: &str) -> bool {
    let pattern = format!(".{field}");
    code.match_indices(&pattern).any(|(start, _)| {
        let after = code[start + pattern.len()..].chars().next();
        after.is_none_or(|ch| ch != '_' && !ch.is_ascii_alphanumeric())
    })
}

fn code_contains_record_field(code: &str, field: &str) -> bool {
    code.match_indices(field).any(|(start, _)| {
        let before = code[..start].chars().next_back();
        if before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            return false;
        }
        let after = code[start + field.len()..].trim_start();
        after.starts_with(':')
    })
}

/// Returns true when the oracle text structurally pins the same error variant
/// as the seam requires.
///
/// The seam variant is extracted from `RequiredDiscriminator::ErrorVariant { variant }`
/// (which holds the full `Err(<variant>)` expression such as
/// `"return Err(MyError::TooLarge)"`). The assertion variant is extracted from
/// the oracle text using the same `exact_error_variant` + `enum_variant_values`
/// parsers.
///
/// Fail-closed: returns `false` whenever either side cannot be parsed or they
/// do not share a common variant value.
fn error_variant_oracle_matches_seam_variant(seam: &RepoSeam, oracle_text: &str) -> bool {
    use super::classify::{enum_variant_values, exact_error_variant};
    use crate::analysis::seams::RequiredDiscriminator;

    // Extract the seam's required variant from the discriminator expression.
    let seam_variant = match seam.required_discriminator() {
        RequiredDiscriminator::ErrorVariant { variant } => {
            // The `variant` field is the full expression such as
            // `"return Err(MyError::TooLarge);"` — extract the inner variant.
            // Fail-closed: if unparseable, return false.
            match exact_error_variant(variant) {
                Some(v) => v,
                None => {
                    return error_constructor_payload_oracle_matches_seam(variant, oracle_text)
                        || error_string_payload_oracle_matches_seam(variant, oracle_text);
                }
            }
        }
        _ => return false,
    };

    // Extract the variant(s) named in the oracle assertion text.
    // For `assert_eq!(err, MyError::Negative)` there is no `Err(` in the text,
    // so we use `enum_variant_values` directly on the full oracle text.
    let oracle_variants = if oracle_text.contains("Err(") {
        // Classic inline form: `assert_matches!(result, Err(MyError::Negative))`
        match exact_error_variant(oracle_text) {
            Some(v) => vec![v],
            None => return false,
        }
    } else {
        // Unwrap_err-bound form: `assert_eq!(err, MyError::Negative)`
        enum_variant_values(oracle_text)
    };

    // Credit only when the oracle names the seam's exact variant.
    oracle_variants.iter().any(|v| v == &seam_variant)
}

fn error_constructor_payload_oracle_matches_seam(seam_text: &str, oracle_text: &str) -> bool {
    use super::classify::error_constructor_payloads;

    let seam_payloads = error_constructor_payloads(seam_text);
    let oracle_payloads = error_constructor_payloads(oracle_text);
    seam_payloads
        .iter()
        .filter(|seam| !seam.string_literals.is_empty())
        .any(|seam| {
            oracle_payloads.iter().any(|oracle| {
                oracle.path == seam.path && oracle.string_literals == seam.string_literals
            })
        })
}

fn error_string_payload_oracle_matches_seam(seam_text: &str, oracle_text: &str) -> bool {
    use super::classify::error_result_payload_literal_sets;

    let seam_payloads = error_result_payload_literal_sets(seam_text);
    if seam_payloads.is_empty() {
        return false;
    }
    let oracle_payloads = oracle_string_payload_literal_sets(oracle_text);
    seam_payloads.iter().any(|seam| {
        oracle_payloads
            .iter()
            .any(|oracle| payload_literals_match(seam, oracle))
    })
}

fn oracle_string_payload_literal_sets(oracle_text: &str) -> Vec<Vec<String>> {
    use super::classify::{error_result_payload_literal_sets, rust_string_literals};
    use super::extract::equality_assertion_arguments;

    let result_payloads = error_result_payload_literal_sets(oracle_text);
    if !result_payloads.is_empty() {
        return result_payloads;
    }
    let Some(args) = equality_assertion_arguments(oracle_text) else {
        return Vec::new();
    };
    args.into_iter()
        .take(2)
        .filter_map(|arg| {
            let literals = rust_string_literals(&arg);
            (!literals.is_empty()).then_some(literals)
        })
        .collect()
}

fn payload_literals_match(seam_literals: &[String], oracle_literals: &[String]) -> bool {
    seam_literals
        .iter()
        .filter(|literal| payload_literal_has_fixed_text(literal))
        .any(|seam_literal| {
            oracle_literals
                .iter()
                .filter(|literal| payload_literal_has_fixed_text(literal))
                .any(|oracle_literal| format_literal_matches(seam_literal, oracle_literal))
        })
}

fn format_literal_matches(pattern: &str, observed: &str) -> bool {
    if pattern == observed {
        return true;
    }
    let fragments = format_literal_fixed_fragments(pattern);
    if !fragments.removed_placeholder {
        return false;
    }
    let meaningful = fragments
        .values
        .iter()
        .filter(|fragment| substantial_literal_fragment(fragment))
        .collect::<Vec<_>>();
    if meaningful.is_empty() {
        return false;
    }
    let mut search_from = 0usize;
    let last_index = meaningful.len().saturating_sub(1);
    for (index, fragment) in meaningful.iter().enumerate() {
        let fragment = fragment.as_str();
        let Some(relative) = observed[search_from..].find(fragment) else {
            return false;
        };
        let absolute = search_from + relative;
        if index == 0 && !fragments.starts_with_placeholder && absolute != 0 {
            return false;
        }
        let fragment_end = absolute + fragment.len();
        if index == last_index && !fragments.ends_with_placeholder && fragment_end != observed.len()
        {
            return false;
        }
        search_from = fragment_end;
    }
    true
}

fn payload_literal_has_fixed_text(literal: &str) -> bool {
    format_literal_fixed_fragments(literal)
        .values
        .iter()
        .any(|fragment| fragment.chars().any(|ch| ch.is_alphanumeric()))
}

struct FormatLiteralFragments {
    values: Vec<String>,
    removed_placeholder: bool,
    starts_with_placeholder: bool,
    ends_with_placeholder: bool,
}

fn format_literal_fixed_fragments(pattern: &str) -> FormatLiteralFragments {
    let mut fragments = Vec::new();
    let mut current = String::new();
    let mut removed_placeholder = false;
    let mut saw_content = false;
    let mut starts_with_placeholder = false;
    let mut ends_with_placeholder = false;
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    let _ = chars.next();
                    current.push('{');
                    saw_content = true;
                    ends_with_placeholder = false;
                    continue;
                }
                if !current.is_empty() {
                    fragments.push(std::mem::take(&mut current));
                }
                if !saw_content {
                    starts_with_placeholder = true;
                }
                saw_content = true;
                removed_placeholder = true;
                ends_with_placeholder = true;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    let _ = chars.next();
                    current.push('}');
                    saw_content = true;
                    ends_with_placeholder = false;
                }
            }
            _ => {
                current.push(ch);
                saw_content = true;
                ends_with_placeholder = false;
            }
        }
    }
    if !current.is_empty() {
        fragments.push(current);
    }
    FormatLiteralFragments {
        values: fragments,
        removed_placeholder,
        starts_with_placeholder,
        ends_with_placeholder,
    }
}

fn substantial_literal_fragment(fragment: &str) -> bool {
    fragment.chars().filter(|ch| ch.is_alphanumeric()).count() >= 8
}

/// Returns true when `oracle_kind` is an acceptable discriminator for `seam_kind`.
///
/// This is the single source of truth for the kind-matching rule used by both
/// the grader (via `oracle_kind_matches_seam`) and the exemplar selector in
/// `output::agent_seam_packets::nearest_strong_test_to_imitate`.
/// Do not duplicate this rule — call this function instead.
pub(crate) fn oracle_kind_matches_seam_kind(seam_kind: SeamKind, oracle_kind: &OracleKind) -> bool {
    match seam_kind {
        SeamKind::PredicateBoundary
        | SeamKind::ReturnValue
        | SeamKind::MatchArm
        | SeamKind::FieldConstruction => matches!(
            oracle_kind,
            OracleKind::ExactValue
                | OracleKind::WholeObjectEquality
                | OracleKind::Snapshot
                | OracleKind::RelationalCheck
        ),
        SeamKind::ErrorVariant => matches!(oracle_kind, OracleKind::ExactErrorVariant),
        SeamKind::SideEffect | SeamKind::CallPresence => {
            matches!(oracle_kind, OracleKind::MockExpectation)
        }
    }
}

fn oracle_kind_matches_seam(seam: &RepoSeam, oracle: &OracleKind) -> bool {
    oracle_kind_matches_seam_kind(seam.kind(), oracle)
}

pub(crate) fn oracle_semantics_for(
    kind: &OracleKind,
    strength: &OracleStrength,
    seam_kind: SeamKind,
) -> OracleSemantics {
    if matches!(strength, OracleStrength::None) {
        return OracleSemantics {
            observes: "no recognized test oracle".to_string(),
            missing: "an observable discriminator for this seam".to_string(),
            upgrade_suggestion: Some(upgrade_suggestion_for_seam(seam_kind).to_string()),
        };
    }

    match kind {
        OracleKind::ExactValue => OracleSemantics {
            observes: "the exact value or value pattern asserted by the test".to_string(),
            missing: "no obvious value-shape discriminator gap under static scope".to_string(),
            upgrade_suggestion: None,
        },
        OracleKind::ExactErrorVariant => OracleSemantics {
            observes: "the exact error variant".to_string(),
            missing: "error payload details if the changed behavior depends on payload".to_string(),
            upgrade_suggestion: Some(
                "assert the payload inside the matched error variant when payload behavior changed"
                    .to_string(),
            ),
        },
        OracleKind::WholeObjectEquality => OracleSemantics {
            observes: "whole output object equality".to_string(),
            missing:
                "field-specific intent only if the whole-object assertion is too broad to review"
                    .to_string(),
            upgrade_suggestion: None,
        },
        OracleKind::Snapshot => OracleSemantics {
            observes: "a snapshot of rendered or debug output".to_string(),
            missing: "a small explicit discriminator if the snapshot is too broad to review"
                .to_string(),
            upgrade_suggestion: Some(
                "add an exact assertion for the changed field or value when the snapshot is broad"
                    .to_string(),
            ),
        },
        OracleKind::RelationalCheck => OracleSemantics {
            observes: "a partial relationship or broad predicate about the result".to_string(),
            missing: "the exact changed value or boundary discriminator".to_string(),
            upgrade_suggestion: Some(upgrade_suggestion_for_seam(seam_kind).to_string()),
        },
        OracleKind::BroadError => OracleSemantics {
            observes: "some error occurred".to_string(),
            missing:
                "the exact error variant or payload that would discriminate the changed behavior"
                    .to_string(),
            upgrade_suggestion: Some(upgrade_suggestion_for_seam(seam_kind).to_string()),
        },
        OracleKind::SmokeOnly => OracleSemantics {
            observes: "the call completed or returned a broad ok/some/none shape".to_string(),
            missing: "the output value, error variant, field, effect, or call discriminator"
                .to_string(),
            upgrade_suggestion: Some(upgrade_suggestion_for_seam(seam_kind).to_string()),
        },
        OracleKind::MockExpectation => OracleSemantics {
            observes: "an expected call, event, state write, or persistence effect".to_string(),
            missing:
                "effect payload, count, order, or state details if those discriminate the behavior"
                    .to_string(),
            upgrade_suggestion: None,
        },
        OracleKind::Unknown => OracleSemantics {
            observes: "no recognized concrete oracle shape".to_string(),
            missing: "a discriminator assertion for the seam's observable behavior".to_string(),
            upgrade_suggestion: Some(upgrade_suggestion_for_seam(seam_kind).to_string()),
        },
    }
}

fn upgrade_suggestion_for_seam(seam_kind: SeamKind) -> &'static str {
    match seam_kind {
        SeamKind::PredicateBoundary => {
            "add an exact returned-value assertion at the missing boundary value"
        }
        SeamKind::ErrorVariant => "assert the exact error variant with matches! or assert_matches!",
        SeamKind::ReturnValue => "add an exact returned-value assertion for the changed output",
        SeamKind::FieldConstruction => {
            "assert the specific output field that carries the changed behavior"
        }
        SeamKind::SideEffect => {
            "assert the event, state write, persistence effect, or mock expectation payload"
        }
        SeamKind::MatchArm => "assert the exact enum or value produced by the changed match arm",
        SeamKind::CallPresence => "assert the expected call happened with the relevant arguments",
    }
}

fn related_test_grip(
    seam: &RepoSeam,
    test: &TestSummary,
    reason: RelationReason,
    index: &RustIndex,
) -> RelatedTestGrip {
    let (kind, strength) = best_oracle(test, seam);
    let summary = if matches!(strength, OracleStrength::None) {
        "no oracle in test body".to_string()
    } else {
        match kind {
            OracleKind::ExactValue => "exact value assertion".to_string(),
            OracleKind::ExactErrorVariant => "exact error-variant assertion".to_string(),
            OracleKind::WholeObjectEquality => "whole-object equality".to_string(),
            OracleKind::Snapshot => "snapshot oracle".to_string(),
            OracleKind::RelationalCheck => "relational check".to_string(),
            OracleKind::BroadError => "is_err / broad-error assertion".to_string(),
            OracleKind::SmokeOnly => "smoke-only assertion".to_string(),
            OracleKind::MockExpectation => "mock expectation".to_string(),
            OracleKind::Unknown => "no recognised oracle".to_string(),
        }
    };
    let confidence = reason.confidence();
    RelatedTestGrip {
        test_name: test.name.clone(),
        file: test.file.clone(),
        line: test.start_line,
        test_target: test_target_evidence(index, seam, test, reason),
        oracle_kind: kind,
        oracle_strength: strength,
        evidence_summary: summary,
        relation_reason: reason,
        relation_confidence: confidence,
    }
}

fn test_target_evidence(
    index: &RustIndex,
    seam: &RepoSeam,
    test: &TestSummary,
    relation: RelationReason,
) -> Option<TestTargetEvidence> {
    let file = index.files.get(&test.file)?;
    let function = file.functions.iter().find(|function| {
        function.is_test && function.name == test.name && function.start_line == test.start_line
    })?;
    Some(TestTargetEvidence::from_index(
        function.id.clone(),
        function.file.clone(),
        function.start_line,
        if function.file == seam.file() {
            TestKind::InlineUnit
        } else {
            TestKind::Integration
        },
        relation,
    ))
}

fn best_oracle(test: &TestSummary, seam: &RepoSeam) -> (OracleKind, OracleStrength) {
    let mut best_kind = OracleKind::Unknown;
    let mut best_strength = OracleStrength::None;
    let mut best_matching_kind = OracleKind::Unknown;
    let mut best_matching_strength = OracleStrength::None;
    for oracle in &test.assertions {
        if oracle.strength.rank() > best_strength.rank() {
            best_strength = oracle.strength.clone();
            best_kind = oracle.kind.clone();
        } else if oracle.strength.rank() == best_strength.rank()
            && oracle_kind_matches_seam(seam, &oracle.kind)
        {
            best_kind = oracle.kind.clone();
        }
        if oracle_discriminates_seam(seam, oracle)
            && oracle.strength.rank() > best_matching_strength.rank()
        {
            best_matching_strength = oracle.strength.clone();
            best_matching_kind = oracle.kind.clone();
        }
    }
    if seam.kind() == SeamKind::FieldConstruction && best_matching_strength != OracleStrength::None
    {
        (best_matching_kind, best_matching_strength)
    } else {
        (best_kind, best_strength)
    }
}

// --- Argument-extraction helpers, lifted from analysis::classifier and
// trimmed to the shape this module needs. The classifier originals stay
// authoritative for diff-scoped findings; copying keeps the seam path
// from getting tangled in `Probe`-flavored helpers.

fn call_arguments(text: &str, callee: &str) -> Option<Vec<String>> {
    let start = named_call_open_paren_index(text, callee)?;
    let inside = delimited_contents_at(text, start)?;
    Some(split_top_level_commas(&inside))
}

fn named_call_open_paren_index(text: &str, callee: &str) -> Option<usize> {
    text.match_indices(callee).find_map(|(start, _)| {
        let before = text[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return None;
        }
        let after_start = start + callee.len();
        let after = &text[after_start..];
        after.starts_with('(').then_some(after_start)
    })
}

fn delimited_contents_at(text: &str, start: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let open = *bytes.get(start)?;
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    let open = char::from(open);
    let close = char::from(close);
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut content_start = None;
    for (offset, ch) in text[start..].char_indices() {
        let idx = start + offset;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            c if c == open => {
                depth += 1;
                if depth == 1 {
                    content_start = Some(idx + ch.len_utf8());
                }
            }
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    let content_start = content_start?;
                    return text.get(content_start..idx).map(str::to_string);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for ch in input.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trailing = current.trim().to_string();
    if !trailing.is_empty() {
        out.push(trailing);
    }
    out
}

/// Extract literal scalar values from a single call argument.
///
/// Identifiers are intentionally rejected: a value-fact reflects a
/// concrete activation seen at the call site. A bare identifier (e.g.,
/// `amount`, `t`) means the test gets the value through a helper, so
/// the activation is opaque and should not be counted as observed.
fn scalar_values(arg: &str) -> Vec<String> {
    let trimmed = arg.trim().trim_end_matches([',', ';']);
    if trimmed.is_empty() {
        return Vec::new();
    }
    // String / char literal.
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        return vec![trimmed.to_string()];
    }
    // Numeric literal (optionally negative, decimal, with `_` separators).
    let numeric_body = trimmed.strip_prefix('-').unwrap_or(trimmed);
    if !numeric_body.is_empty()
        && numeric_body
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        && numeric_body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
    {
        return vec![trimmed.to_string()];
    }
    // Path-shaped enum-variant literal, e.g. `Color::Red` or
    // `AuthError::RevokedToken`. Must contain `::` and otherwise be
    // identifier-shaped.
    if trimmed.contains("::")
        && trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
    {
        return vec![trimmed.to_string()];
    }
    Vec::new()
}

fn sort_value_facts(values: &mut Vec<ValueFact>) {
    values.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.value.cmp(&b.value))
            .then(a.text.cmp(&b.text))
    });
    values.dedup_by(|a, b| a.line == b.line && a.value == b.value && a.text == b.text);
}

#[cfg(test)]
mod tests;
