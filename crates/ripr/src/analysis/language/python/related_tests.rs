use super::source_utils::normalized_path;
use super::{
    PythonAssertion, PythonImport, PythonOwner, PythonTest, first_python_string_literal,
    line_prefix_before, python_callee_start_has_boundary, python_prefix_hides_code,
    python_string_literal_value,
};
use crate::domain::{ExposureClass, OracleKind, OracleStrength, OwnerKind, RelatedTest};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PythonRelationKind {
    SyntacticCall,
    ImportAliasCall,
    ApiClientRouteCall,
    ConstructCall,
    LocalBinding,
    SameStem,
    TestNameSimilarity,
    FixtureName,
}

impl PythonRelationKind {
    fn rank(self) -> u8 {
        match self {
            Self::SyntacticCall => 5,
            Self::ImportAliasCall => 4,
            Self::ApiClientRouteCall => 4,
            Self::ConstructCall => 4,
            Self::LocalBinding => 4,
            Self::SameStem => 3,
            Self::TestNameSimilarity => 2,
            Self::FixtureName => 1,
        }
    }

    pub(super) fn uses_oracle(self) -> bool {
        matches!(
            self,
            Self::SyntacticCall
                | Self::ImportAliasCall
                | Self::ApiClientRouteCall
                | Self::ConstructCall
                | Self::LocalBinding
        )
    }

    pub(super) fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::SyntacticCall => "syntactic_call",
            Self::ImportAliasCall => "import_alias_call",
            Self::ApiClientRouteCall => "api_client_route_call",
            Self::ConstructCall => "construct_call",
            Self::LocalBinding => "local_binding",
            Self::SameStem => "same_stem",
            Self::TestNameSimilarity => "test_name_similarity",
            Self::FixtureName => "fixture_name",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PythonRelatedCandidate<'a> {
    pub(super) test: &'a PythonTest,
    pub(super) relation: PythonRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonRepairPlacement {
    pub(super) repair_action: &'static str,
    pub(super) suggested_test_file: String,
    pub(super) suggested_test_name: String,
    pub(super) suggested_test_node_id: Option<String>,
    pub(super) verify_command: String,
    pub(super) verify_command_confidence: &'static str,
    pub(super) location_reason: &'static str,
}

pub(super) fn related_test_candidates<'a>(
    owner: &PythonOwner,
    all_tests: &'a [PythonTest],
) -> Vec<PythonRelatedCandidate<'a>> {
    let mut candidates: Vec<PythonRelatedCandidate<'a>> = all_tests
        .iter()
        .filter_map(|test| {
            related_test_relation(test, owner)
                .map(|relation| PythonRelatedCandidate { test, relation })
        })
        .collect();
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
            .then_with(|| left.test.name.cmp(&right.test.name))
    });
    candidates
}

pub(super) fn find_related_tests(
    owner: &PythonOwner,
    all_tests: &[PythonTest],
) -> Vec<RelatedTest> {
    related_test_candidates(owner, all_tests)
        .into_iter()
        .map(|candidate| {
            let strongest = candidate
                .relation
                .uses_oracle()
                .then(|| strongest_assertion(&candidate.test.assertions))
                .flatten();
            let (oracle_kind, oracle_strength, oracle) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion.text.clone()),
                ),
                None if candidate.relation.uses_oracle() && candidate.test.parametrized => (
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                    Some("pytest.mark.parametrize".to_string()),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: candidate.test.name.clone(),
                file: candidate.test.file.clone(),
                line: candidate.test.line,
                oracle,
                oracle_kind,
                oracle_strength,
                relation_reason: None,
                relation_confidence: None,
            }
        })
        .collect()
}

pub(super) fn verify_command_for_test(test: &PythonTest) -> Option<String> {
    let path = normalized_path(&test.file);
    match test.framework {
        "pytest" => {
            let node = test.qualified_name.replace('.', "::");
            Some(format!("pytest {}::{node}", shell_quote_file_arg(&path)))
        }
        "unittest" => {
            let module = shell_quote_file_arg(&unittest_module_for_path(&path));
            Some(format!(
                "python -m unittest {module}.{}",
                test.qualified_name
            ))
        }
        _ => None,
    }
}

fn unittest_module_for_path(path: &str) -> String {
    path.strip_suffix(".py")
        .unwrap_or(path)
        .replace(['/', '\\'], ".")
}

