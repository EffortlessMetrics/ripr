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
receiver_fingerprint = "Path::new(env!(\"CARGO_MANIFEST_DIR\")) .parent() .and_then(Path::parent)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now() .duration_since(UNIX_EPOCH)"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "SystemTime::now() .duration_since(UNIX_EPOCH)"

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
receiver_fingerprint = "fs::write( root.join(\"src/lib.rs\"), r#\" pub fn price(amount: i32, threshold: i32) -> i32 { if amount >= threshold { amount - 10 } else { amount } } \"#, )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write( root.join(\"tests/pricing.rs\"), r#\" #[test] fn premium_customer_gets_discount() { let total = x::price(10000, 100); assert!(total > 0); } \"#, )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "fs::write( root.join(\"diff.patch\"), r#\"diff --git a/src/lib.rs b/src/lib.rs index 0000000..1111111 100644 --- a/src/lib.rs +++ b/src/lib.rs @@ -1,3 +1,3 @@ pub fn price(amount: i32, threshold: i32) -> i32 { + if amount >= threshold { amount - 10 } else { amount } } \"#, )"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions { root: root.clone(), base: None, diff_file: Some(root.join(\"diff.patch\")), mode: AnalysisMode::Draft, include_unchanged_tests: true, })"

[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
receiver_fingerprint = "run_analysis(&AnalysisOptions { root: root.clone(), base: None, diff_file: Some(root.join(\"diff.patch\")), mode: AnalysisMode::Instant, include_unchanged_tests: true, })"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

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
container = "closure_407398"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "collect_semantic_panic_findings_integration"
callee = "expect"
receiver_fingerprint = "collect_semantic_panic_findings(&root, &patterns)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_408609"
callee = "unwrap"
receiver_fingerprint = "collect_panic_findings(root, &patterns)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_536396"
callee = "unwrap"
receiver_fingerprint = "result"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
receiver_fingerprint = "fs::create_dir_all(&dir)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::set_current_dir(old)"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_403284"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_404262"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_404828"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_405408"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_406066"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Semantic selector test infrastructure"

[allow.selector]
kind = "method_call"
container = "closure_406712"
callee = "unwrap"
receiver_fingerprint = "root.join(\"allowlist.toml\").to_str()"
```

## Analysis Notes

- **Total Entries Proposed**: 25
- **Kind Distribution**:
  - `macro_call`: 1
  - `method_call`: 24
