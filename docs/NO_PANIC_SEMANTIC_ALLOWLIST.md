# No-Panic Semantic Allowlist

The `cargo xtask check-no-panic-family` gate rejects panic-family call sites
(`unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`)
in production and test code unless each occurrence is listed in
`policy/no-panic-allowlist.toml` with a human-reviewed explanation.

This document defines the canonical schema 0.3 allowlist. Schema 0.3 keeps the
same semantic selector identity as schema 0.2 and adds governance fields.

## Identity

A schema 0.3 allowlist entry is identified by **path + family + selector**, not by
line or column number. Line and column are recorded as advisory locator hints
only (see [last_seen](#last_seen)).

When a code change moves an allowed call to a different line, the schema 0.3 entry
still matches because the selector describes the *structural* call site rather
than its position in the file.

## Schema version

```toml
schema_version = "0.3"
policy = "no-panic-allowlist"
owner = "core/policy"
status = "canonical"
```

The canonical file is `policy/no-panic-allowlist.toml`. The checker uses
`schema_version = "0.3"` to activate semantic selector matching and governance
field validation. The legacy `.ripr/no-panic-allowlist.toml` schema 0.2 file is
retained only as a compatibility mirror while older branches drain.

## Entry structure

```toml
[[allow]]
id = "panic-0001"
path = "src/some_file.rs"
family = "unwrap"
classification = "test_only"
owner = "core/tests"
explanation = "Human-readable reason this call site is allowed"
expires = "2026-12-31"

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
| `id` | `[[allow]]` | Stable identifier referenced in PRs and cleanup work |
| `classification` | `[[allow]]` | Entry classification, such as `test_only` or `test_helper` |
| `owner` | `[[allow]]` | Team or area responsible for the exception |
| `explanation` | `[[allow]]` | Human-readable reason for the exception |
| `expires` | `[[allow]]` | Date the entry must be re-justified |
| `kind` | `[allow.selector]` | Selector kind (see below) |

### Optional fields

| Field | Location | Description |
|---|---|---|
| `container` | `[allow.selector]` | Enclosing function or method name |
| `callee` | `[allow.selector]` | Exact callee name |
| `receiver_fingerprint` | `[allow.selector]` | Receiver type or expression fingerprint |
| `text_contains` | `[allow.selector]` | Required text fragment for `string_literal` selectors |
| `line` | `[allow.last_seen]` | Advisory: last known line number |
| `column` | `[allow.last_seen]` | Advisory: last known column number |

`container` and `callee` are optional in the TOML shape because
`string_literal` selectors use `text_contains` instead. For `method_call`,
`call`, and `macro_call` selectors, the checker requires both `container` and
`callee`, then verifies that the selector matches exactly one current finding.
Use `receiver_fingerprint` to disambiguate repeated calls in the same
container.

## Selector kinds

The checker supports four selector kinds:

- `method_call`
- `macro_call`
- `call`
- `string_literal`

The active checked-in allowlist may use only a subset of these kinds at a given
time. Migration tooling may also propose a subset first. That does not narrow
the checker contract: supported selector kinds are the four kinds listed above.

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
container = "build_error"
callee = "panic!"
```

The callee must include the trailing `!`.

### call

Matches a free-function or associated-function call by exact callee name.

```toml
[allow.selector]
kind = "call"
container = "build_output_path"
callee = "unwrap"
```

Call matching is exact after normalization. Qualified and associated call forms
such as `Option::unwrap(some_value)` are normalized so the callee is `unwrap`.

### string_literal

Matches a panic-family finding caused by source text inside a string literal.
This selector is for source-text false positives, such as documentation,
fixture text, or expected-output snippets that intentionally mention
panic-family spellings without executing them.

```toml
[allow.selector]
kind = "string_literal"
text_contains = "unwrap()"
```

`text_contains` is required for `string_literal` selectors. The checker rejects
a `string_literal` selector without it, because string-literal matching needs a
human-reviewed source-text fragment rather than only a structural call shape.

## last_seen

The `[allow.last_seen]` section records the last known line and column where
the allowed call site appeared. It is **advisory only** — it is not part of
the entry identity.

The checker emits a hint when `last_seen` drifts from the current location:

```text
allowed by semantic selector; last_seen changed from 42:17 to src/file.rs:55:17 (panic-0001 ...)
```

This helps reviewers locate the entry in the file during allowlist audits
without causing build failures when code moves.

## v0.1 backward compatibility

Entries without a `[allow.selector]` section are matched by path + line +
column in v0.1 mode. This legacy behavior exists so old fixtures can continue
to prove migration behavior while canonical entries use schema 0.3 selectors.

v0.1 entries tied to exact line numbers will fail when the code moves. Prefer
migrating to semantic selectors for stable entries.

The canonical schema 0.3 file does not permit new line/column-only entries.
Compatibility behavior exists only for legacy files and tests.

## Migration proposals

The xtask may generate migration proposals that convert v0.1 entries to v0.2
selectors. These proposals are **review-only**. They are not adoption-ready and
must not be applied automatically. Each proposed selector should be verified
against the actual call site before being committed to the allowlist.

Generate proposals with:

```bash
cargo xtask check-no-panic-family --propose
```

The command writes:

- `target/ripr/reports/no-panic-allowlist-proposals.md`
- `target/ripr/reports/no-panic-allowlist-proposals.toml`

Each candidate records the current finding, suggested selector, confidence,
whether it replaces a v0.1 entry, old coordinates when available, new
`last_seen` values, preserved reason text, and warnings for ambiguous containers
or duplicate selector risk. The TOML report is a review aid, not an
auto-adopted patch.

## Anti-patterns

- **Do not** use `closure_NNNNN` (byte-offset closures) as stable selector
  anchors. Closure offsets are unstable across edits; the checker rejects
  selectors with synthetic closure containers.

- **Do not** rely on `last_seen` for matching. It is a hint, not identity.

- **Do not** leave selectors ambiguous. If one entry matches multiple current
  findings, the checker fails and asks for a narrower selector.

- **Do not** duplicate semantic identities. Two entries with the same
  `path + family + selector` fail even when they point at the same current
  finding.

## Schema 0.3 governance

Schema 0.3 adds governed review fields to the semantic selector model:

- `id` — stable identifier referenced in PR descriptions, ADRs, and follow-up
  cleanup work.
- `owner` — team/area responsible for the entry.
- `expires` — date the entry must be re-justified.

The selector model is unchanged from schema 0.2: `path + family + selector` is
the identity, and `last_seen` is advisory.
