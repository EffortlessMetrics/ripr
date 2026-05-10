# No-Panic Policy

`ripr` targets panic-free production code and panic-free tests. There are no
carveouts for test code.

## What is prohibited

Production and test code may not use:

- `unwrap()` or `expect(...)` on `Option` or `Result`
- `panic!(...)`
- `todo!(...)`
- `unimplemented!(...)`
- `unreachable!(...)`
- Unchecked slice indexing (`&slice[i]`, `&slice[i..j]`) — tracked separately
  under `indexing_slicing` / `string_slice` Clippy lints (see `docs/CLIPPY_POLICY.md`).
- Unchecked string slicing.

`clippy.toml` does not enable `allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests`, `allow-dbg-in-tests`, or `allow-indexing-slicing-in-tests`.

## The dual-rail model

Panic-safety enforcement runs on two complementary rails.

**Rail A — Clippy (code-shape, fast feedback)**

Catches panic-family code shapes close to the editor and on every `cargo clippy`.
Active lints: `dbg_macro`, `todo`, `unimplemented`, `panic`, `unreachable`,
`should_panic_without_expect`, `unwrap_used`, `expect_used`, `get_unwrap`,
`unwrap_in_result`.

**Rail B — Semantic checker (authoritative, identity-stable)**

`cargo xtask check-no-panic-family` parses the AST and matches each call site
against the canonical no-panic allowlist using `path + family + selector`
identity. Line/column drift is advisory; the allowlist still matches when code
moves.

The checker reports the policy state in structured sections:

- `Allowed findings` lists current panic-family call sites matched by reviewed
  allowlist entries.
- `Advisory drift` lists `last_seen` line or column movement without failing
  the gate.
- `Stale entries` lists allowlist entries that no longer match a current
  finding and fails the gate.
- `Unallowed findings` lists current call sites without an allowlist entry and
  fails the gate.
- `Warnings` lists invalid review conditions such as ambiguous selector matches
  or duplicate semantic identities. These warnings fail until the selector is
  made unique.

See `docs/NO_PANIC_SEMANTIC_ALLOWLIST.md` for schema and selector design.

## Allowlist location

The canonical allowlist is `policy/no-panic-allowlist.toml` (schema 0.3).

`policy/no-panic-allowlist.toml` supersedes `.ripr/no-panic-allowlist.toml`
(schema 0.2). The `.ripr/` file is retained as a compatibility mirror while
older branches drain; new entries go only in `policy/no-panic-allowlist.toml`.

## Allowlist identity

Entry identity is:

```text
identity = path + family + selector
```

Never:

```text
identity = path + line + column
```

`last_seen` in the allowlist is advisory drift information only. Moving a call
site to a different line does not invalidate the allowlist entry.

Selectors must be specific enough to match exactly one current finding.
`method_call`, `call`, and `macro_call` selectors require both `container` and
`callee`; add `receiver_fingerprint` when the same container has multiple
panic-family calls with the same callee.

## Exception criteria

An exception is allowed only when **all** of the following are true:

1. The call site cannot reasonably be converted to a fallible path.
2. The invariant that prevents a panic at this call site is documented in the
   `explanation` field.
3. The entry has an `owner` and an `expires` date no more than 12 months from
   when the entry was added.
4. The entry has a `selector` that identifies the call site structurally.

## Test code

Test setup, fixture plumbing, and helper functions must be fallible. Convert:

```rust
// before
let content = std::fs::read_to_string(&path).unwrap();
```

```rust
// after
let content = std::fs::read_to_string(&path)?;
// test function returns anyhow::Result<()>
```

Test assertions (`assert!`, `assert_eq!`, `assert_ne!`) are allowed as test
oracles in the current policy (v1). A later optional campaign (PR 18) will
introduce fallible assertion helpers for setups where panic-free assertions are
needed.

## Checking compliance

```bash
cargo xtask check-no-panic-family
```

Run this locally before pushing. It runs in CI as a required gate.

To generate review-only migration hints for legacy line/column entries or
ambiguous selectors, run:

```bash
cargo xtask check-no-panic-family --propose
```

This writes `target/ripr/reports/no-panic-allowlist-proposals.md` and
`target/ripr/reports/no-panic-allowlist-proposals.toml`. The command does not
rewrite policy files and its TOML output must be reviewed before adoption.

## Adding an exception

1. Convert the call site to a fallible form if possible.
2. If not possible, add an entry to `policy/no-panic-allowlist.toml` following
   the schema-0.3 template in that file.
3. The entry must have `id`, `path`, `family`, `classification`, `owner`,
   `explanation`, `expires`, and `[allow.selector]`.
4. Run `cargo xtask check-no-panic-family` to verify the entry matches.
5. Do not add new entries to `.ripr/no-panic-allowlist.toml`; it is a legacy
   compatibility mirror, not the canonical reader.

If the checker reports ambiguity, add the smallest selector field that makes the
match unique. Prefer `receiver_fingerprint` for repeated method calls inside
the same function.
