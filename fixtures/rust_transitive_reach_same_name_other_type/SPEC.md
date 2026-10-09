# Fixture: rust_transitive_reach_same_name_other_type

Spec: RIPR-SPEC-0115

## Given

A Rust crate where the changed private owner `Queue::full_build` is reached
from `Site::build`, and an integration test helper in `tests/common/mod.rs`
calls `site.build()`. A unit test in `src/render.rs` calls `cache.build()` on
an unrelated `Cache` type. Name-only reach facts cannot tell the two `build`
methods apart, and `src/render.rs` sorts before `tests/site.rs` (#5481).

## When

```bash
cargo xtask fixtures rust_transitive_reach_same_name_other_type
```

or:

```bash
ripr check --root fixtures/rust_transitive_reach_same_name_other_type/input --diff fixtures/rust_transitive_reach_same_name_other_type/diff.patch --mode fast
```

## Then

`ripr` should emit `no_static_path` with
`static_limit_kind: "rust_integration_public_api_path_unresolved"` and a witness
that names `atom_written` in `tests/site.rs` with entry `build_site`. The unit
test `cache_builds` stays a counted candidate but is not named.

## Must Not

- Name `cache_builds`, whose `build` call lands on `Cache::build`.
- Fall back to `rust_transitive_reach_unresolved` because of file order.
- Promote classification beyond `no_static_path`.
- Add any witnessing test to `related_tests`.
