use super::{ResolvedIncludeParent, RustIncludeLimitation, RustIndex};
use crate::analysis::syntax::rust_include_directives;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

const MAX_INCLUDE_EDGES: usize = 512;
const MAX_INCLUDE_DEPTH: usize = 32;
const MAX_INCLUDED_FILE_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn resolve_repository_local_includes(root: &Path, index: &mut RustIndex) {
    let mut limitations = Vec::new();
    let mut edges = Vec::new();
    let mut directive_count = 0;
    let mut edge_limit_exceeded = false;
    let canonical_root = std::fs::canonicalize(root).ok();

    'files: for (parent, facts) in &index.files {
        if facts.used_lexical_fallback || !might_contain_include_macro(&facts.source) {
            continue;
        }
        // This token-shaped check is only a cheap negative prefilter.
        // Parser-backed macro nodes remain the semantic authority, so comments
        // and strings cannot create include relationships.
        let directives = match rust_include_directives(parent, &facts.source, MAX_INCLUDE_EDGES) {
            Ok(directives) => directives,
            Err(reason_code) => {
                limitations.push(limitation(parent, 0, "include!", &reason_code));
                continue;
            }
        };
        for directive in directives {
            directive_count += 1;
            if !include_edge_count_within_limit(directive_count) {
                edge_limit_exceeded = true;
                break 'files;
            }
            if !directive.is_file_level {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_nested_module_context",
                ));
                continue;
            }
            let Some(literal) = directive.literal_path else {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_dynamic_expression",
                ));
                continue;
            };
            let Some(candidate) = repository_relative_target(parent, &literal) else {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_path_outside_repository",
                ));
                continue;
            };
            let full = root.join(&candidate);
            let canonical_target = match std::fs::canonicalize(&full) {
                Ok(path) => path,
                Err(_) => {
                    limitations.push(limitation(
                        parent,
                        directive.line,
                        &directive.expression,
                        "rust_include_target_missing",
                    ));
                    continue;
                }
            };
            if canonical_root
                .as_ref()
                .is_none_or(|root| !canonical_target.starts_with(root))
            {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_symlink_escape",
                ));
                continue;
            }
            let Some(target_facts) = index.files.get(&candidate) else {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_target_not_indexed",
                ));
                continue;
            };
            if !included_file_within_size_limit(&target_facts.source) {
                limitations.push(limitation(
                    parent,
                    directive.line,
                    &directive.expression,
                    "rust_include_file_too_large",
                ));
                continue;
            }
            edges.push((
                candidate,
                parent.clone(),
                directive.line,
                directive.expression,
                directive.requires_test,
            ));
        }
    }

    edges.sort();
    if edge_limit_exceeded {
        limitations.push(limitation(
            Path::new("."),
            0,
            "include!",
            "rust_include_edge_limit_exceeded",
        ));
        index.include_parents.clear();
        sort_limitations(&mut limitations);
        index.include_limitations = limitations;
        return;
    }

    let mut parents_by_child: BTreeMap<PathBuf, Vec<(PathBuf, usize, String, bool)>> =
        BTreeMap::new();
    // Preserve physical include discovery independently of whether the
    // contextual role map can choose one parent. A conflict must fail closed
    // for role grants, but it must not erase the fragment's source directory
    // when resolving its own default modules.
    index.include_targets = edges.iter().map(|(child, ..)| child.clone()).collect();
    for (child, parent, line, expression, requires_test) in edges {
        parents_by_child
            .entry(child)
            .or_default()
            .push((parent, line, expression, requires_test));
    }

    let mut parents = BTreeMap::new();
    for (child, owners) in parents_by_child {
        let distinct = owners
            .iter()
            .map(|(parent, _, _, _)| parent)
            .collect::<BTreeSet<_>>();
        // Ambiguous ownership, or one parent whose include invocations
        // disagree on the cfg-test requirement (#3533): neither resolves to
        // one context, so the child keeps its standalone roles.
        let requirements = owners
            .iter()
            .map(|(_, _, _, requires_test)| *requires_test)
            .collect::<BTreeSet<_>>();
        if distinct.len() != 1 {
            for (parent, line, expression, _) in owners {
                limitations.push(limitation(
                    &parent,
                    line,
                    &expression,
                    "rust_include_ambiguous_parent",
                ));
            }
            continue;
        }
        if requirements.len() != 1 {
            for (parent, line, expression, _) in owners {
                limitations.push(limitation(
                    &parent,
                    line,
                    &expression,
                    "rust_include_conflicting_cfg_requirement",
                ));
            }
            continue;
        }
        if let Some((parent, _, _, requires_test)) = owners.into_iter().next() {
            parents.insert(
                child,
                ResolvedIncludeParent {
                    parent,
                    requires_test,
                },
            );
        }
    }

    let unsafe_children = cycle_or_depth_limited_children(&parents);
    for child in &unsafe_children {
        let parent = match parents.get(child) {
            Some(edge) => edge.parent.as_path(),
            None => Path::new("."),
        };
        limitations.push(limitation(
            parent,
            0,
            &format!("include!({})", child.display()),
            "rust_include_cycle_or_depth_limit",
        ));
    }
    parents.retain(|child, _| !unsafe_children.contains(child));

    index.include_parents = parents;
    sort_limitations(&mut limitations);
    index.include_limitations = limitations;
    rebase_function_identities(index);
}

