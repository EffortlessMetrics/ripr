use super::*;
use std::cell::{OnceCell, RefCell};

/// Precomputed per-test facts for repo seam evidence consumers. This
/// avoids repeatedly tokenizing the same test assertions and import
/// lines while classifying every seam in a workspace.
pub(crate) struct CompactGripContext<'a> {
    pub(in crate::analysis::test_grip_evidence) index: &'a RustIndex,
    pub(in crate::analysis::test_grip_evidence) tests: Vec<CompactTest<'a>>,
    pub(in crate::analysis::test_grip_evidence) tests_by_call_name: BTreeMap<String, Vec<usize>>,
    pub(in crate::analysis::test_grip_evidence) tests_by_helper_owner_call_name:
        BTreeMap<String, Vec<usize>>,
    pub(in crate::analysis::test_grip_evidence) tests_by_assertion_token:
        BTreeMap<String, Vec<usize>>,
    pub(in crate::analysis::test_grip_evidence) tests_by_file_stem: BTreeMap<String, Vec<usize>>,
    pub(in crate::analysis::test_grip_evidence) tests_by_import_token: BTreeMap<String, Vec<usize>>,
    owner_named_cache: RefCell<BTreeMap<String, Vec<usize>>>,
    same_module_cache: RefCell<BTreeMap<String, Vec<usize>>>,
}

