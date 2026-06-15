# Fixture: ts_runner_detect_ava_devdep

Spec: RIPR-SPEC-0085

## Given

A single-package TypeScript workspace with `ava` as the devDependency test runner:

- `package.json` at the repo root declares `ava` as a devDependency and
  `ava` as the `scripts.test` command.
- No lockfile is present (runner resolution falls back to framework detection).
- `src/math.ts` contains the `add` function.
- `tests/math.test.ts` contains an Ava `test(...)` call with a `t.is(...)`
  exact-value oracle referencing `add`.
- `ripr.toml` enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_runner_detect_ava_devdep
```

or:

```bash
ripr check \
  --root fixtures/ts_runner_detect_ava_devdep/input \
  --diff fixtures/ts_runner_detect_ava_devdep/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds `add` as the owner in `src/math.ts`.
- Detects `framework_hint` = `ava` from `devDependencies`.
- Emits `typescript_test_runner: ava` as an additive evidence field.
- Emits `typescript_framework_hint: ava` as an additive evidence field.
- Emits `typescript_verify_command: ava tests/math.test.ts`.
- Does NOT emit `typescript_package_limitation: typescript_test_runner_unresolved`.

## Must Not

- Guess a runner that is not evidenced by devDeps or scripts.test.
- Emit `typescript_test_runner_unresolved` when ava is clearly declared.
- Change `repair_packet_ready`, `language_status`, or any existing field.