/// Quote one path or module token for a POSIX shell.
///
/// Suggested verify commands are text an agent may paste into a shell. The
/// TypeScript command uses the same character class: a relative path made of
/// ASCII letters, digits, `.`, `_`, `/`, and `-` stays readable, and any other
/// byte is single-quoted so `$()`, backticks, spaces, and quotes are not
/// expanded. Stored node ids and test-file paths stay the raw spelling; only
/// the command text is quoted. This does not run the command.
fn shell_quote_file_arg(file_str: &str) -> String {
    if !file_str.is_empty()
        && file_str
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
    {
        return file_str.to_string();
    }
    format!("'{}'", file_str.replace('\'', "'\\''"))
}

pub(super) fn python_repair_placement(
    class: &ExposureClass,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonRepairPlacement> {
    if !matches!(class, ExposureClass::WeaklyExposed) {
        return None;
    }
    let candidate = related_candidates
        .iter()
        .find(|candidate| candidate.relation.uses_oracle())?;
    let path = normalized_path(&candidate.test.file);
    match candidate.test.framework {
        "pytest" => {
            let node = candidate.test.qualified_name.replace('.', "::");
            let node_id = format!("{path}::{node}");
            Some(PythonRepairPlacement {
                repair_action: "strengthen_existing_test",
                suggested_test_file: path,
                suggested_test_name: candidate.test.name.clone(),
                suggested_test_node_id: Some(node_id),
                verify_command: verify_command_for_test(candidate.test)?,
                verify_command_confidence: "high",
                location_reason: "strengthen existing weak pytest relation",
            })
        }
        "unittest" => Some(PythonRepairPlacement {
            repair_action: "strengthen_existing_test",
            suggested_test_file: path,
            suggested_test_name: candidate.test.name.clone(),
            suggested_test_node_id: None,
            verify_command: verify_command_for_test(candidate.test)?,
            verify_command_confidence: "high",
            location_reason: "strengthen existing weak unittest relation",
        }),
        _ => None,
    }
}

pub(super) fn strongest_assertion(assertions: &[PythonAssertion]) -> Option<&PythonAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

pub(super) fn related_test_relation(
    test: &PythonTest,
    owner: &PythonOwner,
) -> Option<PythonRelationKind> {
    if body_calls_owner(&test.body_text, owner) {
        return Some(PythonRelationKind::SyntacticCall);
    }
    if import_alias_calls_owner(test, owner) {
        return Some(PythonRelationKind::ImportAliasCall);
    }
    if api_client_route_calls_owner(test, owner) {
        return Some(PythonRelationKind::ApiClientRouteCall);
    }
    if construct_call_invokes_owner(test, owner) {
        return Some(PythonRelationKind::ConstructCall);
    }
    if local_binding_calls_owner(test, owner) {
        return Some(PythonRelationKind::LocalBinding);
    }
    // A test is related to an owner only when it references the owner
    // (RIPR-SPEC-0028). File-stem, test-name and fixture-name proximity are
    // ranking labels for a test that already references the owner without a
    // recognized call shape; they never relate a test that only exercises a
    // sibling owner in the same module (flat layout: `loyalty_price` in
    // `pricing.py` was linked to `tests/test_pricing.py` tests that only call
    // `discounted_total`).
    if !test_references_owner(test, owner) {
        return None;
    }
    if same_stem_related(test, owner) {
        return Some(PythonRelationKind::SameStem);
    }
    if test_name_similar_to_owner(test, owner) {
        return Some(PythonRelationKind::TestNameSimilarity);
    }
    if fixture_name_related_to_owner(test, owner) {
        return Some(PythonRelationKind::FixtureName);
    }
    None
}

pub(super) fn body_calls_owner(body_text: &str, owner: &PythonOwner) -> bool {
    contains_call_name(body_text, &owner.name)
        || (owner.qualified_name != owner.name
            && contains_call_name(body_text, &owner.qualified_name))
        || (matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        ) && contains_any_attribute_call(body_text, &owner.name))
}

