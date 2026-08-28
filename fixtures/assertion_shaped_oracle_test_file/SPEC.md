# Fixture: assertion_shaped_oracle_test_file

Spec: RIPR-SPEC-0133

## Given

An assertion-shaped oracle helper lives in a `src/` helper module
(`src/fragment_checks.rs`). Its body is dominated by `assert!`/`assert_eq!`
calls — a boolean invariant over every span plus an exact-equality assertion on
the crate root — and the only caller is a `#[test]` in `tests/fragments.rs`.
This mirrors the hawk case from the linked issue: the changed owner is itself
the discriminator.

The diff changes a value computed inside the helper:

```rust
let missing = if fragment.crate_root.is_none() { 0 } else { 0 };
```

to:

```rust
let missing = if fragment.crate_root.is_none() { 1 } else { 0 };
```

## When

```bash
cargo xtask fixtures assertion_shaped_oracle_test_file
```

or:

```bash
ripr check --root fixtures/assertion_shaped_oracle_test_file/input --diff fixtures/assertion_shaped_oracle_test_file/diff.patch --mode fast
```

## Then

`ripr` keeps its static exposure classification but reframes the
`recommended_next_step` for the oracle: it must NOT suggest teaching ripr about
a fixture/builder (there is none) and must NOT ask for a test that observes the
helper. The finding evidence includes the disclosure line
`owner_shape: assertion_shaped (...)`.

## Must Not

- Change the exposure class because the owner is assertion-shaped.
- Emit the standard code-under-test guidance (`fixture/builder in ripr.toml`,
  `co-located test`, `Replace broad assertions`) for this owner.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
