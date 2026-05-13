# Fixture: python_unsupported_probe_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes generator `yield from` syntax that
the preview adapter does not classify as a supported probe family.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_unsupported_probe_limit
```

or:

```bash
ripr check \
  --root fixtures/python_unsupported_probe_limit/input \
  --diff fixtures/python_unsupported_probe_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits `classification = "static_unknown"`,
`probe.family = "static_unknown"`, and
`static_limit_kind = "unsupported_syntax"` instead of guessing a
predicate or return-value probe.

## Must Not

- Hide unsupported syntax as a generic predicate.
- Claim runtime adequacy from preview static evidence.