/// Detects an inline construct-call `OwnerClass(...)(...)` that invokes a changed
/// `__call__` owner directly (e.g. `LogfmtRenderer()(None, None, event_dict)`), a
/// cross-file shape the name/attribute and import-alias heuristics miss — the
/// changed sink is the class's `__call__`, but the test never names `__call__`.
/// Strictly gated so it never over-links: the changed owner must be a `__call__`
/// method (Guard A); the test must import the owner's class by name or alias
/// (Guard B), which blocks a same-named class from an unrelated module; and the
/// constructed instance must be *immediately* called (the balanced-paren check),
/// which distinguishes the inline `C()(...)` from a bound local `x = C(); x(...)`
/// (the latter stays uncertain, consistent with the local-callable limitation).
fn construct_call_invokes_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // Guard A: only callable-class `__call__` owners.
    if owner.name != "__call__"
        || !matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        )
    {
        return false;
    }
    let Some((class_name, _)) = owner.qualified_name.rsplit_once('.') else {
        return false;
    };
    if class_name.is_empty() || !class_name.chars().all(is_python_identifier_char) {
        return false;
    }
    // Guard B: the test must import the owner's class — blocks same-named classes
    // in unrelated modules from cross-linking.
    let imports_class = test
        .imports
        .iter()
        .any(|import| import.imported == class_name || import.alias == class_name);
    if !imports_class {
        return false;
    }
    let needle = format!("{class_name}(");
    test.body_text.match_indices(&needle).any(|(idx, _)| {
        python_callee_start_has_boundary(&test.body_text, idx)
            && !line_prefix_looks_like_comment_or_string(&test.body_text, idx)
            && construct_result_is_called(&test.body_text, idx + needle.len() - 1)
    })
}

/// Given the byte index of the `(` that opens a constructor call, returns whether
/// its matching `)` is immediately followed (skipping spaces/tabs) by another `(`
/// — i.e. the constructed instance is called inline, `C(...)(...)`.
pub(super) fn construct_result_is_called(text: &str, open_paren_idx: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open_paren_idx;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    let mut next = index + 1;
                    while matches!(bytes.get(next), Some(b' ' | b'\t')) {
                        next += 1;
                    }
                    return bytes.get(next) == Some(&b'(');
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

/// Detects a single unambiguous local binding `local = OwnerClass(...)` whose
/// bound local is then called `local(...)`, invoking a changed `__call__` owner
/// indirectly — the tenacity `stop = stop_after_attempt(3); assertTrue(stop(3))`
/// shape that the Tier B judging measured as a false-actionable (#1160). The test
/// genuinely reaches the owner, so the relation is *direct*: surfacing it lets the
/// existing oracle on the bound call (here a broad-boolean smoke `assertTrue`) be
/// reported instead of dropped, correcting the misleading "no direct test exists"
/// diagnosis. This NEVER credits `exposed` — a smoke oracle stays below `Strong`,
/// so the classification remains `weakly_exposed` (matching the sibling
/// `python_broad_boolean_assertion` golden), only the relation/oracle/card change.
///
/// Strictly gated so it never over-links and never collides with `ConstructCall`
/// (the inline `C()(...)` shape, which is checked first):
///   A. the owner is a `__call__` method;
///   B. the test imports the owner's class by name or alias (blocks a same-named
///      class in an unrelated module);
///   C. exactly one real `Class(` construction appears (call boundary, not a
///      comment/string) and it is *not* called inline (inline is `ConstructCall`);
///   D. that construction is a direct assignment `local = Class(` on its line —
///      keyword-arg / wrapper shapes like `Retrying(stop=stop_after_attempt(3))`
///      fail here and stay uncertain;
///   E. the bound `local` is itself called `local(`, and it is assigned exactly
///      once (a reassigned / rebound local is ambiguous and is rejected).
pub(super) fn local_binding_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // Guard A: only callable-class `__call__` owners.
    if owner.name != "__call__"
        || !matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        )
    {
        return false;
    }
    let Some((class_name, _)) = owner.qualified_name.rsplit_once('.') else {
        return false;
    };
    if class_name.is_empty() || !class_name.chars().all(is_python_identifier_char) {
        return false;
    }
    // Guard B: the test must import the owner's class.
    let imports_class = test
        .imports
        .iter()
        .any(|import| import.imported == class_name || import.alias == class_name);
    if !imports_class {
        return false;
    }
    let body = &test.body_text;
    let needle = format!("{class_name}(");
    // Guard C: exactly one real, non-inline `Class(` construction.
    let mut constructions = body.match_indices(&needle).filter(|(idx, _)| {
        python_callee_start_has_boundary(body, *idx)
            && !line_prefix_looks_like_comment_or_string(body, *idx)
    });
    let Some((idx, _)) = constructions.next() else {
        return false;
    };
    if constructions.next().is_some() {
        // More than one construction of the class — ambiguous; stay conservative.
        return false;
    }
    // An inline `Class()(...)` is `ConstructCall` territory, not a bound local.
    if construct_result_is_called(body, idx + needle.len() - 1) {
        return false;
    }
    // Guard D: the construction is a direct assignment `local = Class(` on its line.
    let Some(local_var) = binding_target_for_construction(body, idx) else {
        return false;
    };
    // Guard E: the bound local is called, and assigned exactly once.
    contains_call_name(body, &local_var) && assignment_count(body, &local_var) == 1
}

