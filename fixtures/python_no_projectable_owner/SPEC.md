# Fixture: python_no_projectable_owner

Spec: RIPR-SPEC-0028

Note: the fixture name is historical. It now pins the transition from
unprojected module-level changes to a durable `<module>` owner.

## Given

A Python module-level constant changes. The preview adapter should attach the
change to a durable module-level owner instead of guessing a nearby function
owner.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_no_projectable_owner
```

or:

```bash
ripr check \
  --root fixtures/python_no_projectable_owner/input \
  --diff fixtures/python_no_projectable_owner/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits a preview finding with
`probe.owner = "python:src/constants.py::<module>"` and
`owner_kind = "module_function"`.

## Must Not

- Guess a function owner for a module-level constant.
- Cross-route to tests by text mention alone.
- Add editor routing.
