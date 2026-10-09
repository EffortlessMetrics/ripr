# Fixture: typescript_reexport_directory_star_barrel

Spec: RIPR-SPEC-0095

## Given

A TypeScript preview workspace changes `withoutBase` in `src/utils.ts`.

The package entry `src/index.ts` is a star barrel:

```typescript
export * from './url';
export * from './utils';
```

The test imports through the DIRECTORY specifier, as unjs/ufo's
`test/base.test.ts` does:

```typescript
import { withoutBase } from '../src';
```

## When

```bash
cargo xtask fixtures typescript_reexport_directory_star_barrel
```

## Then

The TypeScript preview adapter:

- resolves `../src` to `src/index.ts` (no `src.ts` file module exists);
- follows the `export * from './utils'` hop because `src/utils.ts` itself
  exports `withoutBase` and no other star source exports that name;
- credits both tests with `relation_reason: re_export_chain_followed`
  (`relation_confidence: medium`), so the changed return is not reported as
  `no_static_path`.

## Purpose

Regression fixture for the unjs/ufo false `no_static_path` (commits eb29945
and 5cd9e67 import `withBase`/`withoutBase` from `../src`). Before the fix the
directory specifier resolved to module `src`, which matched no barrel, so every
changed line reported `no_static_path` with 0 related tests.

## Must Not

- Report `no_static_path` or 0 related tests for the changed owner.
- Credit through the star barrel a name the forwarded module does not export
  (covered by `typescript_reexport_barrel_other_name` and the
  `reexport_chain_tests` unit tests).
