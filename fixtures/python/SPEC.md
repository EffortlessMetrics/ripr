Spec: RIPR-SPEC-0028

# Python Project Detection Fixture Corpus

## Given

`fixtures/python/basic` is a Python-only repository root with `pyproject.toml`,
production source under `src/`, and pytest-style tests under `tests/`.

The fixture intentionally omits `ripr.toml` so the repo-shape detector must
enable Python preview analysis from project markers rather than explicit
configuration.

## When

An agent runs:

```bash
ripr pilot --root fixtures/python/basic
ripr check --root fixtures/python/basic --diff fixtures/python/basic/diff.patch --json
```

## Then

The commands complete without requiring a Cargo workspace, and diff-scoped
analysis emits Python preview metadata for the changed Python source.

## Must Not

Detection must not override an explicit `ripr.toml`, must not inspect virtualenv
or build output directories, and must not promote Python beyond preview or
advisory language.
