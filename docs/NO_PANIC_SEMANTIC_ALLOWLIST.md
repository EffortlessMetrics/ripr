# No-Panic Semantic Allowlist

The `cargo xtask check-no-panic-family` gate rejects panic-family call sites
(`unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`)
in production and test code unless each occurrence is listed in
`.ripr/no-panic-allowlist.toml` with a human-reviewed explanation.

This document defines the v0.2 schema for that allowlist as implemented by
#308 and #309.

## Identity

A v0.2 allowlist entry is identified by **path + family + selector**, not by
line or column number. Line and column are recorded as advisory locator hints
only (see [last_seen](#last_seen)).

When a code change moves an allowed call to a different line, the v0.2 entry
still matches because the selector describes the *structural* call site rather
than its position in the file.

## Schema version

```toml
schema_version = "0.2"
```

The file must begin with `schema_version = "0.2"`. The checker uses this to
activate the v0.2 parsing and matching path. Files without this header are
parsed in v0.1 (line/column) mode.

## Entry structure

```toml
[[allow]]
path = "src/some_file.rs"
family = "unwrap"
classification = "test_only"
explanation = "Human-readable reason this call site is allowed"

[allow.selector]
kind = "method_call"
container = "test_function_name"
callee = "unwrap"

[allow.last_seen]
line = 42
column = 17
```

### Required fields

| Field | Location | Description |
|---|---|---|
| `path` | `[[allow]]` | Repository-relative file path |
| `family` | `[[allow]]` | Panic family: `unwrap`, `expect`, `panic_macro`, `todo`, `unimplemented`, `unreachable` |
| `explanation` | `[[allow]]` | Human-readable reason for the exception |
| `kind` | `[allow.selector]` | Selector kind (see below) |

### Optional fields

| Field | Location | Description |
|---|---|---|
| `classification` | `[[allow]]` | Entry classification (e.g. `test_only`) |
| `container` | `[allow.selector]` | Enclosing function or method name |
| `callee` | `[allow.selector]` | Exact callee name |
| `receiver_fingerprint` | `[allow.selector]` | Receiver type or expression fingerprint |
| `text_contains` | `[allow.selector]` | Required for `string_literal` kind |
| `line` | `[allow.last_seen]` | Advisory: last known line number |
| `column` | `[allow.last_seen]` | Advisory: last known column number |

## Selector kinds

The checker supports four selector kinds.

### method_call

Matches a method invocation on a receiver, such as `x.unwrap()`.

```toml
[allow.selector]
kind = "method_call"
container = "build_output_path"
callee = "unwrap"
```

This matches `some_value.unwrap()` inside the function `build_output_path`.
If the function is renamed, the entry becomes stale and the checker reports it.

### macro_call

Matches a macro invocation by exact callee name, such as `panic!(...)` or
`todo!(...)`.

```toml
[allow.selector]
kind = "macro_call"
callee = "panic!"
```

The callee must include the trailing `!`.

### call

Matches a free-function or associated-function call by exact callee name.

```toml
[allow.selector]
kind = "call"
callee = "unwrap"
```

Call matching is exact after normalization. Qualified and associated call forms
such as `Option::unwrap(some_value)` are normalized so the callee is `unwrap`.

### string_literal

Matches a panic-family occurrence inside a string literal. This is the
recommended kind for fixture text, error message samples, and documentation
examples that mention panic-family vocabulary but are not actual call sites.

**`string_literal` selectors require `text_contains`.**

```toml
[allow.selector]
kind = "string_literal"
text_contains = "this should never happen"
```

The checker verifies that the string literal's content contains the
`text_contains` value. Without `text_contains`, the entry is rejected at parse
time.

#### Example: fixture false positive

A test file contains a string `"expected unwrap to succeed"` in a fixture
input. The text mentions `unwrap` but is not a real call. A `string_literal`
selector suppresses the false positive without tying the exception to a line
number:

```toml
[[allow]]
path = "crates/ripr/tests/integration.rs"
family = "unwrap"
classification = "test_only"
explanation = "Fixture text mentions unwrap in expected output, not a call site"

[allow.selector]
kind = "string_literal"
text_contains = "expected unwrap to succeed"
```

## last_seen

The `[allow.last_seen]` section records the last known line and column where
the allowed call site appeared. It is **advisory only** — it is not part of
the entry identity.

The checker emits a hint when `last_seen` drifts from the current location:

```
allowed by semantic selector; last_seen changed from line 42 to line 55 (src/file.rs:55:17)
```

This helps reviewers locate the entry in the file during allowlist audits
without causing build failures when code moves.

## v0.1 backward compatibility

Entries without a `[allow.selector]` section are matched by path + line +
column in v0.1 mode. This allows incremental migration: new entries use v0.2
selectors, and existing v0.1 entries continue to work until they are converted.

v0.1 entries tied to exact line numbers will fail when the code moves. Prefer
migrating to v0.2 selectors for stable entries.

## Migration proposals

The xtask may generate migration proposals that convert v0.1 entries to v0.2
selectors. These proposals are **review-only**. They are not adoption-ready and
must not be applied automatically. Each proposed selector should be verified
against the actual call site before being committed to the allowlist.

## Anti-patterns

- **Do not** use `closure_NNNNN` (byte-offset closures) as stable selector
  anchors. Closure offsets are unstable across edits and will produce stale
  entries.

- **Do not** omit `text_contains` on `string_literal` selectors. The checker
  rejects such entries.

- **Do not** rely on `last_seen` for matching. It is a hint, not identity.
