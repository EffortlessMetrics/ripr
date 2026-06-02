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

use super::facts::CallFact;
use super::rust_index::{
    self, FunctionSummary, OracleFact, RustIndex, TestSummary, extract_call_facts,
    extract_identifier_tokens,
};
use super::seams::{ExpectedSink, RepoSeam, SeamId, SeamKind};
use crate::domain::{
    Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence, StageState,
    ValueContext, ValueFact,
};
use serde::{Deserialize, Serialize};
use std::cell::{OnceCell, RefCell};
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
    pub(crate) oracle_kind: OracleKind,
    pub(crate) oracle_strength: OracleStrength,
    pub(crate) evidence_summary: String,
    pub(crate) relation_reason: RelationReason,
    pub(crate) relation_confidence: RelationConfidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OracleSemantics {
    pub(crate) observes: String,
    pub(crate) missing: String,
    pub(crate) upgrade_suggestion: Option<String>,
}

/// Precomputed per-test facts for repo seam evidence consumers. This
/// avoids repeatedly tokenizing the same test assertions and import
/// lines while classifying every seam in a workspace.
pub(crate) struct CompactGripContext<'a> {
    index: &'a RustIndex,
    tests: Vec<CompactTest<'a>>,
    tests_by_call_name: BTreeMap<String, Vec<usize>>,
    tests_by_helper_owner_call_name: BTreeMap<String, Vec<usize>>,
    tests_by_assertion_token: BTreeMap<String, Vec<usize>>,
    tests_by_file_stem: BTreeMap<String, Vec<usize>>,
    tests_by_import_token: BTreeMap<String, Vec<usize>>,
    owner_named_cache: RefCell<BTreeMap<String, Vec<usize>>>,
    same_module_cache: RefCell<BTreeMap<String, Vec<usize>>>,
}

struct CompactTest<'a> {
    test: &'a TestSummary,
    path_normalized: String,
    module_path: Option<String>,
    name_lower: String,
    call_names: BTreeSet<String>,
    assertion_tokens: BTreeSet<String>,
    helper_owner_call_names: BTreeSet<String>,
    target_affinity_owner_call_names: BTreeSet<String>,
    code_lines: Vec<String>,
    value_facts: OnceCell<super::value_resolution::ValueEnvFacts>,
}

impl<'a> CompactGripContext<'a> {
    pub(crate) fn new(index: &'a RustIndex) -> Self {
        let mut tests_by_call_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut tests_by_helper_owner_call_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut tests_by_assertion_token: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut tests_by_file_stem: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut tests_by_import_token: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let same_file_helper_owner_calls_by_file = helper_owner_calls_by_file(index);
        let helper_owner_calls_by_file = strict_helper_owner_calls_by_file(index);
        let unambiguous_test_helper_owner_calls_by_name =
            unambiguous_test_helper_owner_calls_by_name(&helper_owner_calls_by_file);
        let helper_owner_calls_by_module_path =
            helper_owner_calls_by_module_path(&helper_owner_calls_by_file);
        let direct_helper_import_aliases_by_file =
            direct_helper_import_aliases_by_file(index, &helper_owner_calls_by_module_path);
        let production_helper_owner_calls_by_package =
            production_helper_owner_calls_by_package(&helper_owner_calls_by_file);
        let target_affinity_production_owner_calls_by_package =
            target_affinity_production_owner_calls_by_package(index);
        let target_affinity_production_owner_calls_by_module_path =
            target_affinity_production_owner_calls_by_module_path(index);
        let module_import_aliases_by_file = module_import_aliases_by_file(index);
        let function_names_by_file = local_function_names_by_file(index);
        let test_scoped_function_names_by_file = test_scoped_function_names_by_file(index);
        let helper_owner_lookup = HelperOwnerCallLookup {
            helpers: &helper_owner_calls_by_file,
            unique_helpers: &unambiguous_test_helper_owner_calls_by_name,
            qualified_helpers: &helper_owner_calls_by_module_path,
            production_helpers: &production_helper_owner_calls_by_package,
            local_function_names_by_file: &function_names_by_file,
            direct_helper_import_aliases_by_file: &direct_helper_import_aliases_by_file,
        };
        let tests = index
            .tests
            .iter()
            .enumerate()
            .map(|(test_index, test)| {
                let call_names = test
                    .calls
                    .iter()
                    .map(|call| call.name.clone())
                    .collect::<BTreeSet<_>>();
                let mut assertion_tokens = BTreeSet::new();
                for assertion in &test.assertions {
                    for token in extract_identifier_tokens(&assertion.text) {
                        assertion_tokens.insert(token);
                    }
                }
                let local_function_names = function_names_by_file.get(&test.file);
                let code_lines = test
                    .body
                    .lines()
                    .map(strip_comments_and_strings)
                    .collect::<Vec<_>>();
                let module_import_aliases = module_import_aliases_by_file.get(&test.file);
                let mut helper_owner_call_names = helper_owner_call_names_for_test(
                    test,
                    &call_names,
                    &helper_owner_lookup,
                    module_import_aliases,
                );
                helper_owner_call_names.extend(same_file_helper_owner_call_names_for_test(
                    test,
                    &call_names,
                    &same_file_helper_owner_calls_by_file,
                ));
                let mut target_affinity_owner_call_names =
                    helper_owner_call_names_from_qualified_calls(
                        &test.calls,
                        &target_affinity_production_owner_calls_by_module_path,
                        module_import_aliases,
                    );
                target_affinity_owner_call_names.extend(
                    helper_owner_call_names_from_production_helpers(
                        test,
                        &call_names,
                        &target_affinity_production_owner_calls_by_package,
                        local_function_names,
                    ),
                );
                target_affinity_owner_call_names.extend(
                    helper_owner_call_names_from_same_file_unit_production_helpers(
                        test,
                        &target_affinity_production_owner_calls_by_package,
                        local_function_names,
                        test_scoped_function_names_by_file.get(&test.file),
                    ),
                );
                for call_name in &call_names {
                    tests_by_call_name
                        .entry(call_name.clone())
                        .or_default()
                        .push(test_index);
                }
                for owner_name in &helper_owner_call_names {
                    tests_by_helper_owner_call_name
                        .entry(owner_name.clone())
                        .or_default()
                        .push(test_index);
                }
                for token in &assertion_tokens {
                    tests_by_assertion_token
                        .entry(token.clone())
                        .or_default()
                        .push(test_index);
                }
                if let Some(stem) = test.file.file_stem().and_then(|stem| stem.to_str()) {
                    tests_by_file_stem
                        .entry(stem.to_string())
                        .or_default()
                        .push(test_index);
                }
                for token in import_affinity_tokens(&code_lines) {
                    tests_by_import_token
                        .entry(token)
                        .or_default()
                        .push(test_index);
                }
                CompactTest {
                    test,
                    path_normalized: normalize_path(&test.file),
                    module_path: module_path_for(&test.file),
                    name_lower: test.name.to_ascii_lowercase(),
                    call_names,
                    assertion_tokens,
                    helper_owner_call_names,
                    target_affinity_owner_call_names,
                    code_lines,
                    value_facts: OnceCell::new(),
                }
            })
            .collect();
        Self {
            index,
            tests,
            tests_by_call_name,
            tests_by_helper_owner_call_name,
            tests_by_assertion_token,
            tests_by_file_stem,
            tests_by_import_token,
            owner_named_cache: RefCell::new(BTreeMap::new()),
            same_module_cache: RefCell::new(BTreeMap::new()),
        }
    }

    fn owner_named_indices(&self, owner_name_lower: &str) -> Vec<usize> {
        if owner_name_lower.is_empty() {
            return Vec::new();
        }
        if let Some(indices) = self.owner_named_cache.borrow().get(owner_name_lower) {
            return indices.clone();
        }
        let indices = self
            .tests
            .iter()
            .enumerate()
            .filter_map(|(index, test)| test.name_lower.contains(owner_name_lower).then_some(index))
            .collect::<Vec<_>>();
        self.owner_named_cache
            .borrow_mut()
            .insert(owner_name_lower.to_string(), indices.clone());
        indices
    }

    fn same_module_indices(&self, owner_module: &str) -> Vec<usize> {
        if owner_module.is_empty() {
            return Vec::new();
        }
        if let Some(indices) = self.same_module_cache.borrow().get(owner_module) {
            return indices.clone();
        }
        let indices = self
            .tests
            .iter()
            .enumerate()
            .filter_map(|(index, test)| {
                test.module_path
                    .as_deref()
                    .is_some_and(|test_module| same_module(owner_module, test_module))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        self.same_module_cache
            .borrow_mut()
            .insert(owner_module.to_string(), indices.clone());
        indices
    }
}

type HelperOwnerCallsByFile = BTreeMap<PathBuf, BTreeMap<String, BTreeSet<String>>>;
type HelperOwnerCallsByName = BTreeMap<String, BTreeSet<String>>;
type HelperOwnerCallsByModulePath = BTreeMap<String, BTreeMap<String, BTreeSet<String>>>;
type HelperOwnerCallsByPackage = BTreeMap<String, HelperOwnerCallsByName>;
type ModuleImportAliasesByFile = BTreeMap<PathBuf, BTreeMap<String, String>>;
type DirectFunctionImportAliasesByFile = BTreeMap<PathBuf, BTreeMap<String, ImportedFunctionAlias>>;
type ProductionOwnerNamesByPackage = BTreeMap<String, BTreeSet<String>>;
type OwnerNamesByModulePath = BTreeMap<String, BTreeSet<String>>;
type OwnerNamesByPackageAndModulePath = BTreeMap<String, OwnerNamesByModulePath>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportedFunctionAlias {
    module_path: String,
    name: String,
}

struct HelperOwnerCallLookup<'a> {
    helpers: &'a HelperOwnerCallsByFile,
    unique_helpers: &'a HelperOwnerCallsByName,
    qualified_helpers: &'a HelperOwnerCallsByModulePath,
    production_helpers: &'a HelperOwnerCallsByPackage,
    local_function_names_by_file: &'a BTreeMap<PathBuf, BTreeSet<String>>,
    direct_helper_import_aliases_by_file: &'a DirectFunctionImportAliasesByFile,
}

fn same_file_helper_owner_call_names_for_test(
    test: &TestSummary,
    call_names: &BTreeSet<String>,
    helpers: &HelperOwnerCallsByFile,
) -> BTreeSet<String> {
    if rust_index::is_test_file(&test.file) {
        return BTreeSet::new();
    }
    let Some(file_helpers) = helpers.get(&test.file) else {
        return BTreeSet::new();
    };
    call_names
        .iter()
        .filter_map(|call_name| file_helpers.get(call_name))
        .flat_map(|owner_calls| owner_calls.iter().cloned())
        .collect()
}

fn helper_owner_calls_by_file(index: &RustIndex) -> HelperOwnerCallsByFile {
    helper_owner_calls_by_file_with_fanout(index, true)
}

fn strict_helper_owner_calls_by_file(index: &RustIndex) -> HelperOwnerCallsByFile {
    helper_owner_calls_by_file_with_fanout(index, false)
}

fn helper_owner_calls_by_file_with_fanout(
    index: &RustIndex,
    allow_fanout_wrappers: bool,
) -> HelperOwnerCallsByFile {
    let mut helpers: HelperOwnerCallsByFile = BTreeMap::new();
    let function_names_by_file = local_function_names_by_file(index);
    let direct_function_import_aliases_by_file = direct_function_import_aliases_by_file(index);
    let unambiguous_production_owner_names_by_package =
        unambiguous_production_owner_names_by_package(index);
    let owner_names_by_module_path = production_owner_names_by_module_path(index);
    let production_owner_names = production_owner_names(index);
    for function in index.functions.iter().filter(|function| !function.is_test) {
        let helper_name_lower = function.name.to_ascii_lowercase();
        let local_function_names = function_names_by_file.get(&function.file);
        let external_owner_names =
            rust_index::is_test_file(&function.file).then_some(&production_owner_names);
        let mut owner_calls = function
            .calls
            .iter()
            .filter(|call| {
                (helper_name_carries_owner_token(&helper_name_lower, &call.name)
                    || helper_directly_delegates_to_specific_owner(
                        function,
                        call,
                        local_function_names,
                        external_owner_names,
                        allow_fanout_wrappers,
                    ))
                    && call_text_contains_named_call(&call.text, &call.name)
            })
            .map(|call| call.name.clone())
            .collect::<BTreeSet<_>>();
        if let Some(package) = package_scope(&function.file) {
            owner_calls.extend(strict_direct_imported_owner_calls_for_helper(
                function,
                direct_function_import_aliases_by_file.get(&function.file),
                unambiguous_production_owner_names_by_package.get(&package),
                &owner_names_by_module_path,
            ));
        }
        if owner_calls.is_empty() {
            continue;
        }
        helpers
            .entry(function.file.clone())
            .or_default()
            .insert(function.name.clone(), owner_calls);
    }
    extend_helper_owner_calls_through_bounded_graph(&mut helpers);
    helpers
}

fn strict_direct_imported_owner_calls_for_helper(
    function: &FunctionSummary,
    direct_function_import_aliases: Option<&BTreeMap<String, ImportedFunctionAlias>>,
    unambiguous_production_owner_names: Option<&BTreeSet<String>>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
) -> BTreeSet<String> {
    let owner_calls = direct_imported_owner_calls_for_function(
        function,
        direct_function_import_aliases,
        unambiguous_production_owner_names,
        owner_names_by_module_path,
    );
    if owner_calls.len() == 1 {
        owner_calls
    } else {
        BTreeSet::new()
    }
}

fn extend_helper_owner_calls_through_bounded_graph(helpers: &mut HelperOwnerCallsByFile) {
    for _ in 1..HELPER_OWNER_CALL_GRAPH_MAX_HOPS {
        let snapshot = helpers.clone();
        let mut changed = false;
        for (file, file_helpers) in helpers.iter_mut() {
            let Some(snapshot_helpers) = snapshot.get(file) else {
                continue;
            };
            for helper_name in file_helpers.keys().cloned().collect::<Vec<_>>() {
                let Some(owner_calls) = file_helpers.get(&helper_name).cloned() else {
                    continue;
                };
                let mut expanded_owner_calls = owner_calls.clone();
                for owner_call in owner_calls {
                    let Some(transitive_owner_calls) = snapshot_helpers.get(&owner_call) else {
                        continue;
                    };
                    for transitive_owner_call in transitive_owner_calls {
                        if transitive_owner_call != &helper_name
                            && expanded_owner_calls.insert(transitive_owner_call.clone())
                        {
                            changed = true;
                        }
                    }
                }
                file_helpers.insert(helper_name, expanded_owner_calls);
            }
        }
        if !changed {
            break;
        }
    }
}

fn extend_helper_owner_calls_through_bounded_name_graph(helpers: &mut HelperOwnerCallsByName) {
    for _ in 1..HELPER_OWNER_CALL_GRAPH_MAX_HOPS {
        let snapshot = helpers.clone();
        let mut changed = false;
        for helper_name in helpers.keys().cloned().collect::<Vec<_>>() {
            let Some(owner_calls) = helpers.get(&helper_name).cloned() else {
                continue;
            };
            let mut expanded_owner_calls = owner_calls.clone();
            for owner_call in owner_calls {
                let Some(transitive_owner_calls) = snapshot.get(&owner_call) else {
                    continue;
                };
                for transitive_owner_call in transitive_owner_calls {
                    if transitive_owner_call != &helper_name
                        && expanded_owner_calls.insert(transitive_owner_call.clone())
                    {
                        changed = true;
                    }
                }
            }
            helpers.insert(helper_name, expanded_owner_calls);
        }
        if !changed {
            break;
        }
    }
}

fn unambiguous_test_helper_owner_calls_by_name(
    helpers: &HelperOwnerCallsByFile,
) -> HelperOwnerCallsByName {
    let mut by_name: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
    for (file, file_helpers) in helpers {
        if !rust_index::is_test_file(file) {
            continue;
        }
        for (helper_name, owner_calls) in file_helpers {
            by_name
                .entry(helper_name.clone())
                .or_default()
                .push(owner_calls.clone());
        }
    }
    by_name
        .into_iter()
        .filter_map(|(helper_name, owner_sets)| common_helper_owner_calls(helper_name, owner_sets))
        .collect()
}

fn helper_owner_calls_by_module_path(
    helpers: &HelperOwnerCallsByFile,
) -> HelperOwnerCallsByModulePath {
    helpers
        .iter()
        .filter_map(|(file, file_helpers)| {
            if !rust_index::is_test_file(file) {
                return None;
            }
            let module_path = module_path_for(file)?.replace('/', "::");
            Some((module_path, file_helpers.clone()))
        })
        .collect()
}

fn production_helper_owner_calls_by_package(
    helpers: &HelperOwnerCallsByFile,
) -> HelperOwnerCallsByPackage {
    let mut by_package: BTreeMap<String, BTreeMap<String, Vec<BTreeSet<String>>>> = BTreeMap::new();
    for (file, file_helpers) in helpers {
        if rust_index::is_test_file(file) {
            continue;
        }
        let Some(package) = package_scope(file) else {
            continue;
        };
        for (helper_name, owner_calls) in file_helpers {
            by_package
                .entry(package.clone())
                .or_default()
                .entry(helper_name.clone())
                .or_default()
                .push(owner_calls.clone());
        }
    }
    by_package
        .into_iter()
        .filter_map(|(package, helper_sets)| {
            let mut helpers = helper_sets
                .into_iter()
                .filter_map(|(helper_name, owner_sets)| {
                    common_helper_owner_calls(helper_name, owner_sets)
                })
                .collect::<HelperOwnerCallsByName>();
            extend_helper_owner_calls_through_bounded_name_graph(&mut helpers);
            (!helpers.is_empty()).then_some((package, helpers))
        })
        .collect()
}

fn target_affinity_production_owner_calls_by_package(
    index: &RustIndex,
) -> HelperOwnerCallsByPackage {
    let function_names_by_file = local_function_names_by_file(index);
    let imported_module_aliases_by_file = module_import_aliases_by_file(index);
    let direct_function_import_aliases_by_file = direct_function_import_aliases_by_file(index);
    let unambiguous_production_owner_names_by_package =
        unambiguous_production_owner_names_by_package(index);
    let owner_names_by_module_path = production_owner_names_by_module_path(index);
    let owner_names_by_package_and_module_path =
        production_owner_names_by_package_and_module_path(index);
    let mut by_package: BTreeMap<String, BTreeMap<String, Vec<BTreeSet<String>>>> = BTreeMap::new();
    for function in index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
    {
        let Some(package) = package_scope(&function.file) else {
            continue;
        };
        let Some(local_function_names) = function_names_by_file.get(&function.file) else {
            continue;
        };
        let owner_calls = target_affinity_direct_owner_calls_for_function(
            function,
            local_function_names,
            imported_module_aliases_by_file.get(&function.file),
            direct_function_import_aliases_by_file.get(&function.file),
            unambiguous_production_owner_names_by_package.get(&package),
            &owner_names_by_module_path,
            owner_names_by_package_and_module_path.get(&package),
        );
        if owner_calls.is_empty() {
            continue;
        }
        by_package
            .entry(package)
            .or_default()
            .entry(function.name.clone())
            .or_default()
            .push(owner_calls);
    }
    by_package
        .into_iter()
        .filter_map(|(package, helper_sets)| {
            let helpers = helper_sets
                .into_iter()
                .filter_map(|(helper_name, owner_sets)| {
                    common_helper_owner_calls(helper_name, owner_sets)
                })
                .collect::<HelperOwnerCallsByName>();
            (!helpers.is_empty()).then_some((package, helpers))
        })
        .collect()
}

fn target_affinity_production_owner_calls_by_module_path(
    index: &RustIndex,
) -> HelperOwnerCallsByModulePath {
    let function_names_by_file = local_function_names_by_file(index);
    let imported_module_aliases_by_file = module_import_aliases_by_file(index);
    let direct_function_import_aliases_by_file = direct_function_import_aliases_by_file(index);
    let unambiguous_production_owner_names_by_package =
        unambiguous_production_owner_names_by_package(index);
    let owner_names_by_module_path = production_owner_names_by_module_path(index);
    let owner_names_by_package_and_module_path =
        production_owner_names_by_package_and_module_path(index);
    let mut by_module_path: HelperOwnerCallsByModulePath = BTreeMap::new();
    for function in index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
    {
        let Some(module_path) = module_path_for(&function.file) else {
            continue;
        };
        let Some(package) = package_scope(&function.file) else {
            continue;
        };
        let Some(local_function_names) = function_names_by_file.get(&function.file) else {
            continue;
        };
        let owner_calls = target_affinity_direct_owner_calls_for_function(
            function,
            local_function_names,
            imported_module_aliases_by_file.get(&function.file),
            direct_function_import_aliases_by_file.get(&function.file),
            unambiguous_production_owner_names_by_package.get(&package),
            &owner_names_by_module_path,
            owner_names_by_package_and_module_path.get(&package),
        );
        if owner_calls.is_empty() {
            continue;
        }
        by_module_path
            .entry(module_path.replace('/', "::"))
            .or_default()
            .insert(function.name.clone(), owner_calls);
    }
    by_module_path
}

fn target_affinity_direct_owner_calls_for_function(
    function: &FunctionSummary,
    local_function_names: &BTreeSet<String>,
    imported_module_aliases: Option<&BTreeMap<String, String>>,
    direct_function_import_aliases: Option<&BTreeMap<String, ImportedFunctionAlias>>,
    unambiguous_production_owner_names: Option<&BTreeSet<String>>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
    crate_local_owner_names_by_module_path: Option<&OwnerNamesByModulePath>,
) -> BTreeSet<String> {
    let mut owner_calls = function
        .calls
        .iter()
        .filter(|call| call.name != function.name)
        .filter(|call| supported_helper_owner_call_name(&call.name, local_function_names, None))
        .filter(|call| owner_token_is_specific_enough(&call.name.to_ascii_lowercase()))
        .filter(|call| call_text_contains_named_call(&call.text, &call.name))
        .map(|call| call.name.clone())
        .collect::<BTreeSet<_>>();
    owner_calls.extend(qualified_external_owner_calls_for_function(
        function,
        imported_module_aliases,
        owner_names_by_module_path,
    ));
    owner_calls.extend(direct_imported_owner_calls_for_function(
        function,
        direct_function_import_aliases,
        unambiguous_production_owner_names,
        owner_names_by_module_path,
    ));
    owner_calls.extend(crate_qualified_owner_calls_for_function(
        function,
        crate_local_owner_names_by_module_path,
    ));
    owner_calls
}

fn crate_qualified_owner_calls_for_function(
    function: &FunctionSummary,
    crate_local_owner_names_by_module_path: Option<&OwnerNamesByModulePath>,
) -> BTreeSet<String> {
    let Some(crate_local_owner_names_by_module_path) = crate_local_owner_names_by_module_path
    else {
        return BTreeSet::new();
    };
    function
        .calls
        .iter()
        .filter(|call| call.name != function.name)
        .filter(|call| owner_token_is_specific_enough(&call.name.to_ascii_lowercase()))
        .filter(|call| {
            call_text_contains_crate_qualified_owner_call(
                &call.text,
                &call.name,
                crate_local_owner_names_by_module_path,
            )
        })
        .map(|call| call.name.clone())
        .collect()
}

fn call_text_contains_crate_qualified_owner_call(
    text: &str,
    call_name: &str,
    owner_names_by_module_path: &OwnerNamesByModulePath,
) -> bool {
    let cleaned = strip_comments_and_strings(text);
    owner_names_by_module_path
        .iter()
        .any(|(module_path, owner_names)| {
            owner_names.contains(call_name)
                && code_contains_crate_qualified_helper_call(&cleaned, module_path, call_name)
        })
}

fn code_contains_crate_qualified_helper_call(
    code: &str,
    module_path: &str,
    helper_name: &str,
) -> bool {
    let pattern = format!("crate::{module_path}::{helper_name}(");
    code.match_indices(&pattern).any(|(start, _)| {
        code[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !is_rust_path_identifier_char(before))
    })
}

fn direct_imported_owner_calls_for_function(
    function: &FunctionSummary,
    direct_function_import_aliases: Option<&BTreeMap<String, ImportedFunctionAlias>>,
    unambiguous_production_owner_names: Option<&BTreeSet<String>>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
) -> BTreeSet<String> {
    let Some(direct_function_import_aliases) = direct_function_import_aliases else {
        return BTreeSet::new();
    };
    let Some(unambiguous_production_owner_names) = unambiguous_production_owner_names else {
        return BTreeSet::new();
    };
    function
        .calls
        .iter()
        .filter(|call| call.name != function.name)
        .filter(|call| call_text_contains_named_call(&call.text, &call.name))
        .filter_map(|call| direct_function_import_aliases.get(&call.name))
        .filter(|imported| unambiguous_production_owner_names.contains(&imported.name))
        .filter(|imported| {
            owner_names_by_module_path
                .get(&imported.module_path)
                .is_some_and(|owner_names| owner_names.contains(&imported.name))
        })
        .filter(|imported| owner_token_is_specific_enough(&imported.name.to_ascii_lowercase()))
        .map(|imported| imported.name.clone())
        .collect()
}

fn qualified_external_owner_calls_for_function(
    function: &FunctionSummary,
    imported_module_aliases: Option<&BTreeMap<String, String>>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
) -> BTreeSet<String> {
    let Some(imported_module_aliases) = imported_module_aliases else {
        return BTreeSet::new();
    };
    function
        .calls
        .iter()
        .filter(|call| call.name != function.name)
        .filter(|call| owner_token_is_specific_enough(&call.name.to_ascii_lowercase()))
        .filter(|call| {
            call_text_contains_imported_module_owner_call(
                &call.text,
                &call.name,
                imported_module_aliases,
                owner_names_by_module_path,
            )
        })
        .map(|call| call.name.clone())
        .collect()
}

fn call_text_contains_imported_module_owner_call(
    text: &str,
    call_name: &str,
    imported_module_aliases: &BTreeMap<String, String>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
) -> bool {
    let cleaned = strip_comments_and_strings(text);
    imported_module_aliases.iter().any(|(alias, module_path)| {
        owner_names_by_module_path
            .get(module_path)
            .is_some_and(|owner_names| owner_names.contains(call_name))
            && code_contains_qualified_helper_call(&cleaned, alias, call_name)
    })
}

fn production_owner_names_by_module_path(index: &RustIndex) -> OwnerNamesByModulePath {
    let mut by_module_path: OwnerNamesByModulePath = BTreeMap::new();
    for function in index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
    {
        let Some(module_path) = module_path_for(&function.file) else {
            continue;
        };
        by_module_path
            .entry(module_path.replace('/', "::"))
            .or_default()
            .insert(function.name.clone());
    }
    by_module_path
}

fn production_owner_names_by_package_and_module_path(
    index: &RustIndex,
) -> OwnerNamesByPackageAndModulePath {
    let mut by_package: OwnerNamesByPackageAndModulePath = BTreeMap::new();
    for function in index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
    {
        let Some(package) = package_scope(&function.file) else {
            continue;
        };
        let Some(module_path) = module_path_for(&function.file) else {
            continue;
        };
        by_package
            .entry(package)
            .or_default()
            .entry(module_path.replace('/', "::"))
            .or_default()
            .insert(function.name.clone());
    }
    by_package
}

fn module_import_aliases_by_file(index: &RustIndex) -> ModuleImportAliasesByFile {
    index
        .files
        .iter()
        .filter_map(|(file, facts)| {
            let aliases = module_import_aliases(&facts.source);
            (!aliases.is_empty()).then_some((file.clone(), aliases))
        })
        .collect()
}

fn direct_function_import_aliases_by_file(index: &RustIndex) -> DirectFunctionImportAliasesByFile {
    index
        .files
        .iter()
        .filter_map(|(file, facts)| {
            let aliases = direct_function_import_aliases(&facts.source);
            (!aliases.is_empty()).then_some((file.clone(), aliases))
        })
        .collect()
}

fn direct_helper_import_aliases_by_file(
    index: &RustIndex,
    qualified_helpers: &HelperOwnerCallsByModulePath,
) -> DirectFunctionImportAliasesByFile {
    let allowed_module_paths = qualified_helpers.keys().cloned().collect::<BTreeSet<_>>();
    index
        .files
        .iter()
        .filter_map(|(file, facts)| {
            let aliases = direct_helper_import_aliases(&facts.source, &allowed_module_paths);
            (!aliases.is_empty()).then_some((file.clone(), aliases))
        })
        .collect()
}

fn module_import_aliases(source: &str) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    for line in source.lines() {
        let line = strip_comments_and_strings(line);
        let Some(import) = line.trim().strip_prefix("use ") else {
            continue;
        };
        collect_module_import_aliases_from_use(import.trim(), &mut aliases);
    }
    aliases
}

