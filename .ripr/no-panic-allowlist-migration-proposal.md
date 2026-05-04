# No-Panic Allowlist Migration Report

This report proposes v0.2 schema entries with semantic selectors.

## Proposed TOML (Schema v0.2)

```toml
schema_version = "0.2"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now()
            .duration_since(UNIX_EPOCH)"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(&dir)"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(root.join("src"))"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(root.join("tests"))"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(
            root.join("diff.patch"),
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn price(amount: i32, threshold: i32) -> i32 {
+    if amount >= threshold { amount - 10 } else { amount }
 }
"#,
        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Instant,
            include_unchanged_tests: true,
        })"

[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for running CLI tests"

[allow.selector]
kind = "method_call"
container = "run_ripr"
callee = "unwrap"
receiver_fingerprint = "Command::new(bin).args(args).output()"

[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for running CLI tests"

[allow.selector]
kind = "method_call"
container = "workspace_root"
callee = "unwrap"
receiver_fingerprint = "Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "false_positive"
explanation = "Pattern matching in function that converts panic patterns to family names, not an actual panic"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "false_positive"
explanation = "Comment text describing pattern matching behavior, not a runtime call"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now()
            .duration_since(UNIX_EPOCH)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: creates temporary directories for test isolation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(&dir)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: creates parent directories for temporary files"

[allow.selector]
kind = "method_call"
container = "write"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(parent)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: writes test files to temporary directory"

[allow.selector]
kind = "method_call"
container = "write"
callee = "unwrap"
receiver_fingerprint = "fs::write(path, text)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: acquires lock for current working directory management"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "CWD_LOCK.get_or_init(|| Mutex::new(())).lock()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: saves current working directory before test"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::current_dir()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: changes to temporary directory for test isolation"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::set_current_dir(&root)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: restores original working directory after test"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::set_current_dir(old)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_parses_valid_entries"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_parses_valid_entries"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_path"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_line"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_family"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_explanation"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_rejects_unknown_fields"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_rejects_duplicate_locations"
callee = "unwrap"
receiver_fingerprint = "root.join("allowlist.toml").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
receiver_fingerprint = "TestIntentKind::from_str(value)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test fixture: string literal in test harness code"

[allow.selector]
kind = "method_call"
container = "collect_panic_findings_finds_exact_locations"
callee = "unwrap"
receiver_fingerprint = "collect_panic_findings(root, &patterns)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: campaign manifest parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: campaign manifest parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_reports_violations_for_invalid_file"
callee = "unwrap"
receiver_fingerprint = "result"
```

## Analysis Notes

- **Total Entries Proposed**: 32
- **Kind Distribution**:
  - `macro_call`: 1
  - `method_call`: 31