pub(in crate::analysis::test_grip_evidence) struct CompactTest<'a> {
    pub(in crate::analysis::test_grip_evidence) test: &'a TestSummary,
    pub(in crate::analysis::test_grip_evidence) path_normalized: String,
    pub(in crate::analysis::test_grip_evidence) module_path: Option<String>,
    pub(in crate::analysis::test_grip_evidence) name_lower: String,
    pub(in crate::analysis::test_grip_evidence) call_names: BTreeSet<String>,
    pub(in crate::analysis::test_grip_evidence) assertion_tokens: BTreeSet<String>,
    pub(in crate::analysis::test_grip_evidence) helper_owner_call_names: BTreeSet<String>,
    pub(in crate::analysis::test_grip_evidence) target_affinity_owner_call_names: BTreeSet<String>,
    pub(in crate::analysis::test_grip_evidence) ambiguous_target_affinity_owner_call_names:
        BTreeSet<String>,
    pub(in crate::analysis::test_grip_evidence) code_lines: Vec<String>,
    pub(in crate::analysis::test_grip_evidence) value_facts:
        OnceCell<crate::analysis::value_resolution::ValueEnvFacts>,
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
            helper_owner_calls_by_module_path(index, &helper_owner_calls_by_file);
        let direct_helper_import_aliases_by_file =
            direct_helper_import_aliases_by_file(index, &helper_owner_calls_by_module_path);
        let production_helper_owner_calls_by_package =
            production_helper_owner_calls_by_package(&helper_owner_calls_by_file);
        let target_affinity_production_owner_calls_by_package =
            target_affinity_production_owner_calls_by_package(index);
        let ambiguous_target_affinity_owner_calls_by_package =
            ambiguous_target_affinity_owner_calls_by_package(index);
        let target_affinity_production_owner_calls_by_module_path =
            target_affinity_production_owner_calls_by_module_path(index);
        let unambiguous_production_owner_names_by_package =
            unambiguous_production_owner_names_by_package(index);
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
                let test_scoped_function_names = test_scoped_function_names_by_file.get(&test.file);
                let production_owner_names = package_scope(&test.file).and_then(|package| {
                    unambiguous_production_owner_names_by_package.get(&package)
                });
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
                    test_scoped_function_names,
                    production_owner_names,
                );
                helper_owner_call_names.extend(same_file_helper_owner_call_names_for_test(
                    test,
                    &call_names,
                    &same_file_helper_owner_calls_by_file,
                    test_scoped_function_names,
                    production_owner_names,
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
                let ambiguous_target_affinity_owner_call_names =
                    ambiguous_owner_call_names_from_production_helpers(
                        test,
                        &ambiguous_target_affinity_owner_calls_by_package,
                        local_function_names,
                    );
                target_affinity_owner_call_names.extend(
                    helper_owner_call_names_from_same_file_unit_production_helpers(
                        test,
                        &target_affinity_production_owner_calls_by_package,
                        local_function_names,
                        test_scoped_function_names,
                        production_owner_names,
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
                let stem = normalized_file_stem(&test.file);
                if !stem.is_empty() {
                    tests_by_file_stem.entry(stem).or_default().push(test_index);
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
                    module_path: module_path_for_index(index, &test.file),
                    name_lower: test.name.to_ascii_lowercase(),
                    call_names,
                    assertion_tokens,
                    helper_owner_call_names,
                    target_affinity_owner_call_names,
                    ambiguous_target_affinity_owner_call_names,
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

    pub(super) fn owner_named_indices(&self, owner_name_lower: &str) -> Vec<usize> {
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

    pub(super) fn same_module_indices(&self, owner_module: &str) -> Vec<usize> {
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

pub(in crate::analysis::test_grip_evidence) type HelperOwnerCallsByFile =
    BTreeMap<PathBuf, BTreeMap<String, BTreeSet<String>>>;
pub(in crate::analysis::test_grip_evidence) type HelperOwnerCallsByName =
    BTreeMap<String, BTreeSet<String>>;
pub(in crate::analysis::test_grip_evidence) type HelperOwnerCallsByModulePath =
    BTreeMap<String, BTreeMap<String, BTreeSet<String>>>;
pub(in crate::analysis::test_grip_evidence) type HelperOwnerCallsByPackage =
    BTreeMap<String, HelperOwnerCallsByName>;
pub(in crate::analysis::test_grip_evidence) type ModuleImportAliasesByFile =
    BTreeMap<PathBuf, BTreeMap<String, ScopedModuleImportAlias>>;
pub(in crate::analysis::test_grip_evidence) type DirectFunctionImportAliasesByFile =
    BTreeMap<PathBuf, BTreeMap<String, ImportedFunctionAlias>>;
pub(in crate::analysis::test_grip_evidence) type ScopedDirectFunctionImportAliasesByFile =
    BTreeMap<PathBuf, BTreeMap<String, ScopedImportedFunctionAlias>>;
pub(in crate::analysis::test_grip_evidence) type ProductionOwnerNamesByPackage =
    BTreeMap<String, BTreeSet<String>>;
pub(in crate::analysis::test_grip_evidence) type OwnerNamesByModulePath =
    BTreeMap<String, BTreeSet<String>>;
pub(in crate::analysis::test_grip_evidence) type OwnerNamesByPackageAndModulePath =
    BTreeMap<String, OwnerNamesByModulePath>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::analysis::test_grip_evidence) struct ImportedFunctionAlias {
    module_path: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::analysis::test_grip_evidence) struct ScopedImportedFunctionAlias {
    bindings: Vec<ScopedImportedFunctionBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::analysis::test_grip_evidence) struct ScopedModuleImportAlias {
    bindings: Vec<ScopedModuleImportBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopedModuleImportBinding {
    module_path: String,
    start_line: usize,
    end_line: usize,
    /// Lines inside nested module bodies are not covered by this import.
    nested_module_lines: Vec<(usize, usize)>,
    /// The owning scope closes part-way through `end_line`, with code after
    /// the closing brace. See `LexicalUseScan::line_closes_scope_midline`.
    end_is_partial: bool,
}

impl ScopedModuleImportAlias {
    /// #3413: resolve the nearest lexically enclosing module binding for
    /// `line`. A nested test module may rebind an alias without discarding
    /// the outer binding, so an outer call keeps its outer target.
    ///
    /// Two same-scope bindings for one alias name different targets that no
    /// static reading can choose between — `#[cfg]`-gated platform imports
    /// are the compiling case — so the alias stays unresolved rather than
    /// crediting whichever import was parsed last. Repeating the same target
    /// is not an ambiguity.
    ///
    /// This runs once per call site per alias during classification, so it
    /// resolves in one pass without allocating.
    pub(in crate::analysis::test_grip_evidence) fn module_path_at(
        &self,
        line: usize,
    ) -> Option<&str> {
        let mut nearest: Option<&ScopedModuleImportBinding> = None;
        let mut ambiguous = false;
        for binding in &self.bindings {
            if binding.start_line > line || line > binding.end_line {
                continue;
            }
            if binding.end_is_partial && line == binding.end_line {
                // The scope closes part-way through this line, so the call may
                // sit either side of the closing brace. Line coordinates cannot
                // tell which, and guessing would credit the wrong owner.
                return None;
            }
            if binding
                .nested_module_lines
                .iter()
                .any(|(first, last)| *first <= line && line <= *last)
            {
                continue;
            }
            match nearest {
                Some(current) if current.start_line > binding.start_line => {}
                Some(current) if current.start_line == binding.start_line => {
                    ambiguous |= current.module_path != binding.module_path;
                }
                _ => {
                    nearest = Some(binding);
                    ambiguous = false;
                }
            }
        }
        let binding = nearest?;
        (!ambiguous).then_some(binding.module_path.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::analysis::test_grip_evidence) struct ScopedImportedFunctionBinding {
    pub(in crate::analysis::test_grip_evidence) module_path: String,
    pub(in crate::analysis::test_grip_evidence) name: String,
    start_line: usize,
    end_line: usize,
    /// Lines inside nested module bodies are not covered by this import.
    nested_module_lines: Vec<(usize, usize)>,
    /// The owning scope closes part-way through `end_line`, with code after
    /// the closing brace. See `LexicalUseScan::line_closes_scope_midline`.
    end_is_partial: bool,
}

impl ScopedImportedFunctionAlias {
    pub(in crate::analysis::test_grip_evidence) fn binding_at(
        &self,
        line: usize,
    ) -> Option<&ScopedImportedFunctionBinding> {
        if self
            .bindings
            .iter()
            .any(|binding| binding.end_is_partial && binding.end_line == line)
        {
            // See `ScopedModuleImportAlias::module_path_at`: a scope closing
            // part-way through this line leaves the call unattributable.
            return None;
        }
        let candidates = self
            .bindings
            .iter()
            .filter(|binding| {
                binding.start_line <= line
                    && line <= binding.end_line
                    && !binding
                        .nested_module_lines
                        .iter()
                        .any(|(first, last)| *first <= line && line <= *last)
            })
            .collect::<Vec<_>>();
        let max_start = candidates.iter().map(|binding| binding.start_line).max()?;
        let mut best = candidates
            .into_iter()
            .filter(|binding| binding.start_line == max_start);
        let binding = best.next()?;
        best.next().is_none().then_some(binding)
    }
}

pub(in crate::analysis::test_grip_evidence) struct HelperOwnerCallLookup<'a> {
    helpers: &'a HelperOwnerCallsByFile,
    unique_helpers: &'a HelperOwnerCallsByName,
    qualified_helpers: &'a HelperOwnerCallsByModulePath,
    production_helpers: &'a HelperOwnerCallsByPackage,
    local_function_names_by_file: &'a BTreeMap<PathBuf, BTreeSet<String>>,
    direct_helper_import_aliases_by_file: &'a DirectFunctionImportAliasesByFile,
}

pub(in crate::analysis::test_grip_evidence) fn same_file_helper_owner_call_names_for_test(
    test: &TestSummary,
    call_names: &BTreeSet<String>,
    helpers: &HelperOwnerCallsByFile,
    test_scoped_function_names: Option<&BTreeSet<String>>,
    production_owner_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    if rust_index::is_test_file(&test.file) {
        return BTreeSet::new();
    }
    let Some(file_helpers) = helpers.get(&test.file) else {
        return BTreeSet::new();
    };
    test.calls
        .iter()
        .filter(|call| call_names.contains(&call.name))
        .filter(|call| {
            same_file_unit_production_helper_call_is_allowed(
                call,
                test_scoped_function_names,
                production_owner_names,
            )
        })
        .filter_map(|call| file_helpers.get(&call.name))
        .flat_map(|owner_calls| owner_calls.iter().cloned())
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_calls_by_file(
    index: &RustIndex,
) -> HelperOwnerCallsByFile {
    helper_owner_calls_by_file_with_fanout(index, true)
}

pub(in crate::analysis::test_grip_evidence) fn strict_helper_owner_calls_by_file(
    index: &RustIndex,
) -> HelperOwnerCallsByFile {
    helper_owner_calls_by_file_with_fanout(index, false)
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_calls_by_file_with_fanout(
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
    let actual_tests = actual_test_keys(index);
    for function in index.functions.iter().filter(|function| {
        !actual_tests.contains(&ActualTestKey {
            file: &function.file,
            name: &function.name,
            start_line: function.start_line,
        })
    }) {
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

pub(in crate::analysis::test_grip_evidence) fn strict_direct_imported_owner_calls_for_helper(
    function: &FunctionSummary,
    direct_function_import_aliases: Option<&BTreeMap<String, ScopedImportedFunctionAlias>>,
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

pub(in crate::analysis::test_grip_evidence) fn extend_helper_owner_calls_through_bounded_graph(
    helpers: &mut HelperOwnerCallsByFile,
) {
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

pub(in crate::analysis::test_grip_evidence) fn extend_helper_owner_calls_through_bounded_name_graph(
    helpers: &mut HelperOwnerCallsByName,
) {
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

pub(in crate::analysis::test_grip_evidence) fn unambiguous_test_helper_owner_calls_by_name(
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

pub(in crate::analysis::test_grip_evidence) fn helper_owner_calls_by_module_path(
    index: &RustIndex,
    helpers: &HelperOwnerCallsByFile,
) -> HelperOwnerCallsByModulePath {
    helpers
        .iter()
        .filter_map(|(file, file_helpers)| {
            if !rust_index::is_test_file(file) {
                return None;
            }
            let module_path = module_path_for_index(index, file)?.replace('/', "::");
            Some((module_path, file_helpers.clone()))
        })
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn production_helper_owner_calls_by_package(
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

pub(in crate::analysis::test_grip_evidence) fn target_affinity_production_owner_calls_by_package(
    index: &RustIndex,
) -> HelperOwnerCallsByPackage {
    target_affinity_production_owner_call_sets_by_package(index)
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

pub(in crate::analysis::test_grip_evidence) fn ambiguous_target_affinity_owner_calls_by_package(
    index: &RustIndex,
) -> HelperOwnerCallsByPackage {
    target_affinity_production_owner_call_sets_by_package(index)
        .into_iter()
        .filter_map(|(package, helper_sets)| {
            let helpers = helper_sets
                .into_iter()
                .filter_map(|(helper_name, owner_sets)| {
                    ambiguous_helper_owner_calls(helper_name, owner_sets)
                })
                .collect::<HelperOwnerCallsByName>();
            (!helpers.is_empty()).then_some((package, helpers))
        })
        .collect()
}

fn target_affinity_production_owner_call_sets_by_package(
    index: &RustIndex,
) -> BTreeMap<String, BTreeMap<String, Vec<BTreeSet<String>>>> {
    let function_names_by_file = local_function_names_by_file(index);
    let imported_module_aliases_by_file = module_import_aliases_by_file(index);
    let direct_function_import_aliases_by_file = direct_function_import_aliases_by_file(index);
    let unambiguous_production_owner_names_by_package =
        unambiguous_production_owner_names_by_package(index);
    let owner_names_by_module_path = production_owner_names_by_module_path(index);
    let owner_names_by_package_and_module_path =
        production_owner_names_by_package_and_module_path(index);
    let mut by_package: BTreeMap<String, BTreeMap<String, Vec<BTreeSet<String>>>> = BTreeMap::new();
    for function in index.functions.iter().filter(|function| {
        !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
    }) {
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
}

pub(in crate::analysis::test_grip_evidence) fn target_affinity_production_owner_calls_by_module_path(
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
    for function in index.functions.iter().filter(|function| {
        !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
    }) {
        let Some(module_path) = module_path_for_index(index, &function.file) else {
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

pub(in crate::analysis::test_grip_evidence) fn target_affinity_direct_owner_calls_for_function(
    function: &FunctionSummary,
    local_function_names: &BTreeSet<String>,
    imported_module_aliases: Option<&BTreeMap<String, ScopedModuleImportAlias>>,
    direct_function_import_aliases: Option<&BTreeMap<String, ScopedImportedFunctionAlias>>,
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

pub(in crate::analysis::test_grip_evidence) fn crate_qualified_owner_calls_for_function(
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

pub(in crate::analysis::test_grip_evidence) fn call_text_contains_crate_qualified_owner_call(
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

pub(in crate::analysis::test_grip_evidence) fn code_contains_crate_qualified_helper_call(
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

pub(in crate::analysis::test_grip_evidence) fn direct_imported_owner_calls_for_function(
    function: &FunctionSummary,
    direct_function_import_aliases: Option<&BTreeMap<String, ScopedImportedFunctionAlias>>,
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
        .filter_map(|call| {
            direct_function_import_aliases
                .get(&call.name)
                .and_then(|imported| imported.binding_at(call.line))
        })
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

pub(in crate::analysis::test_grip_evidence) fn qualified_external_owner_calls_for_function(
    function: &FunctionSummary,
    imported_module_aliases: Option<&BTreeMap<String, ScopedModuleImportAlias>>,
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
                call.line,
            )
        })
        .map(|call| call.name.clone())
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn call_text_contains_imported_module_owner_call(
    text: &str,
    call_name: &str,
    imported_module_aliases: &BTreeMap<String, ScopedModuleImportAlias>,
    owner_names_by_module_path: &OwnerNamesByModulePath,
    line: usize,
) -> bool {
    let cleaned = strip_comments_and_strings(text);
    imported_module_aliases.iter().any(|(alias, scoped)| {
        scoped
            .module_path_at(line)
            .and_then(|module_path| owner_names_by_module_path.get(module_path))
            .is_some_and(|owner_names| owner_names.contains(call_name))
            && code_contains_qualified_helper_call(&cleaned, alias, call_name)
    })
}

pub(in crate::analysis::test_grip_evidence) fn production_owner_names_by_module_path(
    index: &RustIndex,
) -> OwnerNamesByModulePath {
    let mut by_module_path: OwnerNamesByModulePath = BTreeMap::new();
    for function in index.functions.iter().filter(|function| {
        !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
    }) {
        let Some(module_path) = module_path_for_index(index, &function.file) else {
            continue;
        };
        by_module_path
            .entry(module_path.replace('/', "::"))
            .or_default()
            .insert(function.name.clone());
    }
    by_module_path
}

pub(in crate::analysis::test_grip_evidence) fn production_owner_names_by_package_and_module_path(
    index: &RustIndex,
) -> OwnerNamesByPackageAndModulePath {
    let mut by_package: OwnerNamesByPackageAndModulePath = BTreeMap::new();
    for function in index.functions.iter().filter(|function| {
        !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
    }) {
        let Some(package) = package_scope(&function.file) else {
            continue;
        };
        let Some(module_path) = module_path_for_index(index, &function.file) else {
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

pub(in crate::analysis::test_grip_evidence) fn module_import_aliases_by_file(
    index: &RustIndex,
) -> ModuleImportAliasesByFile {
    index
        .files
        .iter()
        .filter_map(|(file, facts)| {
            let aliases = module_import_aliases(&facts.source);
            (!aliases.is_empty()).then_some((file.clone(), aliases))
        })
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn direct_function_import_aliases_by_file(
    index: &RustIndex,
) -> ScopedDirectFunctionImportAliasesByFile {
    index
        .files
        .iter()
        .filter_map(|(file, facts)| {
            let aliases = direct_function_import_aliases(&facts.source);
            (!aliases.is_empty()).then_some((file.clone(), aliases))
        })
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn direct_helper_import_aliases_by_file(
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

pub(in crate::analysis::test_grip_evidence) fn module_import_aliases(
    source: &str,
) -> BTreeMap<String, ScopedModuleImportAlias> {
    let scan = scan_lexical_use_statements(source);
    let mut aliases: BTreeMap<String, ScopedModuleImportAlias> = BTreeMap::new();
    for statement in &scan.statements {
        let mut parsed = BTreeMap::new();
        collect_module_import_aliases_from_use(&statement.text, &mut parsed);
        if parsed.is_empty() {
            continue;
        }
        let end_line = scan.end_line(statement);
        let mut nested_module_lines = nested_module_line_ranges(
            &scan.line_module_depths,
            statement.start_line,
            end_line,
            statement.module_depth,
        );
        // An inline `mod nested { call(); }` opening line records the
        // parent depth before its brace, so the depth ranges alone miss it;
        // mark it nested explicitly (fail closed, #3544 review).
        for line in &scan.module_open_lines {
            if statement.start_line <= *line && *line <= end_line {
                nested_module_lines.push((*line, *line));
            }
        }
        for (alias, module_path) in parsed {
            aliases
                .entry(alias)
                .or_insert_with(|| ScopedModuleImportAlias {
                    bindings: Vec::new(),
                })
                .bindings
                .push(ScopedModuleImportBinding {
                    module_path,
                    start_line: statement.start_line,
                    end_line,
                    nested_module_lines: nested_module_lines.clone(),
                    end_is_partial: scan.end_is_partial(end_line),
                });
        }
    }
    aliases
}

pub(in crate::analysis::test_grip_evidence) fn direct_helper_import_aliases(
    source: &str,
    allowed_module_paths: &BTreeSet<String>,
) -> BTreeMap<String, ImportedFunctionAlias> {
    let mut aliases: BTreeMap<String, ImportedFunctionAlias> = BTreeMap::new();
    // #3413: two file-scope imports that bind one alias to different targets
    // are ambiguous — `#[cfg]`-gated platform imports are the compiling case.
    // Keeping the textually last one would credit a helper the call may never
    // reach, so the alias stays unresolved instead.
    let mut conflicting: BTreeSet<String> = BTreeSet::new();
    let mut brace_depth = 0usize;
    for line in source.lines() {
        let line = strip_comments_and_strings(line);
        if brace_depth == 0
            && let Some(import) = line.trim().strip_prefix("use ")
        {
            let mut parsed = BTreeMap::new();
            collect_direct_helper_import_aliases_from_use(
                import.trim(),
                allowed_module_paths,
                &mut parsed,
            );
            for (alias, imported) in parsed {
                match aliases.get(&alias) {
                    Some(existing) if *existing == imported => {}
                    Some(_) => {
                        conflicting.insert(alias);
                    }
                    None => {
                        aliases.insert(alias, imported);
                    }
                }
            }
        }
        brace_depth = update_brace_depth(brace_depth, &line);
    }
    aliases.retain(|alias, _| !conflicting.contains(alias));
    aliases
}

pub(in crate::analysis::test_grip_evidence) fn update_brace_depth(
    mut depth: usize,
    line: &str,
) -> usize {
    for ch in line.chars() {
        match ch {
            '{' => depth = depth.saturating_add(1),
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// One `use` statement together with the lexical scope that owns it.
///
/// #3413: alias resolution needs the enclosing scope, not just the import
/// line, so an outer binding survives a nested test module that rebinds the
/// same alias.
struct LexicalUseStatement {
    /// The statement body after `use ` and before the terminating `;`.
    text: String,
    /// First line of the block that owns this import (1 at file scope).
    start_line: usize,
    /// Brace depth of the owning block (0 at file scope).
    scope_depth: usize,
    module_depth: usize,
    /// Line on which the statement terminated.
    use_line: usize,
}

struct LexicalUseScan {
    statements: Vec<LexicalUseStatement>,
    line_depths_after: Vec<usize>,
    /// Per line: a `}` closed a scope and code still follows it on that line
    /// (`} run();`). A call on such a line may precede or follow the brace,
    /// and line coordinates alone cannot say which.
    line_closes_scope_midline: Vec<bool>,
    line_module_depths: Vec<usize>,
    /// Lines where a `mod` scope opened. The depth vector records the
    /// pre-line depth, so an inline `mod nested { call(); }` body keeps the
    /// parent depth on its opening line; those lines are ambiguous for
    /// line-based alias resolution (#3544 review).
    module_open_lines: Vec<usize>,
}

impl LexicalUseScan {
    fn end_line(&self, statement: &LexicalUseStatement) -> usize {
        if statement.scope_depth == 0 {
            return usize::MAX;
        }
        self.line_depths_after
            .iter()
            .enumerate()
            .skip(statement.use_line - 1)
            .find_map(|(index, depth)| (*depth < statement.scope_depth).then_some(index + 1))
            .unwrap_or(self.line_depths_after.len())
    }

    fn end_is_partial(&self, end_line: usize) -> bool {
        end_line
            .checked_sub(1)
            .and_then(|index| self.line_closes_scope_midline.get(index))
            .copied()
            .unwrap_or(false)
    }
}

/// Collect every `use` statement with its owning lexical scope.
///
/// This is the single lexical owner for import ancestry in this module:
/// module aliases and direct function aliases both resolve against it, so
/// nested-scope behavior cannot drift between the two.
fn scan_lexical_use_statements(source: &str) -> LexicalUseScan {
    let mut statements = Vec::new();
    let mut line_depths_after = Vec::new();
    let mut line_closes_scope_midline = Vec::new();
    let mut scope_starts = vec![1usize];
    let mut module_scopes = vec![false];
    let mut module_open_lines: Vec<usize> = Vec::new();
    let mut line_module_depths = Vec::new();
    let mut pending_use: Option<(usize, usize, usize, String)> = None;
    let mut brace_depth = 0usize;
    for (line_index, line) in source.lines().enumerate() {
        let line = strip_comments_and_strings(line);
        let line_number = line_index + 1;
        let scope_start = scope_starts[brace_depth];
        let module_depth = module_scopes.iter().filter(|scope| **scope).count();
        line_module_depths.push(module_depth);
        let trimmed = line.trim();
        let mut is_use_fragment = false;
        if pending_use.is_some()
            && [
                "use ", "fn ", "pub ", "mod ", "impl ", "struct ", "enum ", "const ", "static ",
                "type ", "#[",
            ]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
        {
            pending_use = None;
        }
        if let Some((start_line, scope_depth, use_module_depth, import)) = pending_use.as_mut() {
            is_use_fragment = true;
            import.push(' ');
            import.push_str(trimmed);
            if trimmed.contains(';') {
                let text = import
                    .split_once(';')
                    .map_or(import.as_str(), |(head, _)| head)
                    .to_string();
                statements.push(LexicalUseStatement {
                    text,
                    start_line: *start_line,
                    scope_depth: *scope_depth,
                    module_depth: *use_module_depth,
                    use_line: line_number,
                });
                pending_use = None;
            }
        } else if let Some(import) = trimmed.strip_prefix("use ") {
            is_use_fragment = true;
            if import.contains(';') {
                let text = import
                    .split_once(';')
                    .map_or(import, |(head, _)| head)
                    .to_string();
                statements.push(LexicalUseStatement {
                    text,
                    start_line: scope_start,
                    scope_depth: brace_depth,
                    module_depth,
                    use_line: line_number,
                });
            } else {
                pending_use = Some((scope_start, brace_depth, module_depth, import.to_string()));
            }
        }
        let lexical_suffix = if is_use_fragment {
            line.split_once(';').map_or("", |(_, suffix)| suffix)
        } else {
            &line
        };
        let mut last_scope_close = None;
        let mut segment = String::new();
        for (index, ch) in lexical_suffix.chars().enumerate() {
            match ch {
                '{' => {
                    brace_depth = brace_depth.saturating_add(1);
                    scope_starts.push(line_number);
                    let declares_module = segment_declares_module(&segment);
                    module_scopes.push(declares_module);
                    if declares_module && module_open_lines.last() != Some(&line_number) {
                        module_open_lines.push(line_number);
                    }
                    segment.clear();
                }
                '}' if brace_depth > 0 => {
                    brace_depth -= 1;
                    scope_starts.pop();
                    module_scopes.pop();
                    last_scope_close = Some(index);
                }
                ';' => segment.clear(),
                _ => segment.push(ch),
            }
        }
        // A bare statement terminator after the brace carries no call, so it
        // does not make the line ambiguous.
        line_closes_scope_midline.push(last_scope_close.is_some_and(|index| {
            lexical_suffix
                .chars()
                .skip(index + 1)
                .any(|ch| !ch.is_whitespace() && ch != ';')
        }));
        line_depths_after.push(brace_depth);
    }
    LexicalUseScan {
        statements,
        line_depths_after,
        line_closes_scope_midline,
        line_module_depths,
        module_open_lines,
    }
}

pub(in crate::analysis::test_grip_evidence) fn direct_function_import_aliases(
    source: &str,
) -> BTreeMap<String, ScopedImportedFunctionAlias> {
    let scan = scan_lexical_use_statements(source);
    let mut aliases: BTreeMap<String, ScopedImportedFunctionAlias> = BTreeMap::new();
    for statement in &scan.statements {
        let Some(parsed) =
            collect_direct_function_import_aliases_from_use_unambiguous(&statement.text)
        else {
            continue;
        };
        if parsed.is_empty() {
            continue;
        }
        let end_line = scan.end_line(statement);
        let mut nested_module_lines = nested_module_line_ranges(
            &scan.line_module_depths,
            statement.start_line,
            end_line,
            statement.module_depth,
        );
        // An inline `mod nested { call(); }` opening line records the
        // parent depth before its brace, so the depth ranges alone miss it;
        // mark it nested explicitly (fail closed, #3544 review).
        for line in &scan.module_open_lines {
            if statement.start_line <= *line && *line <= end_line {
                nested_module_lines.push((*line, *line));
            }
        }
        for (alias, imported) in parsed {
            aliases
                .entry(alias)
                .or_insert_with(|| ScopedImportedFunctionAlias {
                    bindings: Vec::new(),
                })
                .bindings
                .push(ScopedImportedFunctionBinding {
                    module_path: imported.module_path,
                    name: imported.name,
                    start_line: statement.start_line,
                    end_line,
                    nested_module_lines: nested_module_lines.clone(),
                    end_is_partial: scan.end_is_partial(end_line),
                });
        }
    }
    aliases
}

fn segment_declares_module(segment: &str) -> bool {
    segment
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .any(|token| token == "mod")
}

fn nested_module_line_ranges(
    line_module_depths: &[usize],
    start_line: usize,
    end_line: usize,
    module_depth: usize,
) -> Vec<(usize, usize)> {
    let last_line = end_line.min(line_module_depths.len());
    let mut ranges = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    for line in start_line..=last_line {
        if line_module_depths[line - 1] == module_depth {
            if let Some(range) = open.take() {
                ranges.push(range);
            }
        } else {
            match open.as_mut() {
                Some((_, last)) => *last = line,
                None => open = Some((line, line)),
            }
        }
    }
    if let Some(range) = open {
        ranges.push(range);
    }
    ranges
}

fn collect_direct_function_import_aliases_from_use_unambiguous(
    import: &str,
) -> Option<BTreeMap<String, ImportedFunctionAlias>> {
    let import = import.trim_end_matches(';').trim();
    let mut aliases = BTreeMap::new();
    if let Some((base, rest)) = import.split_once("::{") {
        let module_path = normalize_module_import_path(base)?;
        let body = rest.strip_suffix('}')?;
        for item in body.split(',').map(str::trim) {
            if item.contains("::") || item.contains('{') || item.contains('}') {
                continue;
            }
            let mut candidate = BTreeMap::new();
            collect_direct_function_import_alias(item, &module_path, &mut candidate);
            for (alias, imported) in candidate {
                if aliases.insert(alias, imported).is_some() {
                    return None;
                }
            }
        }
        return Some(aliases);
    }
    let (module_path, item) = import.rsplit_once("::")?;
    let module_path = normalize_module_import_path(module_path)?;
    collect_direct_function_import_alias(item.trim(), &module_path, &mut aliases);
    Some(aliases)
}

pub(in crate::analysis::test_grip_evidence) fn collect_module_import_aliases_from_use(
    import: &str,
    aliases: &mut BTreeMap<String, String>,
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

pub(in crate::analysis::test_grip_evidence) fn collect_direct_helper_import_aliases_from_use(
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

pub(in crate::analysis::test_grip_evidence) fn collect_direct_function_import_alias(
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

pub(in crate::analysis::test_grip_evidence) fn normalize_helper_module_import_path(
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

pub(in crate::analysis::test_grip_evidence) fn normalize_module_import_path(
    path: &str,
) -> Option<String> {
    let path = path.trim().strip_prefix("crate::")?.trim();
    if path.is_empty() || path.starts_with("super::") || path.starts_with("self::") {
        return None;
    }
    Some(path.to_string())
}

pub(in crate::analysis::test_grip_evidence) fn unambiguous_production_owner_names_by_package(
    index: &RustIndex,
) -> ProductionOwnerNamesByPackage {
    let mut counts_by_package: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for function in index.functions.iter().filter(|function| {
        !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
    }) {
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

pub(in crate::analysis::test_grip_evidence) fn local_function_names_by_file(
    index: &RustIndex,
) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut names_by_file: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    // #3286: local-name resolution must see cfg(test)-module helpers too —
    // excluding them would misread a helper's call to a sibling test-module
    // function as a potential owner call.
    let actual_tests = actual_test_keys(index);
    for function in index.functions.iter().filter(|function| {
        !actual_tests.contains(&ActualTestKey {
            file: &function.file,
            name: &function.name,
            start_line: function.start_line,
        })
    }) {
        names_by_file
            .entry(function.file.clone())
            .or_default()
            .insert(function.name.clone());
    }
    names_by_file
}

/// Borrowed (file, name, start_line) identity of a function that is an
/// actual executable test (`TestFact` member), so graph-admission lookups
/// need no per-function key clones. #3273 widened the function
/// source role to cover plain helpers inside inline `#[cfg(test)]`
/// modules (now the typed `FunctionFact::source_role`, #3531); those
/// helpers are evidence-role, not tests, so
/// helper-graph admission must exclude only real tests (#3286) while
/// seam/production-owner filters keep using the wider role. The
/// start_line component makes the key exact identity rather than a
/// same-file same-name over-exclusion.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ActualTestKey<'a> {
    file: &'a Path,
    name: &'a str,
    start_line: usize,
}

fn actual_test_keys(index: &RustIndex) -> BTreeSet<ActualTestKey<'_>> {
    index
        .tests
        .iter()
        .map(|test| ActualTestKey {
            file: test.file.as_path(),
            name: test.name.as_str(),
            start_line: test.start_line,
        })
        .collect()
}

/// Names that can shadow same-file production helpers while resolving test evidence.
///
/// `FunctionFact::source_role` is the parser-owned source-role authority. Reusing it here
/// keeps `cfg(all(..., test))` and future supported test-role forms aligned with seam
/// exclusion instead of maintaining a second lexical cfg scanner.
pub(in crate::analysis::test_grip_evidence) fn test_scoped_function_names_by_file(
    index: &RustIndex,
) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut names_by_file: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for function in index.functions.iter().filter(|function| {
        function.source_role.is_evidence_role() || rust_index::is_test_file(&function.file)
    }) {
        if let Some(names) = names_by_file.get_mut(&function.file) {
            names.insert(function.name.clone());
        } else {
            names_by_file.insert(
                function.file.clone(),
                BTreeSet::from([function.name.clone()]),
            );
        }
    }
    names_by_file
}

pub(in crate::analysis::test_grip_evidence) fn production_owner_names(
    index: &RustIndex,
) -> BTreeSet<String> {
    index
        .functions
        .iter()
        .filter(|function| {
            !function.source_role.is_evidence_role() && !rust_index::is_test_file(&function.file)
        })
        .map(|function| function.name.clone())
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn helper_directly_delegates_to_specific_owner(
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

pub(in crate::analysis::test_grip_evidence) fn call_text_routes_directly_to_named_call(
    text: &str,
    name: &str,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_call_prefix_is_allowed(prefix: &str) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_condition_prefix_is_allowed(
    prefix: &str,
) -> bool {
    matches!(prefix.trim(), "if" | "if !")
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_parenthesized_macro_name_before_argument(
    prefix: &str,
) -> Option<String> {
    let (macro_prefix, argument_prefix) = prefix.rsplit_once("!(")?;
    let macro_name = trailing_rust_identifier(macro_prefix);
    if macro_name.is_empty()
        || !direct_delegate_macro_argument_prefix_is_allowed(&macro_name, argument_prefix)
    {
        return None;
    }
    Some(macro_name)
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_macro_argument_prefix_is_allowed(
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_eager_later_argument_macro_is_allowed(
    macro_name: &str,
) -> bool {
    matches!(
        macro_name,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne"
    )
}

pub(in crate::analysis::test_grip_evidence) fn argument_prefix_has_trailing_top_level_comma(
    prefix: &str,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_wrapper_name(open: &str) -> String {
    let open = open.trim_end();
    if let Some((before_turbofish, generic_tail)) = open.rsplit_once("::<")
        && generic_tail.trim_end().ends_with('>')
    {
        return trailing_rust_identifier(before_turbofish);
    }
    trailing_rust_identifier(open)
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_block_prefix_is_allowed(
    prefix: &str,
) -> bool {
    let Some(open) = prefix.strip_suffix('{') else {
        return false;
    };
    let open = open.trim_end();
    open.is_empty() || open == "return" || open.ends_with('=') || open.ends_with("=>")
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_std_identity_prefix_is_allowed(
    prefix: &str,
) -> bool {
    let Some(open) = prefix.strip_suffix('(') else {
        return false;
    };
    let open = open.trim_end();
    let open = open.strip_prefix("return ").unwrap_or(open).trim_start();
    direct_delegate_std_identity_path_is_allowed(open)
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_std_identity_path_is_allowed(
    path: &str,
) -> bool {
    matches!(
        path.trim(),
        "std::convert::identity"
            | "::std::convert::identity"
            | "core::convert::identity"
            | "::core::convert::identity"
    )
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_field_initializer_prefix_is_allowed(
    prefix: &str,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_receiver_method_prefix_is_allowed(
    prefix: &str,
) -> bool {
    let Some(receiver_prefix) = prefix.strip_suffix('.') else {
        return false;
    };
    let receiver = direct_receiver_method_condition_receiver(receiver_prefix);
    !receiver.is_empty()
        && receiver
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && receiver
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
}

pub(in crate::analysis::test_grip_evidence) fn direct_receiver_method_condition_receiver(
    prefix: &str,
) -> &str {
    let receiver = prefix.trim();
    if let Some(after_if) = receiver.strip_prefix("if !") {
        return after_if.trim_start();
    }
    if let Some(after_if) = receiver.strip_prefix("if ") {
        return after_if.trim_start();
    }
    receiver
}

pub(in crate::analysis::test_grip_evidence) fn trailing_rust_identifier(text: &str) -> String {
    text.chars()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
}

pub(in crate::analysis::test_grip_evidence) fn call_text_contains_named_call(
    text: &str,
    name: &str,
) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
        let after = &text[start + name.len()..];
        after.starts_with("::") || after.trim_start().starts_with('(')
    })
}

pub(in crate::analysis::test_grip_evidence) fn supported_helper_owner_call_name(
    call_name: &str,
    local_function_names: &BTreeSet<String>,
    external_owner_names: Option<&BTreeSet<String>>,
) -> bool {
    local_function_names.contains(call_name)
        || external_owner_names.is_some_and(|owner_names| owner_names.contains(call_name))
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_parenthesized_macro_is_allowed(
    call_name: &str,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_container_macro_is_allowed(
    call_name: &str,
) -> bool {
    matches!(call_name, "vec")
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_extra_call_is_inert(
    call_name: &str,
) -> bool {
    matches!(
        call_name,
        "clone"
            | "default"
            | "expect"
            | "extend"
            | "format"
            | "from"
            | "into"
            | "new"
            | "to_string"
            | "trim"
            | "unwrap"
            | "unwrap_or_default"
            | "Err"
            | "Ok"
            | "Some"
    )
}

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_extra_call_is_allowed(
    candidate: &CallFact,
    owner_call: &CallFact,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_post_owner_method_is_allowed(
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

pub(in crate::analysis::test_grip_evidence) fn direct_delegate_std_identity_call_is_allowed(
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

pub(in crate::analysis::test_grip_evidence) fn matching_close_paren(text: &str) -> Option<usize> {
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

pub(in crate::analysis::test_grip_evidence) fn helper_name_carries_owner_token(
    helper_name_lower: &str,
    owner_name: &str,
) -> bool {
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

pub(in crate::analysis::test_grip_evidence) fn owner_token_is_specific_enough(
    owner_name_lower: &str,
) -> bool {
    owner_name_lower.contains('_')
        || (owner_name_lower.len() >= 8
            && !matches!(
                owner_name_lower,
                "builder" | "convert" | "fixture" | "helper" | "parse" | "render"
            ))
}

pub(in crate::analysis::test_grip_evidence) fn is_helper_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| ch == '_' || !ch.is_alphanumeric())
}

pub(in crate::analysis::test_grip_evidence) fn common_helper_owner_calls(
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

pub(in crate::analysis::test_grip_evidence) fn ambiguous_helper_owner_calls(
    helper_name: String,
    owner_sets: Vec<BTreeSet<String>>,
) -> Option<(String, BTreeSet<String>)> {
    if owner_sets.len() < 2 {
        return None;
    }
    let ambiguous = owner_sets.into_iter().flatten().collect::<BTreeSet<_>>();
    (!ambiguous.is_empty()).then_some((helper_name, ambiguous))
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_for_test(
    test: &TestSummary,
    call_names: &BTreeSet<String>,
    lookup: &HelperOwnerCallLookup<'_>,
    module_import_aliases: Option<&BTreeMap<String, ScopedModuleImportAlias>>,
    test_scoped_function_names: Option<&BTreeSet<String>>,
    production_owner_names: Option<&BTreeSet<String>>,
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
        for call in test.calls.iter().filter(|call| {
            call_names.contains(&call.name)
                && same_file_unit_production_helper_call_is_allowed(
                    call,
                    test_scoped_function_names,
                    production_owner_names,
                )
        }) {
            if let Some(helper_owner_names) = file_helpers.get(&call.name) {
                owner_names.extend(helper_owner_names.iter().cloned());
            }
            if let Some(helper_owner_names) = lookup.unique_helpers.get(&call.name) {
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

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_from_direct_imported_helpers(
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

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_from_qualified_calls(
    calls: &[CallFact],
    qualified_helpers: &HelperOwnerCallsByModulePath,
    module_import_aliases: Option<&BTreeMap<String, ScopedModuleImportAlias>>,
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
                    call.line,
                )
            {
                owner_names.extend(helper_owner_names.iter().cloned());
            }
        }
    }
    owner_names
}

pub(in crate::analysis::test_grip_evidence) fn code_contains_aliased_module_helper_call(
    code: &str,
    module_path: &str,
    helper_name: &str,
    module_import_aliases: Option<&BTreeMap<String, ScopedModuleImportAlias>>,
    line: usize,
) -> bool {
    module_import_aliases.is_some_and(|aliases| {
        aliases.iter().any(|(alias, scoped)| {
            scoped.module_path_at(line) == Some(module_path)
                && code_contains_qualified_helper_call(code, alias, helper_name)
        })
    })
}

pub(in crate::analysis::test_grip_evidence) fn code_contains_qualified_helper_call(
    code: &str,
    module_path: &str,
    helper_name: &str,
) -> bool {
    ["", "crate::", "self::", "super::"]
        .into_iter()
        .any(|prefix| {
            code_contains_qualified_helper_call_with_prefix(code, prefix, module_path, helper_name)
        })
}

pub(in crate::analysis::test_grip_evidence) fn code_contains_qualified_helper_call_with_prefix(
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

pub(in crate::analysis::test_grip_evidence) fn is_rust_path_identifier_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch.is_ascii_alphanumeric()
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_from_unique_helpers(
    call_names: &BTreeSet<String>,
    unique_helpers: &HelperOwnerCallsByName,
) -> BTreeSet<String> {
    call_names
        .iter()
        .filter_map(|helper_name| unique_helpers.get(helper_name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_from_production_helpers(
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

pub(in crate::analysis::test_grip_evidence) fn ambiguous_owner_call_names_from_production_helpers(
    test: &TestSummary,
    production_helpers: &HelperOwnerCallsByPackage,
    local_function_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let Some(package) = package_scope(&test.file) else {
        return BTreeSet::new();
    };
    let Some(package_helpers) = production_helpers.get(&package) else {
        return BTreeSet::new();
    };
    test.calls
        .iter()
        .filter(|call| {
            !local_function_names.is_some_and(|names| names.contains(&call.name))
                && call_text_contains_unqualified_named_call(&call.text, &call.name)
        })
        .filter_map(|call| package_helpers.get(&call.name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

fn call_text_contains_unqualified_named_call(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        if before
            .is_some_and(|ch| ch == ':' || ch == '.' || ch.is_ascii_alphanumeric() || ch == '_')
        {
            return false;
        }
        text[start + name.len()..].trim_start().starts_with('(')
    })
}

pub(in crate::analysis::test_grip_evidence) fn helper_owner_call_names_from_same_file_unit_production_helpers(
    test: &TestSummary,
    production_helpers: &HelperOwnerCallsByPackage,
    local_function_names: Option<&BTreeSet<String>>,
    test_scoped_function_names: Option<&BTreeSet<String>>,
    production_owner_names: Option<&BTreeSet<String>>,
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
            same_file_unit_production_helper_call_is_allowed(
                call,
                test_scoped_function_names,
                production_owner_names,
            )
        })
        .filter_map(|call| package_helpers.get(&call.name))
        .flat_map(|owner_names| owner_names.iter().cloned())
        .collect()
}

pub(in crate::analysis::test_grip_evidence) fn same_file_unit_production_helper_call_is_allowed(
    call: &CallFact,
    test_scoped_function_names: Option<&BTreeSet<String>>,
    production_owner_names: Option<&BTreeSet<String>>,
) -> bool {
    let cleaned = strip_comments_and_strings(&call.text);
    if code_contains_parent_qualified_helper_call(&cleaned, &call.name) {
        return true;
    }
    !(test_scoped_function_names.is_some_and(|names| names.contains(&call.name))
        && production_owner_names.is_some_and(|names| names.contains(&call.name)))
        && call_text_contains_named_call(&cleaned, &call.name)
}

pub(in crate::analysis::test_grip_evidence) fn code_contains_parent_qualified_helper_call(
    code: &str,
    helper_name: &str,
) -> bool {
    let pattern = format!("super::{helper_name}(");
    code.match_indices(&pattern).any(|(start, _)| {
        code[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !is_rust_path_identifier_char(before))
    })
}

// `RelationReason` and `RelationConfidence` now live in `crate::domain::evidence`.
// They are re-exported at the top of this file so callers can still import them
// from here without source-level changes.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::{FileFacts, FunctionFact, FunctionSourceRole, RustIndex};
    use crate::domain::SymbolId;

    #[test]
    fn cfg_all_test_role_blocks_same_file_shadow_credit() {
        let file = PathBuf::from("crate/src/lib.rs");
        let production = function_fact(&file, "wrapper", 1, FunctionSourceRole::Production);
        let evidence_shadow = function_fact(&file, "wrapper", 6, FunctionSourceRole::CfgTestModule);
        let source = r#"fn wrapper() {}

#[cfg(all(feature = "slow", test))]
mod tests {
    fn wrapper() {}
}
"#;
        let index = RustIndex {
            files: BTreeMap::from([(
                file.clone(),
                FileFacts {
                    path: file.clone(),
                    functions: vec![production.clone(), evidence_shadow.clone()],
                    source: source.to_string(),
                    ..FileFacts::default()
                },
            )]),
            functions: vec![production, evidence_shadow],
            ..RustIndex::default()
        };

        let scoped = test_scoped_function_names_by_file(&index);
        let scoped_names = scoped.get(&file);
        assert!(scoped_names.is_some_and(|names| names.contains("wrapper")));

        let production_names = BTreeSet::from(["wrapper".to_string()]);
        let call = CallFact {
            line: 8,
            name: "wrapper".to_string(),
            text: "wrapper()".to_string(),
        };
        assert!(!same_file_unit_production_helper_call_is_allowed(
            &call,
            scoped_names,
            Some(&production_names),
        ));
    }

    fn function_fact(
        file: &Path,
        name: &str,
        start_line: usize,
        source_role: FunctionSourceRole,
    ) -> FunctionFact {
        FunctionFact {
            id: SymbolId(format!("symbol:{name}:{start_line}")),
            name: name.to_string(),
            file: file.to_path_buf(),
            start_line,
            end_line: start_line,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }
}