fn direct_helper_import_aliases(
    source: &str,
    allowed_module_paths: &BTreeSet<String>,
) -> BTreeMap<String, ImportedFunctionAlias> {
    let mut aliases = BTreeMap::new();
    let mut brace_depth = 0usize;
    for line in source.lines() {
        let line = strip_comments_and_strings(line);
        if brace_depth == 0
            && let Some(import) = line.trim().strip_prefix("use ")
        {
            collect_direct_helper_import_aliases_from_use(
                import.trim(),
                allowed_module_paths,
                &mut aliases,
            );
        }
        brace_depth = update_brace_depth(brace_depth, &line);
    }
    aliases
}

fn update_brace_depth(mut depth: usize, line: &str) -> usize {
    for ch in line.chars() {
        match ch {
            '{' => depth = depth.saturating_add(1),
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn direct_function_import_aliases(source: &str) -> BTreeMap<String, ImportedFunctionAlias> {
    let mut aliases = BTreeMap::new();
    for line in source.lines() {
        let line = strip_comments_and_strings(line);
        let Some(import) = line.trim().strip_prefix("use ") else {
            continue;
        };
        collect_direct_function_import_aliases_from_use(import.trim(), &mut aliases);
    }
    aliases
}

fn collect_module_import_aliases_from_use(import: &str, aliases: &mut BTreeMap<String, String>) {
    let import = import.trim_end_matches(';').trim();
    if let Some((base, rest)) = import.split_once("::{") {
        let Some(module_path) = normalize_module_import_path(base) else {
            return;
        };
        let Some(body) = rest.strip_suffix('}') else {
            return;
        };
        for item in body.split(',').map(str::trim) {
            if item == "self" {
                if let Some(alias) = module_path.rsplit("::").next() {
                    aliases.insert(alias.to_string(), module_path.clone());
                }
            } else if let Some(alias) = item.strip_prefix("self as ").map(str::trim)
                && !alias.is_empty()
            {
                aliases.insert(alias.to_string(), module_path.clone());
            }
        }
        return;
    }

    let (path, alias) = match import.split_once(" as ") {
        Some((path, alias)) => (path.trim(), Some(alias.trim())),
        None => (import, None),
    };
    let Some(module_path) = normalize_module_import_path(path) else {
        return;
    };
    let alias = alias
        .filter(|alias| !alias.is_empty())
        .or_else(|| module_path.rsplit("::").next());
    if let Some(alias) = alias {
        aliases.insert(alias.to_string(), module_path);
    }
}

fn collect_direct_helper_import_aliases_from_use(
    import: &str,
    allowed_module_paths: &BTreeSet<String>,
    aliases: &mut BTreeMap<String, ImportedFunctionAlias>,
) {
    let import = import.trim_end_matches(';').trim();
    if let Some((base, rest)) = import.split_once("::{") {
        let Some(module_path) = normalize_helper_module_import_path(base, allowed_module_paths)
        else {
            return;
        };
        let Some(body) = rest.strip_suffix('}') else {
            return;
        };
        for item in body.split(',').map(str::trim) {
            collect_direct_function_import_alias(item, &module_path, aliases);
        }
        return;
    }
    let Some((module_path, item)) = import.rsplit_once("::") else {
        return;
    };
    let Some(module_path) = normalize_helper_module_import_path(module_path, allowed_module_paths)
    else {
        return;
    };
    collect_direct_function_import_alias(item.trim(), &module_path, aliases);
}

fn collect_direct_function_import_aliases_from_use(
    import: &str,
    aliases: &mut BTreeMap<String, ImportedFunctionAlias>,
) {
    let import = import.trim_end_matches(';').trim();
    if let Some((base, rest)) = import.split_once("::{") {
        let Some(module_path) = normalize_module_import_path(base) else {
            return;
        };
        let Some(body) = rest.strip_suffix('}') else {
            return;
        };
        for item in body.split(',').map(str::trim) {
            collect_direct_function_import_alias(item, &module_path, aliases);
        }
        return;
    }
    let Some((module_path, item)) = import.rsplit_once("::") else {
        return;
    };
    let Some(module_path) = normalize_module_import_path(module_path) else {
        return;
    };
    collect_direct_function_import_alias(item.trim(), &module_path, aliases);
}

fn collect_direct_function_import_alias(
    item: &str,
    module_path: &str,
    aliases: &mut BTreeMap<String, ImportedFunctionAlias>,
) {
    if item.is_empty() || item == "self" || item == "*" {
        return;
    }
    if item.starts_with("self as ") {
        return;
    }
    let (name, alias) = match item.split_once(" as ") {
        Some((name, alias)) => (name.trim(), Some(alias.trim())),
        None => (item.trim(), None),
    };
    if name.is_empty() || name == "self" || name == "*" {
        return;
    }
    let alias = alias.filter(|alias| !alias.is_empty()).unwrap_or(name);
    aliases.insert(
        alias.to_string(),
        ImportedFunctionAlias {
            module_path: module_path.to_string(),
            name: name.to_string(),
        },
    );
}

fn normalize_helper_module_import_path(
    path: &str,
    allowed_module_paths: &BTreeSet<String>,
) -> Option<String> {
    let path = path.trim().strip_prefix("crate::").unwrap_or(path).trim();
    if path.is_empty()
        || path.starts_with("self::")
        || path.starts_with("super::")
        || path.starts_with("::")
    {
        return None;
    }
    allowed_module_paths
        .contains(path)
        .then(|| path.to_string())
}

fn normalize_module_import_path(path: &str) -> Option<String> {
    let path = path.trim().strip_prefix("crate::")?.trim();
    if path.is_empty() || path.starts_with("super::") || path.starts_with("self::") {
        return None;
    }
    Some(path.to_string())
}

fn unambiguous_production_owner_names_by_package(
    index: &RustIndex,
) -> ProductionOwnerNamesByPackage {
    let mut counts_by_package: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for function in index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
    {
        let Some(package) = package_scope(&function.file) else {
            continue;
        };
        *counts_by_package
            .entry(package)
            .or_default()
            .entry(function.name.clone())
            .or_default() += 1;
    }
    counts_by_package
        .into_iter()
        .filter_map(|(package, counts)| {
            let names = counts
                .into_iter()
                .filter_map(|(name, count)| (count == 1).then_some(name))
                .collect::<BTreeSet<_>>();
            (!names.is_empty()).then_some((package, names))
        })
        .collect()
}

fn local_function_names_by_file(index: &RustIndex) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut names_by_file: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for function in index.functions.iter().filter(|function| !function.is_test) {
        names_by_file
            .entry(function.file.clone())
            .or_default()
            .insert(function.name.clone());
    }
    names_by_file
}

fn test_scoped_function_names_by_file(index: &RustIndex) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut names_by_file: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for (file, facts) in &index.files {
        let cfg_test_module_ranges = cfg_test_module_line_ranges(&facts.source);
        for function in facts.functions.iter().filter(|function| {
            !function.is_test
                && (rust_index::is_test_file(file)
                    || cfg_test_module_ranges.iter().any(|(start, end)| {
                        *start < function.start_line && function.start_line <= *end
                    }))
        }) {
            names_by_file
                .entry(file.clone())
                .or_default()
                .insert(function.name.clone());
        }
    }
    names_by_file
}

fn cfg_test_module_line_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut pending_cfg_test = false;
    let mut depth = 0isize;
    let mut active_modules: Vec<(usize, isize)> = Vec::new();
    let mut ranges = Vec::new();
    let mut last_line = 0usize;
    for (idx, raw_line) in source.lines().enumerate() {
        let line_number = idx + 1;
        last_line = line_number;
        let line = strip_comments_and_strings(raw_line);
        let trimmed = line.trim();
        if trimmed.contains("#[cfg(test)]") {
            pending_cfg_test = true;
        }
        let opens = line.chars().filter(|ch| *ch == '{').count() as isize;
        let closes = line.chars().filter(|ch| *ch == '}').count() as isize;
        if pending_cfg_test && line.contains("mod ") && opens > 0 {
            active_modules.push((line_number, depth + opens));
            pending_cfg_test = false;
        } else if pending_cfg_test && !trimmed.is_empty() && !trimmed.starts_with("#[") {
            pending_cfg_test = false;
        }
        depth += opens - closes;
        while active_modules
            .last()
            .is_some_and(|(_start, module_depth)| depth < *module_depth)
        {
            if let Some((start, _module_depth)) = active_modules.pop() {
                ranges.push((start, line_number));
            }
        }
    }
    ranges.extend(
        active_modules
            .into_iter()
            .map(|(start, _module_depth)| (start, last_line)),
    );
    ranges
}

fn production_owner_names(index: &RustIndex) -> BTreeSet<String> {
    index
        .functions
        .iter()
        .filter(|function| !function.is_test && !rust_index::is_test_file(&function.file))
        .map(|function| function.name.clone())
        .collect()
}

fn helper_directly_delegates_to_specific_owner(
    function: &FunctionSummary,
    call: &CallFact,
    local_function_names: Option<&BTreeSet<String>>,
    external_owner_names: Option<&BTreeSet<String>>,
    allow_fanout_wrappers: bool,
) -> bool {
    if call.name == function.name {
        return false;
    }
    let Some(local_function_names) = local_function_names else {
        return false;
    };
    let owner_name_lower = call.name.to_ascii_lowercase();
    if !owner_token_is_specific_enough(&owner_name_lower)
        || !supported_helper_owner_call_name(&call.name, local_function_names, external_owner_names)
        || !call_text_routes_directly_to_named_call(&call.text, &call.name)
    {
        return false;
    }

    let mut direct_local_owner_call_names = BTreeSet::new();
    let mut delegates_to_call = false;
    let mut has_disallowed_extra_call = false;
    for candidate in &function.calls {
        if candidate.name == function.name {
            continue;
        }
        if supported_helper_owner_call_name(
            &candidate.name,
            local_function_names,
            external_owner_names,
        ) && call_text_contains_named_call(&candidate.text, &candidate.name)
            && owner_token_is_specific_enough(&candidate.name.to_ascii_lowercase())
        {
            direct_local_owner_call_names.insert(candidate.name.clone());
            delegates_to_call |= candidate.name == call.name
                && candidate.line == call.line
                && candidate.text == call.text;
        } else if candidate.line == call.line
            && !direct_delegate_extra_call_is_allowed(candidate, call)
        {
            has_disallowed_extra_call = true;
        }
    }

    let owner_call_is_unique_or_same_file_fanout = if allow_fanout_wrappers {
        direct_local_owner_call_names.contains(&call.name)
    } else {
        direct_local_owner_call_names.len() == 1
            && direct_local_owner_call_names.contains(&call.name)
    };

    owner_call_is_unique_or_same_file_fanout && delegates_to_call && !has_disallowed_extra_call
}

fn call_text_routes_directly_to_named_call(text: &str, name: &str) -> bool {
    let cleaned = strip_comments_and_strings(text);
    cleaned.match_indices(name).any(|(start, _)| {
        let before = cleaned[..start].trim_end();
        let after = &cleaned[start + name.len()..];
        let named_call = after.starts_with("::") || after.trim_start().starts_with('(');
        if !named_call {
            return false;
        }
        direct_call_prefix_is_allowed(before)
    })
}

fn direct_call_prefix_is_allowed(prefix: &str) -> bool {
    if prefix.is_empty() || prefix == "return" || prefix.ends_with('=') || prefix.ends_with("=>") {
        return true;
    }
    if direct_delegate_condition_prefix_is_allowed(prefix) {
        return true;
    }
    if direct_receiver_method_prefix_is_allowed(prefix) {
        return true;
    }
    if let Some(macro_name) = direct_delegate_parenthesized_macro_name_before_argument(prefix) {
        return direct_delegate_parenthesized_macro_is_allowed(&macro_name);
    }
    if direct_delegate_block_prefix_is_allowed(prefix) {
        return true;
    }
    if direct_delegate_std_identity_prefix_is_allowed(prefix) {
        return true;
    }
    if direct_delegate_field_initializer_prefix_is_allowed(prefix) {
        return true;
    }
    let Some(open) = prefix.strip_suffix('(') else {
        let Some(open) = prefix.strip_suffix('[') else {
            return false;
        };
        let open = open.trim_end();
        if open.is_empty() || open == "return" || open.ends_with('=') || open.ends_with("=>") {
            return true;
        }
        if let Some(macro_prefix) = open.strip_suffix('!') {
            let macro_name = trailing_rust_identifier(macro_prefix);
            return direct_delegate_container_macro_is_allowed(&macro_name);
        }
        return false;
    };
    let open = open.trim_end();
    if open.is_empty() || open == "return" || open.ends_with('=') || open.ends_with("=>") {
        return true;
    }
    if let Some(macro_prefix) = open.strip_suffix('!') {
        let macro_name = trailing_rust_identifier(macro_prefix);
        return direct_delegate_parenthesized_macro_is_allowed(&macro_name);
    }
    let wrapper_name = direct_delegate_wrapper_name(open);
    direct_delegate_extra_call_is_inert(&wrapper_name)
}

fn direct_delegate_condition_prefix_is_allowed(prefix: &str) -> bool {
    prefix.trim() == "if"
}

fn direct_delegate_parenthesized_macro_name_before_argument(prefix: &str) -> Option<String> {
    let (macro_prefix, argument_prefix) = prefix.rsplit_once("!(")?;
    let macro_name = trailing_rust_identifier(macro_prefix);
    if macro_name.is_empty()
        || !direct_delegate_macro_argument_prefix_is_allowed(&macro_name, argument_prefix)
    {
        return None;
    }
    Some(macro_name)
}

fn direct_delegate_macro_argument_prefix_is_allowed(
    macro_name: &str,
    argument_prefix: &str,
) -> bool {
    if argument_prefix
        .chars()
        .all(|ch| ch.is_whitespace() || ch == ',')
    {
        return true;
    }
    direct_delegate_eager_later_argument_macro_is_allowed(macro_name)
        && argument_prefix_has_trailing_top_level_comma(argument_prefix)
}

fn direct_delegate_eager_later_argument_macro_is_allowed(macro_name: &str) -> bool {
    matches!(
        macro_name,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne"
    )
}

fn argument_prefix_has_trailing_top_level_comma(prefix: &str) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut saw_top_level_comma = false;
    let mut only_ws_after_last_comma = true;

    for ch in prefix.chars() {
        match ch {
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                only_ws_after_last_comma = false;
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                only_ws_after_last_comma = false;
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                only_ws_after_last_comma = false;
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                only_ws_after_last_comma = false;
            }
            '{' => {
                brace_depth = brace_depth.saturating_add(1);
                only_ws_after_last_comma = false;
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                only_ws_after_last_comma = false;
            }
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                saw_top_level_comma = true;
                only_ws_after_last_comma = true;
            }
            _ => {
                if !ch.is_whitespace() {
                    only_ws_after_last_comma = false;
                }
            }
        }
    }

    saw_top_level_comma && only_ws_after_last_comma
}

fn direct_delegate_wrapper_name(open: &str) -> String {
    let open = open.trim_end();
    if let Some((before_turbofish, generic_tail)) = open.rsplit_once("::<")
        && generic_tail.trim_end().ends_with('>')
    {
        return trailing_rust_identifier(before_turbofish);
    }
    trailing_rust_identifier(open)
}

fn direct_delegate_block_prefix_is_allowed(prefix: &str) -> bool {
    let Some(open) = prefix.strip_suffix('{') else {
        return false;
    };
    let open = open.trim_end();
    open.is_empty() || open == "return" || open.ends_with('=') || open.ends_with("=>")
}

fn direct_delegate_std_identity_prefix_is_allowed(prefix: &str) -> bool {
    let Some(open) = prefix.strip_suffix('(') else {
        return false;
    };
    let open = open.trim_end();
    let open = open.strip_prefix("return ").unwrap_or(open).trim_start();
    direct_delegate_std_identity_path_is_allowed(open)
}

fn direct_delegate_std_identity_path_is_allowed(path: &str) -> bool {
    matches!(
        path.trim(),
        "std::convert::identity"
            | "::std::convert::identity"
            | "core::convert::identity"
            | "::core::convert::identity"
    )
}

fn direct_delegate_field_initializer_prefix_is_allowed(prefix: &str) -> bool {
    let Some(before_colon) = prefix.trim_end().strip_suffix(':') else {
        return false;
    };
    let before_colon = before_colon.trim_end();
    let field_name = trailing_rust_identifier(before_colon);
    if field_name.is_empty() {
        return false;
    }
    let before_field = before_colon[..before_colon.len() - field_name.len()].trim_end();
    before_field.is_empty() || before_field.ends_with('{') || before_field.ends_with(',')
}

fn direct_receiver_method_prefix_is_allowed(prefix: &str) -> bool {
    let Some(receiver_prefix) = prefix.strip_suffix('.') else {
        return false;
    };
    let receiver = receiver_prefix.trim();
    !receiver.is_empty()
        && receiver
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && receiver
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
}

fn trailing_rust_identifier(text: &str) -> String {
    text.chars()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
}

fn call_text_contains_named_call(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
        let after = &text[start + name.len()..];
        after.starts_with("::") || after.trim_start().starts_with('(')
    })
}

fn supported_helper_owner_call_name(
    call_name: &str,
    local_function_names: &BTreeSet<String>,
    external_owner_names: Option<&BTreeSet<String>>,
) -> bool {
    local_function_names.contains(call_name)
        || external_owner_names.is_some_and(|owner_names| owner_names.contains(call_name))
}

fn direct_delegate_parenthesized_macro_is_allowed(call_name: &str) -> bool {
    matches!(
        call_name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "assert_matches"
            | "dbg"
            | "format"
            | "format_args"
            | "matches"
    )
}

fn direct_delegate_container_macro_is_allowed(call_name: &str) -> bool {
    matches!(call_name, "vec")
}

fn direct_delegate_extra_call_is_inert(call_name: &str) -> bool {
    matches!(
        call_name,
        "clone"
            | "default"
            | "expect"
            | "format"
            | "from"
            | "into"
            | "new"
            | "to_string"
            | "trim"
            | "unwrap"
            | "Err"
            | "Ok"
            | "Some"
    )
}

fn direct_delegate_extra_call_is_allowed(candidate: &CallFact, owner_call: &CallFact) -> bool {
    direct_delegate_extra_call_is_inert(&candidate.name)
        || direct_delegate_parenthesized_macro_is_allowed(&candidate.name)
        || direct_delegate_container_macro_is_allowed(&candidate.name)
        || direct_delegate_post_owner_method_is_allowed(
            &candidate.name,
            &candidate.text,
            &owner_call.name,
        )
        || direct_delegate_std_identity_call_is_allowed(
            &candidate.name,
            &candidate.text,
            &owner_call.name,
        )
}

fn direct_delegate_post_owner_method_is_allowed(
    method_name: &str,
    text: &str,
    owner_name: &str,
) -> bool {
    if !matches!(method_name, "as_ref" | "cloned") {
        return false;
    }
    let cleaned = strip_comments_and_strings(text);
    cleaned.match_indices(owner_name).any(|(start, _)| {
        let before = cleaned[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
        let after_name = &cleaned[start + owner_name.len()..];
        let after_name = after_name.trim_start();
        if !after_name.starts_with('(') {
            return false;
        }
        let Some(close_index) = matching_close_paren(after_name) else {
            return false;
        };
        after_name[close_index + 1..]
            .trim_start()
            .starts_with(&format!(".{method_name}("))
    })
}

fn direct_delegate_std_identity_call_is_allowed(
    call_name: &str,
    text: &str,
    owner_name: &str,
) -> bool {
    if call_name != "identity" {
        return false;
    }
    let cleaned = strip_comments_and_strings(text);
    cleaned.match_indices(owner_name).any(|(start, _)| {
        let before = cleaned[..start].trim_end();
        if !direct_delegate_std_identity_prefix_is_allowed(before) {
            return false;
        }
        let before_owner = cleaned[..start].chars().next_back();
        if before_owner.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
        cleaned[start + owner_name.len()..]
            .trim_start()
            .starts_with('(')
    })
}

fn matching_close_paren(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn helper_name_carries_owner_token(helper_name_lower: &str, owner_name: &str) -> bool {
    let owner_name_lower = owner_name.to_ascii_lowercase();
    if !owner_token_is_specific_enough(&owner_name_lower) {
        return false;
    }
    helper_name_lower
        .match_indices(&owner_name_lower)
        .any(|(start, _)| {
            let before = helper_name_lower[..start].chars().next_back();
            let after = helper_name_lower[start + owner_name_lower.len()..]
                .chars()
                .next();
            is_helper_token_boundary(before) && is_helper_token_boundary(after)
        })
}

fn owner_token_is_specific_enough(owner_name_lower: &str) -> bool {
    owner_name_lower.contains('_')
        || (owner_name_lower.len() >= 8
            && !matches!(
                owner_name_lower,
                "builder" | "convert" | "fixture" | "helper" | "parse" | "render"
            ))
}

fn is_helper_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| ch == '_' || !ch.is_alphanumeric())
}

fn common_helper_owner_calls(
    helper_name: String,
    owner_sets: Vec<BTreeSet<String>>,
) -> Option<(String, BTreeSet<String>)> {
    let mut owner_sets = owner_sets.into_iter();
    let mut common = owner_sets.next()?;
    for owner_set in owner_sets {
        common = common.intersection(&owner_set).cloned().collect();
        if common.is_empty() {
            return None;
        }
    }
    Some((helper_name, common))
}

fn helper_owner_call_names_for_test(
    test: &TestSummary,
    call_names: &BTreeSet<String>,
    lookup: &HelperOwnerCallLookup<'_>,
    module_import_aliases: Option<&BTreeMap<String, String>>,
) -> BTreeSet<String> {
    let mut owner_names = helper_owner_call_names_from_qualified_calls(
        &test.calls,
        lookup.qualified_helpers,
        module_import_aliases,
    );
    let local_function_names = lookup.local_function_names_by_file.get(&test.file);
    owner_names.extend(helper_owner_call_names_from_direct_imported_helpers(
        &test.calls,
        lookup.direct_helper_import_aliases_by_file.get(&test.file),
        lookup.qualified_helpers,
        local_function_names,
    ));
    if let Some(file_helpers) = lookup.helpers.get(&test.file) {
        for helper_name in call_names {
            if let Some(helper_owner_names) = file_helpers.get(helper_name) {
                owner_names.extend(helper_owner_names.iter().cloned());
            }
            if let Some(helper_owner_names) = lookup.unique_helpers.get(helper_name) {
                owner_names.extend(helper_owner_names.iter().cloned());
            }
        }
    } else {
        owner_names.extend(helper_owner_call_names_from_unique_helpers(
            call_names,
            lookup.unique_helpers,
        ));
    }
    owner_names.extend(helper_owner_call_names_from_production_helpers(
        test,
        call_names,
        lookup.production_helpers,
        local_function_names,
    ));
    owner_names
}

