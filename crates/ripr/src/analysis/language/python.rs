//! Python preview adapter.
//!
//! See `docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md` and
//! `docs/adr/0009-python-parser-substrate.md`.
//!
//! This slice extracts the first useful syntax-first Python facts:
//!
//! - owners for module functions, async functions, class methods, and
//!   `@staticmethod` / `@classmethod` methods;
//! - pytest `test_*` functions, parametrized pytest tests, and
//!   `unittest.TestCase.test_*` methods;
//! - pytest, unittest, and mock assertion/oracle facts;
//! - related-test references by direct calls, import-alias calls, and
//!   conservative same-stem proximity.
//!
//! Import-graph matching, static limits, editor routing, generated tests,
//! runtime execution, and provider calls remain out of scope.
//! Strong exact-value assertions can produce `exposed`; weaker or unknown
//! related-test oracles produce `weakly_exposed`; missing related tests produce
//! `no_static_path`.

use super::super::{
    AnalysisOptions, diff::ChangedFile, fingerprint_probe_id, normalize_expression,
};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::{
    OraclePolicy, is_detectable_excluded_python_path, is_detectable_generated_python_path,
};
#[cfg(test)]
use crate::domain::{
    DeltaKind, FlowSinkKind, LanguageId as DomainLanguageId, LanguageStatus, RelatedTest,
    StageState,
};
use crate::domain::{
    ExposureClass, Finding, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind,
    ProbeFamily, StaticLimitKind, StopReason, SymbolId,
};
use rustpython_parser::ast::Expr;
use std::{
    collections::BTreeMap,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};
mod classify;
use classify::{PythonNoBehaviorContext, classify_change_with_context};
#[cfg(test)]
use classify::{classify_change, classify_change_with_old};
mod discriminators;
mod no_behavior;
mod oracles;
mod owners_tests;
mod probe_shape;
mod related_tests;
mod repo;
mod sink_alignment;
mod source_facts;
#[cfg(test)]
use discriminators::{
    first_python_keyword_argument, is_literal_python_model_field_value,
    is_python_constructor_callee, is_simple_python_model_field_value,
    python_keyword_argument_parts, python_output_or_call_discriminator,
    python_return_constructor_field_parts, python_return_dict_field_parts,
    python_route_response_field_discriminator, split_python_constructor_call, top_level_equals,
};
use discriminators::{
    first_python_string_literal, is_python_control_predicate_line, parse_attribute_assignment,
    python_assignment_constructor_field_parts, python_dict_field_segment_parts,
    python_exit_code_discriminator, python_return_constructor_field_discriminator,
    python_return_dict_field_discriminator, python_string_literal_value, split_python_assignment,
    top_level_python_segments,
};
#[cfg(test)]
use no_behavior::{
    analyze_call_args, changed_default_value_params, free_function_call_arglists,
    is_annotation_only_def_change, is_annotation_only_var_change, is_python_no_behavior_line,
};
use oracles::collect_assertions_from_statements;
#[cfg(test)]
use probe_shape::{
    canonical_python_gap_for, classify_probe_shape, contains_mock_initializer,
    looks_like_call_expression,
};
#[cfg(test)]
use related_tests::{
    PythonRelationKind, binding_target_for_construction, body_calls_owner,
    construct_result_is_called, contains_any_attribute_call, find_related_tests,
    has_unclosed_quote, imported_module_matches_owner, local_binding_calls_owner,
    normalize_similarity_key, normalize_test_stem, owner_similarity_keys, related_test_candidates,
    related_test_relation, same_stem_related, similarity_key_contains, verify_command_for_test,
};
use related_tests::{
    first_parenthesized_string_argument, import_source_module_matches_owner,
    strong_test_calls_owner_method_on_bound_receiver, strong_test_imports_owner_from_module,
};
#[cfg(test)]
use sink_alignment::strong_oracle_observes_owner;
#[cfg(test)]
use sink_alignment::{
    classify_sink_alignment, dict_changed_keys_and_values, fstring_change_is_length_invariant,
    fstring_template, oracle_is_pure_len_aggregate, oracle_text_observes_token,
    parse_dict_literal_fields,
};
pub(crate) use source_facts::detect_python_test_framework;
#[cfg(test)]
use source_facts::parse_module;
use source_facts::{extract_source_facts, source_fact_snapshot_observation};
mod source_utils;
#[cfg(test)]
use source_utils::line_for_offset;
use source_utils::{is_test_file, normalized_path};
mod static_limits;
use static_limits::{
    PythonStaticLimit, has_identifier_boundary, line_prefix_before,
    python_callee_start_has_boundary, python_prefix_hides_code,
};
#[cfg(test)]
use static_limits::{
    contains_dynamic_dispatch, contains_dynamic_import, contains_metaprogramming,
    is_known_mock_constructor_import, is_transparent_owner_decorator,
    is_transparent_owner_decorator_for_owner, line_uses_imported_symbol, static_limit_for_change,
    test_has_mocked_module,
};

