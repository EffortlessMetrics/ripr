# Fixture: ts_pkg_discovery_monorepo

Spec: RIPR-SPEC-0085

## Given

A monorepo TypeScript workspace:

- `pnpm-workspace.yaml` at the repo root declares `packages/*`.
- `package.json` at the repo root (no framework deps).
- `pnpm-lock.yaml` at the repo root (pnpm lockfile).
- `packages/auth/package.json` declares `jest` as a devDependency.
- `packages/auth/src/token.ts` contains the `createToken` function.
- `packages/auth/tests/token.test.ts` contains a Jest test with a
  `toBe` strong oracle referencing `createToken`.
- `ripr.toml` at the repo root enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_pkg_discovery_monorepo
```

or:

```bash
ripr check \
  --root fixtures/ts_pkg_discovery_monorepo/input \
  --diff fixtures/ts_pkg_discovery_monorepo/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds `createToken` as the owner in `packages/auth/src/token.ts`.
- Finds the `test(...)` in `packages/auth/tests/token.test.ts` with a
  `toBe` strong oracle.
- Resolves `package_root` = `packages/auth` (nearest `package.json`).
- Resolves `workspace_root` = `.` (repo root has `pnpm-workspace.yaml`).
- Detects `framework_hint` = `jest` from the sub-package `package.json`.
- Detects `runner_hint` = `pnpm` from `pnpm-lock.yaml` at the repo root.
- Emits `typescript_package_confidence: high`.
- Appends discovery evidence lines to the finding.
- `repair_packet_ready` stays `false`.
- `language_status` stays `preview`.

## Must Not

- Resolve `package_root` to the monorepo root.
- Invent a `package_root` from the file extension alone.
- Emit `repair_packet_ready: true`.
- Change any existing field on the finding (additive evidence only).
- Use mutation-runtime outcome vocabulary.
