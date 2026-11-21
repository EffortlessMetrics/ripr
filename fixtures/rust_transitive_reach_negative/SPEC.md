# Fixture: rust_transitive_reach_negative

Spec: RIPR-SPEC-0114

## Given

A Rust crate with a changed boundary predicate in `apply_fee` and no tests at all
(no test calls anything that reaches `apply_fee`). The transitive-reach walk finds
no candidate path, so the limitation must NOT fire.

## When

```bash
cargo xtask fixtures rust_transitive_reach_negative
```

or:

```bash
ripr check --root fixtures/rust_transitive_reach_negative/input --diff fixtures/rust_transitive_reach_negative/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` with NO `static_limit_kind` field (null / omitted).
This confirms the limitation only fires when a candidate path exists, not on every
`no_static_path` finding.

## Must Not

- Emit `static_limit_kind` when no candidate transitive path exists.
- Change classification from `no_static_path`.