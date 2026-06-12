//! Related test candidate discovery for the TypeScript preview adapter.

use super::*;

pub(crate) fn related_test_candidates<'a>(
    owner: &TypeScriptOwner,
    all_tests: &'a [TypeScriptTest],
) -> Vec<TypeScriptRelatedCandidate<'a>> {
    let mut candidates: Vec<TypeScriptRelatedCandidate<'a>> = all_tests
        .iter()
        .filter_map(|test| {
            owner_call_relation(test, owner)
                .map(|relation| TypeScriptRelatedCandidate { test, relation })
        })
        .collect();
    if candidates.is_empty() {
        candidates = all_tests
            .iter()
            .filter_map(|test| {
                heuristic_relation(test, owner)
                    .map(|relation| TypeScriptRelatedCandidate { test, relation })
            })
            .collect();
    }
    sort_related_candidates(&mut candidates);
    candidates
}

pub(crate) fn sort_related_candidates(candidates: &mut [TypeScriptRelatedCandidate<'_>]) {
    candidates.sort_by(|left, right| {
        right
            .relation
            .rank()
            .cmp(&left.relation.rank())
            .then_with(|| {
                let left_rank = strongest_assertion(&left.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                let right_rank = strongest_assertion(&right.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                right_rank.cmp(&left_rank)
            })
            .then_with(|| left.test.file.cmp(&right.test.file))
            .then_with(|| left.test.line.cmp(&right.test.line))
            .then_with(|| left.test.name.cmp(&right.test.name))
    });
}

pub(crate) fn owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptRelationKind> {
    if owner.owner_kind == OwnerKind::ModuleFunction {
        return module_initializer_observer_relation(test, owner)
            .then_some(TypeScriptRelationKind::ModuleValueReference);
    }
    if owner.owner_kind == OwnerKind::Method {
        return receiver_owner_call_relation(test, owner)
            .then_some(TypeScriptRelationKind::ReceiverOwnerCall);
    }
    if owner.owner_kind == OwnerKind::ClassMethod {
        return class_method_owner_call_relation(test, owner)
            .then_some(TypeScriptRelationKind::ClassMethodCall);
    }
    if contains_call_name(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner)
    {
        return Some(TypeScriptRelationKind::DirectOwnerCall);
    }
    if test.imports_in_file.iter().any(|import| {
        import_source_matches_owner(import, &test.file, owner)
            && import_references_owner_call(import, &test.body_text, owner)
    }) {
        return Some(TypeScriptRelationKind::ImportedOwnerCall);
    }
    None
}

pub(crate) fn module_initializer_observer_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> bool {
    if owner.owner_kind != OwnerKind::ModuleFunction || test_mocks_owner_module(test, owner) {
        return false;
    }
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file)
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
        && expect_actual_references_identifier(&test.body_text, &owner.name)
    {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        if !import_source_matches_owner(import, &test.file, owner) {
            return false;
        }
        if import.namespace {
            return expect_actual_references_member(&test.body_text, &import.local, &owner.name);
        }
        import.imported.as_deref() == Some(owner.name.as_str())
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
            && expect_actual_references_identifier(&test.body_text, &import.local)
    })
}

pub(crate) fn receiver_owner_call_relation(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    if owner.owner_kind != OwnerKind::Method || test_mocks_owner_module(test, owner) {
        return false;
    }
    let constructor_names = constructor_names_for_method_owner(test, owner);
    if constructor_names.is_empty() {
        return false;
    }
    receiver_names_for_constructor_calls(&test.body_text, &constructor_names)
        .iter()
        .any(|receiver| contains_member_call_name(&test.body_text, receiver, &owner.name))
}

pub(crate) fn class_method_owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> bool {
    if owner.owner_kind != OwnerKind::ClassMethod || test_mocks_owner_module(test, owner) {
        return false;
    }
    let class_names = class_names_for_class_method_owner(test, owner);
    if class_names.is_empty() {
        return false;
    }
    class_names
        .iter()
        .any(|class_name| contains_member_call_name(&test.body_text, class_name, &owner.name))
}

pub(crate) fn class_names_for_class_method_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Vec<String> {
    let Some(class_name) = owner.class_name.as_deref() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file)
        && !local_identifier_declared_in_test_body(&test.body_text, class_name)
    {
        push_unique_string(&mut names, class_name.to_string());
    }
    for import in &test.imports_in_file {
        if import.namespace || !import_source_matches_owner(import, &test.file, owner) {
            continue;
        }
        if import.imported.as_deref() == Some(class_name)
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
        {
            push_unique_string(&mut names, import.local.clone());
        }
    }
    names
}

pub(crate) fn constructor_names_for_method_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Vec<String> {
    let Some(class_name) = owner.class_name.as_deref() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file) {
        push_unique_string(&mut names, class_name.to_string());
    }
    for import in &test.imports_in_file {
        if import.namespace || !import_source_matches_owner(import, &test.file, owner) {
            continue;
        }
        if import.imported.as_deref() == Some(class_name) {
            push_unique_string(&mut names, import.local.clone());
        }
    }
    names
}

