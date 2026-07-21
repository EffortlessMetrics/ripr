use super::*;

pub(super) mod context;

pub(crate) use context::CompactGripContext;
pub(super) use context::{CompactTest, call_text_contains_named_call};

/// Walk `index.tests` and return tests that plausibly relate to `seam`,
/// each tagged with the single highest-priority `RelationReason` it
/// satisfies. The two-step "match then rank" replaces the old binary
/// `calls_owner || same_file_or_named` check from earlier campaigns.
///
/// Detection per reason — strict ordering: the first reason that fires
/// wins, so e.g. a test that both `calls owner` and `is in same file`
/// carries `direct_owner_call`, never `same_test_file`.
pub(super) fn find_related_tests_with_context<'context, 'index>(
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
pub(super) struct OwnerContext {
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
pub(super) fn assertion_target_tokens(seam: &RepoSeam) -> BTreeSet<String> {
    let discriminator_tokens = match seam.required_discriminator() {
        crate::analysis::seams::RequiredDiscriminator::MatchArmTaken { arm } => {
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

pub(super) fn match_arm_assertion_target_tokens(arm: &str) -> Vec<String> {
    extract_identifier_tokens(arm)
        .into_iter()
        .filter(|token| match_arm_assertion_target_token_is_specific_enough(token))
        .collect()
}

pub(super) fn match_arm_assertion_target_token_is_specific_enough(token: &str) -> bool {
    token.chars().next().is_some_and(|ch| ch.is_uppercase())
}

pub(super) fn call_presence_callee_target_tokens(expression: &str) -> Vec<String> {
    let mut tokens = extract_call_facts(expression, 0)
        .into_iter()
        .flat_map(|call| extract_identifier_tokens(&call.name))
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

pub(super) fn call_presence_assertion_affinity_token_is_specific_enough(token: &str) -> bool {
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

pub(super) fn match_direct_owner_call(
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

pub(super) fn match_helper_owner_call(
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

pub(super) fn match_target_affinity_owner_call(
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

pub(super) fn test_assertion_mentions_any_target_token(
    test: &CompactTest<'_>,
    target_tokens: &BTreeSet<String>,
) -> bool {
    target_tokens
        .iter()
        .any(|token| test.assertion_tokens.contains(token))
}

pub(super) fn match_assertion_target_affinity(
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

pub(super) fn match_same_test_file(
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

pub(super) fn match_same_module(
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

pub(super) fn match_owner_named_test(
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

pub(super) fn match_import_path_affinity(
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

pub(super) fn match_fixture_owner_affinity(
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

pub(super) fn dedupe_related_candidates<'context, 'index>(
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

pub(super) fn insert_related_candidate(
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

pub(super) fn find_related_tests_compact<'a>(
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
pub(super) struct RelatedTestRankKey {
    relation_confidence: u8,
    relation_reason: u8,
    oracle_strength: Reverse<u8>,
    activation_overlap: Reverse<usize>,
    file: PathBuf,
    test_name: String,
    line: usize,
}

pub(super) fn sort_related_tests_for_seam(
    seam: &RepoSeam,
    context: &CompactGripContext<'_>,
    related: &mut [(&CompactTest<'_>, RelationReason)],
) {
    related.sort_by_cached_key(|entry| {
        let (indexed, reason) = *entry;
        related_test_rank_key(seam, context, indexed, reason)
    });
}

pub(super) fn related_test_rank_key(
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

pub(super) fn fixture_names_for_owner_file(facts: &rust_index::FileFacts) -> BTreeSet<String> {
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
pub(super) fn required_discriminator_tokens(seam: &RepoSeam) -> Vec<String> {
    extract_identifier_tokens(required_discriminator_text(seam))
}

pub(super) fn required_discriminator_text(seam: &RepoSeam) -> &str {
    use crate::analysis::seams::RequiredDiscriminator;
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
pub(super) fn assertion_targets_seam(test: &TestSummary, tokens: &[String]) -> bool {
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
pub(super) fn same_test_file(test_file: &Path, owner_stem: &str) -> bool {
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
pub(super) fn module_path_for(file: &Path) -> Option<String> {
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
pub(super) fn same_module(owner_module: &str, test_module: &str) -> bool {
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
pub(super) fn test_imports_owner_compact(test: &CompactTest<'_>, owner_name: &str) -> bool {
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

pub(super) fn import_affinity_tokens(code_lines: &[String]) -> BTreeSet<String> {
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
pub(super) fn strip_comments_and_strings(line: &str) -> String {
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

pub(super) fn is_fixture_named(name: &str) -> bool {
    let prefixes = ["fixture_", "setup_", "make_", "build_", "new_", "mock_"];
    let suffixes = ["_fixture", "_factory"];
    prefixes.iter().any(|p| name.starts_with(p)) || suffixes.iter().any(|s| name.ends_with(s))
}

pub(super) fn find_owner_function<'a>(
    seam: &RepoSeam,
    index: &'a RustIndex,
) -> Option<&'a FunctionSummary> {
    rust_index::find_owner_function(index, seam.file(), seam.display_line())
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(super) fn package_prefix(path: &Path) -> Option<String> {
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

pub(super) fn package_scope(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(prefix) = package_prefix(path) {
        return Some(prefix);
    }
    if normalized.starts_with("src/") || normalized.starts_with("tests/") {
        return Some(String::new());
    }
    None
}