fn helper_owner_call_names_from_direct_imported_helpers(
    calls: &[CallFact],
    direct_helper_import_aliases: Option<&BTreeMap<String, ImportedFunctionAlias>>,
    qualified_helpers: &HelperOwnerCallsByModulePath,
    local_function_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let Some(direct_helper_import_aliases) = direct_helper_import_aliases else {
        return BTreeSet::new();
    };
    let mut owner_names = BTreeSet::new();
    for call in calls {
        if local_function_names.is_some_and(|names| names.contains(&call.name)) {
            continue;
        }
        let cleaned = strip_comments_and_strings(&call.text);
        if !call_text_contains_named_call(&cleaned, &call.name) {
            continue;
        }
        let Some(imported) = direct_helper_import_aliases.get(&call.name) else {
            continue;
        };
        let Some(helpers) = qualified_helpers.get(&imported.module_path) else {
            continue;
        };
        if let Some(helper_owner_names) = helpers.get(&imported.name) {
            owner_names.extend(helper_owner_names.iter().cloned());
        }
    }
    owner_names
}

fn helper_owner_call_names_from_qualified_calls(
    calls: &[CallFact],
    qualified_helpers: &HelperOwnerCallsByModulePath,
    module_import_aliases: Option<&BTreeMap<String, String>>,
) -> BTreeSet<String> {
    let mut owner_names = BTreeSet::new();
    for call in calls {
        let cleaned = strip_comments_and_strings(&call.text);
        for (module_path, helpers) in qualified_helpers {
            let Some(helper_owner_names) = helpers.get(&call.name) else {
                continue;
            };
            if code_contains_qualified_helper_call(&cleaned, module_path, &call.name)
                || code_contains_aliased_module_helper_call(
                    &cleaned,
                    module_path,
                    &call.name,
                    module_import_aliases,
                )
            {
                owner_names.extend(helper_owner_names.iter().cloned());
            }
        }
    }
    owner_names
}

fn code_contains_aliased_module_helper_call(
    code: &str,
    module_path: &str,
    helper_name: &str,
    module_import_aliases: Option<&BTreeMap<String, String>>,
) -> bool {
    module_import_aliases.is_some_and(|aliases| {
        aliases.iter().any(|(alias, imported_module_path)| {
            imported_module_path == module_path
                && code_contains_qualified_helper_call(code, alias, helper_name)
        })
    })
}

fn code_contains_qualified_helper_call(code: &str, module_path: &str, helper_name: &str) -> bool {
    ["", "crate::", "self::", "super::"]
        .into_iter()
        .any(|prefix| {
            code_contains_qualified_helper_call_with_prefix(code, prefix, module_path, helper_name)
        })
}

fn code_contains_qualified_helper_call_with_prefix(
    code: &str,
    prefix: &str,
    module_path: &str,
    helper_name: &str,
) -> bool {
    let pattern = format!("{prefix}{module_path}::{helper_name}(");
    code.match_indices(&pattern).any(|(start, _)| {
        code[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !is_rust_path_identifier_char(before))
    })
}

fn is_rust_path_identifier_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch.is_ascii_alphanumeric()
}

fn helper_owner_call_names_from_unique_helpers(
    call_names: &BTreeSet<String>,
    unique_helpers: &HelperOwnerCallsByName,
) -> BTreeSet<String> {
    call_names
        .iter()
        .filter_map(|helper_name| unique_helpers.get(helper_name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

fn helper_owner_call_names_from_production_helpers(
    test: &TestSummary,
    call_names: &BTreeSet<String>,
    production_helpers: &HelperOwnerCallsByPackage,
    local_function_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let Some(package) = package_scope(&test.file) else {
        return BTreeSet::new();
    };
    let Some(package_helpers) = production_helpers.get(&package) else {
        return BTreeSet::new();
    };
    call_names
        .iter()
        .filter(|helper_name| {
            !local_function_names.is_some_and(|names| names.contains(*helper_name))
        })
        .filter_map(|helper_name| package_helpers.get(helper_name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

fn helper_owner_call_names_from_same_file_unit_production_helpers(
    test: &TestSummary,
    production_helpers: &HelperOwnerCallsByPackage,
    local_function_names: Option<&BTreeSet<String>>,
    test_scoped_function_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let Some(local_function_names) = local_function_names else {
        return BTreeSet::new();
    };
    let Some(package) = package_scope(&test.file) else {
        return BTreeSet::new();
    };
    let Some(package_helpers) = production_helpers.get(&package) else {
        return BTreeSet::new();
    };
    test.calls
        .iter()
        .filter(|call| local_function_names.contains(&call.name))
        .filter(|call| {
            same_file_unit_production_helper_call_is_allowed(call, test_scoped_function_names)
        })
        .filter_map(|call| package_helpers.get(&call.name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

fn same_file_unit_production_helper_call_is_allowed(
    call: &CallFact,
    test_scoped_function_names: Option<&BTreeSet<String>>,
) -> bool {
    let cleaned = strip_comments_and_strings(&call.text);
    if code_contains_parent_qualified_helper_call(&cleaned, &call.name) {
        return true;
    }
    !test_scoped_function_names.is_some_and(|names| names.contains(&call.name))
        && call_text_contains_named_call(&cleaned, &call.name)
}

fn code_contains_parent_qualified_helper_call(code: &str, helper_name: &str) -> bool {
    let pattern = format!("super::{helper_name}(");
    code.match_indices(&pattern).any(|(start, _)| {
        code[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !is_rust_path_identifier_char(before))
    })
}

/// Why this test is related to the seam. v1: a single highest-priority
/// reason per test (no multi-reason public shape). Priority is pinned
/// by `RelationReason::priority` and exercised by ranking tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RelationReason {
    DirectOwnerCall,
    HelperOwnerCall,
    AssertionTargetAffinity,
    SameTestFile,
    SameModule,
    OwnerNamedTest,
    ImportPathAffinity,
    FixtureOwnerAffinity,
}

impl RelationReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DirectOwnerCall => "direct_owner_call",
            Self::HelperOwnerCall => "helper_owner_call",
            Self::AssertionTargetAffinity => "assertion_target_affinity",
            Self::SameTestFile => "same_test_file",
            Self::SameModule => "same_module",
            Self::OwnerNamedTest => "owner_named_test",
            Self::ImportPathAffinity => "import_path_affinity",
            Self::FixtureOwnerAffinity => "fixture_owner_affinity",
        }
    }

    /// Lower value sorts first. Stable contract pinned by tests.
    fn priority(self) -> u8 {
        match self {
            Self::DirectOwnerCall => 0,
            Self::HelperOwnerCall => 1,
            Self::AssertionTargetAffinity => 2,
            Self::SameTestFile => 3,
            Self::SameModule => 4,
            Self::OwnerNamedTest => 5,
            Self::ImportPathAffinity => 6,
            Self::FixtureOwnerAffinity => 7,
        }
    }

    fn confidence(self) -> RelationConfidence {
        match self {
            Self::DirectOwnerCall | Self::HelperOwnerCall => RelationConfidence::High,
            Self::AssertionTargetAffinity => RelationConfidence::Medium,
            Self::SameTestFile
            | Self::SameModule
            | Self::OwnerNamedTest
            | Self::ImportPathAffinity => RelationConfidence::Medium,
            Self::FixtureOwnerAffinity => RelationConfidence::Low,
        }
    }
}

/// Confidence that the related test grips the seam. Independent of
/// oracle strength: a `Low` relation can still carry a strong oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RelationConfidence {
    High,
    Medium,
    Low,
    Opaque,
}

impl RelationConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Opaque => "opaque",
        }
    }

    /// Lower value sorts first (highest confidence first).
    fn rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
            Self::Opaque => 3,
        }
    }
}

/// Build evidence records for a slice of seams. Output is sorted by
/// `seam_id` so two runs over the same input produce identical bytes.
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
        .map(|(indexed, reason)| related_test_grip(seam, indexed.test, *reason))
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

/// Walk `index.tests` and return tests that plausibly relate to `seam`,
/// each tagged with the single highest-priority `RelationReason` it
/// satisfies. The two-step "match then rank" replaces the old binary
/// `calls_owner || same_file_or_named` check from earlier campaigns.
///
/// Detection per reason — strict ordering: the first reason that fires
/// wins, so e.g. a test that both `calls owner` and `is in same file`
/// carries `direct_owner_call`, never `same_test_file`.
fn find_related_tests_with_context<'context, 'index>(
    seam: &RepoSeam,
    context: &'context CompactGripContext<'index>,
) -> Vec<(&'context CompactTest<'index>, RelationReason)> {
    let owner = OwnerContext::resolve(seam, context);
    let target_tokens = assertion_target_tokens(seam);
    let prefix = owner.prefix.as_deref();

    let mut candidates: BTreeMap<usize, RelationReason> = BTreeMap::new();
    match_direct_owner_call(&mut candidates, context, prefix, &owner);
    match_helper_owner_call(&mut candidates, context, prefix, &owner);
    match_target_affinity_owner_call(
        &mut candidates,
        seam,
        context,
        prefix,
        &owner,
        &target_tokens,
    );
    match_assertion_target_affinity(&mut candidates, context, prefix, &target_tokens);
    match_same_test_file(&mut candidates, context, prefix, &owner);
    match_same_module(&mut candidates, context, prefix, &owner);
    match_owner_named_test(&mut candidates, context, prefix, &owner);
    match_import_path_affinity(&mut candidates, context, prefix, &owner);
    match_fixture_owner_affinity(&mut candidates, context, prefix, &owner);

    dedupe_related_candidates(candidates, context)
}

/// Cached facts about a seam's owning function that every relationship
/// strategy reads from. Resolved once per seam to avoid re-walking the
/// index and re-deriving file stems / module paths per strategy.
struct OwnerContext {
    name: String,
    name_lower: String,
    file_stem: String,
    module_path: Option<String>,
    prefix: Option<String>,
    fixture_names: BTreeSet<String>,
}

impl OwnerContext {
    fn resolve(seam: &RepoSeam, context: &CompactGripContext<'_>) -> Self {
        let owner_fn = find_owner_function(seam, context.index);
        let name = owner_fn.map(|f| f.name.as_str()).unwrap_or("").to_string();
        let name_lower = name.to_ascii_lowercase();
        let owner_file = owner_fn.map(|f| f.file.as_path());
        let file_stem = owner_file
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let module_path = owner_file.and_then(module_path_for);
        let prefix = owner_fn.and_then(|f| package_prefix(&f.file));
        let fixture_names = owner_file
            .and_then(|file| context.index.files.get(file))
            .map(fixture_names_for_owner_file)
            .unwrap_or_default();
        Self {
            name,
            name_lower,
            file_stem,
            module_path,
            prefix,
            fixture_names,
        }
    }
}

/// Tokens from `RequiredDiscriminator` and `ExpectedSink` that an
/// `assertion_target_affinity` match must mention. Already filtered
/// through `extract_identifier_tokens`, so stop-words and short tokens
/// are excluded.
fn assertion_target_tokens(seam: &RepoSeam) -> BTreeSet<String> {
    let discriminator_tokens = match seam.required_discriminator() {
        super::seams::RequiredDiscriminator::MatchArmTaken { arm } => {
            match_arm_assertion_target_tokens(arm)
        }
        _ if seam.kind() == SeamKind::CallPresence => {
            call_presence_callee_target_tokens(required_discriminator_text(seam))
        }
        _ => required_discriminator_tokens(seam),
    };
    let sink_tokens = if seam.kind() == SeamKind::MatchArm {
        Vec::new()
    } else {
        extract_identifier_tokens(seam.expected_sink().as_str())
    };
    let filters_generic_call_tokens = seam.kind() == SeamKind::CallPresence;
    discriminator_tokens
        .into_iter()
        .chain(sink_tokens)
        .filter(|token| {
            !filters_generic_call_tokens
                || call_presence_assertion_affinity_token_is_specific_enough(token)
        })
        .collect()
}

fn match_arm_assertion_target_tokens(arm: &str) -> Vec<String> {
    extract_identifier_tokens(arm)
        .into_iter()
        .filter(|token| match_arm_assertion_target_token_is_specific_enough(token))
        .collect()
}

