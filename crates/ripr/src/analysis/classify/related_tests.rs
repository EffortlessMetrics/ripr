use super::super::rust_index::{
    FunctionSummary, RustIndex, TestSummary, extract_identifier_tokens,
};
use crate::domain::Probe;
use std::path::Path;

/// Minimum token length for the `assertions_reference_owner` signal.
/// Tokens shorter than this threshold are too common to safely assert ownership.
const ASSERTION_TOKEN_MIN_LEN: usize = 5;

pub(in crate::analysis) fn find_related_tests<'a>(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    index: &'a RustIndex,
) -> Vec<&'a TestSummary> {
    let mut related = Vec::new();
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let file_name = probe
        .location
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let owner_package_prefix = owner_fn.and_then(|owner| package_prefix(&owner.file));

    // For module-level struct/field probes (owner_fn is None) derive the package
    // prefix from the probe's source file so cross-crate spurious matches are
    // still filtered out.
    let struct_package_prefix = if owner_fn.is_none() {
        package_prefix(&probe.location.file)
    } else {
        None
    };

    // Probe tokens that are long enough to assert ownership via assertion text.
    let long_probe_tokens: Vec<&str> = probe_tokens
        .iter()
        .filter(|t| t.len() >= ASSERTION_TOKEN_MIN_LEN)
        .map(String::as_str)
        .collect();

    for test in &index.tests {
        if let Some(prefix) = &owner_package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        // Apply the same package-prefix guard for the owner_fn=None path.
        if let Some(prefix) = &struct_package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || test.body.contains(owner_name));

        // Signal: a test's assertion observed_tokens intersect the probe's
        // long-enough identifier tokens. Only fires when the probe has no named
        // function owner (i.e. it is a module-level struct/field), so it cannot
        // override the `calls_owner` path for function-bodied probes.
        let assertions_reference_owner = owner_fn.is_none()
            && !long_probe_tokens.is_empty()
            && test.assertions.iter().any(|oracle| {
                oracle.observed_tokens.iter().any(|obs_tok| {
                    obs_tok.len() >= ASSERTION_TOKEN_MIN_LEN
                        && long_probe_tokens.contains(&obs_tok.as_str())
                })
            });

        let test_name = test.name.to_ascii_lowercase();
        let owner_name = owner_name.to_ascii_lowercase();
        let same_file_or_named = normalize_path(&test.file).contains(file_name)
            || (!owner_name.is_empty() && test_name.contains(&owner_name))
            || probe_tokens
                .iter()
                .any(|token| token.len() > 2 && test_name.contains(&token.to_ascii_lowercase()));

        if calls_owner || assertions_reference_owner || same_file_or_named {
            related.push(test);
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.file.cmp(&b.file)));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{CallFact, OracleFact, extract_identifier_tokens};
    use crate::domain::{
        DeltaKind, OracleKind, OracleStrength, ProbeFamily, ProbeId, SourceLocation, SymbolId,
    };
    use std::path::PathBuf;

    #[test]
    fn given_owner_function_when_tests_share_name_across_packages_then_filters_to_package() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            tests: vec![
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "crate_a_score_test");
    }

    #[test]
    fn given_same_named_tests_when_finding_related_then_orders_by_file_path() {
        let owner = function("src/lib.rs", "score");
        let index = RustIndex {
            tests: vec![
                test("tests/z_case.rs", "score_shared", "score(3)"),
                test("tests/a_case.rs", "score_shared", "score(1)"),
            ],
            ..RustIndex::default()
        };
        let probe = probe("src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index);

        assert_eq!(related.len(), 2);
        assert_eq!(related[0].file, PathBuf::from("tests/a_case.rs"));
        assert_eq!(related[1].file, PathBuf::from("tests/z_case.rs"));
    }

    #[test]
    fn given_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related() {
        let owner = function("src/lib.rs", "tax_total");
        let index = RustIndex {
            tests: vec![test(
                "tests/tax.rs",
                "vat_boundary_is_checked_by_macro",
                "assert_eq!(macro_tax_case!(100), 120);",
            )],
            ..RustIndex::default()
        };
        let probe = probe("src/lib.rs", "vat >= threshold");

        let related = find_related_tests(&probe, Some(&owner), &index);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "vat_boundary_is_checked_by_macro");
    }

    #[test]
    fn given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers() {
        assert_eq!(
            package_prefix(Path::new("crates/foo/src/support/src/lib.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/foo/tests/support/tests/cases.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("vendor/foo/src/support/src/lib.rs")).as_deref(),
            Some("vendor/foo/src/support/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/ripr/examples/sample/src/lib.rs")).as_deref(),
            Some("crates/ripr/examples/sample/")
        );
    }

    #[test]
    fn given_non_workspace_paths_when_extracting_package_prefix_then_returns_none() {
        assert_eq!(package_prefix(Path::new("src/lib.rs")), None);
        assert_eq!(package_prefix(Path::new("tests/basic.rs")), None);
        assert_eq!(package_prefix(Path::new("README.md")), None);
    }

    #[test]
    fn given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form() {
        let normalized = normalize_path(Path::new("./crates\\ripr\\src\\lib.rs"));
        assert_eq!(normalized, "crates/ripr/src/lib.rs");
    }

    fn function(file: &str, name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        }
    }

    fn test(file: &str, name: &str, body: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body: body.to_string(),
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: body.to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn probe(file: &str, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new(file, 2, 1),
            owner: Some(SymbolId(format!("{file}::owner"))),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    /// A probe with no function owner — models a module-level struct field.
    fn struct_field_probe(file: &str, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:struct-field".to_string()),
            location: SourceLocation::new(file, 46, 1),
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Value,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: assertion.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(assertion),
        }
    }

    fn test_with_assertions(
        file: &str,
        name: &str,
        body: &str,
        assertions: Vec<OracleFact>,
    ) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 10,
            body: body.to_string(),
            calls: Vec::new(),
            assertions,
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    // --- #1052 repro ---
    // A module-level struct-field probe (owner_fn=None) whose changed expression
    // token (`open_in`) appears in the assertion observed_tokens of an exact-value
    // oracle test.  The oracle test MUST be selected as related.
    #[test]
    fn given_struct_field_probe_when_assertion_references_field_token_then_oracle_test_is_related()
    {
        let assertion_text = "assert_eq!(lane.open_in, OpenIn::Browser);";
        let oracle_test = test_with_assertions(
            "crates/ripr/tests/repo_lane.rs",
            "repo_lane_deserializes_fields_correctly",
            &format!("let lane = RepoLane {{ open_in: OpenIn::Browser, .. }}; {assertion_text}"),
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![oracle_test],
            ..RustIndex::default()
        };
        // Struct field probe: expression contains `open_in` (7 chars, >= 5)
        let probe = struct_field_probe("crates/ripr/src/config.rs", "open_in");

        let related = find_related_tests(&probe, None, &index);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "repo_lane_deserializes_fields_correctly");
    }

    // --- #1054 repro ---
    // Two sibling struct-field probes (`open_in` and `open_cap`) are both
    // asserted by the SAME exact-value oracle test.  Both MUST select that test
    // consistently (no asymmetry).
    #[test]
    fn given_sibling_struct_field_probes_when_both_asserted_then_both_select_same_oracle_test() {
        let assertion_open_in = "assert_eq!(lane.open_in, OpenIn::Browser);";
        let assertion_open_cap = "assert_eq!(lane.open_cap, 8);";
        let body =
            format!("let lane = RepoLane::default(); {assertion_open_in} {assertion_open_cap}");
        let oracle_test = test_with_assertions(
            "crates/ripr/tests/repo_lane.rs",
            "repo_lane_fields_have_expected_defaults",
            &body,
            vec![
                oracle_fact(
                    assertion_open_in,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                ),
                oracle_fact(
                    assertion_open_cap,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                ),
            ],
        );
        let index = RustIndex {
            tests: vec![oracle_test],
            ..RustIndex::default()
        };

        let probe_open_in = struct_field_probe("crates/ripr/src/config.rs", "open_in");
        let probe_open_cap = struct_field_probe("crates/ripr/src/config.rs", "open_cap");

        let related_in = find_related_tests(&probe_open_in, None, &index);
        let related_cap = find_related_tests(&probe_open_cap, None, &index);

        assert_eq!(
            related_in.len(),
            1,
            "open_in probe must find the oracle test"
        );
        assert_eq!(
            related_cap.len(),
            1,
            "open_cap probe must find the oracle test"
        );
        assert_eq!(
            related_in[0].name, related_cap[0].name,
            "sibling fields must select the same oracle test"
        );
    }

    // --- Anti-over-association: short 4-char token rejected ---
    // A struct-field probe whose only shared token with a test assertion is a
    // common 4-char name (`port`) must NOT select that test via the
    // assertions_reference_owner signal — the >= 5 threshold rejects it.
    // The test and probe live in different-stem files and the test name does
    // not contain the probe token, so same_file_or_named also does not fire.
    #[test]
    fn given_struct_field_probe_when_shared_token_is_4_chars_then_assertions_signal_does_not_fire()
    {
        // Test file stem "rendering" != probe file stem "scheduler".
        // Test name "rendering_is_initialized" does not contain "port".
        let assertion_text = "assert_eq!(srv.port, 8080);";
        let unrelated_test = test_with_assertions(
            "crates/ripr/tests/rendering.rs",
            "rendering_is_initialized",
            assertion_text,
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![unrelated_test],
            ..RustIndex::default()
        };
        // Probe expression is `port` (4 chars) — below the >= 5 threshold.
        let probe = struct_field_probe("crates/ripr/src/scheduler.rs", "port");

        let related = find_related_tests(&probe, None, &index);

        assert!(
            related.is_empty(),
            "a 4-char shared token must not trigger assertions_reference_owner"
        );
    }

    // --- Anti-over-association: owner_fn=Some blocks the signal ---
    // When owner_fn is Some, assertions_reference_owner must NOT fire even if
    // probe tokens appear in assertion observed_tokens.  The calls_owner / name
    // proximity gates govern function-bodied probes.
    #[test]
    fn given_function_owner_probe_when_assertion_references_probe_token_then_signal_does_not_fire()
    {
        let owner = function("src/lib.rs", "discounted_total");
        let assertion_text =
            "assert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");";
        let unrelated_test = test_with_assertions(
            "tests/tokens.rs",
            "token_label_includes_token_text",
            assertion_text,
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![unrelated_test],
            ..RustIndex::default()
        };
        // Probe expression contains `discount_threshold` (>= 5 chars) which is
        // also in the assertion observed_tokens — but owner_fn is Some, so the
        // assertions_reference_owner signal must NOT fire.
        let probe = probe("src/lib.rs", "amount >= discount_threshold");

        let related = find_related_tests(&probe, Some(&owner), &index);

        assert!(
            related.is_empty(),
            "assertions_reference_owner must not fire when owner_fn is Some"
        );
    }
}
