# Fixture: python_method_owner

Spec: RIPR-SPEC-0028

## Given

A Python class method changes a predicate. The same file also contains
`@staticmethod` and `@classmethod` owners so the owner extractor has to keep
decorated method context syntax-first.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_method_owner
```

or:

```bash
ripr check \
  --root fixtures/python_method_owner/input \
  --diff fixtures/python_method_owner/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the changed `DiscountPolicy.apply` method owner,
- recognises the related pytest-style test that calls `.apply(`,
- emits `owner_kind = "method"` for the changed owner,
- keeps staticmethod/classmethod recognition in syntax context without
  resolving decorator runtime behavior.

## Must Not

- Treat decorators as runtime semantics.
- Cross into editor or CI projection.
- Claim assertion strength before the assertion-facts slice lands.