fn might_contain_include_macro(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find("include") {
        let start = offset + relative;
        let end = start + "include".len();
        let has_identifier_prefix =
            start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let has_identifier_suffix =
            end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
        if !has_identifier_prefix && !has_identifier_suffix {
            let mut cursor = end;
            loop {
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_whitespace())
                {
                    cursor += 1;
                }
                if bytes.get(cursor..cursor + 2) == Some(b"//") {
                    cursor += 2;
                    while bytes.get(cursor).is_some_and(|byte| *byte != b'\n') {
                        cursor += 1;
                    }
                    continue;
                }
                if bytes.get(cursor..cursor + 2) == Some(b"/*") {
                    cursor += 2;
                    let mut depth = 1usize;
                    while depth > 0 {
                        match bytes.get(cursor..cursor + 2) {
                            Some(b"/*") => {
                                depth += 1;
                                cursor += 2;
                            }
                            Some(b"*/") => {
                                depth -= 1;
                                cursor += 2;
                            }
                            Some(_) => cursor += 1,
                            None => break,
                        }
                    }
                    if depth == 0 {
                        continue;
                    }
                }
                break;
            }
            if bytes.get(cursor) == Some(&b'!') {
                return true;
            }
        }
        offset = end;
    }
    false
}

fn sort_limitations(limitations: &mut Vec<RustIncludeLimitation>) {
    limitations.sort_by(|left, right| {
        left.parent
            .cmp(&right.parent)
            .then(left.line.cmp(&right.line))
            .then(left.reason_code.cmp(&right.reason_code))
            .then(left.expression.cmp(&right.expression))
    });
    limitations.dedup();
}

