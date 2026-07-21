# Fixture: rust_macro_wrapped_test_call_limitation

Spec: RIPR-SPEC-0119

## Given

A Rust crate where an integration test invokes an exported same-repo macro
directly. The macro definition lexically mentions the changed owner `inner`,
but RIPR does not expand the macro.

## When

```bash
cargo xtask fixtures rust_macro_wrapped_test_call_limitation
```

or:

```bash
ripr check --root fixtures/rust_macro_wrapped_test_call_limitation/input --diff fixtures/rust_macro_wrapped_test_call_limitation/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` with
`static_limit_kind: "rust_macro_wrapped_test_call_unresolved"` and
`stop_reasons: ["macro_reach_unresolved"]`.

## Must Not

- Promote classification beyond `no_static_path`.
- Add the macro witness to `related_tests`.
- Claim the macro was expanded.
- Claim the test reaches, covers, tests, or exercises the changed behavior.