/// Given the byte index of a `Class(` construction, returns the single local
/// variable it is directly assigned to on the same line — `local = Class(` yields
/// `Some("local")`. Returns `None` for keyword-argument (`stop=Class(`), chained
/// (`a = b = Class(`), augmented, attribute-target (`self.x = Class(`), or any
/// non-bare-identifier assignment, so wrapper/dispatch shapes stay uncertain.
pub(super) fn binding_target_for_construction(
    body_text: &str,
    construction_idx: usize,
) -> Option<String> {
    let line_start = body_text[..construction_idx]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let prefix = body_text[line_start..construction_idx].trim();
    // Require the line to be exactly `<identifier> =` immediately before `Class(`.
    let assign = prefix.strip_suffix('=')?;
    // Reject compound/comparison/augmented operators (`==`, `!=`, `<=`, `+=`, ...).
    if assign.ends_with([
        '=', '!', '<', '>', '+', '-', '*', '/', '%', '&', '|', '^', '~', ':',
    ]) {
        return None;
    }
    let name = assign.trim();
    if name.is_empty()
        || name.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        || !name.chars().all(is_python_identifier_char)
    {
        return None;
    }
    Some(name.to_string())
}

/// Counts direct assignments `name = ...` (whole-token target, not `==`, not an
/// augmented assignment, not a substring of a longer identifier or an attribute
/// like `self.name`) across the test body, so a reassigned binding is rejected.
fn assignment_count(body_text: &str, name: &str) -> usize {
    body_text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix(name) else {
                return false;
            };
            // Whole-token boundary: the char after `name` ends the identifier.
            if rest.chars().next().is_some_and(is_python_identifier_char) {
                return false;
            }
            let rest = rest.trim_start();
            rest.starts_with('=') && !rest.starts_with("==")
        })
        .count()
}

fn import_alias_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // A method/classmethod cannot be imported directly by its bare name, so the
    // `import.imported == owner.name` branch would only ever match a same-named
    // free function in an unrelated module — a false relation that then feeds a
    // false-`exposed`. Restrict that branch to non-method owners.
    let is_method_owner = matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    );
    test.imports.iter().any(|import| {
        (!is_method_owner
            && import.imported == owner.name
            && import.alias != owner.name
            && contains_call_name(&test.body_text, &import.alias))
            || (imported_module_matches_owner(import, owner)
                && contains_attribute_call(&test.body_text, &import.alias, &owner.name))
    })
}

pub(super) fn imported_module_matches_owner(import: &PythonImport, owner: &PythonOwner) -> bool {
    owner
        .file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| import.imported.rsplit('.').next() == Some(stem))
}

/// The dotted module paths under which the owner file can be imported.
///
/// The first entry is the owner file's full repository-relative module path:
/// `src/handler.py` → `src.handler`, `src/pkg/__init__.py` → `src.pkg`.
/// Identity comparisons must use a full dotted path — a bare file stem is the
/// token-coincidence family (`src/tests/test_handler.py` importing `.handler`
/// resolves to `src.tests.handler`, a different module with the same stem).
///
/// A directory named `src` is the PyPA *src layout* import root: with
/// `src/pricing/discounts.py`, tests import `pricing.discounts` (pytest
/// `pythonpath = ["src"]` or an installed package), never `src.pricing...`. So
/// for every `src` directory segment the dotted path *below* it is also an
/// importable name of the same file (`pricing.discounts`, and in a monorepo
/// `packages/foo/src/foo/bar.py` → `foo.bar`). The full path is kept too, so
/// projects that really write `from src.pricing.discounts import ...` still
/// match. Each form is a complete module path compared by exact equality; no
/// stem or suffix matching is introduced.
pub(super) fn owner_module_paths(file: &Path) -> Vec<String> {
    let normalized = normalized_path(file);
    let mut parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if let Some(last) = parts.last_mut() {
        if let Some(stem) = last.strip_suffix(".py") {
            *last = stem;
        }
        if *last == "__init__" {
            parts.pop();
        }
    }
    let mut paths = vec![parts.join(".")];
    // Only directory segments are import roots: the final segment is the module
    // itself (`src.py` / `src/__init__.py` is a module named `src`).
    let directory_count = parts.len().saturating_sub(1);
    for (idx, part) in parts.iter().enumerate().take(directory_count) {
        if *part == "src" {
            let below = parts.get(idx + 1..).unwrap_or_default().join(".");
            if !below.is_empty() && !paths.contains(&below) {
                paths.push(below);
            }
        }
    }
    paths
}

