# Fixture: python_transitive_reach_negative

Spec: RIPR-SPEC-0201

## Given

A Python class method is unchanged-reach from tests that import a same-named
class from another module and construct that other class. The owner's class
is never constructed or called.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_transitive_reach_negative
```

or:

```bash
ripr check \
  --root fixtures/python_transitive_reach_negative/input \
  --diff fixtures/python_transitive_reach_negative/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- keeps classification `no_static_path`;
- does **not** emit `python_transitive_reach_unresolved`;
- does not treat a same-named other-module class as the owner.

## Must Not

- Name a limitation from token coincidence on the class name.
- Relate the other-module test as covering the owner.
