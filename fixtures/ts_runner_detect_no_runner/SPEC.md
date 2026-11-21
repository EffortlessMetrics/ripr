# Fixture: ts_runner_detect_no_runner

Spec: RIPR-SPEC-0085

## Given

A TypeScript package with `package.json` present but NO known test runner
in devDependencies or scripts.test — fail-closed behavior:

- `package.json` has `"test": "xo && npm run build"` — no known test runner.
- `devDependencies` contains only `typescript` and `xo`.
- No lockfile is present.
- `src/util.ts` contains `format`.
- `tests/util.test.ts` contains a test with a `toBe` oracle.
- `ripr.toml` enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_runner_detect_no_runner
```

or:

```bash
ripr check \
  --root fixtures/ts_runner_detect_no_runner/input \
  --diff fixtures/ts_runner_detect_no_runner/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds `package.json` and resolves `package_root` = `.`.
- Detects no known test runner from devDeps or scripts.test value.
- Does NOT emit `typescript_test_runner: <name>` (fail-closed — nothing to emit).
- Emits `typescript_package_limitation: typescript_test_runner_unresolved`.

## Must Not

- Match `xo` or `build` as a known test runner.
- Emit a guessed `typescript_test_runner` value.
- Panic or crash on an unrecognised scripts.test value.