fn repository_relative_target(parent: &Path, literal: &Path) -> Option<PathBuf> {
    if literal.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    if let Some(parent_dir) = parent.parent() {
        normalized.push(parent_dir);
    }
    for component in literal.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

fn included_file_within_size_limit(source: &str) -> bool {
    source.len() <= MAX_INCLUDED_FILE_BYTES
}

fn include_edge_count_within_limit(count: usize) -> bool {
    count <= MAX_INCLUDE_EDGES
}

fn cycle_or_depth_limited_children(
    parents: &BTreeMap<PathBuf, ResolvedIncludeParent>,
) -> BTreeSet<PathBuf> {
    let mut unsafe_children = BTreeSet::new();
    for child in parents.keys() {
        let mut seen = BTreeSet::new();
        let mut cursor = child;
        let mut cycle_detected = false;
        for _ in 0..=MAX_INCLUDE_DEPTH {
            if !seen.insert(cursor.clone()) {
                cycle_detected = true;
                break;
            }
            let Some(parent) = parents.get(cursor).map(|edge| &edge.parent) else {
                break;
            };
            cursor = parent;
        }
        if cycle_detected || seen.len() > MAX_INCLUDE_DEPTH {
            unsafe_children.extend(seen);
        }
    }
    unsafe_children
}

fn rebase_function_identities(index: &mut RustIndex) {
    let parents = &index.include_parents;
    for function in &mut index.functions {
        rebase_function_identity(function, parents);
    }
    for facts in index.files.values_mut() {
        for function in &mut facts.functions {
            rebase_function_identity(function, parents);
        }
    }
}

fn rebase_function_identity(
    function: &mut super::FunctionFact,
    parents: &BTreeMap<PathBuf, ResolvedIncludeParent>,
) {
    let compilation_unit = compilation_unit_path_from_parents(parents, &function.file);
    if compilation_unit == function.file {
        return;
    }
    // Function identities are composed from the stable `/`-separated
    // path text (`stable_path_text`, #3469 family) while `file` keeps the
    // producing host's separators, so the prefix must be built from the
    // same stable form — on a backslash host the display form never
    // matches and the include rebase silently never fires (Windows-only
    // failure of `rust_include_compilation_unit`, #3631-adjacent).
    let source_prefix = format!("{}::", crate::analysis::stable_path_text(&function.file));
    if let Some(suffix) = function.id.0.strip_prefix(&source_prefix) {
        function.id.0 = format!(
            "{}::{suffix}",
            crate::analysis::stable_path_text(&compilation_unit)
        );
    }
}

pub(crate) fn compilation_unit_path_from_parents(
    parents: &BTreeMap<PathBuf, ResolvedIncludeParent>,
    file: &Path,
) -> PathBuf {
    let mut cursor = file.to_path_buf();
    for _ in 0..MAX_INCLUDE_DEPTH {
        let Some(parent) = parents.get(&cursor).map(|edge| edge.parent.clone()) else {
            break;
        };
        cursor = parent;
    }
    cursor
}

fn limitation(
    parent: &Path,
    line: usize,
    expression: &str,
    reason_code: &str,
) -> RustIncludeLimitation {
    RustIncludeLimitation {
        parent: parent.to_path_buf(),
        line,
        expression: expression.to_string(),
        reason_code: reason_code.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rebase prefix must be composed from the same stable identity
    /// text the symbol id uses, never from `file.display()`: identity is
    /// canonical `/`-separated text (#3469 family) while `file` keeps the
    /// producing host's separators, and a filename `%` makes the two
    /// forms diverge on every host — so this pins the contract on Linux
    /// CI too, where the Windows separator mismatch itself cannot run
    /// (#3638 review).
    #[test]
    fn rebase_prefix_uses_the_stable_identity_form_not_display() {
        let mut parents = BTreeMap::new();
        parents.insert(
            PathBuf::from("src/frag_50%.rs"),
            ResolvedIncludeParent {
                parent: PathBuf::from("src/lib.rs"),
                requires_test: false,
            },
        );
        let mut function = crate::analysis::rust_index::FunctionFact {
            id: crate::domain::SymbolId("src/frag_50%25.rs::impl W::f".to_string()),
            name: "f".to_string(),
            file: PathBuf::from("src/frag_50%.rs"),
            start_line: 1,
            end_line: 2,
            body: "fn f() {}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: crate::analysis::facts::FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        super::rebase_function_identity(&mut function, &parents);
        assert_eq!(function.id.0, "src/lib.rs::impl W::f");
    }

    #[test]
    fn include_macro_prefilter_rejects_common_identifier_mentions() {
        assert!(!might_contain_include_macro("include_str!(\"data\")"));
        assert!(!might_contain_include_macro("let included = true;"));
        assert!(!might_contain_include_macro("preinclude!(\"fragment.rs\")"));
    }

    #[test]
    fn include_macro_prefilter_accepts_token_separating_trivia() {
        assert!(might_contain_include_macro("include!(\"fragment.rs\")"));
        assert!(might_contain_include_macro(
            "include \n ! (\"fragment.rs\")"
        ));
        assert!(might_contain_include_macro(
            "include /* outer /* nested */ comment */ ! (\"fragment.rs\")"
        ));
        assert!(might_contain_include_macro(
            "include // why this fragment\n ! (\"fragment.rs\")"
        ));
    }

    #[test]
    fn traversal_above_repository_root_is_rejected() {
        assert_eq!(
            repository_relative_target(Path::new("src/lib.rs"), Path::new("../shared.rs")),
            Some(PathBuf::from("shared.rs"))
        );
        assert_eq!(
            repository_relative_target(Path::new("src/lib.rs"), Path::new("../../escaped.rs")),
            None
        );
    }

    #[test]
    fn include_depth_over_bound_invalidates_the_chain() {
        let mut parents = BTreeMap::new();
        for depth in 0..=MAX_INCLUDE_DEPTH {
            parents.insert(
                PathBuf::from(format!("src/fragment-{depth}.rs")),
                ResolvedIncludeParent {
                    parent: if depth == MAX_INCLUDE_DEPTH {
                        PathBuf::from("src/lib.rs")
                    } else {
                        PathBuf::from(format!("src/fragment-{}.rs", depth + 1))
                    },
                    requires_test: false,
                },
            );
        }

        let limited = cycle_or_depth_limited_children(&parents);

        assert!(limited.contains(Path::new("src/fragment-0.rs")));
        assert!(limited.len() > MAX_INCLUDE_DEPTH);
    }

    #[test]
    fn include_resource_limits_are_inclusive_and_bounded() {
        assert!(include_edge_count_within_limit(MAX_INCLUDE_EDGES));
        assert!(!include_edge_count_within_limit(MAX_INCLUDE_EDGES + 1));

        let at_limit = "x".repeat(MAX_INCLUDED_FILE_BYTES);
        let over_limit = "x".repeat(MAX_INCLUDED_FILE_BYTES + 1);
        assert!(included_file_within_size_limit(&at_limit));
        assert!(!included_file_within_size_limit(&over_limit));
    }

    /// One parent whose include invocations disagree on the cfg-test
    /// requirement resolves to no single context: the child keeps its
    /// standalone roles and the conflict is named (#3533).
    #[test]
    fn conflicting_include_requirements_fail_closed() -> Result<(), String> {
        // Producer level: the invocation's own attributes classify through
        // the shared cfg authority.
        let source = "include!(\"fragment.rs\");\n\n#[cfg(test)]\ninclude!(\"fragment.rs\");\n";
        let directives =
            crate::analysis::syntax::rust_include_directives(Path::new("src/lib.rs"), source, 16)?;
        assert_eq!(directives.len(), 2);
        assert!(!directives[0].requires_test);
        assert!(directives[1].requires_test);
        Ok(())
    }
}
