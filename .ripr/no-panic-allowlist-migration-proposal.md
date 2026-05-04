# No-Panic Allowlist Migration Report

This report proposes v0.2 schema entries with semantic selectors.

## Proposed TOML (Schema v0.2)

```toml
schema_version = "0.2"

[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "run_ripr"
callee = "unwrap"
receiver_fingerprint = "Command::new(bin).args(args).output()"

[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "workspace_root"
callee = "unwrap"
receiver_fingerprint = "Path::new(env!(\"CARGO_MANIFEST_DIR\"))\n        .parent()\n        .and_then(Path::parent)"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "false_positive"
explanation = "Static analysis detects panic-family string literal in match guard within panic_family_from_pattern(), not a runtime call"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now()\n            .duration_since(UNIX_EPOCH)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::set_current_dir(&root)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_parses_valid_entries"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_parses_valid_entries"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_path"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_line"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_requires_family"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_rejects_unknown_fields"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
receiver_fingerprint = "TestIntentKind::from_str(value)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml_rejects_duplicate_locations"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "collect_panic_findings_finds_exact_locations"
callee = "unwrap"
receiver_fingerprint = "collect_panic_findings(root, &patterns)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_reports_violations_for_invalid_file"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now()\n            .duration_since(UNIX_EPOCH)"

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
receiver_fingerprint = "fs::create_dir_all(root.join(\"src\"))"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(root.join(\"tests\"))"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(\n            root.join(\"src/lib.rs\"),\n            r#\"\npub fn price(amount: i32, threshold: i32) -> i32 {\n    if amount >= threshold { amount - 10 } else { amount }\n}\n\"#,\n        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(\n            root.join(\"tests/pricing.rs\"),\n            r#\"\n#[test]\nfn premium_customer_gets_discount() {\n    let total = x::price(10000, 100);\n    assert!(total > 0);\n}\n\"#,\n        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write(\n            root.join(\"diff.patch\"),\n            r#\"diff --git a/src/lib.rs b/src/lib.rs\nindex 0000000..1111111 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn price(amount: i32, threshold: i32) -> i32 {\n+    if amount >= threshold { amount - 10 } else { amount }\n }\n\"#,\n        )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions {\n            root: root.clone(),\n            base: None,\n            diff_file: Some(root.join(\"diff.patch\")),\n            mode: AnalysisMode::Draft,\n            include_unchanged_tests: true,\n        })"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions {\n            root: root.clone(),\n            base: None,\n            diff_file: Some(root.join(\"diff.patch\")),\n            mode: AnalysisMode::Instant,\n            include_unchanged_tests: true,\n        })"
```

## Analysis Notes

- **Total Entries Proposed**: 25
- **Kind Distribution**:
  - `macro_call`: 1
  - `method_call`: 24
