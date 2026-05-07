# Clippy and Lint Policy

`ripr` runs panic safety on a **dual-rail** design. Both rails must pass for a
PR to land, and both rails describe the same policy from different angles.

## The two rails

```text
Rail A — Clippy (code-shape, fast feedback)
  Catches panic-family code shapes (panic!, unreachable!, todo!, dbg!,
  unimplemented!, etc.) close to the editor and on every `cargo clippy`.
  Levels live in `[workspace.lints.clippy]` in Cargo.toml and apply to every
  crate via `[lints] workspace = true`.

Rail B — Semantic checker (authoritative, identity-stable)
  `cargo xtask check-no-panic-family` parses the AST and matches each call
  site against `.ripr/no-panic-allowlist.toml` using
  `path + family + selector` identity. Line/column drift is advisory; the
  allowlist still matches when code moves.
  Schema and selectors: docs/NO_PANIC_SEMANTIC_ALLOWLIST.md.
```

The two rails serve different purposes:

- Clippy is the **fast** rail. It surfaces problems in IDEs and on the first
  `cargo clippy` invocation, before any xtask runs.
- The semantic checker is the **stable** rail. It owns the allowlist of
  intentional exceptions, with classification and selector identity that
  survives refactors.

A panic-family call site is acceptable only if it satisfies **both rails**.

## What the policy says

- Production code may not panic, unwrap, expect, `panic!`, `todo!`,
  `unimplemented!`, or `unreachable!` outside an explicit, allowlisted
  exception with a written explanation.
- Test code follows the same rule. There are no test carveouts.
  `clippy.toml` does not enable `allow-unwrap-in-tests`,
  `allow-expect-in-tests`, `allow-panic-in-tests`, or
  `allow-indexing-slicing-in-tests`.
- Every suppression carries a written reason. `clippy::allow_attributes_without_reason`
  is denied at the workspace level, so `#[allow(...)]` and `#[expect(...)]`
  must include `reason = "..."`.
- Blanket category enables (`clippy::all`, `clippy::pedantic`,
  `clippy::nursery`, `clippy::restriction`) are not used as the policy
  surface. Lints are listed individually so the active set is reviewable.
- `unsafe_code = "forbid"` workspace-wide. Adding an unsafe island requires
  a separate, dedicated PR and review.

## Active lints

The authoritative source is `[workspace.lints.*]` in Cargo.toml. A reviewable
ledger lives in [`policy/clippy-lints.toml`](../policy/clippy-lints.toml),
including planned 1.94 / 1.95 lint flips that are tracked but not yet active.

Currently denied at the workspace level (selected highlights):

- Panic family: `clippy::panic`, `clippy::unreachable`, `clippy::todo`,
  `clippy::unimplemented`, `clippy::dbg_macro`,
  `clippy::should_panic_without_expect`.
- Memory / drop footguns: `clippy::mem_forget`, `clippy::forget_non_drop`,
  `clippy::drop_non_drop`.
- Numeric correctness: `clippy::float_cmp`, `clippy::float_cmp_const`.
- Silent failure: `clippy::let_underscore_future`,
  `clippy::let_underscore_lock`, `clippy::unused_result_ok`,
  `clippy::map_err_ignore`, `clippy::assertions_on_result_states`,
  `clippy::lines_filter_map_ok`. `clippy::let_underscore_must_use` is
  intentionally **not** yet active — best-effort cleanup patterns
  (`let _ = fs::remove_dir_all(&dir)`) are pervasive across tests, and the
  flip is tracked as a follow-up. Tests asserting that a `Result` is `Err`
  should use `.expect_err("why")` rather than `assert!(x.is_err())`.
- Format / I/O footguns: `clippy::format_in_format_args`,
  `clippy::to_string_in_format_args`, `clippy::unused_format_specs`,
  `clippy::suspicious_open_options`, `clippy::nonsensical_open_options`,
  `clippy::ineffective_open_options`, `clippy::path_buf_push_overwrite`,
  `clippy::join_absolute_paths`.
- API / trait correctness: `clippy::iter_not_returning_iterator`,
  `clippy::expl_impl_clone_on_copy`, `clippy::infallible_try_from`,
  `clippy::fallible_impl_from`, `clippy::error_impl_error`.
- Suppression governance: `clippy::allow_attributes_without_reason`,
  `clippy::blanket_clippy_restriction_lints`.
- Rust: `unsafe_code = "forbid"`, `unused_must_use`,
  `const_item_interior_mutations`, `function_casts_as_integer`.

`clippy::unwrap_used` and `clippy::expect_used` are intentionally **not**
denied at the Clippy rail today. The semantic checker carries that policy
because matching `path + family + selector` is more robust against refactors
than per-call-site `#[expect]` annotations. A future PR can flip the Clippy
rail to deny once the allowlist entries are mirrored in source-level
`#[expect(clippy::unwrap_used, reason = "policy:no-panic:<id>")]` attributes.

## Adding an exception

If a call site genuinely needs a panic-family construct:

1. Add a `[[allow]]` entry to `.ripr/no-panic-allowlist.toml` with
   `path`, `family`, `classification`, `explanation`, and a stable selector.
   See [`docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`](NO_PANIC_SEMANTIC_ALLOWLIST.md).
2. If Clippy also fires (for example `clippy::panic`), add an
   `#[expect(clippy::<lint>, reason = "...")]` attribute on the enclosing
   item. The reason should reference the matching allowlist explanation.
3. CI runs `cargo clippy -- -D warnings` and `cargo xtask
   check-no-panic-family`. Both must succeed.

## Reviewing a flip from `warn` to `deny`

Flipping a lint to `deny` is a behavior change. The PR doctrine in
[CLAUDE.md](../CLAUDE.md) and [`docs/SCOPED_PR_CONTRACT.md`](SCOPED_PR_CONTRACT.md)
applies:

- Document the flip in `policy/clippy-lints.toml`.
- Resolve every existing finding in the same PR or receipt it with a
  reasoned suppression that points to the underlying policy entry.
- Do not bundle unrelated cleanup. A lint flip is one production behavior.

## See also

- [`policy/clippy-lints.toml`](../policy/clippy-lints.toml) — declarative
  ledger and planned flips.
- [`docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`](NO_PANIC_SEMANTIC_ALLOWLIST.md) —
  selector-based allowlist schema.
- [`docs/FILE_POLICY.md`](FILE_POLICY.md) — non-Rust file policy.
- [`.ripr/no-panic-allowlist.toml`](../.ripr/no-panic-allowlist.toml) —
  current entries.
