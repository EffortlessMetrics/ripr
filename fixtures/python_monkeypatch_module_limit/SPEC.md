# Fixture: python_monkeypatch_module_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a returned call, and a related pytest test
uses `monkeypatch.setattr(...)` to replace module behavior at runtime.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_monkeypatch_module_limit
```

or:

```bash
ripr check \
  --root fixtures/python_monkeypatch_module_limit/input \
  --diff fixtures/python_monkeypatch_module_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `fetch_total` function owner,
- relates the pytest test that calls `fetch_total`,
- emits `static_limit_kind = "mocked_module"` for the monkeypatch runtime
  substitution,
- keeps the finding preview/advisory and non-repairable.

## Must Not

- Execute pytest or resolve monkeypatch substitutions.
- Emit a Python repair card.
- Emit a canonical gap ID.
- Route this finding into an agent packet or swarm queue.
