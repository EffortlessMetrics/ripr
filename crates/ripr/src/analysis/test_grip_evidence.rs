//! Voice B test-grip evidence per RIPR-SPEC-0005, v1.
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
use std::path::{Path, PathBuf};

/// Per-seam test-grip evidence record.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub(crate) struct RelatedTestGrip {
    pub(crate) test_name: String,
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) oracle_kind: OracleKind,
    pub(crate) oracle_strength: OracleStrength,
    pub(crate) evidence_summary: String,
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
    let related = find_related_tests(seam, index);
    let owner_fn = find_owner_function(seam, index);

    let reach = reach_evidence(seam, &related);
    let (activate, observed_values, missing_discriminators) =
        activate_evidence(seam, &related, owner_fn);
    let propagate = propagate_evidence(seam, &related);
    let observe = observe_evidence(&related);
    let discriminate = discriminate_evidence(seam, &related);

    let mut related_tests: Vec<RelatedTestGrip> = related
        .iter()
        .map(|test| related_test_grip(seam, test))
        .collect();
    related_tests.sort_by(|a, b| {
        a.test_name
            .cmp(&b.test_name)
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });
    related_tests
        .dedup_by(|a, b| a.test_name == b.test_name && a.file == b.file && a.line == b.line);

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

/// Walk index.tests and return tests that plausibly relate to `seam`.
/// Mirrors the matching idiom used by `analysis::classifier::find_related_tests`
/// but works directly from `RepoSeam` fields rather than `Probe`.
fn find_related_tests<'a>(seam: &RepoSeam, index: &'a RustIndex) -> Vec<&'a TestSummary> {
    let owner_fn = find_owner_function(seam, index);
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let owner_name_lower = owner_name.to_ascii_lowercase();
    let expression_tokens = extract_identifier_tokens(seam.expression());
    let file_stem = seam
        .file()
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let prefix = owner_fn.and_then(|f| package_prefix(&f.file));

    let mut related: Vec<&'a TestSummary> = Vec::new();
    for test in &index.tests {
        if let Some(prefix) = &prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || test.body.contains(owner_name));
        let test_name_lower = test.name.to_ascii_lowercase();
        let same_file_or_named = (!file_stem.is_empty()
            && normalize_path(&test.file).contains(file_stem))
            || (!owner_name_lower.is_empty() && test_name_lower.contains(&owner_name_lower))
            || expression_tokens.iter().any(|token| {
                token.len() > 2 && test_name_lower.contains(&token.to_ascii_lowercase())
            });
        if calls_owner || same_file_or_named {
            related.push(test);
        }
    }
    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.file.cmp(&b.file)));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
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
            let equality_seen = observed
                .iter()
                .any(|v| v.value.contains(boundary_token.as_str()));
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

fn related_test_grip(seam: &RepoSeam, test: &TestSummary) -> RelatedTestGrip {
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
    RelatedTestGrip {
        test_name: test.name.clone(),
        file: test.file.clone(),
        line: test.start_line,
        oracle_kind: kind,
        oracle_strength: strength,
        evidence_summary: summary,
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
}
