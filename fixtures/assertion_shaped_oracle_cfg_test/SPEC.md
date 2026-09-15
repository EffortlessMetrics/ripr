# Fixture: assertion_shaped_oracle_cfg_test

Spec: RIPR-SPEC-0133

## Given

An assertion-shaped oracle helper lives in a production file behind
`#[cfg(test)]` (`src/lib.rs`, `mod tests`). Its body is dominated by
`assert!`/`assert_eq!` calls and its only caller is a `#[test]` in the same
module.

The diff changes a predicate computed inside the helper:

```rust
let changed = if normalized.len() >= path.len() { 1 } else { 0 };
```

to:

```rust
let changed = if normalized.len() > path.len() { 1 } else { 0 };
```

## When

```bash
cargo xtask fixtures assertion_shaped_oracle_cfg_test
```

or:

```bash
ripr check --root fixtures/assertion_shaped_oracle_cfg_test/input --diff fixtures/assertion_shaped_oracle_cfg_test/diff.patch --mode fast
```

## Then

`ripr` does not emit a production finding for the changed predicate because
the owner is inside an inline `#[cfg(test)]` module. The helper remains indexed
as test/evidence input, but its harness plumbing does not create a recursive
production proof obligation. This fixture is therefore a source-role control
for #3213; assertion-shaped-owner guidance remains covered by production-role
fixtures. The prior `weakly_exposed` expectation is intentionally obsolete.

<!--
`ripr` previously classified the change `weakly_exposed` but reframed the
`recommended_next_step`: it must NOT say "Replace broad assertions with exact
equality" (the hawk complaint — exact equality is unavailable for a boolean
invariant and an exact-equality assertion already exists in the same helper).
The guidance instead advises tightening the loosest assertion, and the finding
evidence includes `owner_shape: assertion_shaped (...)`.
-->

## Must Not

- Reclassify the helper as production because it lives in `src/lib.rs`.
- Drop the helper from the evidence index or related-test inputs.
- Emit assertion-shaped-owner guidance for test-only plumbing.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