/// Whether a `from M import Y` statement's source module `M` is the owner's
/// module. `M` must equal one of the owner's full dotted module paths (see
/// [`owner_module_paths`]): `from src.handler import validate`, a resolved
/// `from .handler import validate` in a sibling file, and — for the src layout
/// — `from handler import validate` all match an owner in `src/handler.py`;
/// `from src.checker import validate` or `from other.handler import validate`
/// do not. A plain `import X` has an empty `source_module` and so never
/// matches — fail closed.
pub(super) fn import_source_module_matches_owner(
    import: &PythonImport,
    owner: &PythonOwner,
) -> bool {
    if import.source_module.is_empty() {
        return false;
    }
    owner_module_paths(&owner.file).contains(&import.source_module)
}

/// Free-function module-identity evidence: a strong observing test imports the
/// owner's function *from the owner's module*. This is what distinguishes a
/// genuine `from src.handler import validate` from a same-named function pulled in
/// via `from src.checker import validate` — the bare function-name token alone is
/// not identity-bearing for a free-function owner.
pub(super) fn strong_test_imports_owner_from_module(
    strong_tests: &[&RelatedTest],
    all_tests: &[PythonTest],
    owner: &PythonOwner,
) -> bool {
    strong_tests.iter().any(|related_test| {
        all_tests.iter().any(|test| {
            test.name == related_test.name
                && test.file == related_test.file
                && test.imports.iter().any(|import| {
                    import.imported == owner.name
                        && import_source_module_matches_owner(import, owner)
                })
        })
    })
}

fn api_client_route_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    owner
        .route_paths
        .iter()
        .any(|route| body_calls_api_client_route(&test.body_text, route))
}

fn body_calls_api_client_route(body_text: &str, route: &str) -> bool {
    [
        "client.get",
        "client.post",
        "client.put",
        "client.patch",
        "client.delete",
        "client.options",
        "client.head",
    ]
    .into_iter()
    .any(|callee| contains_python_call_with_first_string_argument(body_text, callee, route))
}

fn contains_python_call_with_first_string_argument(
    text: &str,
    callee: &str,
    expected: &str,
) -> bool {
    text.match_indices(callee).any(|(idx, _)| {
        if !python_callee_start_has_boundary(text, idx)
            || python_prefix_hides_code(line_prefix_before(text, idx))
        {
            return false;
        }
        let Some(argument) = first_parenthesized_string_argument(
            text.get(idx + callee.len()..)
                .unwrap_or_default()
                .trim_start(),
        ) else {
            return false;
        };
        argument == expected
    })
}

pub(super) fn first_parenthesized_string_argument(text: &str) -> Option<String> {
    let body = text.strip_prefix('(')?.trim_start();
    let literal = first_python_string_literal(body)?;
    body.starts_with(&literal)
        .then(|| python_string_literal_value(&literal))
        .flatten()
}

fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    let needle = format!("{call_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        python_callee_start_has_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
    })
}

fn contains_attribute_call(body_text: &str, receiver: &str, attr: &str) -> bool {
    let needle = format!("{receiver}.{attr}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        python_callee_start_has_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
    })
}

pub(super) fn contains_any_attribute_call(body_text: &str, attr: &str) -> bool {
    let needle = format!(".{attr}(");
    body_text
        .match_indices(&needle)
        .any(|(idx, _)| !line_prefix_looks_like_comment_or_string(body_text, idx))
}

/// Given the byte index of the `(` that opens a `Class(` construction, returns
/// whether its matching `)` is immediately followed (skipping spaces/tabs) by
/// `.method(` — i.e. the constructed instance's method is called inline,
/// `Class(...).method(...)`. Companion to [`construct_result_is_called`] (which
/// detects the `Class()()` callable-instance shape).
fn construct_result_calls_method(text: &str, open_paren_idx: usize, method: &str) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open_paren_idx;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    let mut next = index + 1;
                    while matches!(bytes.get(next), Some(b' ' | b'\t')) {
                        next += 1;
                    }
                    return text[next..].starts_with(&format!(".{method}("));
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

