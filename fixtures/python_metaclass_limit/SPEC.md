# Fixture: python_metaclass_limit

Spec: RIPR-SPEC-0028

## Given

A Python class declaration changes to opt into a metaclass, and a pytest test
references the class.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_metaclass_limit
```

or:

```bash
ripr check \
  --root fixtures/python_metaclass_limit/input \
  --diff fixtures/python_metaclass_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `InvoiceRecord` class owner,
- records heuristic related-test evidence,
- emits `static_limit_kind = "metaprogramming"`,
- keeps the finding preview/advisory.

## Must Not

- Infer metaclass-created behavior.
- Execute Python imports or pytest.
- Emit a repair card, canonical repair gap, or agent packet.
