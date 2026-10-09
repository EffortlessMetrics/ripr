# Fixture: python_functools_memoization_not_indirection

Spec: RIPR-SPEC-0028

## Given

Two Python production owners are decorated with `functools.lru_cache` (one
bare, one with call arguments). The changed lines are otherwise plain
return-value shapes.

A pytest test calls `format_year` with an exact-value assertion. A twin test
calls `century_index` without an oracle that observes the changed return.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_functools_memoization_not_indirection
```

or:

```bash
ripr check \
  --root fixtures/python_functools_memoization_not_indirection/input \
  --diff fixtures/python_functools_memoization_not_indirection/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- does not emit `static_limit_kind = "decorator_indirection"` for either owner,
- credits `format_year` as `exposed` because a related test uses a strong
  exact-value oracle on the changed return,
- reports `century_index` as a gap (`weakly_exposed`) because the related
  test reaches the owner without a recognized oracle for the changed return.

`fixtures/python_decorator_indirection_limit` remains the `@retry` negative
control and is unchanged.

## Must Not

- Resolve decorator runtime behavior or execute pytest.
- Treat a local same-named `lru_cache` not imported from `functools` as
  transparent (unit tests cover that negative).
- Claim runtime mutation outcomes from this static evidence.
