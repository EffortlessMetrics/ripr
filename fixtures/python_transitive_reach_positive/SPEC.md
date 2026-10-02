# Fixture: python_transitive_reach_positive

Spec: RIPR-SPEC-0201

## Given

A Python class method is reached only through other same-class `self.` calls.
A pytest test constructs the class but never names the changed method.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_transitive_reach_positive
```

or:

```bash
ripr check \
  --root fixtures/python_transitive_reach_positive/input \
  --diff fixtures/python_transitive_reach_positive/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- keeps classification `no_static_path`;
- emits `static_limit_kind = "python_transitive_reach_unresolved"`;
- names a witness that constructs `Table` without adding `related_tests`;
- uses "may" language and does not claim coverage.

## Must Not

- Promote the finding above `no_static_path`.
- Add the constructing test to `related_tests`.
- Absorb function-to-helper / cross-module façade reach (#4568).
