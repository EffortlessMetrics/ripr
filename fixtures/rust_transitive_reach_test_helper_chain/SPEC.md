# Fixture: rust_transitive_reach_test_helper_chain

Spec: RIPR-SPEC-0114

## Given

A Rust crate where an integration test calls a local test helper, the helper
calls a public API function, and that public API flows through a bounded internal
helper chain to a changed private owner. This mirrors the semver "integration
test helper -> public API -> internal helper chain -> changed owner" shape
without copying the external repository.

## When

```bash
cargo xtask fixtures rust_transitive_reach_test_helper_chain
```

or:

```bash
ripr check --root fixtures/rust_transitive_reach_test_helper_chain/input --diff fixtures/rust_transitive_reach_test_helper_chain/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` with
`static_limit_kind: "rust_transitive_reach_unresolved"` and a concrete witness
that names the integration test helper entry point. The finding must not be
promoted to `weakly_exposed` or `exposed`.

## Must Not

- Promote classification beyond `no_static_path`.
- Add the witnessing integration test to `related_tests`.
- Claim the test reaches, covers, exercises, or observes the changed behavior.