mod workspace;
#[cfg(test)]
use workspace::visit_workspace;
use workspace::{
    collect_workspace_python_files, line_is_in_ranges, owner_for_changed_line,
    reconstruct_old_source,
};

/// Python preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PythonAdapter;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonOwner {
    name: String,
    qualified_name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    owner_kind: Option<OwnerKind>,
    decorators: Vec<String>,
    imports: Vec<PythonImport>,
    cli_receiver_names: Vec<String>,
    route_paths: Vec<String>,
    dynamic_route_decorators: Vec<String>,
}

impl PythonOwner {
    fn symbol_id(&self) -> SymbolId {
        SymbolId(format!(
            "python:{}::{}",
            normalized_path(&self.file),
            self.qualified_name
        ))
    }

    fn is_module_owner(&self) -> bool {
        self.qualified_name == "<module>"
    }

    fn specificity_rank(&self) -> usize {
        if self.owner_kind.is_some() {
            0
        } else if self.is_module_owner() {
            2
        } else {
            1
        }
    }

    fn span_width(&self) -> usize {
        self.end_line.saturating_sub(self.start_line)
    }

    fn kind_label(&self) -> &'static str {
        match self.owner_kind {
            Some(kind) => kind.as_str(),
            None if self.is_module_owner() => "module_function",
            None => "class",
        }
    }

    fn missing_test_reference(&self) -> String {
        if self.is_module_owner() {
            format!("module-level behavior in `{}`", normalized_path(&self.file))
        } else {
            format!("`{}(`", self.name)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTest {
    name: String,
    qualified_name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    imports: Vec<PythonImport>,
    decorators: Vec<String>,
    fixtures: Vec<String>,
    parametrized: bool,
    framework: &'static str,
    assertions: Vec<PythonAssertion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonImport {
    imported: String,
    alias: String,
    /// The dotted source module of a `from M import Y` statement (e.g. `src.handler`).
    /// Relative imports are resolved against the importing file when possible (for
    /// example, `from .handler import validate` in `tests/test_api.py` resolves to
    /// `tests.handler`). Empty for a plain `import X`.
    source_module: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonAssertion {
    text: String,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
    oracle_shape: PythonOracleShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PythonOracleShape {
    ExactAssertion,
    BoundaryAssertion,
    ExceptionAssertion,
    FieldAssertion,
    OutputAssertion,
    StatusCodeAssertion,
    BroadSmokeAssertion,
    MockExpectation,
    UnknownCustomHelper,
}

impl PythonOracleShape {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactAssertion => "exact_assertion",
            Self::BoundaryAssertion => "boundary_assertion",
            Self::ExceptionAssertion => "exception_assertion",
            Self::FieldAssertion => "field_assertion",
            Self::OutputAssertion => "output_assertion",
            Self::StatusCodeAssertion => "status_code_assertion",
            Self::BroadSmokeAssertion => "broad_smoke_assertion",
            Self::MockExpectation => "mock_expectation",
            Self::UnknownCustomHelper => "unknown_custom_helper",
        }
    }
}

fn expr_full_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => expr_full_name(attribute.value.as_ref())
            .map(|prefix| format!("{prefix}.{}", attribute.attr)),
        Expr::Call(call) => expr_full_name(call.func.as_ref()),
        _ => None,
    }
}

fn stop_reason_for_python_static_limit(limit: &PythonStaticLimit) -> StopReason {
    match limit.kind {
        StaticLimitKind::DynamicDispatch => StopReason::DynamicDispatchUnresolved,
        _ => StopReason::StaticProbeUnknown,
    }
}

fn python_weak_missing_summary(
    owner: &PythonOwner,
    probe_family: &ProbeFamily,
    strongest_kind: &OracleKind,
) -> String {
    let shape = match probe_family {
        ProbeFamily::Predicate => "the changed boundary",
        ProbeFamily::ReturnValue => "the returned value",
        ProbeFamily::ErrorPath => "the exact exception type/message",
        ProbeFamily::FieldConstruction => "the changed field or object value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "the changed output/log/call effect",
        ProbeFamily::MatchArm => "the changed match arm",
        ProbeFamily::StaticUnknown => "the changed behavior",
    };
    format!(
        "Related Python test reaches `{}` but the strongest extracted oracle is `{}`; add or strengthen a focused assertion for {shape}.",
        owner.name,
        strongest_kind.as_str()
    )
}

fn python_recommended_next_step(
    class: &ExposureClass,
    probe_family: &ProbeFamily,
    has_oracle_eligible_relation: bool,
    missing_discriminators: &[MissingDiscriminatorFact],
) -> Option<String> {
    match class {
        ExposureClass::StaticUnknown | ExposureClass::NoStaticPath => None,
        ExposureClass::Exposed => {
            Some("Python preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed behavior.".to_string())
        }
        _ if !has_oracle_eligible_relation => None,
        _ => {
            let missing = &missing_discriminators.first()?.value;
            let action = match probe_family {
                ProbeFamily::Predicate => "strengthen the existing related test with a focused boundary assertion",
                ProbeFamily::ReturnValue => {
                    "strengthen the existing related test with an exact return-value assertion"
                }
                ProbeFamily::ErrorPath => {
                    "strengthen the existing related test with an exception assertion"
                }
                ProbeFamily::FieldConstruction => {
                    "strengthen the existing related test with a field/object assertion"
                }
                ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
                    "strengthen the existing related test with an output/log/call-effect assertion"
                }
                ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => {
                    "strengthen the existing related test with a focused assertion"
                }
            };
            Some(format!(
                "Python preview: {action} for missing discriminator `{missing}`."
            ))
        }
    }
}