fn match_arm_assertion_target_token_is_specific_enough(token: &str) -> bool {
    token.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn call_presence_callee_target_tokens(expression: &str) -> Vec<String> {
    let mut tokens = extract_call_facts(expression, 0)
        .into_iter()
        .flat_map(|call| extract_identifier_tokens(&call.name))
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn call_presence_assertion_affinity_token_is_specific_enough(token: &str) -> bool {
    if matches!(
        token,
        "arg"
            | "args"
            | "arm"
            | "class"
            | "clone"
            | "context"
            | "count"
            | "counts"
            | "data"
            | "dedup"
            | "description"
            | "entry"
            | "evidence"
            | "field"
            | "file"
            | "files"
            | "from"
            | "input"
            | "is_empty"
            | "iter"
            | "item"
            | "items"
            | "kind"
            | "line"
            | "lines"
            | "missing"
            | "model"
            | "name"
            | "owner"
            | "output"
            | "path"
            | "paths"
            | "probe"
            | "result"
            | "results"
            | "side_effect"
            | "sink"
            | "sort"
            | "summary"
            | "target"
            | "test"
            | "tests"
            | "text"
            | "u64"
            | "value"
            | "values"
            | "variant"
            | "byte"
            | "bytes"
    ) {
        return false;
    }
    true
}

fn match_direct_owner_call(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    if owner.name.is_empty() {
        return;
    }
    let Some(indices) = context.tests_by_call_name.get(&owner.name) else {
        return;
    };
    for test_index in indices {
        insert_related_candidate(
            candidates,
            context,
            prefix,
            *test_index,
            RelationReason::DirectOwnerCall,
        );
    }
}

fn match_helper_owner_call(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    if owner.name.is_empty() {
        return;
    }
    let Some(indices) = context.tests_by_helper_owner_call_name.get(&owner.name) else {
        return;
    };
    for test_index in indices {
        insert_related_candidate(
            candidates,
            context,
            prefix,
            *test_index,
            RelationReason::HelperOwnerCall,
        );
    }
}

fn match_target_affinity_owner_call(
    candidates: &mut BTreeMap<usize, RelationReason>,
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
    target_tokens: &BTreeSet<String>,
) {
    if requires_concrete_activation_values(seam)
        || owner.name.is_empty()
        || target_tokens.is_empty()
    {
        return;
    }
    for (test_index, test) in context.tests.iter().enumerate() {
        if !test
            .target_affinity_owner_call_names
            .contains(owner.name.as_str())
            || !test_assertion_mentions_any_target_token(test, target_tokens)
        {
            continue;
        }
        insert_related_candidate(
            candidates,
            context,
            prefix,
            test_index,
            RelationReason::HelperOwnerCall,
        );
    }
}

fn test_assertion_mentions_any_target_token(
    test: &CompactTest<'_>,
    target_tokens: &BTreeSet<String>,
) -> bool {
    target_tokens
        .iter()
        .any(|token| test.assertion_tokens.contains(token))
}

fn match_assertion_target_affinity(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    target_tokens: &BTreeSet<String>,
) {
    for token in target_tokens {
        if let Some(indices) = context.tests_by_assertion_token.get(token) {
            for test_index in indices {
                insert_related_candidate(
                    candidates,
                    context,
                    prefix,
                    *test_index,
                    RelationReason::AssertionTargetAffinity,
                );
            }
        }
    }
}

fn match_same_test_file(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    if owner.file_stem.is_empty() {
        return;
    }
    let stems = [
        owner.file_stem.clone(),
        format!("{}_test", owner.file_stem),
        format!("{}_tests", owner.file_stem),
    ];
    for stem in stems {
        if let Some(indices) = context.tests_by_file_stem.get(&stem) {
            for test_index in indices {
                insert_related_candidate(
                    candidates,
                    context,
                    prefix,
                    *test_index,
                    RelationReason::SameTestFile,
                );
            }
        }
    }
}

fn match_same_module(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    let Some(module_path) = owner.module_path.as_deref() else {
        return;
    };
    for test_index in context.same_module_indices(module_path) {
        insert_related_candidate(
            candidates,
            context,
            prefix,
            test_index,
            RelationReason::SameModule,
        );
    }
}

fn match_owner_named_test(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    for test_index in context.owner_named_indices(&owner.name_lower) {
        insert_related_candidate(
            candidates,
            context,
            prefix,
            test_index,
            RelationReason::OwnerNamedTest,
        );
    }
}

fn match_import_path_affinity(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    if owner.name.is_empty() {
        return;
    }
    let Some(indices) = context.tests_by_import_token.get(&owner.name) else {
        return;
    };
    for test_index in indices {
        if !context
            .tests
            .get(*test_index)
            .is_some_and(|indexed| test_imports_owner_compact(indexed, &owner.name))
        {
            continue;
        }
        insert_related_candidate(
            candidates,
            context,
            prefix,
            *test_index,
            RelationReason::ImportPathAffinity,
        );
    }
}

fn match_fixture_owner_affinity(
    candidates: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    owner: &OwnerContext,
) {
    for fixture_name in &owner.fixture_names {
        if let Some(indices) = context.tests_by_call_name.get(fixture_name) {
            for test_index in indices {
                insert_related_candidate(
                    candidates,
                    context,
                    prefix,
                    *test_index,
                    RelationReason::FixtureOwnerAffinity,
                );
            }
        }
    }
}

fn dedupe_related_candidates<'context, 'index>(
    candidates: BTreeMap<usize, RelationReason>,
    context: &'context CompactGripContext<'index>,
) -> Vec<(&'context CompactTest<'index>, RelationReason)> {
    let mut related: Vec<(&'context CompactTest<'index>, RelationReason)> = Vec::new();
    let mut seen: std::collections::HashSet<(String, PathBuf, usize)> =
        std::collections::HashSet::new();

    for (test_index, reason) in candidates {
        let Some(indexed) = context.tests.get(test_index) else {
            continue;
        };
        let key = (
            indexed.test.name.clone(),
            indexed.test.file.clone(),
            indexed.test.start_line,
        );
        if seen.insert(key) {
            related.push((indexed, reason));
        }
    }
    related
}

fn insert_related_candidate(
    candidate_reasons: &mut BTreeMap<usize, RelationReason>,
    context: &CompactGripContext<'_>,
    prefix: Option<&str>,
    test_index: usize,
    reason: RelationReason,
) {
    if candidate_reasons.contains_key(&test_index) {
        return;
    }
    let Some(indexed) = context.tests.get(test_index) else {
        return;
    };
    if let Some(prefix) = prefix
        && !indexed.path_normalized.starts_with(prefix)
    {
        return;
    }
    candidate_reasons.insert(test_index, reason);
}

fn find_related_tests_compact<'a>(
    seam: &RepoSeam,
    context: &'a CompactGripContext<'_>,
) -> Vec<&'a CompactTest<'a>> {
    let mut related = find_related_tests_with_context(seam, context);
    sort_related_tests_for_seam(seam, context, &mut related);
    related
        .into_iter()
        .take(COMPACT_RELATED_TEST_LIMIT)
        .map(|(indexed, _reason)| indexed)
        .collect()
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RelatedTestRankKey {
    relation_confidence: u8,
    relation_reason: u8,
    oracle_strength: Reverse<u8>,
    activation_overlap: Reverse<usize>,
    file: PathBuf,
    test_name: String,
    line: usize,
}

fn sort_related_tests_for_seam(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
    related: &mut [(&CompactTest<'_>, RelationReason)],
) {
    related.sort_by_cached_key(|entry| {
        let (indexed, reason) = *entry;
        related_test_rank_key(seam, context, indexed, reason)
    });
}

fn related_test_rank_key(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
    indexed: &CompactTest<'_>,
    reason: RelationReason,
) -> RelatedTestRankKey {
    let (_oracle_kind, oracle_strength) = best_oracle(indexed.test, seam);
    RelatedTestRankKey {
        relation_confidence: reason.confidence().rank(),
        relation_reason: reason.priority(),
        oracle_strength: Reverse(oracle_strength.rank()),
        activation_overlap: Reverse(activation_overlap_score(seam, context, indexed)),
        file: indexed.test.file.clone(),
        test_name: indexed.test.name.clone(),
        line: indexed.test.start_line,
    }
}

fn fixture_names_for_owner_file(facts: &rust_index::FileFacts) -> BTreeSet<String> {
    facts
        .functions
        .iter()
        .filter(|f| !f.is_test && (is_fixture_named(&f.name) || f.body.contains("#[fixture]")))
        .map(|f| f.name.clone())
        .collect()
}

/// Tokens drawn from a `RepoSeam`'s `RequiredDiscriminator`. Filtered
/// through `extract_identifier_tokens` so common short words and
/// stop-tokens are already excluded.
fn required_discriminator_tokens(seam: &RepoSeam) -> Vec<String> {
    extract_identifier_tokens(required_discriminator_text(seam))
}

fn required_discriminator_text(seam: &RepoSeam) -> &str {
    use super::seams::RequiredDiscriminator;
    match seam.required_discriminator() {
        RequiredDiscriminator::BoundaryValue { description }
        | RequiredDiscriminator::ReturnValue { description } => description.as_str(),
        RequiredDiscriminator::ErrorVariant { variant } => variant.as_str(),
        RequiredDiscriminator::FieldValue { field } => field.as_str(),
        RequiredDiscriminator::Effect { sink } => sink.as_str(),
        RequiredDiscriminator::MatchArmTaken { arm } => arm.as_str(),
        RequiredDiscriminator::CallSite { target } => target.as_str(),
    }
}

/// Token-aware: does any assertion text in `test` contain at least one
/// of `tokens` as a whole identifier? Substring match would let
/// `discount` accidentally match `discount_threshold`; we want exact
/// identifier hits.
#[cfg(test)]
fn assertion_targets_seam(test: &TestSummary, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    for assertion in &test.assertions {
        let assertion_tokens = extract_identifier_tokens(&assertion.text);
        if assertion_tokens
            .iter()
            .any(|at| tokens.iter().any(|t| at == t))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
fn same_test_file(test_file: &Path, owner_stem: &str) -> bool {
    let stem = match test_file.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };
    if stem == owner_stem {
        return true;
    }
    // Suffix check avoids the allocation that `stem == format!("{owner_stem}_test")`
    // would do per call. Two suffix variants cover the common naming
    // conventions: `*_test.rs` and `*_tests.rs`.
    if let Some(prefix) = stem.strip_suffix("_test")
        && prefix == owner_stem
    {
        return true;
    }
    if let Some(prefix) = stem.strip_suffix("_tests")
        && prefix == owner_stem
    {
        return true;
    }
    false
}

/// Module path slug for a Rust source file: the path components below
/// `src/` or `tests/`, joined by `/`, dropping the file extension.
/// Returns `None` for files that do not sit under one of those roots.
/// Examples (Unix-style after normalize):
/// - `crates/ripr/src/auth/login.rs` → `auth/login`
/// - `tests/cli_smoke.rs`            → `cli_smoke`
fn module_path_for(file: &Path) -> Option<String> {
    let normalized = normalize_path(file);
    let body = normalized
        .rfind("/src/")
        .map(|idx| &normalized[idx + "/src/".len()..])
        .or_else(|| {
            normalized
                .rfind("/tests/")
                .map(|idx| &normalized[idx + "/tests/".len()..])
        })
        .or_else(|| normalized.strip_prefix("src/"))
        .or_else(|| normalized.strip_prefix("tests/"))?;
    let trimmed = body.strip_suffix(".rs").unwrap_or(body);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Two files share a module if any non-leaf segment of the owner's
/// module path appears as a prefix of the test's module path. The leaf
/// stem is excluded so this does not duplicate `same_test_file`.
fn same_module(owner_module: &str, test_module: &str) -> bool {
    let parent = match owner_module.rsplit_once('/') {
        Some((parent, _leaf)) => parent,
        None => return false,
    };
    if parent.is_empty() {
        return false;
    }
    test_module == parent
        || test_module.starts_with(&format!("{parent}/"))
        || test_module.starts_with(&format!("{}/", parent.replace('/', "_")))
}

/// Body mentions the owner via an explicit qualified-path or `use`
/// shape — without calling it. The direct-call check has already
/// excluded callers, so this fires for tests that import the symbol
/// (or qualify it via a path) but route through some wrapper (common
/// in integration tests).
///
/// Tightened per #310 review: pure token co-occurrence
/// (owner_name appearing as a bare identifier somewhere in the body)
/// was too easy to satisfy with local bindings, comments, or
/// unrelated identifiers. The detector now requires either:
///
/// 1. a `module::owner_name` qualified path anywhere in the body
///    (catches `crate::pricing::discounted_total`,
///    `super::pricing::discounted_total`, `pricing::discounted_total`
///    — they all contain `::owner_name`); or
/// 2. an inline `use ... owner_name` line in the test body. File-
///    scope `use` lines are not in `test.body` so this only covers
///    in-function imports.
fn test_imports_owner_compact(test: &CompactTest<'_>, owner_name: &str) -> bool {
    if owner_name.is_empty() {
        return false;
    }
    let qualified = format!("::{owner_name}");
    for code in &test.code_lines {
        if code.contains(&qualified) {
            return true;
        }
        if code.trim_start().starts_with("use ")
            && extract_identifier_tokens(code)
                .iter()
                .any(|token| token == owner_name)
        {
            return true;
        }
    }
    false
}

fn import_affinity_tokens(code_lines: &[String]) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for code in code_lines {
        let trimmed = code.trim_start();
        if code.contains("::") || trimmed.starts_with("use ") {
            tokens.extend(extract_identifier_tokens(code));
        }
    }
    tokens
}

/// Drop everything after a `//` line comment and replace string-literal
/// contents with empty strings. v1 best-effort: handles `"..."` with
/// `\\` and `\"` escapes; raw strings (`r#"..."#`), char literals
/// (`'a'`), and block comments (`/* ... */`) are out of scope — those
/// shapes are rare inside test bodies and treating them as code is a
/// safe over-match (the previous helper accepted them all).
fn strip_comments_and_strings(line: &str) -> String {
    // Strip `//` line comments first; everything after is non-code.
    let without_comment = match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    };
    let mut out = String::with_capacity(without_comment.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in without_comment.chars() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        out.push(ch);
    }
    out
}

fn is_fixture_named(name: &str) -> bool {
    let prefixes = ["fixture_", "setup_", "make_", "build_", "new_", "mock_"];
    let suffixes = ["_fixture", "_factory"];
    prefixes.iter().any(|p| name.starts_with(p)) || suffixes.iter().any(|s| name.ends_with(s))
}

fn find_owner_function<'a>(seam: &RepoSeam, index: &'a RustIndex) -> Option<&'a FunctionSummary> {
    rust_index::find_owner_function(index, seam.file(), seam.display_line())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn package_prefix(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if prefix.is_empty() {
                return None;
            }
            return Some(format!("{prefix}/"));
        }
    }
    None
}

fn package_scope(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(prefix) = package_prefix(path) {
        return Some(prefix);
    }
    if normalized.starts_with("src/") || normalized.starts_with("tests/") {
        return Some(String::new());
    }
    None
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

    let state = if related.is_empty() {
        StageState::No
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
        if !observed.is_empty() {
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
    let state = if related.is_empty() {
        StageState::No
    } else if direct_owner_call {
        StageState::Yes
    } else {
        StageState::Unknown
    };
    let stage = StageEvidence::new(
        state.clone(),
        if direct_owner_call {
            Confidence::Medium
        } else {
            Confidence::Low
        },
        format!(
            "Compact activation evidence for seam `{}` is `{}`",
            seam.expression()
                .lines()
                .next()
                .unwrap_or(seam.expression()),
            state.as_str()
        ),
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
    let mut best_kind_matches_seam = false;
    for test in related {
        for oracle in &test.assertions {
            if oracle.strength.rank() > best.rank() {
                best = oracle.strength.clone();
            }
            if oracle_kind_matches_seam(seam, &oracle.kind) {
                best_kind_matches_seam = true;
            }
        }
    }
    let state = match (best_kind_matches_seam, &best) {
        (_, OracleStrength::None) => StageState::No,
        (_, OracleStrength::Unknown) => StageState::Unknown,
        (_, OracleStrength::Weak | OracleStrength::Smoke) => StageState::Weak,
        (true, OracleStrength::Strong | OracleStrength::Medium) => StageState::Yes,
        (false, OracleStrength::Strong | OracleStrength::Medium) => StageState::Weak,
    };
    let summary = format!(
        "Strongest oracle for seam kind `{}` is `{}` (kind-match {})",
        seam.kind().as_str(),
        best.as_str(),
        best_kind_matches_seam
    );
    StageEvidence::new(state, Confidence::Medium, summary)
}

fn oracle_kind_matches_seam(seam: &RepoSeam, oracle: &OracleKind) -> bool {
    match seam.kind() {
        SeamKind::PredicateBoundary
        | SeamKind::ReturnValue
        | SeamKind::MatchArm
        | SeamKind::FieldConstruction => matches!(
            oracle,
            OracleKind::ExactValue
                | OracleKind::WholeObjectEquality
                | OracleKind::Snapshot
                | OracleKind::RelationalCheck
        ),
        SeamKind::ErrorVariant => matches!(oracle, OracleKind::ExactErrorVariant),
        SeamKind::SideEffect | SeamKind::CallPresence => {
            matches!(oracle, OracleKind::MockExpectation)
        }
    }
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
        oracle_kind: kind,
        oracle_strength: strength,
        evidence_summary: summary,
        relation_reason: reason,
        relation_confidence: confidence,
    }
}

fn best_oracle(test: &TestSummary, seam: &RepoSeam) -> (OracleKind, OracleStrength) {
    let mut best_kind = OracleKind::Unknown;
    let mut best_strength = OracleStrength::None;
    for oracle in &test.assertions {
        if oracle.strength.rank() > best_strength.rank() {
            best_strength = oracle.strength.clone();
            best_kind = oracle.kind.clone();
        } else if oracle.strength.rank() == best_strength.rank()
            && oracle_kind_matches_seam(seam, &oracle.kind)
        {
            best_kind = oracle.kind.clone();
        }
    }
    (best_kind, best_strength)
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
mod tests {
    use super::*;
    use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use crate::analysis::seam_inventory::inventory_seams_from_index;

    fn index_from_files(files: &[(PathBuf, &str)]) -> Result<RustIndex, String> {
        let adapter = RaRustSyntaxAdapter;
        let mut index = RustIndex::default();
        for (path, source) in files {
            let facts = adapter.summarize_file(path, source)?;
            index.tests.extend(facts.tests.iter().cloned());
            index.functions.extend(facts.functions.iter().cloned());
            index.files.insert(path.clone(), facts);
        }
        Ok(index)
    }

    #[test]
    fn call_arguments_uses_identifier_boundary_for_callee_name() {
        let text = "fn borrowed_amount_matches() { let amount = 100; let actual = amount_matches(&amount); }";

        assert_eq!(
            call_arguments(text, "amount_matches"),
            Some(vec!["&amount".to_string()])
        );
    }

    #[test]
    fn latency_trace_line_uses_repo_exposure_trace_shape() {
        let line = latency_trace_line(
            "evidence_for_seams_progress",
            "processed_500_of_12337",
            Duration::from_millis(42),
        );

        assert_eq!(
            line,
            "ripr_repo_exposure_latency phase=evidence_for_seams_progress status=processed_500_of_12337 duration_ms=42"
        );
    }

    #[test]
    fn latency_trace_line_can_report_evidence_context_start() {
        let line = latency_trace_line("evidence_context", "start_seams_12337", Duration::ZERO);

        assert_eq!(
            line,
            "ripr_repo_exposure_latency phase=evidence_context status=start_seams_12337 duration_ms=0"
        );
    }

    #[test]
    fn oracle_semantics_explains_broad_error_gap_and_upgrade() {
        let semantics = oracle_semantics_for(
            &OracleKind::BroadError,
            &OracleStrength::Weak,
            SeamKind::ErrorVariant,
        );

        assert_eq!(semantics.observes, "some error occurred");
        assert_eq!(
            semantics.missing,
            "the exact error variant or payload that would discriminate the changed behavior"
        );
        assert_eq!(
            semantics.upgrade_suggestion.as_deref(),
            Some("assert the exact error variant with matches! or assert_matches!")
        );
    }

    #[test]
    fn oracle_semantics_explains_smoke_only_boundary_gap() {
        let semantics = oracle_semantics_for(
            &OracleKind::SmokeOnly,
            &OracleStrength::Smoke,
            SeamKind::PredicateBoundary,
        );

        assert_eq!(
            semantics.observes,
            "the call completed or returned a broad ok/some/none shape"
        );
        assert_eq!(
            semantics.missing,
            "the output value, error variant, field, effect, or call discriminator"
        );
        assert_eq!(
            semantics.upgrade_suggestion.as_deref(),
            Some("add an exact returned-value assertion at the missing boundary value")
        );
    }

    #[test]
    fn oracle_semantics_keeps_exact_value_without_extra_upgrade() {
        let semantics = oracle_semantics_for(
            &OracleKind::ExactValue,
            &OracleStrength::Strong,
            SeamKind::ReturnValue,
        );

        assert_eq!(
            semantics.observes,
            "the exact value or value pattern asserted by the test"
        );
        assert_eq!(
            semantics.missing,
            "no obvious value-shape discriminator gap under static scope"
        );
        assert!(semantics.upgrade_suggestion.is_none());
    }

    #[test]
    fn oracle_semantics_covers_supported_oracle_families() {
        let cases = [
            (
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
                SeamKind::ErrorVariant,
                "the exact error variant",
                Some(
                    "assert the payload inside the matched error variant when payload behavior changed",
                ),
            ),
            (
                OracleKind::WholeObjectEquality,
                OracleStrength::Strong,
                SeamKind::ReturnValue,
                "whole output object equality",
                None,
            ),
            (
                OracleKind::Snapshot,
                OracleStrength::Medium,
                SeamKind::ReturnValue,
                "a snapshot of rendered or debug output",
                Some(
                    "add an exact assertion for the changed field or value when the snapshot is broad",
                ),
            ),
            (
                OracleKind::RelationalCheck,
                OracleStrength::Weak,
                SeamKind::MatchArm,
                "a partial relationship or broad predicate about the result",
                Some("assert the exact enum or value produced by the changed match arm"),
            ),
            (
                OracleKind::MockExpectation,
                OracleStrength::Medium,
                SeamKind::SideEffect,
                "an expected call, event, state write, or persistence effect",
                None,
            ),
            (
                OracleKind::Unknown,
                OracleStrength::Unknown,
                SeamKind::CallPresence,
                "no recognized concrete oracle shape",
                Some("assert the expected call happened with the relevant arguments"),
            ),
            (
                OracleKind::Unknown,
                OracleStrength::None,
                SeamKind::FieldConstruction,
                "no recognized test oracle",
                Some("assert the specific output field that carries the changed behavior"),
            ),
        ];

        for (kind, strength, seam_kind, observes, upgrade) in cases {
            let semantics = oracle_semantics_for(&kind, &strength, seam_kind);
            assert_eq!(semantics.observes, observes);
            assert_eq!(semantics.upgrade_suggestion.as_deref(), upgrade);
        }
    }

    #[test]
    fn opaque_custom_assertion_helper_stays_unknown_oracle() -> Result<(), String> {
        let files: Vec<(PathBuf, &str)> = vec![
            (
                PathBuf::from("src/pricing.rs"),
                "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                 { if amount >= threshold { amount - 10 } else { amount } }\n",
            ),
            (
                PathBuf::from("tests/pricing_tests.rs"),
                "#[test]\n\
                 fn opaque_helper() {\n\
                     let total = discounted_total(100, 100);\n\
                     assert_discount_is_valid(&total);\n\
                 }\n",
            ),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "related test present".to_string())?;

        assert_eq!(first.oracle_kind, OracleKind::Unknown);
        assert_eq!(first.oracle_strength, OracleStrength::Unknown);
        assert_eq!(evidence.discriminate.state, StageState::Unknown);
        let semantics =
            oracle_semantics_for(&first.oracle_kind, &first.oracle_strength, predicate.kind());
        assert_eq!(semantics.observes, "no recognized concrete oracle shape");
        assert_eq!(
            semantics.upgrade_suggestion.as_deref(),
            Some("add an exact returned-value assertion at the missing boundary value")
        );
        Ok(())
    }

    #[test]
    fn duplicative_equality_assertion_stays_weak_oracle() -> Result<(), String> {
        let files: Vec<(PathBuf, &str)> = vec![
            (
                PathBuf::from("src/pricing.rs"),
                "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                 { if amount >= threshold { amount - 10 } else { amount } }\n",
            ),
            (
                PathBuf::from("tests/pricing_tests.rs"),
                "#[test]\n\
                 fn duplicated_equality() {\n\
                     let total = discounted_total(100, 100);\n\
                     assert_eq!(total, total);\n\
                 }\n",
            ),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "related test present".to_string())?;

        assert_eq!(first.oracle_kind, OracleKind::RelationalCheck);
        assert_eq!(first.oracle_strength, OracleStrength::Weak);
        assert_eq!(evidence.discriminate.state, StageState::Weak);
        let semantics =
            oracle_semantics_for(&first.oracle_kind, &first.oracle_strength, predicate.kind());
        assert_eq!(
            semantics.missing,
            "the exact changed value or boundary discriminator"
        );
        Ok(())
    }

    #[test]
    fn given_boundary_seam_when_tests_skip_equal_value_then_evidence_reports_missing_boundary_discriminator()
    -> Result<(), String> {
        // Production predicate compares amount >= threshold.
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        // Test calls owner with values strictly above and strictly below
        // the threshold but never with the equality case.
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let tests_src = r#"
#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(50, 100), 50);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(10000, 100), 9990);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "expected predicate seam".to_string())?;

        let evidence = evidence_for_seam(predicate, &index);
        if evidence.related_tests.is_empty() {
            return Err("expected reach evidence to find related tests".to_string());
        }
        if evidence.missing_discriminators.is_empty() {
            return Err(format!(
                "expected at least one missing-discriminator hypothesis for boundary seam `{}`",
                predicate.expression()
            ));
        }
        let mentions_threshold = evidence
            .missing_discriminators
            .iter()
            .any(|fact| fact.value.contains("threshold"));
        if !mentions_threshold {
            return Err(format!(
                "missing-discriminator hypothesis should name the boundary identifier; got {:?}",
                evidence
                    .missing_discriminators
                    .iter()
                    .map(|f| f.value.clone())
                    .collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    #[test]
    fn given_boundary_seam_when_test_uses_equal_value_and_exact_assertion_then_discriminate_evidence_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let tests_src = r#"
#[test]
fn equality_boundary_returns_discount() {
    assert_eq!(discounted_total(100, 100), 90);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "expected predicate seam".to_string())?;

        let evidence = evidence_for_seam(predicate, &index);
        if evidence.discriminate.state != StageState::Yes {
            return Err(format!(
                "expected discriminate=Yes, got {} ({})",
                evidence.discriminate.state.as_str(),
                evidence.discriminate.summary
            ));
        }
        assert!(
            evidence.missing_discriminators.is_empty(),
            "equal boundary arguments should satisfy the missing-discriminator hypothesis: {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_seam_when_parameter_operands_are_called_with_equal_strings_then_missing_discriminator_is_cleared()
    -> Result<(), String> {
        let prod = PathBuf::from("src/similarity.rs");
        let prod_src = r#"
pub fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    haystack == needle || haystack.contains(needle)
}
"#;
        let tests = PathBuf::from("tests/similarity_tests.rs");
        let tests_src = r#"
#[test]
fn exact_similarity_key_matches() {
    assert!(similarity_key_contains("apply_discount", "apply_discount"));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/similarity.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("haystack == needle")
            })
            .ok_or_else(|| "expected equality predicate seam".to_string())?;

        let evidence = evidence_for_seam(predicate, &index);

        assert!(
            evidence.missing_discriminators.is_empty(),
            "equal string boundary arguments should clear the missing-discriminator hypothesis: {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_error_variant_seam_when_test_only_asserts_is_err_then_discriminate_evidence_is_weak()
    -> Result<(), String> {
        let prod = PathBuf::from("src/parse.rs");
        let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
        let tests = PathBuf::from("tests/parse_tests.rs");
        let tests_src = r#"
#[test]
fn parse_rejects_empty() {
    assert!(parse("").is_err());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
        let error_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ErrorVariant)
            .ok_or_else(|| "expected error_variant seam".to_string())?;

        let evidence = evidence_for_seam(error_seam, &index);
        if evidence.discriminate.state != StageState::Weak
            && evidence.discriminate.state != StageState::Unknown
        {
            return Err(format!(
                "expected discriminate=Weak|Unknown for is_err-only oracle, got {}",
                evidence.discriminate.state.as_str()
            ));
        }
        Ok(())
    }

    #[test]
    fn given_error_variant_seam_when_test_asserts_exact_variant_then_discriminate_evidence_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/parse.rs");
        let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
        let tests = PathBuf::from("tests/parse_tests.rs");
        let tests_src = r#"
#[test]
fn parse_returns_revoked_token_on_empty() {
    assert!(matches!(parse(""), Err(AuthError::RevokedToken)));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
        let error_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ErrorVariant)
            .ok_or_else(|| "expected error_variant seam".to_string())?;

        let evidence = evidence_for_seam(error_seam, &index);
        if evidence.discriminate.state != StageState::Yes {
            return Err(format!(
                "expected discriminate=Yes for matches!(...AuthError::RevokedToken), got {} ({})",
                evidence.discriminate.state.as_str(),
                evidence.discriminate.summary
            ));
        }
        Ok(())
    }

    #[test]
    fn given_side_effect_seam_when_no_effect_observer_exists_then_observe_evidence_is_weak_or_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/publish.rs");
        // The production function calls `service.publish(...)` — a method
        // whose name matches `is_effect_call_name`, so the parser emits
        // a side_effect probe shape on the call site.
        let prod_src = r#"
pub struct Service;
pub struct Event;

impl Service {
    pub fn publish(&mut self, _event: Event) {}
}

pub fn publish_message(service: &mut Service, event: Event) {
    service.publish(event);
}
"#;
        let tests = PathBuf::from("tests/publish_tests.rs");
        // Test reaches `publish_message` but does not observe the
        // side-effect (no mock, no assertion that the publish happened).
        let tests_src = r#"
#[test]
fn publish_runs_without_panic() {
    let mut service = Service;
    publish_message(&mut service, Event);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/publish.rs")], &index);
        let side_effect = seams
            .iter()
            .find(|s| s.kind() == SeamKind::SideEffect)
            .ok_or_else(|| {
                format!(
                    "expected side_effect seam, got kinds: {:?}",
                    seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
                )
            })?;

        let evidence = evidence_for_seam(side_effect, &index);
        match evidence.observe.state {
            StageState::No | StageState::Weak | StageState::Unknown => Ok(()),
            other => Err(format!(
                "expected observe in {{No, Weak, Unknown}} for side-effect with no observer, got {}",
                other.as_str()
            )),
        }
    }

    #[test]
    fn given_side_effect_seam_when_event_assertion_exists_then_oracle_observes_effect()
    -> Result<(), String> {
        let prod = PathBuf::from("src/publish.rs");
        let prod_src = r#"
pub struct Service;
pub struct Event;

impl Service {
    pub fn publish(&mut self, _event: Event) {}
}

pub fn publish_message(service: &mut Service, event: Event) {
    service.publish(event);
}
"#;
        let tests = PathBuf::from("tests/publish_tests.rs");
        let tests_src = r#"
#[test]
fn publish_records_event() {
    let mut service = Service;
    publish_message(&mut service, Event);
    assert!(service.published_events().contains(&"message"));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/publish.rs")], &index);
        let side_effect = seams
            .iter()
            .find(|s| s.kind() == SeamKind::SideEffect)
            .ok_or_else(|| "expected side_effect seam".to_string())?;

        let evidence = evidence_for_seam(side_effect, &index);
        assert_eq!(evidence.observe.state, StageState::Yes);
        assert_eq!(evidence.propagate.state, StageState::Yes);
        assert_eq!(evidence.discriminate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.oracle_kind == OracleKind::MockExpectation)
        );
        Ok(())
    }

    #[test]
    fn given_opaque_helper_when_values_cannot_be_seen_then_evidence_records_static_limitation()
    -> Result<(), String> {
        // Test reaches the owner only through a helper, so no concrete
        // activation values are visible. Activation should not be Yes.
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let tests_src = r#"
fn make_input() -> (i32, i32) { (50, 100) }

#[test]
fn helper_path_runs() {
    let (a, t) = make_input();
    let _ = discounted_total(a, t);
    assert!(true);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "expected predicate seam".to_string())?;

        let evidence = evidence_for_seam(predicate, &index);
        if evidence.activate.state == StageState::Yes {
            return Err(format!(
                "expected activate != Yes for helper-supplied values, got {} ({})",
                evidence.activate.state.as_str(),
                evidence.activate.summary
            ));
        }
        Ok(())
    }

    #[test]
    fn evidence_for_seams_is_deterministic_across_input_order() -> Result<(), String> {
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let tests_src = r#"
#[test]
fn boundary_case() {
    assert_eq!(discounted_total(100, 100), 90);
}
#[test]
fn below_case() {
    assert_eq!(discounted_total(50, 100), 50);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let mut seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let forward_ids: Vec<String> = evidence_for_seams(&seams, &index)
            .iter()
            .map(|e| e.seam_id.as_str().to_string())
            .collect();
        seams.reverse();
        let reversed_ids: Vec<String> = evidence_for_seams(&seams, &index)
            .iter()
            .map(|e| e.seam_id.as_str().to_string())
            .collect();
        if forward_ids != reversed_ids {
            return Err(format!(
                "evidence order is not stable:\n  forward: {forward_ids:?}\n  reversed: {reversed_ids:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn evidence_for_seams_matches_single_seam_evidence_while_reusing_context() -> Result<(), String>
    {
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let tests_src = r#"
#[test]
fn equality_boundary_returns_discount() {
    assert_eq!(discounted_total(100, 100), 90);
}
#[test]
fn import_only_mentions_owner() {
    use crate::pricing::discounted_total;
    assert_eq!(1, 1);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let batch = evidence_for_seams(&seams, &index);

        for seam in &seams {
            let single = evidence_for_seam(seam, &index);
            let Some(from_batch) = batch.iter().find(|entry| entry.seam_id == *seam.id()) else {
                return Err(format!(
                    "batch evidence missing seam {}",
                    seam.id().as_str()
                ));
            };
            let single_json =
                serde_json::to_string(&single).map_err(|err| format!("encode single: {err}"))?;
            let batch_json =
                serde_json::to_string(from_batch).map_err(|err| format!("encode batch: {err}"))?;
            assert_eq!(single_json, batch_json);
        }
        Ok(())
    }

    #[test]
    fn given_compact_evidence_when_direct_owner_call_reaches_error_seam_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/parse.rs");
        let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
        let tests = PathBuf::from("tests/parse_tests.rs");
        let tests_src = r#"
#[test]
fn parse_rejects_empty() {
    assert!(parse("").is_err());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
        let error_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ErrorVariant)
            .ok_or_else(|| "expected error_variant seam".to_string())?;
        let context = CompactGripContext::new(&index);

        let evidence = compact_evidence_for_seam(error_seam, &context);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert_eq!(evidence.related_tests.len(), 0);
        assert_eq!(evidence.observed_values.len(), 0);
        assert_eq!(evidence.missing_discriminators.len(), 0);
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_no_arg_owner_call_reaches_return_seam_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
        let tests = PathBuf::from("tests/labels_tests.rs");
        let tests_src = r#"
#[test]
fn device_labels_start_empty() {
    assert!(device_labels().is_empty());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
            .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "no-arg activation should not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("direct owner call for value-insensitive seam"),
            "activation summary should explain the value-insensitive owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "return-value no-arg activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_multiline_no_arg_owner_call_reaches_return_seam_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
        let tests = PathBuf::from("tests/labels_tests.rs");
        let tests_src = r#"
#[test]
fn device_labels_start_empty() {
    let labels = device_labels(
    );
    assert!(labels.is_empty());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
            .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "multiline no-arg activation should not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("direct owner call for value-insensitive seam"),
            "activation summary should explain the value-insensitive owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "multiline value-insensitive direct owner calls must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_one_hop_helper_calls_owner_then_value_insensitive_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/labels.rs");
        let source_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}

fn exercise_device_labels() -> Vec<&'static str> {
    device_labels()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_reaches_device_labels() {
        let labels = exercise_device_labels();
        assert!(labels.is_empty());
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::ReturnValue
                    && s.owner().ends_with("::device_labels")
                    && s.expression().contains("Vec::new()")
            })
            .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected helper owner-call related test, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "one-hop helper activation should not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the helper owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "helper owner calls must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_one_hop_helper_does_not_call_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/labels.rs");
        let source_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}

fn exercise_device_labels() -> Vec<&'static str> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_value_contract_mentions_empty_output() {
        let return_value = exercise_device_labels();
        assert!(return_value.is_empty());
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::ReturnValue
                    && s.owner().ends_with("::device_labels")
                    && s.expression().contains("Vec::new()")
            })
            .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "helper that does not call the owner must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "activation summary should keep owner-call limitation, got {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_generic_helper_name_mentions_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/parser.rs");
        let source_src = r#"
pub fn parse() -> String {
    String::new()
}

fn parse_fixture() -> String {
    parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_checks_empty_output() {
        let parsed = parse_fixture();
        assert!(parsed.is_empty());
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parser.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::ReturnValue
                    && s.owner().ends_with("::parse")
                    && s.expression().contains("String::new()")
            })
            .ok_or_else(|| "expected String::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "generic helper-owner token must not get helper relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.observed_values.is_empty(),
            "generic helper route must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_value_insensitive_seam_when_only_affinity_related_then_activation_names_owner_call_limitation()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
#[test]
fn return_value_contract_mentions_empty_output() {
    // Text mentions like `device_labels(` are not owner calls.
    let note = "device_labels(";
    let return_value = Vec::<&str>::new();
    assert!(!note.is_empty());
    assert!(return_value.is_empty());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
            .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
            "expected assertion-target affinity related test, got {:?}",
            evidence.related_tests
        );
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::DirectOwnerCall),
            "string/comment owner mentions must not become direct owner-call evidence: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "activation summary should name the owner-call limitation, got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "affinity-only activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_value_insensitive_seam_when_multi_owner_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub const DEVICE_LABELS_EMPTY: &str = "empty";
pub const USER_LABELS_READY: &str = "ready";

pub fn device_label_status() -> &'static str {
    DEVICE_LABELS_EMPTY
}

pub fn user_label_status() -> &'static str {
    USER_LABELS_READY
}

pub fn exercise_statuses() -> (&'static str, &'static str) {
    (device_label_status(), user_label_status())
}
"#;
        let tests = PathBuf::from("tests/status_contract.rs");
        let tests_src = r#"
use labels::{exercise_statuses, DEVICE_LABELS_EMPTY};

#[test]
fn status_wrapper_observes_device_target() {
    let (return_value, _) = exercise_statuses();
    assert_eq!(return_value, DEVICE_LABELS_EMPTY);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::ReturnValue
                    && s.owner().ends_with("::device_label_status")
                    && s.expression().contains("DEVICE_LABELS_EMPTY")
            })
            .ok_or_else(|| "expected DEVICE_LABELS_EMPTY return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected target-affinity production wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the target-affinity owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "target-affinity wrapper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_value_insensitive_seam_when_multi_owner_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub const DEVICE_LABELS_EMPTY: &str = "empty";
pub const USER_LABELS_READY: &str = "ready";

pub fn device_label_status() -> &'static str {
    DEVICE_LABELS_EMPTY
}

pub fn user_label_status() -> &'static str {
    USER_LABELS_READY
}

pub fn exercise_statuses() -> (&'static str, &'static str) {
    (device_label_status(), user_label_status())
}
"#;
        let tests = PathBuf::from("tests/status_contract.rs");
        let tests_src = r#"
use labels::{exercise_statuses, USER_LABELS_READY};

#[test]
fn status_wrapper_observes_user_target() {
    let (_, user_status) = exercise_statuses();
    assert_eq!(user_status, USER_LABELS_READY);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::ReturnValue
                    && s.owner().ends_with("::device_label_status")
                    && s.expression().contains("DEVICE_LABELS_EMPTY")
            })
            .ok_or_else(|| "expected DEVICE_LABELS_EMPTY return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "other target token must not prove device owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "other-target wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_full_evidence_when_owner_call_with_opaque_args_reaches_return_seam_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/labels.rs");
        let prod_src = r#"
pub fn render_label(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/labels_tests.rs");
        let tests_src = r#"
fn fixture_label() -> String { "alpha".to_string() }

#[test]
fn render_label_matches_fixture() {
    let label = fixture_label();
    assert_eq!(render_label(&label), "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
        let return_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("to_string"))
            .ok_or_else(|| "expected to_string return_value seam".to_string())?;

        let evidence = evidence_for_seam(return_seam, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "opaque direct-call arguments must not become observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("direct owner call for value-insensitive seam"),
            "activation summary should explain the value-insensitive owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "value-insensitive direct owner calls must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_compact_evidence_when_import_affinity_has_no_owner_call_then_activation_is_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/parse.rs");
        let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
        let tests = PathBuf::from("tests/wrapper_tests.rs");
        let tests_src = r#"
fn helper() -> Result<i32, AuthError> { Err(AuthError::RevokedToken) }

#[test]
fn wrapper_rejects_empty() {
    use crate::parse;
    assert!(helper().is_err());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
        let error_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ErrorVariant)
            .ok_or_else(|| "expected error_variant seam".to_string())?;
        let context = CompactGripContext::new(&index);

        let related = find_related_tests_compact(error_seam, &context);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].test.name, "wrapper_rejects_empty");

        let evidence = compact_evidence_for_seam(error_seam, &context);
        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        Ok(())
    }

    #[test]
    fn given_compact_related_tests_when_more_than_limit_match_then_results_are_capped()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pricing.rs");
        let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let mut tests_src = String::new();
        for idx in 0..14 {
            tests_src.push_str(&format!(
                "#[test]\nfn direct_{idx:02}() {{ assert_eq!(discounted_total(100, 100), 90); }}\n"
            ));
        }
        let tests = PathBuf::from("tests/pricing_tests.rs");
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src.as_str())])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "expected predicate seam".to_string())?;
        let context = CompactGripContext::new(&index);

        let related = find_related_tests_compact(predicate, &context);

        assert_eq!(related.len(), COMPACT_RELATED_TEST_LIMIT);
        assert_eq!(related[0].test.name, "direct_00");
        assert_eq!(
            related[COMPACT_RELATED_TEST_LIMIT - 1].test.name,
            "direct_11"
        );
        Ok(())
    }

    #[test]
    fn given_compact_import_affinity_when_owner_only_in_comment_or_string_then_no_relation_is_found()
    -> Result<(), String> {
        let prod = PathBuf::from("src/parse.rs");
        let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
        let tests = PathBuf::from("tests/noise_tests.rs");
        let tests_src = r#"
#[test]
fn wrapper_mentions_owner_only_in_non_code() {
    // use crate::parse;
    let _path = "crate::parse";
    assert!(helper().is_err());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
        let error_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::ErrorVariant)
            .ok_or_else(|| "expected error_variant seam".to_string())?;
        let context = CompactGripContext::new(&index);

        let related = find_related_tests_compact(error_seam, &context);

        assert_eq!(related.len(), 0);
        Ok(())
    }

    // -- relation_reason / relation_confidence ranking ----------------
    //
    // Pins the ranking contract:
    //   confidence (high first) → reason priority → file → name → line.
    // Reason detection is exercised here through `find_related_tests`
    // via `evidence_for_seam`. Each test fabricates a small index and
    // inspects the first emitted RelatedTestGrip per seam.

    fn first_grip_for(
        seam_file: &str,
        prod_src: &str,
        tests: &[(&str, &str)],
    ) -> Result<RelatedTestGrip, String> {
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from(seam_file), prod_src)];
        for (path, src) in tests {
            files.push((PathBuf::from(*path), *src));
        }
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from(seam_file)], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        evidence
            .related_tests
            .into_iter()
            .next()
            .ok_or_else(|| "at least one related test".to_string())
    }

    #[test]
    fn given_direct_owner_call_and_same_file_match_when_related_tests_are_ranked_then_direct_call_is_first()
    -> Result<(), String> {
        // One test in the same file (would match same_test_file) plus
        // one that calls the owner directly. Ranking must put the
        // direct-call test first.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        // Test in pricing_tests.rs has the same file stem as src/pricing.rs.
        let same_file_only = (
            "tests/pricing_tests.rs",
            "#[test] fn pricing_smoke() { assert_eq!(1, 1); }\n",
        );
        // Test in unrelated.rs calls the owner directly.
        let direct = (
            "tests/unrelated.rs",
            "#[test] fn calls_owner() { assert_eq!(discounted_total(100, 100), 90); }\n",
        );

        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(same_file_only.0), same_file_only.1),
            (PathBuf::from(direct.0), direct.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "at least one related test".to_string())?;
        let labels: Vec<_> = evidence
            .related_tests
            .iter()
            .map(|g| (g.test_name.clone(), g.relation_reason))
            .collect();
        assert_eq!(
            first.relation_reason,
            RelationReason::DirectOwnerCall,
            "direct owner call must outrank same-file affinity; got grips {labels:?}"
        );
        assert_eq!(first.relation_confidence, RelationConfidence::High);
        Ok(())
    }

    #[test]
    fn given_owner_named_test_without_call_when_related_tests_are_ranked_then_confidence_is_medium()
    -> Result<(), String> {
        // Test name embeds the owner name but does not call it and is
        // not in the same module / file. Should classify as
        // owner_named_test with medium confidence.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/billing.rs",
            "#[test] fn discounted_total_smoke() { assert_eq!(1, 1); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        assert_eq!(grip.relation_reason, RelationReason::OwnerNamedTest);
        assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
        Ok(())
    }

    #[test]
    fn given_fixture_only_affinity_when_related_tests_are_ranked_then_confidence_is_low()
    -> Result<(), String> {
        // Test calls a fixture-named helper in the owner's source file
        // but never the owner itself, and the test name does not embed
        // the owner. Should classify as fixture_owner_affinity with
        // exactly Low confidence (Opaque is reserved for cases the
        // detector does not yet emit).
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn make_quote() -> i32 { 100 }\n";
        let test = (
            "tests/integration.rs",
            "#[test] fn quote_smoke() { let _ = make_quote(); assert!(true); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        assert_eq!(grip.relation_reason, RelationReason::FixtureOwnerAffinity);
        assert_eq!(grip.relation_confidence, RelationConfidence::Low);
        Ok(())
    }

    #[test]
    fn given_assertion_target_affinity_uses_token_aware_match_not_substring() -> Result<(), String>
    {
        // The seam's required-discriminator description contains the
        // identifier `discount_threshold`. A test whose assertion uses
        // `discount_threshold_factor` (a longer identifier that contains
        // the discriminator string as a substring) must NOT be
        // classified as assertion_target_affinity — token-aware matching
        // requires whole-identifier hits, not substring contains.
        //
        // The test calls a different function (no direct_owner_call)
        // and lives in an unrelated file (no same_test_file/module),
        // and its name does not embed the owner.
        let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/billing.rs",
            "fn other() -> i32 { 0 }\n\
             #[test] fn smoke() { let discount_threshold_factor = 5; assert_eq!(other(), 0); let _ = discount_threshold_factor; }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        // The test must not appear as assertion_target_affinity. It is
        // OK for it to be excluded entirely (no reason fires) — the
        // contract is "do not falsely classify substring hits".
        for grip in &evidence.related_tests {
            assert_ne!(
                grip.relation_reason,
                RelationReason::AssertionTargetAffinity,
                "substring hit (`discount_threshold_factor`) must not match \
                 assertion_target_affinity; got {grip:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn given_call_presence_when_assertion_mentions_only_generic_argument_token_then_no_affinity()
    -> Result<(), String> {
        // Live Lane 1 audit showed many call_presence limitations where
        // assertion-target affinity came from generic argument names
        // such as `path`, plus generic field/method targets such as
        // `description.clone()` and `is_empty()`, enum/match field
        // names such as `variant` and `arm`, and argument/context tokens
        // from full call expressions such as `source`, `current_owner`,
        // and `out`. That is not enough evidence that the test reaches
        // the owner or observes the call site.
        for token in [
            "arm",
            "path",
            "side_effect",
            "description",
            "field",
            "from",
            "is_empty",
            "byte",
            "bytes",
            "context",
            "probe",
            "sink",
            "u64",
            "variant",
        ] {
            assert!(
                !call_presence_assertion_affinity_token_is_specific_enough(token),
                "generic call-presence token must not create assertion-target affinity: {token}"
            );
        }
        assert!(call_presence_assertion_affinity_token_is_specific_enough(
            "zq_quote_target_token"
        ));
        let prod_src = "pub fn zq_call_presence_owner(path: &std::path::Path) -> String { \
                            zq_quote_target_token(&zq_render_target_token(path)) \
                        }\n\
                        fn zq_render_target_token(path: &std::path::Path) -> String { \
                            path.display().to_string() \
                        }\n\
                        fn zq_quote_target_token(input: &str) -> String { input.to_string() }\n\
                        pub fn zq_description_owner(description: &str) -> bool { \
                            description.is_empty() \
                        }\n\
                        pub fn zq_variant_owner(variant: &str, arm: &str) -> String { \
                            let _arm = arm.clone(); \
                            variant.to_string() \
                        }\n\
                        pub fn zq_collect_source_owner(file: &str, source: &str, current_owner: &str, out: &mut Vec<String>) { \
                            collect_source_facts_from_expr(file, source, current_owner, out); \
                        }\n\
                        fn collect_source_facts_from_expr(_file: &str, _source: &str, _current_owner: &str, _out: &mut Vec<String>) { \
                        }\n\
                        pub fn zq_byte_owner(bytes: &[u8]) -> Vec<u64> { \
                            bytes.iter().map(|byte| u64::from(*byte)).collect() \
                        }\n\
                        pub struct ZqContext { \
                            pub probe: String, \
                            pub class: String, \
                            pub evidence: String, \
                        }\n\
                        pub fn zq_probe_owner(context: &ZqContext) -> String { \
                            zq_missing_evidence(&context.probe, &context.class, &context.evidence) \
                        }\n\
                        fn zq_missing_evidence(probe: &str, class: &str, evidence: &str) -> String { \
                            format!(\"{probe}:{class}:{evidence}\") \
                        }\n";
        let test = (
            "tests/unrelated.rs",
            "#[test] fn unrelated_path_assertion() { \
                let path = \"target/ripr\"; \
                assert_eq!(path, \"target/ripr\"); \
            }\n\
             #[test] fn unrelated_side_effect_assertion() { \
                 let side_effect = \"target/ripr\"; \
                 assert_eq!(side_effect, \"target/ripr\"); \
             }\n\
             #[test] fn unrelated_description_assertion() { \
                 let description = \"target/ripr\"; \
                 assert!(!description.is_empty()); \
             }\n\
             #[test] fn unrelated_variant_assertion() { \
                 let variant = \"NotFound\"; \
                 let arm = \"fallback\"; \
                 assert_eq!(variant, \"NotFound\"); \
                 assert_eq!(arm, \"fallback\"); \
             }\n\
             #[test] fn unrelated_call_argument_assertion() { \
                 let source = \"src/lib.rs\"; \
                 let current_owner = \"current_owner\"; \
                 let out = \"out\"; \
                 assert_eq!(source, \"src/lib.rs\"); \
                 assert_eq!(current_owner, \"current_owner\"); \
                 assert_eq!(out, \"out\"); \
             }\n\
             #[test] fn unrelated_byte_assertion() { \
                 let bytes = vec![1u8, 2u8]; \
                 let byte = bytes[0]; \
                 assert_eq!(u64::from(byte), 1); \
             }\n\
             #[test] fn unrelated_probe_context_assertion() { \
                 let context = \"probe\"; \
                 let probe = \"alpha\"; \
                 let evidence = \"beta\"; \
                 assert_eq!(context, \"probe\"); \
                 assert_eq!(probe, \"alpha\"); \
                 assert_eq!(evidence, \"beta\"); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/agent_paths.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/agent_paths.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.expression().contains("zq_render_target_token")
            })
            .ok_or_else(|| "zq_render_target_token call_presence seam present".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            evidence.related_tests.is_empty(),
            "generic argument token `path` must not create assertion-target affinity; got {:?}",
            evidence.related_tests
        );

        let description_call_presence = seams
            .iter()
            .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("is_empty"))
            .ok_or_else(|| "description is_empty call_presence seam present".to_string())?;
        let description_evidence = evidence_for_seam(description_call_presence, &index);
        assert!(
            description_evidence.related_tests.is_empty(),
            "generic field/method tokens `description` and `is_empty` must not create assertion-target affinity; got {:?}",
            description_evidence.related_tests
        );
        assert_eq!(description_evidence.reach.state, StageState::No);

        let variant_call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence && s.expression().contains("variant.to_string")
            })
            .ok_or_else(|| "variant to_string call_presence seam present".to_string())?;
        let variant_evidence = evidence_for_seam(variant_call_presence, &index);
        assert!(
            variant_evidence.related_tests.is_empty(),
            "generic enum/match tokens `variant` and `arm` must not create assertion-target affinity; got {:?}",
            variant_evidence.related_tests
        );
        assert_eq!(variant_evidence.reach.state, StageState::No);

        let arm_call_presence = seams
            .iter()
            .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("arm.clone"))
            .ok_or_else(|| "arm clone call_presence seam present".to_string())?;
        let arm_evidence = evidence_for_seam(arm_call_presence, &index);
        assert!(
            arm_evidence.related_tests.is_empty(),
            "generic match-arm token `arm` must not create assertion-target affinity; got {:?}",
            arm_evidence.related_tests
        );
        assert_eq!(arm_evidence.reach.state, StageState::No);

        let collect_call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.expression().contains("collect_source_facts_from_expr")
            })
            .ok_or_else(|| {
                "collect_source_facts_from_expr call_presence seam present".to_string()
            })?;
        let collect_evidence = evidence_for_seam(collect_call_presence, &index);
        assert!(
            collect_evidence.related_tests.is_empty(),
            "call-presence assertion-target affinity must not match argument/context tokens from the full call expression; got {:?}",
            collect_evidence.related_tests
        );
        assert_eq!(collect_evidence.reach.state, StageState::No);
        let byte_call_presence = seams
            .iter()
            .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("u64::from"))
            .ok_or_else(|| "byte conversion call_presence seam present".to_string())?;
        let byte_evidence = evidence_for_seam(byte_call_presence, &index);
        assert!(
            byte_evidence.related_tests.is_empty(),
            "generic conversion tokens `u64`, `from`, `byte`, and `bytes` must not create assertion-target affinity; got {:?}",
            byte_evidence.related_tests
        );
        assert_eq!(byte_evidence.reach.state, StageState::No);
        let probe_call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence && s.expression().contains("zq_missing_evidence")
            })
            .ok_or_else(|| "probe context call_presence seam present".to_string())?;
        let probe_evidence = evidence_for_seam(probe_call_presence, &index);
        assert!(
            probe_evidence.related_tests.is_empty(),
            "generic context tokens `context`, `probe`, and `evidence` must not create assertion-target affinity; got {:?}",
            probe_evidence.related_tests
        );
        assert_eq!(probe_evidence.reach.state, StageState::No);
        assert_eq!(evidence.reach.state, StageState::No);
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_owner_call_has_mock_expectation_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/receipt.rs");
        let prod_src = r#"
