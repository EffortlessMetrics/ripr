# Fixture: rust_pyo3_cross_language_limit

Spec: RIPR-SPEC-0062

Owner: analysis-fixtures

## Given

A PyO3 crate changes a threshold from `>` to `>=` in two places. The first is
the `#[pyfunction]` `fee`. The second is `Ledger::charge`, which carries no
attribute of its own and is exposed to Python through its `#[pymethods]`
`impl` block. The only test is `tests/test_fee.py`, which calls the compiled
module from Python. No Rust test exists. The PyO3 dependency is omitted from
`Cargo.toml` because the analysis is static and reads only the attributes.

## When

```bash
cargo xtask fixtures rust_pyo3_cross_language_limit
```

The public diff analysis examines the changed predicates on `src/lib.rs:5`
and `src/lib.rs:20`.

## Then

Both findings read `no_static_path` with the named limitation
`cross_language_oracle_visibility_unresolved`. Their next step tells the
reader to verify the external oracle rather than add a same-language test.

## Must Not

- Report a bare `no_static_path` whose next step asks for a co-located Rust
  test, since the tests for a binding live in the other language.
- Miss the method whose binding attribute sits on its `impl` block.
- Claim that the Python test observes the change.