/// Local names the owner class is known by in `test`: its imported name plus any
/// `as` alias. Empty when the class is not imported — conservative by design, so a
/// class defined elsewhere and never imported cannot lend its identity. Only the
/// `imported == class` form is identity-bearing: a different class aliased *to* the
/// owner's name (`from m import Other as OwnerClass`) refers to `Other`, not the
/// owner, so it must not contribute a local.
fn owner_class_locals(test: &PythonTest, class: &str) -> Vec<String> {
    let mut locals = Vec::new();
    for import in &test.imports {
        if import.imported == class && !import.alias.is_empty() && !locals.contains(&import.alias) {
            locals.push(import.alias.clone());
        }
    }
    locals
}

/// Whether `body` calls `method` on a receiver statically bound to the owner class
/// (known locally as `local`). Three bound-receiver shapes, all excluding
/// comment/string occurrences:
///   * `Local.method(...)`         — classmethod / direct call on the class;
///   * `Local(...).method(...)`    — inline construct then method call;
///   * `v = Local(...); v.method(...)` — single local binding then method call.
///
/// A bare `.method(` on an unrelated or unresolved receiver is NOT matched: that
/// is the false-`exposed` guard — importing or merely mentioning the owner class
/// is not evidence the asserted method ran on an instance of it.
fn body_calls_method_on_owner_bound_receiver(body: &str, local: &str, method: &str) -> bool {
    // Pattern 1: `Local.method(` — classmethod / direct call on the class itself.
    if contains_attribute_call(body, local, method) {
        return true;
    }
    let construct = format!("{local}(");
    let constructions: Vec<usize> = body
        .match_indices(&construct)
        .filter(|(idx, _)| {
            python_callee_start_has_boundary(body, *idx)
                && !line_prefix_looks_like_comment_or_string(body, *idx)
        })
        .map(|(idx, _)| idx)
        .collect();
    // Pattern 2: `Local(...).method(` — inline construct then method call.
    if constructions
        .iter()
        .any(|&idx| construct_result_calls_method(body, idx + construct.len() - 1, method))
    {
        return true;
    }
    // Pattern 3: `v = Local(...); v.method(` — a single unambiguous local binding
    // (reuses the LocalBinding guards: one construction, direct assignment, one
    // assignment of the bound local) then a method call on that local.
    if constructions.len() == 1
        && let Some(var) = binding_target_for_construction(body, constructions[0])
        && assignment_count(body, &var) == 1
        && contains_attribute_call(body, &var, method)
    {
        return true;
    }
    false
}

/// Relation-layer receiver-identity evidence for a method/classmethod owner: a
/// strong observing test calls the owner's method on a receiver statically bound
/// to the owner class (see [`body_calls_method_on_owner_bound_receiver`]). This
/// supersedes the weaker "imports + mentions the owner class" gate, which credited
/// `exposed` whenever the class name merely appeared in the test — even as a dead
/// reference or while the asserted `.method(` ran on an unrelated receiver.
pub(super) fn strong_test_calls_owner_method_on_bound_receiver(
    owner_class_token: Option<&String>,
    method_name: Option<&String>,
    strong_tests: &[&RelatedTest],
    all_tests: &[PythonTest],
) -> bool {
    let (Some(class), Some(method)) = (owner_class_token, method_name) else {
        return false;
    };
    strong_tests.iter().any(|related_test| {
        all_tests.iter().any(|test| {
            test.name == related_test.name
                && test.file == related_test.file
                && owner_class_locals(test, class).iter().any(|local| {
                    body_calls_method_on_owner_bound_receiver(&test.body_text, local, method)
                })
        })
    })
}

pub(super) fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with('#') || has_unclosed_quote(prefix)
}

pub(super) fn has_unclosed_quote(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        }
    }
    in_single || in_double
}

