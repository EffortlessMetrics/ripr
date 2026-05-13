# Fixture: python_owner_file_match

Spec: RIPR-SPEC-0028

## Given

Two Python files contain owners on the same line range. Only `src/b.py`
changes, and the tests reference both owners.

## When

```bash
cargo xtask fixtures python_owner_file_match
```

## Then

The Python preview adapter chooses the owner from the changed file before
matching line ranges, so the finding points at `beta_score` and the related
test in `tests/test_b.py`.

## Must Not

- Cross-route owner or test evidence from `src/a.py`.
- Emit editor/LSP preview routing.
