# No-Panic Allowlist Migration Report

This report proposes v0.2 schema entries with semantic selectors.

## Proposed Entries

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for creating temporary directories"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/src/analysis/mod.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/src/analysis/mod.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for writing test files"

[allow.selector]
kind = "method_call"
container = "analyzes_simple_predicate_gap"
callee = "unwrap"
```

### crates/ripr/tests/cli_smoke.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for running CLI tests"

[allow.selector]
kind = "method_call"
container = "run_ripr"
callee = "unwrap"
```

### crates/ripr/tests/cli_smoke.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "crates/ripr/tests/cli_smoke.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function for running CLI tests"

[allow.selector]
kind = "method_call"
container = "workspace_root"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: panic_macro | **Classification**: false_positive

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "false_positive"
explanation = "Pattern matching in function that converts panic patterns to family names, not an actual panic"

[allow.selector]
kind = "method_call"
container = "collect_panic_findings"
callee = "panic_family_from_pattern"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: creates temporary directories for test isolation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: creates temporary directories for test isolation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: creates parent directories for temporary files"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: writes test files to temporary directory"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: acquires lock for current working directory management"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: saves current working directory before test"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: changes to temporary directory for test isolation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test harness: restores original working directory after test"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: TOML parser validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: expect | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
```

### xtask/src/main.rs

**Family**: panic_macro | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"

[allow.selector]
kind = "method_call"
container = "parse_no_panic_allowlist_toml"
callee = "validate_panic_allow_entry"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test fixture: string literal in test harness code"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: expect | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test fixture: string literal in test harness code"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test fixture: string literal in test harness code"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: expect | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test fixture: string literal in test harness code"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: assertion on panic findings collection"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test assertion: panic findings validation"

[allow.selector]
kind = "method_call"
container = "temp_dir"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: expect | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test assertion: panic findings validation"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
```

### xtask/src/main.rs

**Family**: expect | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "expect"
classification = "test_only"
explanation = "Test harness: expects supported test intent kinds to parse successfully"

[allow.selector]
kind = "method_call"
container = "test_intent_kind_round_trips_supported_values"
callee = "expect"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: campaign manifest parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: campaign manifest parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: string parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: unwrap | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test function: string parsing validation"

[allow.selector]
kind = "method_call"
container = "campaign_manifest_parses_valid_file"
callee = "unwrap"
```

### xtask/src/main.rs

**Family**: panic_macro | **Classification**: test_only

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates unreachable status value detection"

[allow.selector]
kind = "method_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"
```