/// Significant identifier and string-literal tokens from a changed source line.
/// These approximate the *changed sink* — the attribute/field/value the change
/// touches — so an oracle that asserts on them is observing the change.
fn significant_change_tokens(line_text: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "self", "cls", "return", "if", "elif", "else", "while", "for", "def", "none", "true",
        "false", "and", "or", "not", "in", "is", "raise", "assert", "yield", "await", "async",
        "class", "import", "from", "as", "with", "try", "except", "lambda",
    ];
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in line_text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            push_identifier(&mut out, &mut current, STOP);
        }
    }
    push_identifier(&mut out, &mut current, STOP);
    for quote in ['"', '\''] {
        for (index, part) in line_text.split(quote).enumerate() {
            if index % 2 == 1 && part.len() >= 2 {
                out.push(part.to_string());
            }
        }
    }
    out
}

fn push_identifier(out: &mut Vec<String>, current: &mut String, stop: &[&str]) {
    if current.len() >= 3
        && !stop.contains(&current.to_ascii_lowercase().as_str())
        && !current.chars().all(|c| c.is_ascii_digit())
    {
        out.push(current.clone());
    }
    current.clear();
}

impl LanguageAdapter for PythonAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Python))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        let workspace_files = collect_workspace_python_files(&options.root);
        let mut all_owners: Vec<PythonOwner> = Vec::new();
        let mut all_tests: Vec<PythonTest> = Vec::new();
        let mut docstring_ranges_by_file: BTreeMap<PathBuf, Vec<RangeInclusive<usize>>> =
            BTreeMap::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            let facts = extract_source_facts(relative, &source);
            debug_assert!(source_fact_snapshot_observation(&facts) > 0);
            docstring_ranges_by_file.insert(relative.clone(), facts.docstring_line_ranges.clone());
            if is_test_file(relative) {
                all_tests.extend(facts.tests);
            } else {
                all_owners.extend(facts.owners);
            }
        }

        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            // Excluded subtrees (vendor, environment, cache, build output)
            // are skipped BEFORE counting (#3672): the diff workspace walk
            // prunes them, so no workspace facts can back a changed file
            // under one and no findings can ever be emitted for it. Counting
            // it would put an uninspected file in the report denominator.
            if !self.accepts_path(&changed.path)
                || is_detectable_generated_python_path(&changed.path)
                || is_detectable_excluded_python_path(&changed.path)
            {
                continue;
            }
            changed_count += 1;
            if is_test_file(&changed.path) {
                continue;
            }
            let new_docstring_ranges = docstring_ranges_by_file
                .get(&changed.path)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let old_docstring_ranges = std::fs::read_to_string(options.root.join(&changed.path))
                .ok()
                .and_then(|source| reconstruct_old_source(&source, changed))
                .map(|source| extract_source_facts(&changed.path, &source).docstring_line_ranges)
                .unwrap_or_default();
            for added in &changed.added_lines {
                // Pair the in-place removed line (same new-side position) so the
                // classifier can credit the changed-sink token on the DELTA only.
                let old_line = changed
                    .removed_lines
                    .iter()
                    .find(|removed| removed.new_side_line == added.line);
                let old_line_text = old_line.map(|removed| removed.text.as_str());
                let no_behavior = PythonNoBehaviorContext {
                    new_line_in_docstring: line_is_in_ranges(added.line, new_docstring_ranges),
                    old_line_in_docstring: old_line.is_some_and(|removed| {
                        line_is_in_ranges(removed.line, &old_docstring_ranges)
                    }),
                };
                if let Some(finding) = classify_change_with_context(
                    &changed.path,
                    added.line,
                    &added.text,
                    old_line_text,
                    &all_owners,
                    &all_tests,
                    no_behavior,
                ) {
                    findings.push(finding);
                }
            }
        }
        Ok(LanguageDiffResult {
            findings,
            harness_projections: Vec::new(),
            changed_files: changed_count,
            candidate_line_count: 0,
            changed_files_by_language: Vec::new(),
            partial_scope: None,
            skipped_files: 0,
            limitations: Vec::new(),
        })
    }

    fn analyze_repo(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // #2109: the shared repo working-set override bounds the run before
        // any read; an invalid override fails closed as a named error.
        let working_set_limit = repo::repo_working_set_limit()?;
        PythonAdapter::analyze_repo_with_limit(&options.root, working_set_limit)
    }
}