pub(super) fn is_python_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Whether the test body references the changed owner, even when the reference
/// is not a recognized call shape. Gates the heuristic relations (same stem,
/// test name, fixture name); the direct relations already imply a reference.
///
/// Counted references, each outside comments and string literals:
///
/// - function or class owner: the bare owner name (`handler = loyalty_price`,
///   `assert callable(loyalty_price)`), unless the test binds a local of the
///   same name (parameter/fixture, assignment, nested `def`/`class`) or the name
///   is a keyword-argument or assignment target (`f(loyalty_price=1)`); a
///   renamed import local (`from pricing import loyalty_price as lp` then `lp`)
///   whose source module is the owner module; or a module-qualified member
///   (`pricing.loyalty_price`, `p.loyalty_price` after `import pricing as p`)
///   through an import of the owner module;
/// - method or class-method owner: an attribute reference `.name` (the direct
///   rule already relates any `.name(` call for these owners), and for a dunder
///   method (`__init__`, `__eq__`, ...) a reference to the owner class, since
///   the class invokes it implicitly;
/// - module owner: a local bound by an import of, or from, the owner module.
///
/// A test that names the owner only in its title, a fixture name, its file
/// stem, a comment or a string does not reference it, and neither does an
/// arbitrary object-member use such as `order.loyalty_price` for a free
/// function owner.
pub(super) fn test_references_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    if owner.is_module_owner() {
        return test_references_owner_module(test, owner);
    }
    if matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    ) {
        if contains_attribute_reference(&test.body_text, &owner.name) {
            return true;
        }
        let is_dunder =
            owner.name.len() > 4 && owner.name.starts_with("__") && owner.name.ends_with("__");
        return is_dunder
            && owner
                .qualified_name
                .rsplit_once('.')
                .is_some_and(|(class, _)| test_references_module_symbol(test, owner, class));
    }
    test_references_module_symbol(test, owner, &owner.name)
}

/// A reference to `symbol`, a top-level name defined in the owner module: the
/// bare name, a renamed import local from the owner module, or a member access
/// through an import of the owner module.
fn test_references_module_symbol(test: &PythonTest, owner: &PythonOwner, symbol: &str) -> bool {
    let body = &test.body_text;
    if !test_binds_local(test, symbol) && contains_name_reference(body, symbol) {
        return true;
    }
    test.imports.iter().any(|import| {
        if import.imported == symbol
            && import.alias != symbol
            && import_source_module_matches_owner(import, owner)
        {
            return !test_binds_local(test, &import.alias)
                && contains_name_reference(body, &import.alias);
        }
        imported_module_matches_owner(import, owner)
            && contains_member_reference(body, &import.alias, symbol)
    })
}

/// Module-owner reference: the test uses a local bound by `from <owner module>
/// import X` or by an import of the owner module itself.
fn test_references_owner_module(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.imports.iter().any(|import| {
        (import_source_module_matches_owner(import, owner)
            || imported_module_matches_owner(import, owner))
            && !test_binds_local(test, &import.alias)
            && contains_name_reference(&test.body_text, &import.alias)
    })
}

/// Whether the test binds its own local named `name`: a parameter (pytest
/// fixture), a direct assignment, a walrus (`name :=`), a `for name in` loop
/// target, an `as name` target (`with`, `except`, in-body `import`), or a
/// nested `def`/`class`. Such a local shadows the imported owner, so bare uses
/// of `name` are not owner references.
fn test_binds_local(test: &PythonTest, name: &str) -> bool {
    test.fixtures.iter().any(|fixture| fixture == name)
        || assignment_count(&test.body_text, name) > 0
        || walrus_binds(&test.body_text, name)
        || ["def ", "class ", "for ", "as "]
            .into_iter()
            .any(|keyword| {
                let needle = format!("{keyword}{name}");
                test.body_text.match_indices(&needle).any(|(idx, _)| {
                    let end = idx + needle.len();
                    python_callee_start_has_boundary(&test.body_text, idx)
                        && !next_char_is_identifier(&test.body_text, end)
                        && !python_text_hides_code(&test.body_text, idx)
                })
            })
}

/// `name :=` with identifier boundaries, outside comments and strings.
fn walrus_binds(body_text: &str, name: &str) -> bool {
    body_text.match_indices(name).any(|(idx, _)| {
        let end = idx + name.len();
        python_callee_start_has_boundary(body_text, idx)
            && !next_char_is_identifier(body_text, end)
            && body_text[end..]
                .trim_start_matches([' ', '\t'])
                .starts_with(":=")
            && !python_text_hides_code(body_text, idx)
    })
}

/// Bare (possibly dotted, for a module alias) name reference: identifier
/// boundaries on both sides, not an attribute of another receiver, not a
/// keyword-argument or assignment target (`name=`, `name = `), and not inside a
/// comment or string literal.
fn contains_name_reference(body_text: &str, name: &str) -> bool {
    if !is_dotted_python_identifier(name) {
        return false;
    }
    body_text.match_indices(name).any(|(idx, _)| {
        let end = idx + name.len();
        python_callee_start_has_boundary(body_text, idx)
            && !next_char_is_identifier(body_text, end)
            && !is_binding_target(body_text, end)
            && !python_text_hides_code(body_text, idx)
    })
}

