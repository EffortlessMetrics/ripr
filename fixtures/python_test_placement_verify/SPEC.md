# Fixture: python_test_placement_verify

Spec: RIPR-SPEC-0028

## Given

A Python preview workspace has two direct weak findings:

- a pytest-related predicate boundary gap,
- and a unittest-related exception-path gap.

Both related tests reach the changed owner but use broad assertions, so the
Python preview adapter can name the missing discriminator without claiming the
gap is closed.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_test_placement_verify
```

or:

```bash
ripr check \
  --root fixtures/python_test_placement_verify/input \
  --diff fixtures/python_test_placement_verify/diff.patch \
  --mode fast
```

## Then

Each actionable finding includes:

- a suggested test file,
- a suggested test name,
- a pytest node ID when the related test framework is pytest,
- a pytest or unittest verify command,
- command confidence,
- and a Python repair card with changed owner, changed behavior, current test
  evidence, missing discriminator, recommended test shape, suggested assertion,
  suggested location, verify command, preview/advisory authority, deferred
  receipt status, stop conditions, and limits.

The check JSON can also feed:

```bash
ripr reports gap-ledger --check-output check.json
```

which derives PR-local Python GapRecords that `ripr agent packet --gap-ledger`
can export with allowed test files, forbidden source files, conflict groups,
verify commands, deferred receipt status, and the same stop conditions.

## Must Not

- Invent placement for missing-test, heuristic-only, or static-limit findings.
- Run pytest, unittest, or import Python modules.
- Emit an agent packet directly from `ripr check`, generate tests, or invent a
  receipt command before the Python outcome-ledger slice exists.
