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
  [`clippy.toml`](../clippy.toml) deliberately does not enable
  `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`,
  `allow-dbg-in-tests`, or `allow-indexing-slicing-in-tests`. The same file
  pins `msrv = "1.93"` so newer Clippy releases — when run via `rustup` or
  in an advisory CI lane on a more recent toolchain — do not suggest APIs
  that violate the workspace's declared minimum supported Rust version.
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
  `clippy::should_panic_without_expect`, `clippy::unwrap_used`,
  `clippy::expect_used`, `clippy::get_unwrap`,
  `clippy::unwrap_in_result`.
- Memory / drop footguns: `clippy::mem_forget`, `clippy::forget_non_drop`,
  `clippy::drop_non_drop`.
- Numeric correctness: `clippy::float_cmp`, `clippy::float_cmp_const`,
  `clippy::float_equality_without_abs`, `clippy::lossy_float_literal`,
  `clippy::invalid_upcast_comparisons`, `clippy::cast_sign_loss`,
  `clippy::cast_abs_to_unsigned`, `clippy::cast_enum_truncation`,
  `clippy::cast_nan_to_int`. Width-loss casts
  (`cast_possible_truncation`, `cast_possible_wrap`,
  `cast_precision_loss`) and `arithmetic_side_effects` are intentionally
  not yet active — they fire heavily on legitimate counter-shape code
  and warrant their own scoped flips with per-site review.
- Silent failure: `clippy::let_underscore_future`,
  `clippy::let_underscore_lock`, `clippy::unused_result_ok`,
  `clippy::map_err_ignore`, `clippy::assertions_on_result_states`,
  `clippy::lines_filter_map_ok`, `clippy::match_result_ok`.
  `clippy::let_underscore_must_use` is
  intentionally **not** yet active — best-effort cleanup patterns
  (`let _ = fs::remove_dir_all(&dir)`) are pervasive across tests, and the
  flip is tracked as a follow-up. Tests asserting that a `Result` is `Err`
  should use `.expect_err("why")` rather than `assert!(x.is_err())`.
- Format / I/O footguns: `clippy::format_in_format_args`,
  `clippy::to_string_in_format_args`, `clippy::unused_format_specs`,
  `clippy::suspicious_open_options`, `clippy::nonsensical_open_options`,
  `clippy::ineffective_open_options`, `clippy::path_buf_push_overwrite`,
  `clippy::join_absolute_paths`.
- AST / UTF-8 / parser safety:
  `clippy::char_indices_as_byte_indices`,
  `clippy::sliced_string_as_bytes`, `clippy::index_refutable_slice`,
  `clippy::out_of_bounds_indexing`. `clippy::indexing_slicing` and
  `clippy::string_slice` are deferred — parser/diff code uses bounded
  slice arithmetic where indices come from validated AST text ranges.
  The flip pairs the two lints together with per-site `#[expect]`
  receipts and is tracked in `policy/clippy-lints.toml` as planned.
- API and trait correctness: `clippy::iter_not_returning_iterator`,
  `clippy::expl_impl_clone_on_copy`, `clippy::infallible_try_from`,
  `clippy::fallible_impl_from`, `clippy::error_impl_error`. Catches trait
  contract mismatches that compile but silently mislead callers.
- Async / concurrency: `clippy::await_holding_lock`,
  `clippy::await_holding_refcell_ref`,
  `clippy::await_holding_invalid_type`, `clippy::arc_with_non_send_sync`,
  `clippy::rc_mutex`, `clippy::mut_mutex_lock`,
  `clippy::readonly_write_lock`. Holding a lock or `RefCell` borrow across
  an `.await` point is the canonical async deadlock shape; the rest catch
  threading-primitive misuse.
- Suppression governance: `clippy::allow_attributes_without_reason`,
  `clippy::blanket_clippy_restriction_lints`.
- Rust: `unsafe_code = "forbid"`, `unused_must_use`,
  `const_item_interior_mutations`, `function_casts_as_integer`.

`clippy::unwrap_used` and `clippy::expect_used` are denied at the Clippy
rail. Every existing call site is receipted on **both** rails:

- the semantic rail: a `[[allow]]` entry in
  `.ripr/no-panic-allowlist.toml` with selector identity;
- the clippy rail: a module-level `#[expect(clippy::unwrap_used, reason
  = "...")]` (or `expect_used`) attribute on the enclosing
  `#[cfg(test)]` block (or, for the `cli_smoke` integration test crate,
  a crate-level `#![expect]`), tracked in
  `.ripr/allow-attributes.txt`.

The module-level attribute scope keeps the receipt count small (one per
test-module surface, not one per call) while remaining auditable. A new
panic-family call site on a production path requires both an allowlist
entry **and** a fresh `#[expect]` receipt with a written reason; the
semantic checker is still the authoritative gate that owns the per-site
selector identity.

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

## Keeping the ledger and Cargo.toml in sync

`cargo xtask check-lint-policy` (run as part of `precommit` and `check-pr`)
verifies that:

- every `[[active.<group>]]` entry in `policy/clippy-lints.toml` is
  configured in `[workspace.lints.*]` in `Cargo.toml` at the same level;
- every `[[planned]]` entry is **not** yet configured in `Cargo.toml`
  (they're future flips, not active);
- every `[workspace.lints.*]` line has a matching ledger entry.

When promoting a planned lint, move the entry from `[[planned]]` to
`[[active.<group>]]`, add the matching `Cargo.toml` line, and update
this doc — the gate makes drift visible immediately.

## Companion ledgers

Two companion ledgers track Clippy state alongside the active/planned table:

- [`policy/clippy-debt.toml`](../policy/clippy-debt.toml) records lints that
  are intentionally **deferred** with a named owner, a blocking dependency,
  and a target date for clearing the debt. Empty by default.
- [`policy/clippy-exceptions.toml`](../policy/clippy-exceptions.toml) records
  per-call-site `#[expect(...)]` / `#[allow(...)]` suppressions with an `id`,
  `owner`, `reason`, `covered_by`, and `expires`. It is the reviewable
  counterpart to `.ripr/allow-attributes.txt`. Empty by default.

These are advisory until the corresponding xtask ledger checks land in a
follow-up PR.

## See also

- [`policy/clippy-lints.toml`](../policy/clippy-lints.toml) — declarative
  ledger and planned flips.
- [`policy/clippy-debt.toml`](../policy/clippy-debt.toml) — deferred lints
  with owner and target date.
- [`policy/clippy-exceptions.toml`](../policy/clippy-exceptions.toml) —
  per-site suppressions.
- [`docs/NO_PANIC_SEMANTIC_ALLOWLIST.md`](NO_PANIC_SEMANTIC_ALLOWLIST.md) —
  selector-based allowlist schema.
- [`docs/FILE_POLICY.md`](FILE_POLICY.md) — non-Rust file policy.
- [`.ripr/no-panic-allowlist.toml`](../.ripr/no-panic-allowlist.toml) —
  current entries.
