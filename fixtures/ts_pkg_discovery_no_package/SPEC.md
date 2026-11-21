# Fixture: ts_pkg_discovery_no_package

Spec: RIPR-SPEC-0085

## Given

A TypeScript workspace with NO `package.json` anywhere in the tree:

- NO `package.json`, `pnpm-workspace.yaml`, lockfile, or any other
  package manifest is present.
- `src/utils.ts` contains the `clamp` function.
- `tests/utils.test.ts` contains a test referencing `clamp`.
- `ripr.toml` enables the TypeScript preview adapter.

## When

```bash
cargo xtask fixtures ts_pkg_discovery_no_package
```

or:

```bash
ripr check \
  --root fixtures/ts_pkg_discovery_no_package/input \
  --diff fixtures/ts_pkg_discovery_no_package/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds `clamp` as the owner in `src/utils.ts`.
- Finds the test in `tests/utils.test.ts`.
- Fails closed: emits the named limitation
  `typescript_package_root_unresolved` because no `package.json` is found.
- Does NOT emit a guessed `typescript_package_root` line.
- Does NOT infer a `framework_hint` or `runner_hint` from the file
  extension alone.
- `repair_packet_ready` stays `false`.
- `language_status` stays `preview`.

## Must Not

- Invent a `package_root` from the file extension alone.
- Emit `repair_packet_ready: true`.
- Emit `typescript_package_root:` when no manifest is found.
- Change any existing field on the finding (additive evidence only).
- Use mutation-runtime outcome vocabulary.
