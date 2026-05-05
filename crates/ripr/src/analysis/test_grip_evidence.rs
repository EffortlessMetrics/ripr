//! Test-grip evidence per RIPR-SPEC-0005, v1.
//!
//! For each `RepoSeam`, build per-stage evidence (reach / activate /
//! propagate / observe / discriminate) using the existing `RustIndex`
//! facts. This is **not** classification: the output is a per-stage
//! evidence record, not a `SeamGripClass`. The classification PR
//! (`analysis/repo-ripr-classification-v1`) consumes these records.
//!
//! Determinism: `evidence_for_seams` sorts by `seam_id`. Within each
//! evidence record, `related_tests` are sorted by `(name, file)` and
//! deduped.

use super::rust_index::{
    self, FunctionSummary, OracleFact, RustIndex, TestSummary, extract_identifier_tokens,
};
use super::seams::{ExpectedSink, RepoSeam, SeamId, SeamKind};
use crate::domain::{
    Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence, StageState,
    ValueContext, ValueFact,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Why this test is related to the seam. v1: a single highest-priority
/// reason per test (no multi-reason public shape). Priority is pinned
/// by `RelationReason::priority` and exercised by ranking tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RelationReason {
    DirectOwnerCall,
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
            Self::AssertionTargetAffinity => 1,
            Self::SameTestFile => 2,
            Self::SameModule => 3,
            Self::OwnerNamedTest => 4,
            Self::ImportPathAffinity => 5,
            Self::FixtureOwnerAffinity => 6,
        }
    }

    fn confidence(self) -> RelationConfidence {
        match self {
            Self::DirectOwnerCall | Self::AssertionTargetAffinity => RelationConfidence::High,
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
    let mut out: Vec<TestGripEvidence> = seams
        .iter()
        .map(|seam| evidence_for_seam(seam, index))
        .collect();
    out.sort_by(|a, b| a.seam_id.as_str().cmp(b.seam_id.as_str()));
    out
}

/// Build evidence for a single seam.
pub(crate) fn evidence_for_seam(seam: &RepoSeam, index: &RustIndex) -> TestGripEvidence {
    let related_with_reason = find_related_tests(seam, index);
    let owner_fn = find_owner_function(seam, index);

    let related: Vec<&TestSummary> = related_with_reason.iter().map(|(t, _)| *t).collect();

    let reach = reach_evidence(seam, &related);
    let (activate, observed_values, missing_discriminators) =
        activate_evidence(seam, &related, owner_fn);
    let propagate = propagate_evidence(seam, &related);
    let observe = observe_evidence(&related);
    let discriminate = discriminate_evidence(seam, &related);

    let mut related_tests: Vec<RelatedTestGrip> = related_with_reason
        .iter()
        .map(|(test, reason)| related_test_grip(seam, test, *reason))
        .collect();
    // Ranked order: confidence (high first) → reason priority → file →
    // name → line. Replaces the previous (name, file, line) sort. The
    // dedup invariant is unchanged — `find_related_tests` already
    // deduped by (name, file, start_line), so this is a stable ranking
    // re-sort, not a uniqueness step.
    related_tests.sort_by(|a, b| {
        a.relation_confidence
            .rank()
            .cmp(&b.relation_confidence.rank())
            .then(
                a.relation_reason
                    .priority()
                    .cmp(&b.relation_reason.priority()),
            )
            .then(a.file.cmp(&b.file))
            .then(a.test_name.cmp(&b.test_name))
            .then(a.line.cmp(&b.line))
    });

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

/// Walk `index.tests` and return tests that plausibly relate to `seam`,
/// each tagged with the single highest-priority `RelationReason` it
/// satisfies. The two-step "match then rank" replaces the old binary
/// `calls_owner || same_file_or_named` check from earlier campaigns.
///
/// Detection per reason — strict ordering: the first reason that fires
/// wins, so e.g. a test that both `calls owner` and `is in same file`
/// carries `direct_owner_call`, never `same_test_file`.
fn find_related_tests<'a>(
    seam: &RepoSeam,
    index: &'a RustIndex,
) -> Vec<(&'a TestSummary, RelationReason)> {
    let owner_fn = find_owner_function(seam, index);
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let owner_name_lower = owner_name.to_ascii_lowercase();
    let owner_file = owner_fn.map(|f| f.file.as_path());
    let owner_file_stem = owner_file
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let owner_module_path = owner_file.and_then(module_path_for);
    let prefix = owner_fn.and_then(|f| package_prefix(&f.file));

    // Tokens from `RequiredDiscriminator` and `ExpectedSink` for
    // `assertion_target_affinity`. Filtered through
    // `extract_identifier_tokens`, so common stop-words and short
    // tokens are already excluded — the residual set is what a test
    // assertion would have to mention to count.
    let discriminator_tokens = required_discriminator_tokens(seam);
    let sink_tokens = extract_identifier_tokens(seam.expected_sink().as_str());
    let target_tokens: Vec<String> = discriminator_tokens
        .into_iter()
        .chain(sink_tokens)
        .collect();

    let mut related: Vec<(&'a TestSummary, RelationReason)> = Vec::new();
    let mut seen: std::collections::HashSet<(String, std::path::PathBuf, usize)> =
        std::collections::HashSet::new();

    for test in &index.tests {
        let test_path = normalize_path(&test.file);
        if let Some(prefix) = &prefix
            && !test_path.starts_with(prefix)
        {
            continue;
        }

        let test_module_path = module_path_for(&test.file);
        let test_name_lower = test.name.to_ascii_lowercase();

        // Reason resolution — strict priority order.
        let reason =
            if !owner_name.is_empty() && test.calls.iter().any(|call| call.name == owner_name) {
                // `direct_owner_call`: test calls the owner directly. The
                // call walker captures the bare callee name, which covers
                // both `owner(...)` and qualified forms like
                // `module::owner(...)` — `CallFact.name` is the unqualified
                // tail.
                Some(RelationReason::DirectOwnerCall)
            } else if !target_tokens.is_empty() && assertion_targets_seam(test, &target_tokens) {
                // `assertion_target_affinity`: at least one assertion in
                // the test mentions a token from the seam's required
                // discriminator or expected sink. Token-aware (full
                // identifier match), so `discount_threshold` does not
                // match `discount_threshold_factor` or random substring.
                Some(RelationReason::AssertionTargetAffinity)
            } else if !owner_file_stem.is_empty() && same_test_file(&test.file, owner_file_stem) {
                // `same_test_file`: physical/virtual sibling — a `tests/`
                // file with the same stem as the owner's source, an inline
                // `#[cfg(test)] mod tests` (test.file == owner.file), or a
                // `*_test.rs` / `*_tests.rs` peer.
                Some(RelationReason::SameTestFile)
            } else if let (Some(owner_mod), Some(test_mod)) =
                (owner_module_path.as_deref(), test_module_path.as_deref())
                && same_module(owner_mod, test_mod)
            {
                // `same_module`: shares the owner's module path beyond
                // just the file stem — e.g., `src/auth/login.rs` ↔
                // `tests/auth/integration.rs`.
                Some(RelationReason::SameModule)
            } else if !owner_name_lower.is_empty() && test_name_lower.contains(&owner_name_lower) {
                // `owner_named_test`: test name embeds the owner name.
                // Conservative: substring on the test name (lowercase),
                // which does not have the false-positive risk of body
                // substring.
                Some(RelationReason::OwnerNamedTest)
            } else if !owner_name.is_empty() && test_imports_owner(test, owner_name) {
                // `import_path_affinity`: test body mentions the owner
                // via an explicit qualified-path (`module::owner`) or
                // inline `use ... owner` shape, without a direct call.
                // Captures the "test imports it but does not invoke
                // it" pattern common in higher-level integration
                // tests. Detection tightened per #310 review — see
                // `test_imports_owner` for the accepted/rejected
                // shapes.
                Some(RelationReason::ImportPathAffinity)
            } else if test_uses_owner_fixture(test, owner_file, index) {
                // `fixture_owner_affinity`: test calls a non-test fn that
                // lives in the owner's source file and whose name follows
                // a fixture / builder convention. Narrow on purpose — the
                // user tightened this to "explicit fixture relationship
                // only", not "any helper call".
                Some(RelationReason::FixtureOwnerAffinity)
            } else {
                None
            };

        let Some(reason) = reason else { continue };
        let key = (test.name.clone(), test.file.clone(), test.start_line);
        if seen.insert(key) {
            related.push((test, reason));
        }
    }
    related
}

/// Tokens drawn from a `RepoSeam`'s `RequiredDiscriminator`. Filtered
/// through `extract_identifier_tokens` so common short words and
/// stop-tokens are already excluded.
fn required_discriminator_tokens(seam: &RepoSeam) -> Vec<String> {
    use super::seams::RequiredDiscriminator;
    let text = match seam.required_discriminator() {
        RequiredDiscriminator::BoundaryValue { description }
        | RequiredDiscriminator::ReturnValue { description } => description.as_str(),
        RequiredDiscriminator::ErrorVariant { variant } => variant.as_str(),
        RequiredDiscriminator::FieldValue { field } => field.as_str(),
        RequiredDiscriminator::Effect { sink } => sink.as_str(),
        RequiredDiscriminator::MatchArmTaken { arm } => arm.as_str(),
        RequiredDiscriminator::CallSite { target } => target.as_str(),
    };
    extract_identifier_tokens(text)
}

/// Token-aware: does any assertion text in `test` contain at least one
/// of `tokens` as a whole identifier? Substring match would let
/// `discount` accidentally match `discount_threshold`; we want exact
/// identifier hits.
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
fn test_imports_owner(test: &TestSummary, owner_name: &str) -> bool {
    if owner_name.is_empty() {
        return false;
    }
    let qualified = format!("::{owner_name}");
    if test.body.contains(&qualified) {
        return true;
    }
    for line in test.body.lines() {
        if line.trim_start().starts_with("use ")
            && extract_identifier_tokens(line)
                .iter()
                .any(|t| t == owner_name)
        {
            return true;
        }
    }
    false
}

/// Test calls a non-test function that lives in the owner's source
/// file and whose name follows a fixture / builder convention. Narrow
/// per the campaign decision: explicit fixture relationship only, not
/// "any helper call".
fn test_uses_owner_fixture(
    test: &TestSummary,
    owner_file: Option<&Path>,
    index: &RustIndex,
) -> bool {
    let Some(owner_file) = owner_file else {
        return false;
    };
    for call in &test.calls {
        let Some(target) = index
            .functions
            .iter()
            .find(|f| f.name == call.name && f.file == owner_file && !f.is_test)
        else {
            continue;
        };
        if is_fixture_named(&target.name) || target.body.contains("#[fixture]") {
            return true;
        }
    }
    false
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
    related: &[&TestSummary],
    owner_fn: Option<&FunctionSummary>,
) -> (StageEvidence, Vec<ValueFact>, Vec<MissingDiscriminatorFact>) {
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let mut observed: Vec<ValueFact> = Vec::new();

    if !owner_name.is_empty() {
        for test in related {
            for call in &test.calls {
                if call.name != owner_name {
                    continue;
                }
                let Some(args) = call_arguments(&call.text, owner_name) else {
                    continue;
                };
                for arg in args {
                    for value in scalar_values(&arg) {
                        observed.push(ValueFact {
                            line: call.line,
                            text: call.text.clone(),
                            value,
                            context: ValueContext::FunctionArgument,
                        });
                    }
                }
            }
        }
    }
    sort_value_facts(&mut observed);

    let missing = missing_discriminators_for(seam, &observed);

    let state = if related.is_empty() {
        StageState::No
    } else if !observed.is_empty() {
        StageState::Yes
    } else {
        // Reach exists but no concrete value seen — most often a helper
        // call that hides the activation, or an integration test.
        StageState::Unknown
    };
    let stage = StageEvidence::new(
        state,
        if observed.is_empty() {
            Confidence::Low
        } else {
            Confidence::Medium
        },
        if observed.is_empty() {
            format!(
                "No concrete activation values observed for seam `{}`",
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        } else {
            format!(
                "Observed {} concrete activation value(s) for seam `{}`",
                observed.len(),
                seam.expression()
                    .lines()
                    .next()
                    .unwrap_or(seam.expression())
            )
        },
    );
    (stage, observed, missing)
}

fn missing_discriminators_for(
    seam: &RepoSeam,
    observed: &[ValueFact],
) -> Vec<MissingDiscriminatorFact> {
    match seam.kind() {
        SeamKind::PredicateBoundary => {
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
    let needle = format!("{callee}(");
    let start = text.find(&needle)?;
    let after = &text[start + needle.len()..];
    let close = after.rfind(')')?;
    let inside = &after[..close];
    Some(split_top_level_commas(inside))
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
        if first.relation_reason != RelationReason::DirectOwnerCall {
            return Err(format!(
                "direct owner call must outrank same-file affinity; got grips {:?}",
                evidence
                    .related_tests
                    .iter()
                    .map(|g| (g.test_name.clone(), g.relation_reason))
                    .collect::<Vec<_>>()
            ));
        }
        if first.relation_confidence != RelationConfidence::High {
            return Err(format!(
                "expected High confidence for direct owner call, got {:?}",
                first.relation_confidence
            ));
        }
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
        if grip.relation_reason != RelationReason::OwnerNamedTest {
            return Err(format!(
                "expected OwnerNamedTest, got {:?}",
                grip.relation_reason
            ));
        }
        if grip.relation_confidence != RelationConfidence::Medium {
            return Err(format!(
                "expected Medium confidence, got {:?}",
                grip.relation_confidence
            ));
        }
        Ok(())
    }

    #[test]
    fn given_fixture_only_affinity_when_related_tests_are_ranked_then_confidence_is_low_or_opaque()
    -> Result<(), String> {
        // Test calls a fixture-named helper in the owner's source file
        // but never the owner itself, and the test name does not embed
        // the owner. Should classify as fixture_owner_affinity with
        // low confidence.
        let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn make_quote() -> i32 { 100 }\n";
        let test = (
            "tests/integration.rs",
            "#[test] fn quote_smoke() { let _ = make_quote(); assert!(true); }\n",
        );
        let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
        if grip.relation_reason != RelationReason::FixtureOwnerAffinity {
            return Err(format!(
                "expected FixtureOwnerAffinity, got {:?}",
                grip.relation_reason
            ));
        }
        if !matches!(
            grip.relation_confidence,
            RelationConfidence::Low | RelationConfidence::Opaque
        ) {
            return Err(format!(
                "expected Low or Opaque confidence, got {:?}",
                grip.relation_confidence
            ));
        }
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
            if grip.relation_reason == RelationReason::AssertionTargetAffinity {
                return Err(format!(
                    "substring hit (`discount_threshold_factor`) must not match \
                     assertion_target_affinity; got {grip:?}"
                ));
            }
        }
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
        if evidence.related_tests.len() < 2 {
            return Err(format!(
                "expected at least 2 related tests, got {}",
                evidence.related_tests.len()
            ));
        }
        // alpha.rs sorts before zeta.rs.
        if evidence.related_tests[0].file != Path::new("tests/alpha.rs") {
            return Err(format!(
                "expected first ranked test in tests/alpha.rs, got {}",
                evidence.related_tests[0].file.display()
            ));
        }
        if evidence.related_tests[1].file != Path::new("tests/zeta.rs") {
            return Err(format!(
                "expected second ranked test in tests/zeta.rs, got {}",
                evidence.related_tests[1].file.display()
            ));
        }
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
        if first.relation_reason != RelationReason::DirectOwnerCall {
            return Err(format!(
                "expected DirectOwnerCall first, got {:?}",
                first.relation_reason
            ));
        }
        if first.relation_confidence != RelationConfidence::High {
            return Err(format!(
                "expected High confidence first, got {:?}",
                first.relation_confidence
            ));
        }
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
        if grip.relation_reason != RelationReason::ImportPathAffinity {
            return Err(format!(
                "expected ImportPathAffinity, got {:?}",
                grip.relation_reason
            ));
        }
        if grip.relation_confidence != RelationConfidence::Medium {
            return Err(format!(
                "expected Medium confidence, got {:?}",
                grip.relation_confidence
            ));
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
            if grip.relation_reason == RelationReason::ImportPathAffinity {
                return Err(format!(
                    "token co-occurrence (`pricing` + `discounted_total` in body \
                     without `::` path syntax) must not match \
                     ImportPathAffinity; got {grip:?}"
                ));
            }
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
        if grip.relation_reason != RelationReason::SameModule {
            return Err(format!(
                "expected SameModule, got {:?}",
                grip.relation_reason
            ));
        }
        if grip.relation_confidence != RelationConfidence::Medium {
            return Err(format!(
                "expected Medium confidence, got {:?}",
                grip.relation_confidence
            ));
        }
        Ok(())
    }
}
