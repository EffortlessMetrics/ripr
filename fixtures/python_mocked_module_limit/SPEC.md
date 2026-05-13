# Fixture: python_mocked_module_limit

Spec: RIPR-SPEC-0028

## Given

A Python related test contains mock syntax while asserting the changed owner.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_mocked_module_limit
```

or:

```bash
ripr check \
  --root fixtures/python_mocked_module_limit/input \
  --diff fixtures/python_mocked_module_limit/diff.patch \
  --mode fast
```

## Then

The Python preview finding emits `static_limit_kind = "mocked_module"`.

## Must Not

- Execute or interpret mock substitution semantics.
- Treat interaction evidence as exact return-value proof.
