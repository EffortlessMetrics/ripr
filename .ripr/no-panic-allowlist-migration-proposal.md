# No-Panic Allowlist Migration Report

This report proposes v0.2 schema entries with semantic selectors.

## Proposed TOML (Schema v0.2)

```toml
schema_version = "0.2"

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
family = "panic_macro"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates unreachable status value detection"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

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
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates panic family pattern matching"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates process policy panic detection"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

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
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates panic family pattern matching"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"

[[allow]]
path = "xtask/src/main.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Test assertion: validates process policy panic detection"

[allow.selector]
kind = "macro_call"
container = "check_report_aggregates_violations_and_status"
callee = "panic!"
```

## Analysis Notes

- **Total Entries Proposed**: 9
- **Kind Distribution**:
  - `macro_call`: 9
