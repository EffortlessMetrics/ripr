# Fixture: ts_runner_detect_ava_script

Spec: RIPR-SPEC-0085

## Given

A Ky-like TypeScript package with ava in the composite `scripts.test` value
but NOT as a devDependency key:

- `package.json` has `"test": "xo && npm run build && ava"` — ava appears
  at the end of a composite script command.
- `devDependencies` contains `xo` and `typescript` but NOT `ava`.
- No lockfile is present.
- `src/fetch.ts` contains `buildUrl`.
- `tests/fetch.test.ts` contains an Ava test with `t.is(...)` oracle.
- `ripr.toml` enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_runner_detect_ava_script
```

or:

```bash
ripr check \
  --root fixtures/ts_runner_detect_ava_script/input \
  --diff fixtures/ts_runner_detect_ava_script/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Detects `ava` from the `scripts.test` value via the script-name fallback
  (dep-key detection yields None; script fallback finds `ava` as a word in
  `"xo && npm run build && ava"`).
- Emits `typescript_test_runner: ava` as an additive evidence field.
- Emits `typescript_framework_hint: ava`.
- Does NOT emit `typescript_package_limitation: typescript_test_runner_unresolved`.

## Must Not

- Emit `typescript_test_runner_unresolved` when ava appears in scripts.test.
- Match `xo` or `build` as a known test runner.
- Guess a runner when only unrecognised tools appear.
