# Fixture: rust_macro_reach_limitation

Spec: RIPR-SPEC-0117

## Given

A Rust crate where an integration test calls `outer()`. The `outer()` function
does not lexically call changed `pub(crate) fn inner()`; it invokes a same-repo
macro `call_inner!`, and that macro definition lexically mentions `inner`.

## When

```bash
cargo xtask fixtures rust_macro_reach_limitation
```

or:

```bash
ripr check --root fixtures/rust_macro_reach_limitation/input --diff fixtures/rust_macro_reach_limitation/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` (classification unchanged) with
`static_limit_kind: "rust_macro_reach_unresolved"` and
`stop_reasons: ["macro_reach_unresolved"]`.

## Must Not

- Promote classification beyond `no_static_path`.
- Add the witness to `related_tests`.
- Claim the macro was expanded.
- Claim the test reaches, covers, tests, or exercises the changed behavior.