pub(crate) fn receiver_names_for_constructor_calls(
    body_text: &str,
    constructor_names: &[String],
) -> Vec<String> {
    let mut receiver_names = Vec::new();
    for line in body_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(after_keyword) = ["const ", "let ", "var "]
            .into_iter()
            .find_map(|keyword| trimmed.strip_prefix(keyword))
        else {
            continue;
        };
        let Some((declaration, initializer)) = after_keyword.split_once('=') else {
            continue;
        };
        let Some(receiver_name) = receiver_name_from_declaration(declaration) else {
            continue;
        };
        if constructor_names
            .iter()
            .any(|constructor| contains_new_constructor_call(initializer, constructor))
        {
            push_unique_string(&mut receiver_names, receiver_name);
        }
    }
    receiver_names
}

pub(crate) fn receiver_name_from_declaration(declaration: &str) -> Option<String> {
    if declaration.contains(',') {
        return None;
    }
    let name = declaration.split(':').next()?.trim();
    is_safe_javascript_identifier(name).then(|| name.to_string())
}

pub(crate) fn contains_new_constructor_call(text: &str, constructor_name: &str) -> bool {
    let needle = format!("new {constructor_name}(");
    text.match_indices(&needle).any(|(idx, _)| {
        text[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
            && !line_prefix_looks_like_comment_or_string(text, idx)
            && !inside_block_comment(text, idx)
    })
}

fn test_mocks_owner_module(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.mocks_in_file.iter().any(|source| {
        normalized_relative_import_module(&test.file, source)
            .is_some_and(|module| module == normalized_module_path(&owner.file))
    })
}

fn heuristic_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptRelationKind> {
    if !heuristic_owner_supported(owner) {
        return None;
    }
    if !heuristic_relation_allowed(test, owner) {
        return None;
    }
    if same_file_proximity_related(test, owner) {
        return Some(TypeScriptRelationKind::SameFileProximity);
    }
    if describe_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::DescribeName);
    }
    if test_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::TestName);
    }
    None
}

fn heuristic_owner_supported(owner: &TypeScriptOwner) -> bool {
    matches!(
        owner.owner_kind,
        OwnerKind::Function | OwnerKind::ArrowFunction | OwnerKind::Component
    )
}

