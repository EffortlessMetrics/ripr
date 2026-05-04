# No-Panic Semantic Allowlist (v0.2)

## Overview

The no-panic allowlist has been refactored from line-number-based matching to AST-backed semantic selectors. This eliminates churn when code moves and improves handling of false positives.

**Problem (v0.1):**
- Allowlist entries keyed by `path + line + column + family`
- PR #235 had to churn line numbers after unrelated source movement
- False positives (string literals containing panic patterns) treated the same as runtime calls

**Solution (v0.2):**
- Allowlist entries keyed by `path + family + semantic_selector`
- Line and column now just hints for human review (`last_seen`)
- AST-based classification distinguishes:
  - Method calls: `unwrap()`, `expect()`
  - Macro calls: `panic!()`, `todo!()`, `unimplemented!()`, `unreachable!()`
  - String literal false positives
  - Test-only context validation

## Schema Changes

### v0.1 Format

```toml
schema_version = "0.1"

[[allow]]
path = "xtask/src/main.rs"
line = 11083
column = 47
family = "unwrap"
classification = "test_only"
explanation = "Test function: panic family pattern matching validation"
```

### v0.2 Format

```toml
schema_version = "0.2"

[[allow]]
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper function: matches std::env::current_dir().unwrap() in with_temp_cwd"

[allow.selector]
kind = "method_call"
container = "with_temp_cwd"
callee = "unwrap"
receiver_fingerprint = "std::env::current_dir()"

[allow.last_seen]
line = 11679
column = 40
```

## Selector Types

### Method Call Selector

```toml
[allow.selector]
kind = "method_call"
container = "optional_function_name"
callee = "unwrap"
receiver_fingerprint = "optional_receiver_pattern"
```

Matches `.unwrap()` and `.expect()` method calls, optionally filtered by:
- `container`: Enclosing function/test/mod
- `callee`: Method name
- `receiver_fingerprint`: The expression before the method call (e.g., in `std::env::current_dir().unwrap()`, the receiver is `std::env::current_dir()`)

### Free Function Call Selector

```toml
[allow.selector]
kind = "call"
container = "optional_function_name"
callee = "some_function"
```

Matches free function calls (e.g., `some_function()`), optionally filtered by:
- `container`: Enclosing function/test/mod
- `callee`: Function name

### Macro Call Selector

```toml
[allow.selector]
kind = "macro_call"
callee = "panic!"
```

Matches `panic!()`, `todo!()`, `unimplemented!()`, `unreachable!()` macro invocations.

### String Literal Selector

```toml
[allow.selector]
kind = "string_literal"
text_contains = "unwrap()"
```

Matches string literals containing panic-family text (e.g., analyzed Rust source fixtures).

## Migration

### Step 1: Generate Migration Report

```bash
cargo xtask no-panic-migration-report
```

Generates `.ripr/no-panic-allowlist-migration-proposal.md` with:
- Proposed v0.2 entries for all existing v0.1 entries
- Semantic selectors extracted via AST analysis
- Preserved explanations and classifications

### Step 2: Review Proposals

Review the migration report to ensure:
- Selectors accurately capture the intended panic calls
- Test context is correctly identified
- False positives (string literals) are properly classified

### Step 3: Convert Allowlist (in separate PR)

Replace `.ripr/no-panic-allowlist.toml` with v0.2 schema once migration report is approved.

### Step 4: Enable Semantic Matching

Update the checker to:
- Parse v0.2 selectors (falls back to v0.1 for backward compatibility during transition)
- Match using semantic selectors instead of exact line/column
- Report stale `last_seen` hints without failing

## Benefits

### Eliminates Churn

✓ Adding blank lines above a test function no longer breaks allowlist matching
✓ Line shifts in unrelated code don't require allowlist updates

### Better False Positive Handling

✓ String literals containing panic patterns classified separately
✓ Can distinguish `// panic!` comments from actual runtime calls
✓ Test-only validation ensures classification accuracy

### Improved Clarity

✓ Selector documents the "why" of each exception
✓ Container name makes intent explicit: `parse_json_with_unwrap`
✓ Semantic fingerprints are less fragile than line numbers

## Implementation Details

### AST Extraction

The `collect_semantic_panic_findings()` function:
1. Parses each Rust file with `ra_ap_syntax`
2. Traverses AST to find panic-family occurrences
3. Extracts metadata:
   - Nearest enclosing function/impl/mod
   - Callee name (method or macro)
   - Receiver expression fingerprint
   - Snippet fingerprint
   - Whether inside `#[cfg(test)]` or `#[test]`

### Selector Matching

For each allowlist entry with a selector:
1. Extract all semantic findings for that path + family
2. Check if any finding matches the selector criteria
3. If match found, occurrence is allowed
4. If no match found, it's a violation
5. If selector matches but line has drifted, warn (don't fail)

### Backward Compatibility

During migration period (v0.1 → v0.2):
- Both schemas supported simultaneously
- v0.1 entries still work (exact line/column matching)
- v0.2 entries use semantic selectors
- Can mix formats in same file during transition

## Testing

Key tests verify:
- Semantic selectors match across line shifts
- Selectors fail when ambiguous (multiple matches)
- Container/callee/receiver extraction works
- Test context detection validates `test_only` classification
- String literal false positives distinguished from runtime calls

## Future Enhancements

Deferred to v0.3 or later:
- Macro expansion support
- Cross-file symbol resolution
- Stable AST hashes (vs fingerprints)
- Automatic selector generation refinement
- Auto-fixing of stale `last_seen` hints

## See Also

- `docs/STATIC_EXPOSURE_MODEL.md` — Why v0.2 matches ripr's semantic anchor strategy
- `policy/no-panic-allowlist-v1.md` — Design rationale (this file is the spec)
- `.ripr/no-panic-allowlist.toml` — Current allowlist (in transition)
