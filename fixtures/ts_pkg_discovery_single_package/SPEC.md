# Fixture: ts_pkg_discovery_single_package

Spec: RIPR-SPEC-0085

## Given

A single-package TypeScript workspace:

- `package.json` at the repo root declares `jest` as a devDependency and
  `jest` as the `scripts.test` runner.
- `package-lock.json` is present at the repo root (npm lockfile).
- `src/math.ts` contains the `add` function.
- `tests/math.test.ts` contains a Jest `test(...)` call with a
  `expect(...).toBe(...)` strong oracle referencing `add`.
- `ripr.toml` enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_pkg_discovery_single_package
```

or:

```bash
ripr check \
  --root fixtures/ts_pkg_discovery_single_package/input \
  --diff fixtures/ts_pkg_discovery_single_package/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds `add` as the owner in `src/math.ts`.
- Finds the `test('add returns sum', ...)` in `tests/math.test.ts` with a
  `toBe` strong oracle.
- Resolves `package_root` = `.` from `package.json` at the workspace root.
- Resolves `workspace_root` = `.` (same as package_root — no monorepo
  indicator above).
- Detects `framework_hint` = `jest` from `devDependencies`.
- Detects `runner_hint` = `npm` from `package-lock.json`.
- Emits `typescript_package_confidence: high` (both framework and runner
  are evidence-backed).
- Appends `typescript_package_root`, `typescript_workspace_root`,
  `typescript_framework_hint`, `typescript_runner_hint`, and
  `typescript_package_confidence` evidence lines to the finding.
- Does NOT emit `typescript_package_limitation: typescript_package_root_unresolved`.
- `repair_packet_ready` stays `false`.
- `language_status` stays `preview`.

## Must Not

- Invent a `package_root` from the file extension alone.
- Emit `repair_packet_ready: true`.
- Change any existing field on the finding (additive evidence only).
- Use mutation-runtime outcome vocabulary.