pub(crate) fn find_related_tests(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
) -> Vec<RelatedTest> {
    related_test_candidates(owner, all_tests)
        .into_iter()
        .map(|candidate| {
            let strongest = candidate
                .relation
                .uses_oracle()
                .then(|| strongest_assertion(&candidate.test.assertions))
                .flatten();
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: candidate.test.name.clone(),
                file: candidate.test.file.clone(),
                line: candidate.test.line,
                oracle: oracle_text,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect()
}

fn heuristic_relation_allowed(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    !owner_name_shadowed_by_unrelated_import(test, owner)
        && !owner_export_imported_from_unrelated_source(test, owner)
}

pub(crate) fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    let needle = format!("{call_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn has_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

fn owner_name_shadowed_by_unrelated_import(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.imports_in_file
        .iter()
        .filter(|import| import.local == owner.name)
        .any(|import| {
            import.namespace
                || !import_source_matches_owner(import, &test.file, owner)
                || import.imported.as_deref().is_some_and(|imported| {
                    imported != owner.name.as_str() && imported != "default"
                })
        })
}

fn owner_export_imported_from_unrelated_source(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> bool {
    test.imports_in_file.iter().any(|import| {
        import.imported.as_deref() == Some(owner.name.as_str())
            && !import_source_matches_owner(import, &test.file, owner)
    })
}

fn import_references_owner_call(
    import: &TypeScriptImport,
    body_text: &str,
    owner: &TypeScriptOwner,
) -> bool {
    if import.namespace {
        return contains_member_call_name(body_text, &import.local, &owner.name);
    }
    import.imported.as_deref() == Some(owner.name.as_str())
        && contains_call_name(body_text, &import.local)
}

fn import_source_matches_owner(
    import: &TypeScriptImport,
    test_file: &Path,
    owner: &TypeScriptOwner,
) -> bool {
    normalized_relative_import_module(test_file, &import.source)
        .is_some_and(|module| module == normalized_module_path(&owner.file))
}

fn normalized_relative_import_module(test_file: &Path, source: &str) -> Option<String> {
    if !source.starts_with("./") && !source.starts_with("../") {
        return None;
    }
    let mut parts = normalized_path(test_file.parent().unwrap_or_else(|| Path::new("")))
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let normalized_source = source.replace('\\', "/");
    for part in normalized_source.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_string()),
        }
    }
    Some(strip_typescript_module_extension(&parts.join("/")))
}

fn normalized_module_path(path: &Path) -> String {
    strip_typescript_module_extension(&normalized_path(path))
}

fn strip_typescript_module_extension(path: &str) -> String {
    for suffix in [".tsx", ".ts", ".jsx", ".js"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn contains_member_call_name(body_text: &str, object_name: &str, method_name: &str) -> bool {
    let needle = format!("{object_name}.{method_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn expect_actual_references_identifier(body_text: &str, identifier: &str) -> bool {
    is_safe_javascript_identifier(identifier)
        && expect_actual_slices(body_text).iter().any(|actual| {
            actual.trim_start().starts_with(identifier)
                && actual
                    .trim_start()
                    .get(identifier.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
        })
}

fn expect_actual_references_member(
    body_text: &str,
    object_name: &str,
    property_name: &str,
) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(property_name)
    {
        return false;
    }
    let reference = format!("{object_name}.{property_name}");
    expect_actual_slices(body_text).iter().any(|actual| {
        actual.trim_start().starts_with(&reference)
            && actual
                .trim_start()
                .get(reference.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|ch| !is_javascript_identifier_char(ch))
    })
}

fn expect_actual_slices(body_text: &str) -> Vec<&str> {
    body_text
        .match_indices("expect(")
        .filter_map(|(idx, _)| {
            if line_prefix_looks_like_comment_or_string(body_text, idx)
                || inside_block_comment(body_text, idx)
            {
                return None;
            }
            body_text.get(idx + "expect(".len()..)
        })
        .collect()
}

fn local_identifier_declared_in_test_body(body_text: &str, identifier: &str) -> bool {
    body_text.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//") && declaration_line_declares_identifier(trimmed, identifier)
    })
}

fn declaration_line_declares_identifier(line: &str, identifier: &str) -> bool {
    ["const ", "let ", "var ", "function "]
        .into_iter()
        .filter_map(|keyword| line.strip_prefix(keyword))
        .filter_map(|after| {
            after
                .split(|ch: char| {
                    ch == ':'
                        || ch == '='
                        || ch == '('
                        || ch == ','
                        || ch == ';'
                        || ch.is_whitespace()
                })
                .find(|part| !part.is_empty())
        })
        .any(|declared| declared == identifier)
}

pub(crate) fn has_member_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

pub(crate) fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with("//") || has_unclosed_quote_or_template(prefix)
}

pub(crate) fn inside_block_comment(body_text: &str, idx: usize) -> bool {
    let prefix = &body_text[..idx];
    let comment_start = prefix.rfind("/*");
    let comment_end = prefix.rfind("*/");
    comment_start.is_some_and(|start| comment_end.is_none_or(|end| start > end))
}

fn has_unclosed_quote_or_template(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double && !in_template {
            in_single = !in_single;
        } else if ch == '"' && !in_single && !in_template {
            in_double = !in_double;
        } else if ch == '`' && !in_single && !in_double {
            in_template = !in_template;
        }
    }
    in_single || in_double || in_template
}

pub(crate) fn is_javascript_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn same_file_proximity_related(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let Some(owner_stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(test_stem) = test.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let owner_key = normalize_typescript_test_stem(owner_stem);
    let test_key = normalize_typescript_test_stem(test_stem);
    !owner_key.is_empty() && owner_key == test_key
}

fn normalize_typescript_test_stem(stem: &str) -> String {
    let mut value = stem.to_string();
    for suffix in [".test", ".spec", "_test", "-test"] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            value = stripped.to_string();
            break;
        }
    }
    for prefix in ["test.", "test_", "test-"] {
        if let Some(stripped) = value.strip_prefix(prefix) {
            value = stripped.to_string();
            break;
        }
    }
    normalize_similarity_key(&value)
}

fn describe_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.describe_names.iter().any(|name| {
        let describe_key = normalize_similarity_key(name);
        owner_similarity_keys(owner)
            .into_iter()
            .any(|key| similarity_key_contains(&describe_key, &key))
    })
}

fn test_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let test_key = normalize_similarity_key(&test.local_name);
    owner_similarity_keys(owner)
        .into_iter()
        .any(|key| similarity_key_contains(&test_key, &key))
}

fn owner_similarity_keys(owner: &TypeScriptOwner) -> Vec<String> {
    let mut keys = Vec::new();
    push_unique_similarity_key(&mut keys, normalize_similarity_key(&owner.name));
    keys
}

fn push_unique_similarity_key(keys: &mut Vec<String>, key: String) {
    if !key.is_empty() && !keys.iter().any(|existing| existing == &key) {
        keys.push(key);
    }
}

fn normalize_similarity_key(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = true;
    let mut previous_was_lower_or_digit = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && !out.is_empty()
                && !last_was_separator
                && previous_was_lower_or_digit
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !out.is_empty() && !last_was_separator {
            out.push('_');
            last_was_separator = true;
            previous_was_lower_or_digit = false;
        } else {
            previous_was_lower_or_digit = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack == needle
        || haystack
            .strip_prefix(needle)
            .is_some_and(|suffix| suffix.starts_with('_'))
        || haystack
            .strip_suffix(needle)
            .is_some_and(|prefix| prefix.ends_with('_'))
        || haystack.contains(&format!("_{needle}_"))
}
