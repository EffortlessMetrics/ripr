# Fixture: rust_transitive_reach_positive

Spec: RIPR-SPEC-0114

## Given

A Rust crate where `pub fn outer()` directly calls changed `pub(crate) fn inner()`,
and an integration test calls `outer()` (not `inner()` directly). The direct-call
classifier finds no related test for `inner`, but the transitive-reach walk detects
that the test -> outer -> inner candidate path exists.

## When

```bash
cargo xtask fixtures rust_transitive_reach_positive
```

or:

```bash
ripr check --root fixtures/rust_transitive_reach_positive/input --diff fixtures/rust_transitive_reach_positive/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` (classification UNCHANGED) but with
`static_limit_kind: "rust_transitive_reach_unresolved"` (the limitation is named).
The finding must NOT be promoted to weakly_exposed or exposed.

## Must Not

- Promote classification beyond `no_static_path`.
- Claim the test reaches or covers the changed behavior.
- Change any schema field names or types.