impl PythonAdapter {
    /// The deterministic repo-mode core of
    /// [`LanguageAdapter::analyze_repo`] with the working-set limit
    /// injected (#2109, #3554 PR C).
    ///
    /// Calls the producer-owned input authority
    /// (`select_repo_input_with_limit`) and the native evidence producer
    /// (`build_repo_evidence`), then projects onto the shared
    /// `LanguageRepoResult`: findings keep their native Python diff-mode
    /// shape, counts come from the input authority, harness projections
    /// stay empty (no registrations in repo mode), and a capped/partial run
    /// discloses itself through `partial_reason` so the pipeline records a
    /// partial `LanguageRun` that gates fail closed on.
    pub(in crate::analysis::language::python) fn analyze_repo_with_limit(
        root: &Path,
        working_set_limit: repo::RepoWorkingSetLimit,
    ) -> Result<LanguageRepoResult, String> {
        let input = repo::select_repo_input_with_limit(root, working_set_limit);
        let evidence = repo::build_repo_evidence(&input, root);
        let partial_reason = repo::partial_disclosure(&evidence);
        let production_files = input.production_files.len();
        let skipped_files = evidence.counts.skipped;
        Ok(LanguageRepoResult {
            findings: evidence.findings,
            // No test-harness registry registrations exist in repo mode
            // (#3532): the projection is empty.
            harness_projections: Vec::new(),
            production_files,
            skipped_files,
            partial_reason,
        })
    }
}

#[cfg(test)]
mod python_tests;

#[cfg(test)]
mod tests;