pub struct Recorder;

impl Recorder {
    pub fn send(&mut self, _value: &str) {}
}

pub fn emit_receipt(recorder: &mut Recorder, value: &str) {
    recorder.send(value);
}
"#;
        let tests = PathBuf::from("tests/receipt_tests.rs");
        let tests_src = r#"
fn receipt_payload() -> String { "sent".to_string() }

#[test]
fn emit_receipt_sends_value() {
    let mut recorder = Recorder;
    let value = receipt_payload();
    emit_receipt(&mut recorder, &value);
    mock_recorder.expect_send().times(1);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/receipt.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::emit_receipt")
                    && s.expression().contains("send")
            })
            .ok_or_else(|| "expected send call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.related_tests.iter().any(|test| {
                test.relation_reason == RelationReason::DirectOwnerCall
                    && test.oracle_kind == OracleKind::MockExpectation
            }),
            "expected direct owner-call related mock expectation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "call_presence owner calls must not create boundary debt"
        );
        assert_eq!(evidence.propagate.state, StageState::Yes);
        assert_eq!(evidence.observe.state, StageState::Yes);
        assert_eq!(evidence.discriminate.state, StageState::Yes);
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_owner_call_uses_turbofish_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline<T: ToString>(input: T) -> String {
    format_output(input.to_string())
}