/// `receiver.member` with identifier boundaries, outside comments and strings.
fn contains_member_reference(body_text: &str, receiver: &str, member: &str) -> bool {
    if !is_dotted_python_identifier(receiver) || !is_dotted_python_identifier(member) {
        return false;
    }
    let needle = format!("{receiver}.{member}");
    body_text.match_indices(&needle).any(|(idx, _)| {
        python_callee_start_has_boundary(body_text, idx)
            && !next_char_is_identifier(body_text, idx + needle.len())
            && !python_text_hides_code(body_text, idx)
    })
}

/// `.attr` on any receiver, outside comments and strings.
fn contains_attribute_reference(body_text: &str, attr: &str) -> bool {
    if !is_dotted_python_identifier(attr) {
        return false;
    }
    let needle = format!(".{attr}");
    body_text.match_indices(&needle).any(|(idx, _)| {
        !next_char_is_identifier(body_text, idx + needle.len())
            && !python_text_hides_code(body_text, idx)
    })
}

fn next_char_is_identifier(text: &str, idx: usize) -> bool {
    text[idx..]
        .chars()
        .next()
        .is_some_and(is_python_identifier_char)
}

/// `name=` / `name = ` (not `==`): a keyword argument or an assignment target.
fn is_binding_target(text: &str, end: usize) -> bool {
    let rest = text[end..].trim_start_matches([' ', '\t']);
    rest.starts_with('=') && !rest.starts_with("==")
}

fn is_dotted_python_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with(|ch: char| ch.is_ascii_digit())
                && segment.chars().all(is_python_identifier_char)
        })
}

/// Whether `idx` sits in a comment or string: a `#` or an open quote earlier on
/// the same line, or an open triple-quoted string (docstring) from an earlier
/// line.
fn python_text_hides_code(text: &str, idx: usize) -> bool {
    python_prefix_hides_code(line_prefix_before(text, idx))
        || inside_triple_quoted_string(text, idx)
}

fn inside_triple_quoted_string(text: &str, idx: usize) -> bool {
    let mut open: Option<&str> = None;
    let mut cursor = 0;
    while cursor < idx {
        let rest = &text[cursor..];
        match open {
            Some(delimiter) if rest.starts_with(delimiter) => {
                open = None;
                cursor += delimiter.len();
            }
            None if rest.starts_with("\"\"\"") => {
                open = Some("\"\"\"");
                cursor += 3;
            }
            None if rest.starts_with("'''") => {
                open = Some("'''");
                cursor += 3;
            }
            _ => cursor += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    open.is_some()
}

pub(super) fn same_stem_related(test: &PythonTest, owner: &PythonOwner) -> bool {
    let Some(owner_stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(test_stem) = test.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    normalize_test_stem(test_stem) == owner_stem
}

pub(super) fn normalize_test_stem(stem: &str) -> &str {
    stem.strip_prefix("test_")
        .or_else(|| stem.strip_suffix("_test"))
        .unwrap_or(stem)
}

fn test_name_similar_to_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    let test_key = normalize_similarity_key(&test.name);
    owner_similarity_keys(owner)
        .into_iter()
        .any(|key| similarity_key_contains(&test_key, &key))
}

fn fixture_name_related_to_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.fixtures.iter().any(|fixture| {
        let fixture_key = normalize_similarity_key(fixture);
        owner_similarity_keys(owner)
            .into_iter()
            .any(|key| similarity_key_contains(&fixture_key, &key))
    })
}

pub(super) fn owner_similarity_keys(owner: &PythonOwner) -> Vec<String> {
    let mut keys = Vec::new();
    if !owner.is_module_owner() {
        keys.push(normalize_similarity_key(&owner.name));
        if owner.qualified_name != owner.name {
            keys.push(normalize_similarity_key(
                &owner.qualified_name.replace('.', "_"),
            ));
        }
    }
    if let Some(stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) {
        keys.push(normalize_similarity_key(stem));
    }
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|key| key.len() >= 4).collect()
}

pub(super) fn normalize_similarity_key(text: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = true;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

pub(super) fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    if haystack.is_empty() || needle.is_empty() {
        return false;
    }
    haystack == needle
        || haystack
            .strip_prefix(needle)
            .is_some_and(|tail| tail.starts_with('_'))
        || haystack
            .strip_suffix(needle)
            .is_some_and(|head| head.ends_with('_'))
        || haystack.contains(&format!("_{needle}_"))
}
