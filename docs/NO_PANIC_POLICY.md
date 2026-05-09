# No-Panic Policy

`ripr` targets panic-free production code and panic-free tests. This document gives the
high-level policy. The detailed allowlist schema is in
[`docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`](NO_PANIC_SEMANTIC_ALLOWLIST.md).

## The Dual-Rail Design

Panic safety is enforced by two complementary rails. Both must pass for a PR to land.

**Rail A — Clippy (code-shape, fast feedback)**

Catches panic-family code shapes close to the editor and on every `cargo clippy`. Configured
in `[workspace.lints.clippy]` in `Cargo.toml`:

```
dbg_macro = "deny"
todo = "deny"
unimplemented = "deny"
panic = "deny"
unreachable = "deny"
should_panic_without_expect = "deny"
unwrap_used = "deny"
expect_used = "deny"
get_unwrap = "deny"
unwrap_in_result = "deny"
```

**Rail B — Semantic checker (authoritative, identity-stable)**

`cargo xtask check-no-panic-family` parses the AST and matches each call site against
`policy/no-panic-allowlist.toml` (target) or `.ripr/no-panic-allowlist.toml` (current) using
`path + family + selector` identity. Line and column drift is advisory; the allowlist still
matches when code moves.

## What Is Forbidden

No new panic-family call sites in production or test code without an allowlist entry:

- `panic!(...)` / `unreachable!(...)`
- `.unwrap()` / `.unwrap_or_else(|_| panic!(...))` / `.get(...).unwrap()`
- `.expect("...")` (even with a message)
- `todo!()` / `unimplemented!()`
- `assert!(false, ...)` (matches `panic` shape)
- Unchecked indexing with `[]` (tracked by `indexing_slicing` lint — PR 07)
- Unchecked string slicing (tracked by `string_slice` lint — PR 07)

## No Test Carveouts

`clippy.toml` deliberately does not enable `allow-unwrap-in-tests`,
`allow-expect-in-tests`, `allow-panic-in-tests`, `allow-dbg-in-tests`, or
`allow-indexing-slicing-in-tests`. The same rule applies to production and test code.

Test setup and fixture plumbing must be fallible. Preferred pattern:

```rust
#[test]
fn my_test() -> anyhow::Result<()> {
    let fixture = std::fs::read_to_string(path)?;
    // ...
    Ok(())
}
```

Test oracles (`assert!`, `assert_eq!`, `assert_ne!`) remain as test oracles in the current
policy (v1). A later optional campaign (PR 18) may introduce fallible assertion helpers for
full panic-free test bodies, but that is not required for the MSRV 1.95 ratchet.

## Allowed Exceptions

Every allowed exception lives in the canonical allowlist with:

- `id`: stable identifier
- `path`: file the entry applies to
- `family`: `unwrap | expect | panic_macro | unreachable | todo | unimplemented`
- `classification`: `test_only | test_helper | release_only | bootstrap | …`
- `owner`: team or area responsible
- `explanation`: why this specific call site is allowed
- `expires`: date when the entry must be re-justified
- `selector`: semantic identity (kind + container + callee, NOT line/column)

**The identity is `path + family + selector`**, never `path + line + column`. Selectors
survive refactors; line numbers do not.

### Adding an Exception

1. Identify the call site: file, enclosing function, callee.
2. Verify no fallible alternative exists.
3. Add an entry to `policy/no-panic-allowlist.toml` with all required fields.
4. Add a corresponding `#[expect(clippy::unwrap_used, reason = "policy:panic-XXXX: ...")]`
   at the call site (or enclosing item for test modules).
5. Add the `#[expect]` attribute to `.ripr/allow-attributes.txt`.
6. Run `cargo xtask check-no-panic-family` and `cargo xtask check-allow-attributes`.

## Allowlist Transition (PR 04)

Current canonical file: `.ripr/no-panic-allowlist.toml` (schema 0.2)
Target canonical file: `policy/no-panic-allowlist.toml` (schema 0.3)

Schema 0.3 adds `id`, `owner`, `expires`, and a richer `selector` model. PR 04 migrates
all entries from schema 0.2 to schema 0.3 and updates the checker to read the new location.
After PR 04, the `.ripr/` file becomes a compatibility redirect or is removed.

## Classification Guide

| Classification | Meaning | Expected lifetime |
|---------------|---------|-----------------|
| `test_only` | Inside a test function or `#[cfg(test)]` block | Short—remove by converting to `?` |
| `test_helper` | Shared test infrastructure | Short—replace with fallible helper |
| `bootstrap` | Startup code before error handling is available | Medium—document invariant |
| `release_only` | Release tooling, never in the critical path | Medium |
| `documented_invariant` | An AST/type invariant makes the panic unreachable | Long—must name the invariant |

Prefer `documented_invariant` over `test_only` when the call site has a proof. Prefer
removing the entry over keeping it with a distant expiry.

## See Also

- [`docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`](NO_PANIC_SEMANTIC_ALLOWLIST.md) — schema reference
- [`docs/CLIPPY_POLICY.md`](CLIPPY_POLICY.md) — full dual-rail design
- [`policy/no-panic-allowlist.toml`](../policy/no-panic-allowlist.toml) — canonical allowlist (target)
- [`.ripr/no-panic-allowlist.toml`](../.ripr/no-panic-allowlist.toml) — current canonical (schema 0.2)
- [`docs/ci/ripr-rollout-plan.md`](ci/ripr-rollout-plan.md) — PRs 04, 05, 08