fn format_output(input: String) -> String {
    input
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

#[test]
fn direct_generic_owner_call_observes_call_presence() {
    let rendered = render_pipeline::<String>("alpha".to_string());
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.related_tests.iter().any(|test| {
                test.relation_reason == RelationReason::DirectOwnerCall
                    && test.test_name == "direct_generic_owner_call_observes_call_presence"
            }),
            "expected turbofish direct owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence turbofish activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "call_presence turbofish owner calls must not create boundary debt"
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("Observed direct owner call for value-insensitive seam"),
            "activation summary should explain direct owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_owner_call_has_space_before_paren_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: String) -> String {
    format_output(input)
}

fn format_output(input: String) -> String {
    input
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

#[test]
fn direct_spaced_owner_call_observes_call_presence() {
    let rendered = render_pipeline ("alpha".to_string());
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.related_tests.iter().any(|test| {
                test.relation_reason == RelationReason::DirectOwnerCall
                    && test.test_name == "direct_spaced_owner_call_observes_call_presence"
            }),
            "expected spaced direct owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence spaced owner calls must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "call_presence spaced owner calls must not create boundary debt"
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("Observed direct owner call for value-insensitive seam"),
            "activation summary should explain direct owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_assertion_mentions_short_specific_call_target_then_affinity_remains()
    -> Result<(), String> {
        let prod = PathBuf::from("src/agent_paths.rs");
        let prod_src = "pub fn zq_call_presence_owner(path: &std::path::Path) -> String { \
                            zq_quote_target_token(&zq_render_target_token(path)) \
                        }\n\
                        fn zq_render_target_token(path: &std::path::Path) -> String { \
                            path.display().to_string() \
                        }\n\
                        fn zq_quote_target_token(input: &str) -> String { input.to_string() }\n";
        let tests = PathBuf::from("tests/target_affinity.rs");
        let tests_src = "#[test] fn unrelated_specific_target_assertion() { \
                let observed = \"zq_render_target_token\"; \
                assert_eq!(observed, \"zq_render_target_token\"); \
            }\n";
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/agent_paths.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.expression().contains("zq_render_target_token")
            })
            .ok_or_else(|| "zq_render_target_token call_presence seam present".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
            "specific target token should remain eligible for assertion-target affinity; got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.related_tests.iter().any(|test| {
                test.relation_reason == RelationReason::AssertionTargetAffinity
                    && test.relation_confidence == RelationConfidence::Medium
            }),
            "assertion-target affinity should stay medium confidence because it does not prove owner execution; got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "specific target affinity alone should remain a named owner-call limitation: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence affinity-only activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_wrapper_directly_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_exercises_pipeline() {
        let output = exercise_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected same-file wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the wrapper owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_wrapper_calls_owner_method_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
struct Pipeline;

impl Pipeline {
    fn render_pipeline(&self, input: &str) -> String {
        input.trim().to_string()
    }
}

fn exercise_pipeline(input: &str) -> String {
    let pipeline = Pipeline;
    pipeline.render_pipeline(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_exercises_pipeline_method() {
        let output = exercise_pipeline(" alpha ");
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::impl Pipeline::render_pipeline")
                    && s.expression().contains("trim")
            })
            .ok_or_else(|| "expected render_pipeline method call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected same-file receiver-method wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "method wrapper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the method wrapper owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_wrapper_uses_dynamic_method_receiver_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
struct Pipeline;

impl Pipeline {
    fn render_pipeline(&self, input: &str) -> String {
        input.trim().to_string()
    }
}

fn pipeline_factory() -> Pipeline {
    Pipeline
}

fn exercise_pipeline(input: &str) -> String {
    pipeline_factory().render_pipeline(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_uses_dynamic_receiver() {
        let output = exercise_pipeline(" alpha ");
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::impl Pipeline::render_pipeline")
                    && s.expression().contains("trim")
            })
            .ok_or_else(|| "expected render_pipeline method call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "dynamic receiver method call must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "dynamic receiver method route must not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "activation summary should keep owner-call limitation, got {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_directly_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected test-local one-hop helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence test-local helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the test-local helper owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_helper_calls_owner_then_logs_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn note() {}

fn exercise_pipeline() -> String {
    let output = render_pipeline("alpha");
    note();
    output
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected helper owner-call relation through direct owner call statement, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_integration_test_calls_production_wrapper_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn production_wrapper_exercises_pipeline() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected production wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "production wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_integration_test_calls_two_hop_production_wrapper_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn build_pipeline(input: &str) -> String {
    render_pipeline(input)
}

pub fn exercise_pipeline(input: &str) -> String {
    build_pipeline(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn production_wrapper_chain_exercises_pipeline() {
    let format_output = exercise_pipeline("alpha");
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected bounded production wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "bounded production wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the bounded helper owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_two_hop_production_wrapper_reaches_multiple_owners_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/loop_commands.rs");
        let prod_src = r#"
pub fn quote_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn normalize_arg(value: &str) -> String {
    value.trim().to_string()
}

pub fn quote_root(root: &str) -> String {
    quote_arg(root)
}

pub fn format_command(root: &str, packet: &str) -> String {
    let root_arg = quote_root(root);
    let packet_arg = normalize_arg(packet);
    format!("ripr start --root {root_arg} --packet {packet_arg}")
}
"#;
        let tests = PathBuf::from("tests/loop_commands_tests.rs");
        let tests_src = r#"
use loop_commands::format_command;

#[test]
fn command_formats_dynamic_args() {
    let command = format_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr start --root tmp\\ root --packet gap 1"
    );
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::quote_arg")
                    && s.expression().contains("replace")
            })
            .ok_or_else(|| "expected quote_arg call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "mixed-owner production graph must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "mixed-owner production graph must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_production_wrapper_calls_same_owner_multiple_times_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/loop_commands.rs");
        let prod_src = r#"
pub fn shell_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn agent_start_command(root: &str, packet: &str) -> String {
    let root_arg = shell_arg(root);
    let packet_arg = shell_arg(packet);
    format!("ripr agent start --root {root_arg} --packet {packet_arg}")
}
"#;
        let tests = PathBuf::from("tests/loop_commands_tests.rs");
        let tests_src = r#"
use loop_commands::agent_start_command;

#[test]
fn command_quotes_each_dynamic_arg() {
    let command = agent_start_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr agent start --root tmp\\ root --packet gap\\ 1"
    );
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::shell_arg")
                    && s.expression().contains("replace")
            })
            .ok_or_else(|| "expected shell_arg call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected same-owner production wrapper relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "same-owner production wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_production_wrapper_calls_multiple_owners_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/loop_commands.rs");
        let prod_src = r#"
pub fn quote_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn normalize_arg(value: &str) -> String {
    value.trim().to_string()
}

pub fn agent_start_command(root: &str, packet: &str) -> String {
    let root_arg = quote_arg(root);
    let packet_arg = normalize_arg(packet);
    format!("ripr agent start --root {root_arg} --packet {packet_arg}")
}
"#;
        let tests = PathBuf::from("tests/loop_commands_tests.rs");
        let tests_src = r#"
use loop_commands::agent_start_command;

#[test]
fn command_formats_dynamic_args() {
    let command = agent_start_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr agent start --root tmp\\ root --packet gap 1"
    );
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::quote_arg")
                    && s.expression().contains("replace")
            })
            .ok_or_else(|| "expected quote_arg call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "mixed-owner production wrapper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "mixed-owner production wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_multi_owner_production_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::exercise_both;

#[test]
fn multi_owner_wrapper_observes_pipeline_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_output"));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected target-affinity production wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "target-affinity call_presence wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_unit_test_calls_same_file_target_affinity_wrapper_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_wrapper_observes_pipeline_call_target() {
        let rendered = exercise_both("alpha");
        assert!(rendered.contains("format_output"));
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected same-file target-affinity wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "same-file target-affinity wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_multi_owner_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::exercise_both;

#[test]
fn multi_owner_wrapper_observes_report_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_report"));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "other target token must not prove pipeline owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "other-target wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_fanout_helper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::{render_pipeline, render_report};

fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

#[test]
fn test_local_fanout_observes_report_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_report"));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "test-local fanout helper must not bypass target affinity: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "test-local fanout helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_production_wrapper_name_is_ambiguous_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn ambiguous_production_wrapper_keeps_pipeline_limited() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "ambiguous production wrapper name must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_module_qualified_ambiguous_production_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
#[test]
fn qualified_wrapper_observes_pipeline_call_target() {
    let format_output = pipeline::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected module-qualified target-affinity wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "qualified target-affinity wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_aliased_module_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
use crate::pipeline as pipe;

#[test]
fn aliased_wrapper_observes_pipeline_call_target() {
    let format_output = pipe::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected aliased module target-affinity wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "aliased target-affinity wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_bare_aliased_module_wrapper_has_target_affinity_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
use pipeline as pipe;

#[test]
fn bare_aliased_wrapper_observes_pipeline_call_target() {
    let format_output = pipe::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "bare module alias must not prove a local wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "bare aliased wrapper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_imported_module_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let loop_commands = PathBuf::from("src/agent/loop_commands.rs");
        let loop_commands_src = r#"
use std::path::Path;

pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

pub fn shell_path(path: &Path) -> String {
    shell_arg(&display_path(path))
}

pub fn shell_arg(value: &str) -> String {
    value.to_string()
}
"#;
        let pilot_commands = PathBuf::from("src/output/pilot/commands.rs");
        let pilot_commands_src = r#"
use crate::agent::loop_commands::{self, display_path};
use std::path::Path;

pub fn pilot_paths(root: &Path, after: &Path) -> String {
    let root = display_path(root);
    let after = loop_commands::shell_path(after);
    format!("display_path={root};shell_arg={after}")
}
"#;
        let tests = PathBuf::from("tests/pilot_commands_tests.rs");
        let tests_src = r#"
use output::pilot::commands::pilot_paths;
use std::path::Path;

#[test]
fn pilot_paths_preserve_shell_arg_route() {
    let rendered = pilot_paths(Path::new("."), Path::new("target/out file.json"));
    assert!(rendered.contains("shell_arg="));
}
"#;
        let index = index_from_files(&[
            (loop_commands, loop_commands_src),
            (pilot_commands, pilot_commands_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/agent/loop_commands.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::shell_path")
                    && s.expression().contains("shell_arg")
            })
            .ok_or_else(|| "expected shell_path call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected imported-module wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "imported module call_presence wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_imported_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
use crate::analysis::classify::{reach_evidence as reach, reveal_evidence};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    let reveal = reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn direct_imported_wrapper_observes_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected direct imported target-affinity wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "direct imported target-affinity wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_crate_qualified_wrapper_has_target_affinity_then_activation_is_yes()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    let reveal = crate::analysis::classify::reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_observes_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected crate-qualified target-affinity wrapper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "crate-qualified target-affinity wrapper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_crate_qualified_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    let reveal = crate::analysis::classify::reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_observes_reveal_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("reveal_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "crate-qualified other target token must not prove wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "crate-qualified other-target wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_bare_qualified_wrapper_has_target_affinity_then_activation_stays_unknown()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = analysis::classify::reach_evidence(input);
    format!("format_marker={reach}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn bare_qualified_wrapper_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "bare qualified owner path must not prove a crate-local owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "bare qualified wrapper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_crate_qualified_owner_exists_only_in_other_package_then_activation_stays_unknown()
    -> Result<(), String> {
        let alpha_owner = PathBuf::from("crates/alpha/src/analysis/other.rs");
        let alpha_owner_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let alpha_wrapper = PathBuf::from("crates/alpha/src/analysis/classifier/evidence.rs");
        let alpha_wrapper_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    format!("format_marker={reach}")
}
"#;
        let beta_owner = PathBuf::from("crates/beta/src/analysis/classify.rs");
        let beta_owner_src = r#"
pub fn reach_evidence(input: &str) -> String {
    beta_marker(input)
}

fn beta_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("crates/alpha/tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use alpha::analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (alpha_owner, alpha_owner_src),
            (alpha_wrapper, alpha_wrapper_src),
            (beta_owner, beta_owner_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(
            &[PathBuf::from("crates/alpha/src/analysis/other.rs")],
            &index,
        );
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected alpha reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "crate-qualified call must not resolve through another package's owner map: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "cross-package crate-qualified miss must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_external_direct_import_matches_local_owner_name_then_activation_stays_unknown()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
use external_crate::{reach_evidence as reach};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    format!("format_marker={reach}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn external_direct_import_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "external direct import must not prove a local owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "external direct import activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_ambiguous_direct_import_owner_name_then_activation_stays_unknown()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let alternate = PathBuf::from("src/analysis/alternate.rs");
        let alternate_src = r#"
pub fn reach_evidence(input: &str) -> String {
    alternate_marker(input)
}

fn alternate_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
use crate::analysis::classify::reach_evidence as reach;

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    format!("format_marker={reach}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn ambiguous_direct_import_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (alternate, alternate_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "ambiguous direct import owner name must not prove wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "ambiguous direct import activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_module_qualified_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
#[test]
fn qualified_wrapper_observes_report_call_target() {
    let rendered = pipeline::exercise_pipeline();
    assert!(rendered.contains("format_report"));
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "other target token must not prove qualified wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "other-target qualified wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_aliased_module_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/contract_tests.rs");
        let tests_src = r#"
use crate::pipeline as pipe;

#[test]
fn aliased_wrapper_observes_report_call_target() {
    let rendered = pipe::exercise_pipeline();
    assert!(rendered.contains("format_report"));
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "aliased other target token must not prove wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "aliased other-target wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_imported_wrapper_asserts_other_target_then_activation_stays_unknown()
    -> Result<(), String> {
        let classify = PathBuf::from("src/analysis/classify.rs");
        let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
        let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
        let evidence_src = r#"
use crate::analysis::classify::{reach_evidence as reach, reveal_evidence};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    let reveal = reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
        let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
        let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn direct_imported_wrapper_observes_reveal_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("reveal_marker"));
}
"#;
        let index = index_from_files(&[
            (classify, classify_src),
            (evidence, evidence_src),
            (tests, tests_src),
        ])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::reach_evidence")
                    && s.expression().contains("format_marker")
            })
            .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "direct imported other target token must not prove wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "direct imported other-target wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_shadows_production_wrapper_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
fn exercise_pipeline() -> String {
    "alpha".to_string()
}

#[test]
fn local_shadow_keeps_pipeline_limited() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "test-local shadow must not inherit production wrapper relation: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_shadows_target_affinity_wrapper_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    fn exercise_both(_: &str) -> String {
        "format_output".to_string()
    }

    #[test]
    fn local_shadow_only_mentions_target() {
        let rendered = exercise_both("alpha");
        assert!(rendered.contains("format_output"));
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "test-local target-affinity shadow must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.observed_values.is_empty(),
            "test-local target-affinity shadow must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_unique_test_support_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let support = PathBuf::from("tests/support.rs");
        let support_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use support::exercise_pipeline;

#[test]
fn pipeline_from_support_helper() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index =
            index_from_files(&[(prod, prod_src), (support, support_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected unique test-support helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence support helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the unique support helper owner-call route: {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_duplicate_test_support_helpers_share_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use support_a::exercise_pipeline;

#[test]
fn pipeline_from_support_helper() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (prod, prod_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected duplicate same-owner support helpers to prove helper-owner relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "call_presence duplicate support helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }
    #[test]
    fn given_call_presence_when_test_support_helper_name_is_ambiguous_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("alpha")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
#[test]
fn ambiguous_support_helper_smoke() {
    let rendered = exercise_pipeline();
    assert!(!rendered.is_empty());
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "ambiguous test-support helper name must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_imported_ambiguous_support_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use support_a::exercise_pipeline;

#[test]
fn direct_imported_support_helper_reaches_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected direct imported support helper to disambiguate owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "direct imported support helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_aliased_direct_imported_support_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use support_a::exercise_pipeline as exercise;

#[test]
fn aliased_direct_imported_support_helper_reaches_pipeline() {
    let rendered = exercise();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected aliased direct imported support helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "aliased direct imported support helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_imported_support_helper_targets_other_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use support_b::exercise_pipeline;

#[test]
fn direct_imported_support_helper_reaches_report() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "beta");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "direct imported support helper for another owner must not prove this owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "other-owner support helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_direct_imported_external_helper_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use external_support::exercise_pipeline;

#[test]
fn external_direct_import_mentions_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[(pipeline, pipeline_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "external direct imported helper must not prove a local owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "external direct imported helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_block_local_direct_imported_helper_is_not_file_scoped_then_activation_stays_unknown()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
#[test]
fn block_local_import_reaches_pipeline() {
    use support_a::exercise_pipeline;
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}

#[test]
fn sibling_without_import_mentions_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_ne!(evidence.activate.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "block-local direct helper import must not leak to sibling tests: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "block-local direct helper import must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_ambiguous_support_helper_is_module_qualified_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
#[test]
fn qualified_support_helper_reaches_pipeline() {
    let rendered = support_a::exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected qualified support helper to disambiguate helper-owner relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "qualified call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_qualified_support_helper_targets_other_owner_then_no_helper_relation()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
#[test]
fn qualified_support_helper_reaches_report() {
    let rendered = support_b::exercise_pipeline();
    assert_eq!(rendered, "beta");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "qualified helper for another owner must not prove pipeline owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "other-owner qualified helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_crate_qualified_support_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
#[test]
fn crate_qualified_support_helper_reaches_pipeline() {
    let rendered = crate::support_a::exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected crate-qualified support helper to prove helper-owner relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "crate-qualified call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_super_qualified_support_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let pipeline = PathBuf::from("src/pipeline.rs");
        let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let report = PathBuf::from("src/report.rs");
        let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
        let support_a = PathBuf::from("tests/support_a.rs");
        let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
        let support_b = PathBuf::from("tests/support_b.rs");
        let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
mod nested {
    #[test]
    fn super_qualified_support_helper_reaches_pipeline() {
        let rendered = super::support_a::exercise_pipeline();
        assert_eq!(rendered, "alpha");
    }
}
"#;
        let index = index_from_files(&[
            (pipeline, pipeline_src),
            (report, report_src),
            (support_a, support_a_src),
            (support_b, support_b_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected super-qualified support helper to prove helper-owner relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "super-qualified call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_qualified_support_helper_only_in_comment_or_string_then_call_match_is_ignored() {
        let cleaned = strip_comments_and_strings(
            "let doc = \"support_a::exercise_pipeline()\"; // support_a::exercise_pipeline()",
        );
        assert!(!code_contains_qualified_helper_call(
            &cleaned,
            "support_a",
            "exercise_pipeline"
        ));
        assert!(code_contains_qualified_helper_call(
            "let rendered = support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(code_contains_qualified_helper_call(
            "let rendered = crate::support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(code_contains_qualified_helper_call(
            "let rendered = self::support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(code_contains_qualified_helper_call(
            "let rendered = super::support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(!code_contains_qualified_helper_call(
            "let rendered = other_support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(!code_contains_qualified_helper_call(
            "let rendered = my_super::support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
        assert!(!code_contains_qualified_helper_call(
            "let rendered = other::support_a::exercise_pipeline();",
            "support_a",
            "exercise_pipeline"
        ));
    }

    #[test]
    fn direct_delegate_wrapper_name_preserves_turbofish_constructor() {
        assert_eq!(direct_delegate_wrapper_name("Ok::<String, ()>"), "Ok");
        assert_eq!(
            direct_delegate_wrapper_name("decorate::<String>"),
            "decorate"
        );
        assert_eq!(direct_delegate_wrapper_name("Box::<String>::new"), "new");
    }

    #[test]
    fn direct_delegate_condition_prefix_accepts_only_leading_condition_owner_call() {
        assert!(direct_delegate_condition_prefix_is_allowed("if"));
        assert!(!direct_delegate_condition_prefix_is_allowed("} else if"));
        assert!(!direct_delegate_condition_prefix_is_allowed(
            "    } else if"
        ));
        assert!(!direct_delegate_condition_prefix_is_allowed("if ready &&"));
        assert!(!direct_delegate_condition_prefix_is_allowed("while"));
        assert!(!direct_delegate_condition_prefix_is_allowed("match"));
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_option_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Option<String> {
    Some(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected option-wrapped helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "option-wrapped call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "option-wrapped call_presence helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_result_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<String, ()> {
    Ok(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected result-wrapped helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "result-wrapped call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "result-wrapped call_presence helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_result_turbofish_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<String, ()> {
    Ok::<String, ()>(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected result turbofish helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "result turbofish helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "result turbofish helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_err_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<(), String> {
    Err(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let format_output = exercise_pipeline().unwrap_err();
    assert_eq!(format_output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected Err-wrapped helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "Err-wrapped call_presence helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "Err-wrapped call_presence helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_unwraps_owner_call_result_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").unwrap()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected unwrap helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "unwrap helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unwrap helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_expects_owner_call_result_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").expect("pipeline should render")
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected expect helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "expect helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "expect helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_borrows_owner_call_result_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").as_ref().unwrap().clone()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected as_ref borrow-chain helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "as_ref borrow-chain helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "as_ref borrow-chain helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_trims_owner_call_result_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline(" alpha ").trim().to_string()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected trim-chain helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "trim-chain helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_uses_unknown_owner_call_chain_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").normalize()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "unknown post-owner method chain must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "unknown post-owner method-chain activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_assertion_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert_eq!(render_pipeline("alpha"), "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected assertion helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "assertion helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "assertion helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_equality_helper_calls_owner_as_later_arg_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output(expected: &str) {
    assert_eq!(expected, render_pipeline("alpha"));
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output("alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected equality helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "equality helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "equality helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_assert_message_arg_calls_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert!(true, "{}", render_pipeline("alpha"));
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "assert message argument must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "assert message argument must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_assert_macro_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert!(render_pipeline("alpha") == "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected assert macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "assert macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "assert macro helper activation must not create boundary debt"
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_assert_macro_short_circuits_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output(enabled: bool) {
    assert!(enabled && render_pipeline("alpha") == "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output(true);
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "short-circuiting assert macro must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "short-circuiting assert macro activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_matches_macro_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> bool {
    matches!(render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline());
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("is_alpha")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected matches macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "matches macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_matches_macro_short_circuits_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> bool {
    matches!(enabled && render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline(true));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("is_alpha")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "short-circuiting matches macro must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "short-circuiting matches macro activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_dbg_macro_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    dbg!(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected dbg macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "dbg macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_dbg_macro_short_circuits_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> bool {
    dbg!(enabled && render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline(true));
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("is_alpha")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "short-circuiting dbg macro must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "short-circuiting dbg macro activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_format_macro_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    format!("pipeline={}", render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "pipeline=alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected format macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "format macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_format_args_macro_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    std::fmt::format(format_args!("pipeline={}", render_pipeline("alpha")))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "pipeline=alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected format_args macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "format_args macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_format_args_macro_short_circuits_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> String {
    std::fmt::format(format_args!("pipeline={}", enabled && render_pipeline("alpha").is_empty()))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline(true);
    assert_eq!(output, "pipeline=false");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "short-circuiting format_args macro must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "short-circuiting format_args macro activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_block_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    { render_pipeline("alpha") }
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected block helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "block helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_conditionally_calls_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> String {
    if enabled { render_pipeline("alpha") } else { "beta".to_string() }
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline(true);
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "conditional block helper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "conditional helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_vec_macro_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Vec<String> {
    vec![render_pipeline("alpha")]
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output[0], "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected vec macro helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "vec macro helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_array_literal_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> [String; 1] {
    [render_pipeline("alpha")]
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output[0], "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected array literal helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "array literal helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_tuple_literal_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> (String, bool) {
    (render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output.0, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected tuple literal helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "tuple literal helper activation must not invent values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_unknown_macro_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

macro_rules! trace_value {
    ($value:expr) => {
        $value
    };
}

fn exercise_pipeline() -> String {
    trace_value!(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "unknown macro wrapper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.observed_values.is_empty(),
            "unknown macro wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_helper_wraps_owner_call_with_non_container_call_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn decorate(input: String) -> String {
    input
}

fn exercise_pipeline() -> String {
    decorate(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "non-container wrapper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.observed_values.is_empty(),
            "non-container wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_std_identity_then_activation_is_yes()
    -> Result<(), String> {
        for identity_path in [
            "std::convert::identity",
            "::std::convert::identity",
            "core::convert::identity",
            "::core::convert::identity",
        ] {
            let prod = PathBuf::from("src/pipeline.rs");
            let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
            let tests = PathBuf::from("tests/pipeline_tests.rs");
            let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    IDENTITY_PATH(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#
            .replace("IDENTITY_PATH", identity_path);
            let index = index_from_files(&[(prod, prod_src), (tests, tests_src.as_str())])?;
            let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
            let call_presence = seams
                .iter()
                .find(|s| {
                    s.kind() == SeamKind::CallPresence
                        && s.owner().ends_with("::render_pipeline")
                        && s.expression().contains("format_output")
                })
                .ok_or_else(|| {
                    format!("expected render_pipeline call_presence seam for {identity_path}")
                })?;

            let evidence = evidence_for_seam(call_presence, &index);

            assert_eq!(evidence.reach.state, StageState::Yes);
            assert_eq!(evidence.activate.state, StageState::Yes);
            assert!(
                evidence
                    .related_tests
                    .iter()
                    .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
                "expected {identity_path} helper owner-call relation, got {:?}",
                evidence.related_tests
            );
            assert!(
                evidence.observed_values.is_empty(),
                "{identity_path} helper activation must not invent observed values: {:?}",
                evidence.observed_values
            );
        }
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_helper_wraps_owner_call_in_local_identity_then_activation_stays_unknown()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn identity(input: String) -> String {
    input
}

fn exercise_pipeline() -> String {
    identity(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "local identity wrapper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "local identity wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_local_two_hop_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn outer_pipeline() -> String {
    exercise_pipeline()
}

#[test]
fn outer_helper_reaches_pipeline_indirectly() {
    let output = outer_pipeline();
    assert_eq!(output, "alpha");
}
"#;
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "test-local two-hop helper should get bounded helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the bounded helper owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "test-local two-hop helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_three_hop_helper_reaches_owner_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/activation.rs");
        let source_src = r#"
pub fn activation_evidence(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    missing_discriminator_facts(rows, parameter)
}

fn missing_discriminator_facts(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    missing_boundary_discriminator(rows, parameter)
}

fn missing_boundary_discriminator(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    parameter_value_set(rows, parameter)
}

fn parameter_value_set(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    let values = observed_parameter_values(rows, parameter);
    if values.is_empty() { None } else { Some(values) }
}

fn observed_parameter_values(rows: &[Vec<String>], parameter: &str) -> Vec<String> {
    rows.iter()
        .flatten()
        .filter(|value| value.as_str() == parameter)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_evidence_reports_parameter_value_set() {
        let rows = vec![vec!["amount".to_string()]];
        let values = activation_evidence(&rows, "amount");
        assert_eq!(values, Some(vec!["amount".to_string()]));
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/activation.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::parameter_value_set")
                    && s.expression()
                        .contains("observed_parameter_values(rows, parameter)")
            })
            .ok_or_else(|| "expected parameter_value_set call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected bounded three-hop same-file helper owner-call relation; got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "three-hop same-file helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_method_chain_owner_through_helpers_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/canonical_gap.rs");
        let source_src = r#"
enum RequiredDiscriminator {
    BoundaryValue { description: String },
    ErrorVariant { variant: String },
}

struct RepoSeam {
    discriminator: RequiredDiscriminator,
}

impl RepoSeam {
    fn required_discriminator(&self) -> &RequiredDiscriminator {
        &self.discriminator
    }
}

struct ClassifiedSeam {
    seam: RepoSeam,
}

fn canonical_gap_identity(entry: &ClassifiedSeam) -> String {
    missing_discriminator_key(entry)
}

fn missing_discriminator_key(entry: &ClassifiedSeam) -> String {
    required_discriminator_text(entry.seam.required_discriminator())
}

fn required_discriminator_text(discriminator: &RequiredDiscriminator) -> String {
    match discriminator {
        RequiredDiscriminator::BoundaryValue { description } => description.clone(),
        RequiredDiscriminator::ErrorVariant { variant } => variant.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_gap_uses_required_discriminator_text() {
        let entry = ClassifiedSeam {
            seam: RepoSeam {
                discriminator: RequiredDiscriminator::BoundaryValue {
                    description: "threshold".to_string(),
                },
            },
        };
        assert_eq!(canonical_gap_identity(&entry), "threshold");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/canonical_gap.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::required_discriminator_text")
                    && s.expression().contains("description.clone()")
            })
            .ok_or_else(|| "expected required_discriminator_text call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected same-file method-chain helper owner-call relation; got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "method-chain helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_condition_helper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/flow.rs");
        let source_src = r#"
fn effect_sink_kind(text: &str) -> &'static str {
    if looks_like_event_call_effect(text) {
        "event"
    } else if looks_like_log_effect(text) {
        "log"
    } else {
        "call"
    }
}

fn looks_like_log_effect(text: &str) -> bool {
    text.contains("log")
}

fn looks_like_event_call_effect(text: &str) -> bool {
    [".publish(", ".emit("]
        .iter()
        .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_sink_detects_event_call() {
        assert_eq!(effect_sink_kind("bus.emit(value)"), "event");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/flow.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::looks_like_event_call_effect")
                    && s.expression().contains(".iter()")
            })
            .ok_or_else(|| {
                "expected looks_like_event_call_effect call_presence seam".to_string()
            })?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "same-file condition helper should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "condition helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "condition helper activation must not create boundary debt: {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_else_if_condition_helper_is_skipped_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/flow.rs");
        let source_src = r#"
fn effect_sink_kind(text: &str) -> &'static str {
    if looks_like_log_effect(text) {
        "log"
    } else if looks_like_event_call_effect(text) {
        "event"
    } else {
        "call"
    }
}

fn looks_like_log_effect(text: &str) -> bool {
    text.contains("log")
}

fn looks_like_event_call_effect(text: &str) -> bool {
    [".publish(", ".emit("]
        .iter()
        .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_sink_detects_log_call() {
        assert_eq!(effect_sink_kind("write log line"), "log");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/flow.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::looks_like_event_call_effect")
                    && s.expression().contains(".iter()")
            })
            .ok_or_else(|| {
                "expected looks_like_event_call_effect call_presence seam".to_string()
            })?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "skipped else-if helper must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "skipped else-if helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_helper_clones_owner_result_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/activation.rs");
        let source_src = r#"
#[derive(Clone)]
struct FlowSinkFact {
    kind: FlowSinkKind,
}

#[derive(PartialEq)]
enum FlowSinkKind {
    Unknown,
    Return,
}

struct MissingDiscriminatorFact {
    flow_sink: Option<FlowSinkFact>,
}

fn activation_evidence(flow_sinks: &[FlowSinkFact]) -> Option<MissingDiscriminatorFact> {
    missing_boundary_discriminator(flow_sinks)
}

fn missing_boundary_discriminator(flow_sinks: &[FlowSinkFact]) -> Option<MissingDiscriminatorFact> {
    Some(MissingDiscriminatorFact {
        flow_sink: first_visible_flow_sink(flow_sinks).cloned(),
    })
}

fn first_visible_flow_sink(flow_sinks: &[FlowSinkFact]) -> Option<&FlowSinkFact> {
    flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_evidence_reports_visible_flow_sink() {
        let sinks = vec![FlowSinkFact { kind: FlowSinkKind::Return }];
        let missing = activation_evidence(&sinks);
        assert!(missing.and_then(|fact| fact.flow_sink).is_some());
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/activation.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::first_visible_flow_sink")
                    && s.expression().contains("flow_sinks")
                    && s.expression().contains(".iter()")
            })
            .ok_or_else(|| "expected first_visible_flow_sink call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "same-file helper with cloned owner result should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "cloned owner-result helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_wrapper_skips_owner_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn render_pipeline_fixture() -> String {
    format_output("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_mentions_pipeline_name_without_calling_owner() {
        let output = render_pipeline_fixture();
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "wrapper that skips the owner must not get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "activation summary should keep owner-call limitation, got {}",
            evidence.activate.summary
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_unit_parent_qualified_wrapper_calls_owner_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn parent_qualified_wrapper_reaches_pipeline() {
        let output = super::exercise_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "parent-qualified same-file unit helper should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the parent-qualified helper route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "parent-qualified same-file unit helper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_same_file_unit_shadow_calls_wrapper_name_then_activation_stays_unknown()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    fn exercise_pipeline() -> String {
        "shadow".to_string()
    }

    #[test]
    fn bare_shadowed_wrapper_name_does_not_reach_pipeline() {
        let output = exercise_pipeline();
        assert_eq!(output, "shadow");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            !evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "test-local shadow must not inherit production wrapper owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence
                .activate
                .summary
                .contains("No direct owner call observed for value-insensitive seam"),
            "activation summary should keep owner-call limitation, got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "test-local shadow must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_calls_two_hop_production_wrapper_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn outer_pipeline() -> String {
    exercise_pipeline()
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_wrapper_reaches_pipeline_indirectly() {
        let output = outer_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "two-hop production wrapper should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence.observed_values.is_empty(),
            "two-hop wrapper must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_production_wrapper_imports_owner_helper_then_activation_is_yes()
    -> Result<(), String> {
        let path_file = PathBuf::from("src/parser/path.rs");
        let path_src = r#"
use std::path::PathBuf;

pub fn parse_new_path_marker(raw: &str) -> Option<PathBuf> {
    let path = parse_diff_path_token(raw)?;
    Some(PathBuf::from(path))
}

fn parse_diff_path_token(raw: &str) -> Option<String> {
    let quoted = raw.strip_prefix('"')?;
    parse_c_quoted_path(quoted)
}

fn parse_c_quoted_path(raw: &str) -> Option<String> {
    let mut chars = raw.chars().peekable();
    let ch = chars.next()?;
    match ch {
        '"' => Some(String::new()),
        '\\' => Some(parse_c_escape(&mut chars).to_string()),
        _ => Some(ch.to_string()),
    }
}

fn parse_c_escape<I>(chars: &mut std::iter::Peekable<I>) -> char
where
    I: Iterator<Item = char>,
{
    chars.next().unwrap_or('\\')
}
"#;
        let parse_file = PathBuf::from("src/parser/parse.rs");
        let parse_src = r#"
use std::path::PathBuf;
use crate::parser::path::parse_new_path_marker;

pub fn parse_unified_diff(raw: &str) -> Option<PathBuf> {
    parse_line(raw)
}

fn parse_line(raw: &str) -> Option<PathBuf> {
    parse_new_path_marker(raw)
}
"#;
        let tests = PathBuf::from("tests/parser_tests.rs");
        let tests_src = r#"
use parser::parse::parse_unified_diff;

#[test]
fn quoted_path_reaches_path_parser() {
    assert_eq!(parse_unified_diff("\"src/lib.rs\"").unwrap(), std::path::PathBuf::from("src/lib.rs"));
}
"#;
        let index = index_from_files(&[
            (path_file, path_src),
            (parse_file, parse_src),
            (tests, tests_src),
        ])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/parser/path.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::parse_c_quoted_path")
                    && s.expression().contains("parse_c_escape")
            })
            .ok_or_else(|| "expected parse_c_quoted_path call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "imported production helper chain should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "imported production helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_call_presence_when_test_calls_same_file_fanout_wrapper_then_activation_is_yes()
    -> Result<(), String> {
        let source = PathBuf::from("src/pipeline.rs");
        let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn collect_pipeline_context() -> String {
    "context".to_string()
}

fn build_pipeline_report() -> String {
    let context = collect_pipeline_context();
    let output = render_pipeline("alpha");
    format!("{context}:{output}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanout_wrapper_reaches_pipeline_indirectly() {
        let output = build_pipeline_report();
        assert!(output.ends_with(":alpha"));
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "fanout production wrapper should get helper-owner relation: {:?}",
            evidence.related_tests
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .activate
                .summary
                .contains("helper owner call for value-insensitive seam"),
            "activation summary should explain the fanout helper owner-call route: {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "fanout helper route must not invent observed values: {:?}",
            evidence.observed_values
        );
        Ok(())
    }

    #[test]
    fn given_related_tests_with_same_confidence_when_sorted_then_order_is_stable_by_file_name_line()
    -> Result<(), String> {
        // Two tests with the same reason (both owner_named_test) but
        // different (file, name). Sort tie-break must be deterministic:
        // file → name → line.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test_a = (
            "tests/zeta.rs",
            "#[test] fn discounted_total_one() { assert_eq!(1, 1); }\n",
        );
        let test_b = (
            "tests/alpha.rs",
            "#[test] fn discounted_total_two() { assert_eq!(1, 1); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test_a.0), test_a.1),
            (PathBuf::from(test_b.0), test_b.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        assert!(
            evidence.related_tests.len() >= 2,
            "expected at least 2 related tests, got {}",
            evidence.related_tests.len()
        );
        // alpha.rs sorts before zeta.rs.
        assert_eq!(evidence.related_tests[0].file, Path::new("tests/alpha.rs"));
        assert_eq!(evidence.related_tests[1].file, Path::new("tests/zeta.rs"));
        Ok(())
    }

    #[test]
    fn given_higher_confidence_related_test_when_sorted_then_it_comes_before_lower_confidence()
    -> Result<(), String> {
        // Two tests, one with high confidence (direct_owner_call) and
        // one with low confidence (fixture_owner_affinity via a fixture
        // helper). High must come first regardless of file/name order.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn make_quote() -> i32 { 100 }\n";
        // The fixture user lives in 'a_first.rs' (alphabetically before)
        // so without confidence ordering it would naively sort first.
        let fixture_user = (
            "tests/a_first.rs",
            "#[test] fn fx() { let _ = make_quote(); assert!(true); }\n",
        );
        // The direct caller lives in 'z_last.rs'.
        let direct_caller = (
            "tests/z_last.rs",
            "#[test] fn caller() { assert_eq!(discounted_total(100, 100), 90); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(fixture_user.0), fixture_user.1),
            (PathBuf::from(direct_caller.0), direct_caller.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "at least one related test".to_string())?;
        assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
        assert_eq!(first.relation_confidence, RelationConfidence::High);
        Ok(())
    }

    #[test]
    fn given_related_tests_with_same_relation_when_ranked_then_strong_oracle_precedes_smoke_oracle()
    -> Result<(), String> {
        // Both tests are direct owner calls. The strong exact-value
        // oracle lives in an alphabetically later file, so the v2
        // ranking must use oracle strength before file/name tie-breaks.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> Result<i32, ()> \
                        { if amount >= threshold { Ok(amount - 10) } else { Ok(amount) } }\n";
        let smoke = (
            "tests/a_smoke.rs",
            "#[test] fn smoke_owner_call() { assert!(discounted_total(100, 100).is_ok()); }\n",
        );
        let strong = (
            "tests/z_exact.rs",
            "#[test] fn exact_owner_call() { assert_eq!(discounted_total(100, 100).unwrap(), 90); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(smoke.0), smoke.1),
            (PathBuf::from(strong.0), strong.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "at least one related test".to_string())?;

        assert_eq!(first.test_name, "exact_owner_call");
        assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
        assert_eq!(first.oracle_strength, OracleStrength::Strong);
        Ok(())
    }

    #[test]
    fn given_related_tests_with_same_relation_and_oracle_when_ranked_then_activation_overlap_precedes_file_order()
    -> Result<(), String> {
        // Both tests are direct owner calls with strong exact-value
        // oracles. The equality-boundary call lives in an
        // alphabetically later file; it should still be the nearest
        // imitation target because its activation values overlap the
        // predicate boundary.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let above = (
            "tests/a_above.rs",
            "#[test] fn above_boundary() { let actual = discounted_total(101, 100); assert_eq!(actual, 91); }\n",
        );
        let equality = (
            "tests/z_equal.rs",
            "#[test] fn equality_boundary() { let actual = discounted_total(100, 100); assert_eq!(actual, 90); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(above.0), above.1),
            (PathBuf::from(equality.0), equality.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let first = evidence
            .related_tests
            .first()
            .ok_or_else(|| "at least one related test".to_string())?;

        assert_eq!(first.test_name, "equality_boundary");
        assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
        assert_eq!(first.oracle_strength, OracleStrength::Strong);
        Ok(())
    }

    // -- import_path_affinity tightening (#310 review) ---------------
    //
    // The detector requires explicit `module::owner_name` qualified-
    // path syntax or an inline `use ... owner_name` line — pure token
    // co-occurrence (owner_name + module token both present in the
    // body without path syntax) must NOT fire.

    #[test]
    fn given_import_path_affinity_without_direct_call_when_related_tests_are_ranked_then_confidence_is_medium()
    -> Result<(), String> {
        // Test references `crate::pricing::discounted_total` as a
        // function value (no parens → not a CallFact, so
        // direct_owner_call cannot fire). The qualified path satisfies
        // the tightened import_path_affinity detector. The test name
        // does not contain "discounted_total" and the file is not
        // pricing-flavoured, so no other reason fires either.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/integration_smoke.rs",
            "#[test] fn smoke() { let _f = crate::pricing::discounted_total; assert_eq!(1, 1); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        assert_eq!(grip.relation_reason, RelationReason::ImportPathAffinity);
        assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
        Ok(())
    }

    #[test]
    fn given_qualified_owner_path_only_in_comment_or_string_when_related_tests_are_ranked_then_import_path_affinity_does_not_fire()
    -> Result<(), String> {
        // Per CodeRabbit on #310: `test_imports_owner` previously did
        // a raw `body.contains("::owner")` which matched substrings
        // inside `// ...` comments and `"..."` string literals. That
        // re-introduced the noise the detector was meant to avoid.
        // After the fix, neither shape should match.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        // Comment carries the qualified path; code does not. Test name
        // and file are both neutral so no other reason fires.
        let comment_only = (
            "tests/integration_a.rs",
            "#[test] fn smoke_a() { \
                // see crate::pricing::discounted_total for background \n\
                assert_eq!(1, 1); \
            }\n",
        );
        // String literal carries the qualified path.
        let string_only = (
            "tests/integration_b.rs",
            "#[test] fn smoke_b() { \
                let _doc = \"crate::pricing::discounted_total\"; \
                let _ = _doc; assert_eq!(1, 1); \
            }\n",
        );
        for (path, src) in [comment_only, string_only] {
            let files: Vec<(PathBuf, &str)> = vec![
                (PathBuf::from("src/pricing.rs"), prod_src),
                (PathBuf::from(path), src),
            ];
            let index = index_from_files(&files)?;
            let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
            let predicate = seams
                .iter()
                .find(|s| s.kind() == SeamKind::PredicateBoundary)
                .ok_or_else(|| "predicate seam present".to_string())?;
            let evidence = evidence_for_seam(predicate, &index);
            for grip in &evidence.related_tests {
                assert_ne!(
                    grip.relation_reason,
                    RelationReason::ImportPathAffinity,
                    "qualified path inside comment/string in {path} must not match \
                     ImportPathAffinity; got {grip:?}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn given_owner_and_module_tokens_without_import_path_when_related_tests_are_ranked_then_import_path_affinity_does_not_fire()
    -> Result<(), String> {
        // Body contains `pricing` and `discounted_total` as bare
        // identifiers but never as a `::path::owner_name` shape and
        // never on a `use ...` line. The pre-tightening detector
        // would have fired (owner token + parent dir token both
        // present); the tightened detector must not.
        //
        // The test name embeds "discounted_total" — that is OK because
        // it triggers `owner_named_test`, a *different* reason. The
        // contract under test is "ImportPathAffinity does not fire on
        // mere token co-occurrence".
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/billing.rs",
            "#[test] fn discounted_total_token_smoke() { \
                let pricing = \"pricing\"; let discounted_total = 5; \
                let _ = (pricing, discounted_total); assert_eq!(1, 1); \
            }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        for grip in &evidence.related_tests {
            assert_ne!(
                grip.relation_reason,
                RelationReason::ImportPathAffinity,
                "token co-occurrence (`pricing` + `discounted_total` in body without \
                 `::` path syntax) must not match ImportPathAffinity; got {grip:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn given_same_module_test_without_direct_call_when_related_tests_are_ranked_then_confidence_is_medium()
    -> Result<(), String> {
        // Owner sits in `src/pricing/discount.rs`; test sits in
        // `tests/pricing/integration.rs`. Different file stem (no
        // same_test_file). Same parent module (`pricing`) so
        // `same_module` is the right reason. No direct call, no
        // owner-named test, no qualified path / use line.
        let prod_src = "pub fn apply_discount(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing/integration.rs",
            "#[test] fn module_neighbour() { assert_eq!(1, 1); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing/discount.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing/discount.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let grip = evidence.related_tests.first().ok_or_else(|| {
            "expected at least one related test for same-module pairing".to_string()
        })?;
        assert_eq!(grip.relation_reason, RelationReason::SameModule);
        assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
        Ok(())
    }

    // -- helper coverage ---------------------------------------------
    //
    // Targeted unit tests for the small private helpers introduced by
    // analysis/related-test-precision-v1. The integration BDD tests
    // above exercise the most common paths through `find_related_tests`,
    // but each helper has a few branches that are not naturally hit by
    // a single BDD scenario. The tests below pin those branches so
    // codecov coverage reflects intent rather than scenario count.

    #[test]
    fn relation_reason_as_str_priority_and_confidence_are_pinned_per_variant() {
        // Pin the (variant -> "string", priority, confidence) mapping
        // for every reason. Catches accidental swaps in the match arms
        // of `as_str` / `priority` / `confidence`.
        let table = [
            (
                RelationReason::DirectOwnerCall,
                "direct_owner_call",
                0u8,
                RelationConfidence::High,
            ),
            (
                RelationReason::HelperOwnerCall,
                "helper_owner_call",
                1,
                RelationConfidence::High,
            ),
            (
                RelationReason::AssertionTargetAffinity,
                "assertion_target_affinity",
                2,
                RelationConfidence::Medium,
            ),
            (
                RelationReason::SameTestFile,
                "same_test_file",
                3,
                RelationConfidence::Medium,
            ),
            (
                RelationReason::SameModule,
                "same_module",
                4,
                RelationConfidence::Medium,
            ),
            (
                RelationReason::OwnerNamedTest,
                "owner_named_test",
                5,
                RelationConfidence::Medium,
            ),
            (
                RelationReason::ImportPathAffinity,
                "import_path_affinity",
                6,
                RelationConfidence::Medium,
            ),
            (
                RelationReason::FixtureOwnerAffinity,
                "fixture_owner_affinity",
                7,
                RelationConfidence::Low,
            ),
        ];
        for (reason, name, prio, conf) in table {
            assert_eq!(reason.as_str(), name, "{reason:?}.as_str()");
            assert_eq!(reason.priority(), prio, "{reason:?}.priority()");
            assert_eq!(reason.confidence(), conf, "{reason:?}.confidence()");
        }
    }

    #[test]
    fn relation_confidence_as_str_and_rank_are_pinned_per_variant() {
        let table = [
            (RelationConfidence::High, "high", 0u8),
            (RelationConfidence::Medium, "medium", 1),
            (RelationConfidence::Low, "low", 2),
            (RelationConfidence::Opaque, "opaque", 3),
        ];
        for (conf, name, rank) in table {
            assert_eq!(conf.as_str(), name, "{conf:?}.as_str()");
            assert_eq!(conf.rank(), rank, "{conf:?}.rank()");
        }
    }

    #[test]
    fn required_discriminator_tokens_extracts_text_from_every_variant() {
        use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator};
        let make = |rd: RequiredDiscriminator| {
            RepoSeam::new(
                "src/x.rs",
                "x::owner",
                SeamKind::PredicateBoundary,
                0,
                1,
                "irrelevant",
                rd,
                ExpectedSink::ReturnValue,
            )
        };
        // Each arm carries a distinctive token so we can confirm the
        // right field was picked. Tokens longer than 2 chars survive
        // `is_interesting_token`.
        let cases: Vec<(RequiredDiscriminator, &str)> = vec![
            (
                RequiredDiscriminator::BoundaryValue {
                    description: "boundary_token".to_string(),
                },
                "boundary_token",
            ),
            (
                RequiredDiscriminator::ReturnValue {
                    description: "returnval_token".to_string(),
                },
                "returnval_token",
            ),
            (
                RequiredDiscriminator::ErrorVariant {
                    variant: "errvar_token".to_string(),
                },
                "errvar_token",
            ),
            (
                RequiredDiscriminator::FieldValue {
                    field: "fieldval_token".to_string(),
                },
                "fieldval_token",
            ),
            (
                RequiredDiscriminator::Effect {
                    sink: "effect_token".to_string(),
                },
                "effect_token",
            ),
            (
                RequiredDiscriminator::MatchArmTaken {
                    arm: "matcharm_token".to_string(),
                },
                "matcharm_token",
            ),
            (
                RequiredDiscriminator::CallSite {
                    target: "callsite_token".to_string(),
                },
                "callsite_token",
            ),
        ];
        for (rd, expected_token) in cases {
            let seam = make(rd.clone());
            let tokens = required_discriminator_tokens(&seam);
            assert!(
                tokens.iter().any(|t| t == expected_token),
                "{rd:?} -> tokens {tokens:?} must contain {expected_token}"
            );
        }
    }

    #[test]
    fn same_test_file_accepts_stem_match_and_test_suffixes() {
        assert!(same_test_file(Path::new("tests/foo.rs"), "foo"));
        assert!(same_test_file(Path::new("tests/foo_test.rs"), "foo"));
        assert!(same_test_file(Path::new("tests/foo_tests.rs"), "foo"));
        assert!(!same_test_file(Path::new("tests/bar.rs"), "foo"));
        assert!(!same_test_file(Path::new(""), "foo"));
    }

    #[test]
    fn module_path_for_handles_every_root_shape() {
        let cases: Vec<(&str, Option<&str>)> = vec![
            ("src/foo.rs", Some("foo")),
            ("tests/cli_smoke.rs", Some("cli_smoke")),
            ("crates/ripr/src/auth/login.rs", Some("auth/login")),
            ("crates/ripr/tests/integration.rs", Some("integration")),
            ("docs/note.rs", None),
            // `body = ".rs"` after stripping `src/`; trimmed = "" → None.
            ("src/.rs", None),
        ];
        for (input, expected) in cases {
            let got = module_path_for(Path::new(input));
            let want = expected.map(str::to_string);
            assert_eq!(got, want, "module_path_for({input})");
        }
    }

    #[test]
    fn same_module_matches_parent_prefix_and_underscore_form() {
        assert!(same_module("pricing/discount", "pricing/integration"));
        assert!(same_module("a/b/c", "a_b/d"));
        assert!(!same_module("flat", "anything"));
        assert!(!same_module("pricing/discount", "billing/integration"));
    }

    #[test]
    fn is_fixture_named_recognises_each_prefix_and_suffix() {
        let positives = [
            "fixture_quote",
            "setup_db",
            "make_quote",
            "build_request",
            "new_user",
            "mock_clock",
            "quote_fixture",
            "quote_factory",
        ];
        for name in positives {
            assert!(is_fixture_named(name), "{name} should be fixture-named");
        }
        for name in ["compute_total", "discount", "verify"] {
            assert!(
                !is_fixture_named(name),
                "{name} should NOT be fixture-named"
            );
        }
    }

    #[test]
    fn given_assertion_target_token_in_test_assertion_when_related_tests_are_ranked_then_assertion_target_affinity_fires()
    -> Result<(), String> {
        // Positive case for `assertion_target_affinity`: the seam's
        // `RequiredDiscriminator::BoundaryValue.description` carries
        // the identifier `discount_threshold`; a test assertion that
        // mentions `discount_threshold` as a whole identifier matches.
        // The test does not call the owner directly, the test file
        // stem is unrelated, and the test name does not embed the
        // owner — so this is the only reason that fires.
        let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/billing.rs",
            "fn other() -> i32 { 0 }\n\
             #[test] fn smoke() { let discount_threshold = 5; assert_eq!(discount_threshold, 5); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        assert_eq!(
            grip.relation_reason,
            RelationReason::AssertionTargetAffinity
        );
        assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
        Ok(())
    }

    #[test]
    fn given_generic_option_match_arm_binding_when_related_tests_are_ranked_then_assertion_target_affinity_does_not_fire()
    -> Result<(), String> {
        let prod_src = r#"
pub fn outcome_command(out_path: Option<&str>) -> &'static str {
    match out_path {
        Some(path) => path,
        None => "missing",
    }
}
"#;
        let test = (
            "tests/status_contract.rs",
            r#"
#[test]
fn path_word_is_not_owner_evidence() {
    let path = "target/ripr/outcome.json";
    assert_eq!(path, "target/ripr/outcome.json");
}
"#,
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/commands.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/commands.rs")], &index);
        let some_arm = seams
            .iter()
            .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("Some(path)"))
            .ok_or_else(|| "expected Some(path) match-arm seam".to_string())?;
        let evidence = evidence_for_seam(some_arm, &index);

        assert!(
            evidence
                .related_tests
                .iter()
                .all(|test| { test.relation_reason != RelationReason::AssertionTargetAffinity }),
            "generic match-arm binding token should not create assertion-target affinity: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_specific_match_arm_variant_when_related_tests_are_ranked_then_assertion_target_affinity_still_fires()
    -> Result<(), String> {
        let prod_src = r#"
pub enum Mode {
    Json,
    Text,
}

pub fn render_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Json => "json",
        Mode::Text => "text",
    }
}
"#;
        let test = (
            "tests/render_contract.rs",
            r#"
#[test]
fn mentions_json_variant() {
    let rendered = "Json";
    assert_eq!(rendered, "Json");
}
"#,
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/render.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/render.rs")], &index);
        let json_arm = seams
            .iter()
            .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("Mode::Json"))
            .ok_or_else(|| "expected Mode::Json match-arm seam".to_string())?;
        let evidence = evidence_for_seam(json_arm, &index);

        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
            "specific enum variant should still support assertion-target affinity: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_match_arm_expected_sink_token_when_related_tests_are_ranked_then_assertion_target_affinity_does_not_fire()
    -> Result<(), String> {
        let prod_src = r#"
pub fn parse_c_escape(ch: char) -> char {
    match ch {
        '"' => '"',
        '\\' => '\\',
        _ => ch,
    }
}
"#;
        let test = (
            "tests/path_contract.rs",
            r#"
#[test]
fn return_value_word_is_not_match_arm_evidence() {
    let return_value = '"';
    assert_eq!(return_value, '"');
}
"#,
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/diff_path.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/diff_path.rs")], &index);
        let quote_arm = seams
            .iter()
            .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("'\"'"))
            .ok_or_else(|| "expected quote match-arm seam".to_string())?;
        let evidence = evidence_for_seam(quote_arm, &index);

        assert!(
            evidence
                .related_tests
                .iter()
                .all(|test| { test.relation_reason != RelationReason::AssertionTargetAffinity }),
            "generic expected-sink token should not create match-arm assertion-target affinity: {:?}",
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn assertion_targets_seam_returns_false_for_empty_token_list() {
        // The `tokens.is_empty()` early-return is the cheap escape
        // hatch when a seam's `RequiredDiscriminator` carries no
        // interesting tokens (e.g. a one-character variable name).
        use crate::analysis::rust_index::TestFact;
        let test = TestFact {
            name: "synth".to_string(),
            file: PathBuf::from("tests/x.rs"),
            start_line: 1,
            end_line: 5,
            body: "assert_eq!(1, 1);".to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        };
        assert!(!assertion_targets_seam(&test, &[]));
    }

    #[test]
    fn package_prefix_resolves_crates_and_nested_src_tests_layouts() {
        // `crates/<name>/src/...` form returns the `crates/<name>/` prefix.
        assert_eq!(
            package_prefix(Path::new("crates/ripr/src/auth/login.rs")).as_deref(),
            Some("crates/ripr/")
        );
        // `crates/<name>/tests/...` form (the second branch of the
        // strip_prefix-and-or guard) also returns the package prefix.
        assert_eq!(
            package_prefix(Path::new("crates/ripr/tests/integration.rs")).as_deref(),
            Some("crates/ripr/")
        );
        // Nested workspace path (rfind branch): the marker scan falls
        // through to the `/src/` rfind path.
        assert_eq!(
            package_prefix(Path::new("workspaces/foo/src/auth/login.rs")).as_deref(),
            Some("workspaces/foo/")
        );
        // Bare `src/...` returns None (prefix would be empty).
        assert_eq!(package_prefix(Path::new("src/foo.rs")), None);
        // Path under neither root.
        assert_eq!(package_prefix(Path::new("docs/note.rs")), None);
    }

    #[test]
    fn given_owner_in_workspace_crate_when_test_is_in_other_crate_then_it_is_filtered_out()
    -> Result<(), String> {
        // Owner lives in `crates/ripr_pricing/src/discount.rs`; a test
        // in a different package (`crates/ripr_other/tests/x.rs`)
        // must not appear as a related test, even if it would
        // otherwise satisfy a reason. Exercises the package-prefix
        // skip branch in `find_related_tests`.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let other_pkg_test = (
            "crates/ripr_other/tests/x.rs",
            "#[test] fn discounted_total_other_pkg() { assert_eq!(1, 1); }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (
                PathBuf::from("crates/ripr_pricing/src/discount.rs"),
                prod_src,
            ),
            (PathBuf::from(other_pkg_test.0), other_pkg_test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(
            &[PathBuf::from("crates/ripr_pricing/src/discount.rs")],
            &index,
        );
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        for grip in &evidence.related_tests {
            assert_ne!(
                grip.file,
                Path::new("crates/ripr_other/tests/x.rs"),
                "test in unrelated package should be filtered by package_prefix; \
                 got {grip:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn given_test_calls_helper_with_fixture_attribute_then_fixture_owner_affinity_fires()
    -> Result<(), String> {
        // `test_uses_owner_fixture` accepts EITHER a fixture-named
        // helper OR a helper whose body contains `#[fixture]`. The
        // earlier `given_fixture_only_affinity_…` test exercises the
        // name-based branch (`make_quote`); this one exercises the
        // body-marker branch by using a non-fixture helper name but
        // placing the `#[fixture]` marker as an inline comment inside
        // the body. `FunctionFact.body` slices from the `fn` keyword
        // to the end of the function, so attributes ABOVE the `fn`
        // line are not captured — the marker must live inside the
        // body block.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn provide_quote() -> i32 {\n    // #[fixture]\n    100\n}\n";
        let test = (
            "tests/integration.rs",
            "#[test] fn quote_smoke() { let _ = provide_quote(); assert!(true); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        assert_eq!(grip.relation_reason, RelationReason::FixtureOwnerAffinity);
        Ok(())
    }

    // -- value-extraction-v2 ------------------------------------------
    //
    // Each test exercises one resolution path through `activate_evidence`:
    // a related test calls the seam owner, the call arg is something
    // `scalar_values` would reject (bare identifier, builder method,
    // table row, rstest case, Some/Err wrapper), and the resolver in
    // `analysis::value_resolution` should turn it into observed values
    // - which `evidence_for_seam` then exposes via
    // `TestGripEvidence.observed_values`. The negative tests pin the
    // false-positive guards for comment/string shadows and unrelated
    // identifiers.

    fn observed_values_for(prod_src: &str, tests: &[(&str, &str)]) -> Result<Vec<String>, String> {
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        for (path, src) in tests {
            files.push((PathBuf::from(*path), *src));
        }
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        Ok(evidence
            .observed_values
            .into_iter()
            .map(|v| v.value)
            .collect())
    }

    #[test]
    fn given_boundary_owner_call_when_argument_is_path_constructor_then_observed_values_are_resolved()
    -> Result<(), String> {
        let source = PathBuf::from("src/agent/loop_commands.rs");
        let source_src = r#"
use std::path::Path;

pub fn workflow_artifact_path(out_dir: &Path, file_name: &str) -> String {
    let out_dir = display_path(out_dir);
    if out_dir == "." {
        file_name.to_string()
    } else {
        format!("{}/{}", out_dir.trim_end_matches('/'), file_name)
    }
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_artifact_path_uses_output_directory() {
        let path = workflow_artifact_path(Path::new("target/ripr/workflow"), "workflow.json");
        assert_eq!(path, "target/ripr/workflow/workflow.json");
    }
}
"#;
        let index = index_from_files(&[(source, source_src)])?;
        let seams =
            inventory_seams_from_index(&[PathBuf::from("src/agent/loop_commands.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary && s.expression().contains("=="))
            .ok_or_else(|| "out_dir predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let values: Vec<String> = evidence
            .observed_values
            .iter()
            .map(|value| value.value.clone())
            .collect();

        assert!(
            values
                .iter()
                .any(|value| value == "\"target/ripr/workflow\""),
            "Path::new literal should become a concrete observed activation value; got {values:?}"
        );
        assert_eq!(evidence.activate.state, StageState::Yes);
        Ok(())
    }

    #[test]
    fn given_let_binding_values_when_owner_call_uses_identifiers_then_observed_values_are_resolved()
    -> Result<(), String> {
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { let amount = 100; let threshold = 100; \
             assert_eq!(discounted_total(amount, threshold), 90); }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "let-resolved 100 must appear in observed values; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_shared_borrowed_let_binding_when_owner_call_uses_reference_then_observed_value_is_resolved()
    -> Result<(), String> {
        let prod_src = "pub fn amount_matches(amount: &i32) -> bool { amount == &100 }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn borrowed_amount_reference() { \
                 let amount = 100; \
                 let actual = amount_matches(&amount); \
                 assert!(actual); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let values: Vec<String> = evidence
            .observed_values
            .iter()
            .map(|value| value.value.clone())
            .collect();
        assert!(
            values.iter().any(|value| value == "100"),
            "borrowed let binding literal must appear in observed values; got {values:?}; activation: {}; related: {:?}",
            evidence.activate.summary,
            evidence.related_tests
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_threshold_is_parameter_then_observed_values_stay_on_input_operand()
    -> Result<(), String> {
        let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert_eq!(
            values,
            vec!["50".to_string()],
            "observed values should describe the tested input operand, not the boundary parameter"
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_input_operand_is_direct_parameter_alias_then_observed_values_are_resolved()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: i32, threshold: i32) -> i32 {
    let amount = raw_amount;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert_eq!(
            values,
            vec!["50".to_string()],
            "direct local aliases of owner parameters should resolve to the original owner-call argument"
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_if_let_alias_parameter_name_has_prefix_then_exact_parameter_is_used()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, raw_amount_extra: Option<i32>, threshold: i32) -> i32 {
    if let Some(amount) = raw_amount_extra {
        if amount >= threshold { amount - 10 } else { amount }
    } else {
        0
    }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(Some(50), Some(60), 50), 60); \
             }\n",
        );
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("amount >= threshold")
            })
            .ok_or_else(|| "amount predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let values: Vec<String> = evidence
            .observed_values
            .iter()
            .map(|v| v.value.clone())
            .collect();
        assert_eq!(
            values,
            vec!["60".to_string()],
            "prefix parameter matches must resolve amount from raw_amount_extra, not raw_amount"
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_match_alias_is_comment_then_operand_stays_unresolved()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    // match raw_amount { Some(amount) => if amount >= threshold { amount - 10 } else { amount }, _ => 0 }
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
        );
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("amount >= threshold")
            })
            .ok_or_else(|| "amount predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        assert!(
            evidence.observed_values.is_empty(),
            "commented match aliases must not resolve boundary operands; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unresolved commented match alias should stay a limitation, not an exact repair candidate; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_inline_match_alias_is_comment_then_operand_stays_unresolved()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _note = 0; // match raw_amount { Some(amount) => if amount >= threshold { amount - 10 } else { amount }, _ => 0 }
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
        );
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("amount >= threshold")
            })
            .ok_or_else(|| "amount predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        assert!(
            evidence.observed_values.is_empty(),
            "inline commented match aliases must not resolve boundary operands; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unresolved inline commented match alias should stay a limitation, not an exact repair candidate; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_match_wrapper_is_comment_then_operand_stays_unresolved()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _seen = match raw_amount { _ => false };
    // Some(amount)
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
        );
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("amount >= threshold")
            })
            .ok_or_else(|| "amount predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        assert!(
            evidence.observed_values.is_empty(),
            "commented wrapper patterns must not resolve boundary operands; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unresolved commented wrapper pattern should stay a limitation, not an exact repair candidate; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_inline_match_wrapper_is_comment_then_operand_stays_unresolved()
    -> Result<(), String> {
        let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _seen = match raw_amount { _ => false }; // Some(amount)
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
        );
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("amount >= threshold")
            })
            .ok_or_else(|| "amount predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        assert!(
            evidence.observed_values.is_empty(),
            "inline commented wrapper patterns must not resolve boundary operands; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unresolved inline commented wrapper should stay a limitation, not an exact repair candidate; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_input_operand_is_iterator_local_then_activation_is_static_limitation()
    -> Result<(), String> {
        let prod_src = "pub fn sum_from_offset(values: &[i32], offset: usize) -> i32 { \
                            let mut total = 0; \
                            for (idx, value) in values.iter().enumerate() { \
                                if idx >= offset { total += *value; } \
                            } \
                            total \
                        }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn sums_after_offset() { \
                 assert_eq!(sum_from_offset(&[1, 2, 3], 1), 5); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.activate.summary.contains("iterator-derived"),
            "unresolved iterator-local boundary must explain why it is limited; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "iterator-local activation values must not be invented from owner-call args; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "unresolved iterator-local boundary must not emit exact candidate discriminator; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_input_operand_is_computed_local_then_activation_stays_static_limitation()
    -> Result<(), String> {
        let prod_src = "pub fn discounted_total(raw_amount: i32, threshold: i32) -> i32 { \
                            let amount = raw_amount + 1; \
                            if amount >= threshold { amount - 10 } else { amount } \
                        }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 51); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.activate.summary.contains("local or computed"),
            "computed local boundary operands must remain a named limitation; got {}",
            evidence.activate.summary
        );
        assert!(
            !evidence.activate.summary.contains("iterator-derived"),
            "computed local boundary operands must not be routed as iterator-derived; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "computed local activation values must not be invented from owner-call args; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "computed local boundary operands must not emit exact candidate discriminator; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_non_comparison_predicate_when_direct_owner_call_exists_then_activation_is_value_insensitive()
    -> Result<(), String> {
        let prod_src = "pub fn has_missing(missing: &[String]) -> bool { \
                            if missing.is_empty() { true } else { false } \
                        }\n";
        let test = (
            "tests/missing_tests.rs",
            "#[test] fn empty_missing_is_true() { \
                 assert!(has_missing(&[])); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/missing.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/missing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("missing.is_empty()")
            })
            .ok_or_else(|| "non-comparison predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .activate
                .summary
                .contains("direct owner call for value-insensitive seam"),
            "non-comparison predicates should not require scalar activation values; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "non-comparison predicate activation must not invent collection literal values; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "non-comparison predicates must not emit exact boundary candidates; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_input_operand_is_closure_local_then_activation_is_static_limitation()
    -> Result<(), String> {
        let prod_src = "pub struct FunctionSummary { pub id: &'static str } \
                        pub fn has_owner(functions: &[FunctionSummary], owner: &str) -> bool { \
                            functions.iter().any(|function| function.id == owner) \
                        }\n";
        let test = (
            "tests/owner_tests.rs",
            "#[test] fn finds_owner() { \
                 let functions = [FunctionSummary { id: \"score\" }]; \
                 assert!(has_owner(&functions, \"score\")); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/owner.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/owner.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::PredicateBoundary
                    && s.expression().contains("function.id == owner")
            })
            .ok_or_else(|| "closure predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.activate.summary.contains("closure-derived"),
            "closure boundary operands must be routed precisely; got {}",
            evidence.activate.summary
        );
        assert!(
            !evidence.activate.summary.contains("iterator-derived"),
            "closure boundary operands must not be routed as iterator-derived; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "closure-local activation values must not be invented from owner-call args; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "closure-local boundary operands must not emit exact candidate discriminator; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn closure_boundary_operand_route_ignores_comment_only_closure_pattern() {
        let owner = FunctionSummary {
            id: crate::domain::SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 5,
            body: "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    // values.iter().any(|amount| amount >= threshold)\n    let amount = raw_amount + 1;\n    amount >= threshold\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        };

        assert!(!boundary_operand_is_closure_derived(&owner, "amount"));
    }

    #[test]
    fn given_boundary_owner_call_when_parameter_field_operands_resolve_then_activation_is_yes()
    -> Result<(), String> {
        let prod_src = "pub struct BoundarySide { pub value: i32 } \
                        pub fn equal_boundary(left: BoundarySide, right: BoundarySide) -> bool { \
                            left.value == right.value \
                        }\n";
        let test = (
            "tests/boundary_tests.rs",
            "#[test] fn equal_field_boundary() { \
                 let left = BoundarySide { value: 10 }; \
                 let right = BoundarySide { value: 10 }; \
                 assert!(equal_boundary(left, right)); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/boundary.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/boundary.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .activate
                .summary
                .contains("Observed 1 concrete activation value(s)"),
            "parameter field operands must resolve through same-test struct field bindings; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence
                .observed_values
                .iter()
                .any(|fact| fact.value == "10" && fact.context == ValueContext::FunctionArgument),
            "expected resolved struct-field activation value; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "field operands with equal observed values must not emit boundary repair debt; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn given_boundary_owner_call_when_parameter_field_operands_are_opaque_then_activation_stays_static_limitation()
    -> Result<(), String> {
        let prod_src = "pub struct BoundarySide { pub value: i32 } \
                        pub fn equal_boundary(left: BoundarySide, right: BoundarySide) -> bool { \
                            left.value == right.value \
                        }\n";
        let test = (
            "tests/boundary_tests.rs",
            "#[test] fn equal_field_boundary() { \
                 let left = make_side(10); \
                 let right = make_side(10); \
                 assert!(equal_boundary(left, right)); \
             }\n",
        );
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/boundary.rs"), prod_src),
            (PathBuf::from(test.0), test.1),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/boundary.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);

        assert_eq!(evidence.activate.state, StageState::Unknown);
        assert!(
            evidence.activate.summary.contains("local or computed"),
            "opaque parameter field operands must remain a named limitation; got {}",
            evidence.activate.summary
        );
        assert!(
            evidence.observed_values.is_empty(),
            "opaque parameter field operands must not invent activation values; got {:?}",
            evidence.observed_values
        );
        assert!(
            evidence.missing_discriminators.is_empty(),
            "opaque parameter field operands must not emit exact candidate discriminator; got {:?}",
            evidence.missing_discriminators
        );
        Ok(())
    }

    #[test]
    fn iterator_boundary_operand_route_only_matches_iterator_loop_bindings() {
        for source in [
            "for (idx, value) in values.iter().enumerate() {",
            "for item in values.iter() {",
            "for item in values.iter_mut() {",
            "for item in values.into_iter() {",
            "for key in values.keys() {",
            "for value in values.values() {",
            "if ready { for idx in values.iter() {",
        ] {
            let operand = if source.contains("idx") {
                "idx"
            } else if source.contains("key") {
                "key"
            } else if source.contains("value in") {
                "value"
            } else {
                "item"
            };
            assert!(
                loop_binds_operand_from_iterator(source, operand),
                "iterator loop should bind {operand}: {source}"
            );
        }

        for (source, operand) in [
            ("let idx = offset + 1;", "idx"),
            ("for idx in 0..values.len() {", "idx"),
            ("for (idx, value) in values.iter().enumerate() {", "offset"),
            ("perform idx boundary checks", "idx"),
        ] {
            assert!(
                !loop_binds_operand_from_iterator(source, operand),
                "non-iterator or unbound operand must not match: {source}"
            );
        }

        assert!(is_boundary_operand_identifier("idx"));
        assert!(is_boundary_operand_identifier("_idx2"));
        assert!(!is_boundary_operand_identifier("idx + 1"));
        assert!(!is_boundary_operand_identifier("100"));
    }

    #[test]
    fn given_same_file_const_when_owner_call_uses_identifier_then_observed_value_is_resolved()
    -> Result<(), String> {
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "const THRESHOLD: i32 = 100;\n\
             #[test] fn at_threshold() { \
                 assert_eq!(discounted_total(THRESHOLD, THRESHOLD), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "const-resolved 100 must appear; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_table_driven_cases_when_owner_call_uses_row_values_then_each_case_value_is_recorded()
    -> Result<(), String> {
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn table() { \
                 for (amount, threshold, expected) in [(50, 100, 50), (100, 100, 90)] { \
                     assert_eq!(discounted_total(amount, threshold), expected); \
                 } \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "50"),
            "table row value 50 must appear; got {values:?}"
        );
        assert!(
            values.iter().any(|v| v == "100"),
            "table row value 100 must appear; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_option_result_constructor_when_owner_call_uses_shape_then_inner_value_is_recorded()
    -> Result<(), String> {
        // Owner takes a wrapped value; test calls with Some(literal).
        // Resolver should peel one level and emit the inner literal.
        let prod_src = "pub fn process(value: Option<i32>, threshold: i32) -> i32 \
                        { match value { Some(v) if v >= threshold => v - 10, _ => 0 } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_boundary() { \
                 assert_eq!(process(Some(100), 100), 90); \
             }\n",
        );
        // The seam in this case is the predicate inside `process`.
        let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
        files.push((PathBuf::from(test.0), test.1));
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        let values: Vec<String> = evidence
            .observed_values
            .iter()
            .map(|v| v.value.clone())
            .collect();
        assert!(
            values.iter().any(|v| v == "100"),
            "Some(100) must unwrap and contribute 100; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_builder_methods_matching_parameter_tokens_then_observed_values_are_recorded()
    -> Result<(), String> {
        // The seam's required-discriminator description carries the
        // identifiers `amount` and `discount_threshold`. A test that
        // builds a value via `.amount(100).discount_threshold(100)`
        // should have those literals counted as observed via the
        // BuilderMethod context. Owner name unused inside the builder
        // call — the test references the owner directly elsewhere so
        // it qualifies as related.
        let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_builder() { \
                 let q = Quote::new().amount(100).discount_threshold(100).build(); \
                 assert_eq!(discounted_total(q.amount, q.discount_threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        // `amount` and `discount_threshold` are seam-discriminator
        // tokens, so the builder method facts should land.
        assert!(
            values.iter().filter(|v| v.as_str() == "100").count() >= 1,
            "builder method 100 must be recorded; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_fixture_factory_override_methods_matching_seam_tokens_then_values_are_recorded()
    -> Result<(), String> {
        // Fixture factories often use explicit override method names
        // like `with_amount`. These should count when the wrapped
        // method token aligns with the changed seam.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_fixture_override() { \
                 let q = QuoteFixture::default().with_amount(100).with_threshold(100).build(); \
                 assert_eq!(discounted_total(q.amount, q.threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().filter(|v| v.as_str() == "100").count() >= 1,
            "fixture override 100 must be recorded; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_same_test_struct_literal_fields_when_owner_call_uses_projection_then_values_are_recorded()
    -> Result<(), String> {
        // Same-test struct literals are a syntactic fixture shape:
        // the field values are explicit in the test body and the owner
        // call passes those field projections directly.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_struct_literal() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "same-test struct literal field value 100 must be recorded; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_same_line_struct_literal_after_owner_call_then_projection_values_stay_unresolved()
    -> Result<(), String> {
        // Call facts preserve only the line and full text, so
        // same-line ordering must stay conservative. A literal that
        // appears after the owner call cannot explain that call's
        // field projections.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_same_line_late_literal() { \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let _ = case; \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "same-line literals introduced after the owner call must not produce fake values; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_struct_literal_shadowed_after_owner_call_then_values_are_recorded()
    -> Result<(), String> {
        // Source order matters for the fixture shape: a later shadow
        // should not erase a direct owner call that already used the
        // same-test literal field projection.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test]\nfn via_later_shadowed_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 let case = make_discount_case();\n    \
                 let _ = case;\n\
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "later-shadowed struct literal field value 100 must still be recorded for the earlier owner call; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_struct_literal_after_owner_call_then_projection_values_stay_unresolved()
    -> Result<(), String> {
        // A later literal cannot explain a field projection that reached
        // the owner before the literal binding existed.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test]\nfn via_late_literal_fixture() {\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 let _ = case;\n\
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "literal struct fields introduced after the owner call must not produce fake values; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_struct_literal_field_mutated_before_owner_call_then_values_stay_unresolved()
    -> Result<(), String> {
        // A mutation before the owner call makes the original literal
        // field stale for activation-value evidence.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test]\nfn via_mutated_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 case.amount = make_amount();\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n\
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "mutated struct literal fields must not reuse stale literal values; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_struct_literal_field_mutated_after_owner_call_then_values_are_recorded()
    -> Result<(), String> {
        // A mutation after the owner call should not erase the literal
        // value observed by the earlier owner call.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test]\nfn via_later_mutated_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 case.amount = make_amount();\n\
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "later-mutated struct literal field value 100 must still be recorded for the earlier owner call; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_helper_built_struct_when_owner_call_uses_projection_then_values_stay_unresolved()
    -> Result<(), String> {
        // Do not infer through helper-returned fixtures. Without the
        // literal struct body in the same test, `case.amount` remains
        // an opaque activation value and should stay a named static
        // limitation instead of becoming user test debt.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_helper_fixture() { \
                 let case = make_discount_case(); \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "helper-built struct projections must not produce fake values; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_shadowed_struct_literal_when_owner_call_uses_projection_then_values_stay_unresolved()
    -> Result<(), String> {
        // A same-test literal stops being a safe activation value once
        // the binding is shadowed before the owner call. The resolver
        // deliberately avoids reusing stale literal fields after the
        // shadowing line.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_shadowed_fixture() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let case = make_discount_case(); \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "shadowed struct projections must not reuse stale literal fields; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_fixture_parameter_and_later_same_name_struct_literal_then_values_stay_unresolved()
    -> Result<(), String> {
        // A fixture/rstest parameter is runtime-provided. A later
        // same-name literal in the same body cannot safely explain the
        // earlier owner-call field projection.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[rstest] fn via_fixture_param(case: DiscountCase) { \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let _ = case; \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "fixture parameter projections must not resolve from later literals; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_for_loop_shadowing_struct_literal_then_values_stay_unresolved() -> Result<(), String> {
        // The whole-test literal map cannot safely explain a projection
        // when a later loop binder reuses the same identifier.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_shadowing_loop() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 for case in helper_cases() { \
                     assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 } \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "loop-shadowed struct projections must not reuse stale literal fields; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_let_pattern_shadowing_struct_literal_then_values_stay_unresolved() -> Result<(), String>
    {
        // Non-simple let patterns can bind a fresh value under the same
        // name. Without source-order scope, that remains a limitation.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_shadowing_let_pattern() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let Some(case) = make_discount_case() else { return; }; \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.is_empty(),
            "let-pattern-shadowed projections must not reuse stale literal fields; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_builder_method_with_unrelated_name_then_value_is_not_counted_for_seam_activation()
    -> Result<(), String> {
        // `.with_seed(42)` is a builder method whose name does NOT
        // align with any seam token. The value 42 must NOT appear
        // among observed values for this seam, even though the test
        // directly calls the owner.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn via_unrelated_builder() { \
                 let _q = Foo::new().with_seed(42).build(); \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            !values.iter().any(|v| v == "42"),
            "unrelated builder literal 42 must NOT count; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_unrelated_string_literal_mentions_value_when_extracting_values_then_no_observed_discriminator_is_recorded()
    -> Result<(), String> {
        // String literal in the body mentions `100` and `threshold`
        // but the call site uses an unresolved identifier. v2 must
        // not pull literals out of strings.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn string_only() { \
                 let _doc = \"threshold = 100\"; \
                 let unresolved = make_amount(); \
                 assert_eq!(discounted_total(unresolved, unresolved), 0); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            !values.iter().any(|v| v == "100"),
            "string literal 100 must NOT be observed; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_shared_fixture_module_constant_when_extracting_v2_values_then_no_cross_file_value_is_resolved()
    -> Result<(), String> {
        // Strict syntactic scope: cross-file constants must NOT
        // resolve. The const lives in tests/common/mod.rs; the test
        // lives in tests/pricing_tests.rs. v2 is single-file scope -
        // cross-file resolution is a future item and must not creep
        // in via "helpful" expansion.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let common = (
            "tests/common/mod.rs",
            "pub const SHARED_THRESHOLD: i32 = 100;\n",
        );
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn cross_file() { \
                 assert_eq!(discounted_total(SHARED_THRESHOLD, SHARED_THRESHOLD), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test, common])?;
        assert!(
            !values.iter().any(|v| v == "100"),
            "cross-file SHARED_THRESHOLD = 100 must NOT resolve in v2; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_let_binding_shadowed_by_comment_when_extracting_then_real_binding_wins()
    -> Result<(), String> {
        // Mirrors #310's comment-stripping defense: a `// let amount = 999;`
        // comment must NOT shadow the real `let amount = 100;` binding.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn at_threshold() { \
                 // let amount = 999; let threshold = 999;\n\
                 let amount = 100; let threshold = 100; \
                 assert_eq!(discounted_total(amount, threshold), 90); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        assert!(
            values.iter().any(|v| v == "100"),
            "real let binding 100 must be observed; got {values:?}"
        );
        assert!(
            !values.iter().any(|v| v == "999"),
            "commented-out let binding 999 must NOT be observed; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn given_unresolved_identifier_arg_when_extracting_values_then_no_observed_value_is_recorded()
    -> Result<(), String> {
        // Identifier resolved through a helper call (no `let` binding,
        // no const, no rstest case, no table row, no Some wrapper).
        // Must stay opaque — the previous behavior is preserved for
        // the unresolved case.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
        let test = (
            "tests/pricing_tests.rs",
            "#[test] fn opaque() { \
                 let amount = make_amount(); \
                 let threshold = make_threshold(); \
                 assert_eq!(discounted_total(amount, threshold), 0); \
             }\n",
        );
        let values = observed_values_for(prod_src, &[test])?;
        // The let RHS isn't a literal, so the binding shouldn't
        // resolve. observed_values for these args should stay empty.
        assert!(
            values.is_empty()
                || values
                    .iter()
                    .all(|v| !matches!(v.as_str(), "100" | "0" | "make_amount")),
            "opaque args must not produce a fake observed value; got {values:?}"
        );
        Ok(())
    }

    #[test]
    fn same_file_test_helper_call_counts_as_owner_call_evidence() {
        let file = PathBuf::from("src/pricing.rs");
        let index = RustIndex {
            functions: vec![FunctionSummary {
                id: crate::domain::SymbolId("src/pricing.rs::discounted_total".to_string()),
                name: "discounted_total".to_string(),
                file: file.clone(),
                start_line: 1,
                end_line: 5,
                body: "pub fn discounted_total(amount: i32, threshold: i32) -> i32 { if amount >= threshold { amount - 10 } else { amount } }".to_string(),
                calls: Vec::new(),
                returns: Vec::new(),
                literals: Vec::new(),
                is_test: false,
                attrs: Vec::new(),
            }, FunctionSummary {
                id: crate::domain::SymbolId("src/pricing.rs::case_at_threshold".to_string()),
                name: "case_at_threshold".to_string(),
                file: file.clone(),
                start_line: 10,
                end_line: 12,
                body: "fn case_at_threshold() -> i32 { discounted_total(100, 100) }".to_string(),
                calls: vec![CallFact {
                    line: 11,
                    name: "discounted_total".to_string(),
                    text: "discounted_total(100, 100)".to_string(),
                }],
                returns: Vec::new(),
                literals: Vec::new(),
                is_test: false,
                attrs: Vec::new(),
            }],
            tests: vec![TestSummary {
                name: "unit_test_uses_same_file_helper".to_string(),
                file: file.clone(),
                start_line: 20,
                end_line: 23,
                body: "#[test] fn unit_test_uses_same_file_helper() { assert_eq!(case_at_threshold(), 90); }".to_string(),
                calls: vec![CallFact {
                    line: 21,
                    name: "case_at_threshold".to_string(),
                    text: "case_at_threshold()".to_string(),
                }],
                assertions: vec![OracleFact {
                    line: 21,
                    kind: OracleKind::ExactValue,
                    strength: OracleStrength::Strong,
                    text: "assert_eq!(case_at_threshold(), 90)".to_string(),
                    observed_tokens: Vec::new(),
                }],
                literals: Vec::new(),
                attrs: Vec::new(),
            }],
            ..RustIndex::default()
        };

        let context = CompactGripContext::new(&index);

        assert!(
            context.tests[0]
                .helper_owner_call_names
                .contains("discounted_total"),
            "same-file helper call must prove the production owner call"
        );
        assert_eq!(
            context
                .tests_by_helper_owner_call_name
                .get("discounted_total")
                .cloned()
                .unwrap_or_default(),
            vec![0]
        );
    }
}
