# No-Panic Allowlist Migration Report

This report proposes v0.2 schema entries with semantic selectors.

## Proposed Entries

### xtask/src/main.rs

**Family**: panic_macro | **Classification**: false_positive

```toml
[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "false_positive"
explanation = "Pattern matching in function that converts panic patterns to family names, not an actual panic"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"
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
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"
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
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"
```

