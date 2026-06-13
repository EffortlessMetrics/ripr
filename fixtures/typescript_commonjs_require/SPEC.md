# Fixture: typescript_commonjs_require

Spec: RIPR-SPEC-0085

## Given

A TypeScript production module (`src/format.ts`) changes a predicate. The
related test file (`tests/format.test.js`) uses the CommonJS form:

```js
const { formatAmount } = require('../src/format');
```

This is a destructured-binding `require()` call — not an ES module import.

## When

```bash
cargo xtask fixtures typescript_commonjs_require
```

or:

```bash
ripr check \
  --root fixtures/typescript_commonjs_require/input \
  --diff fixtures/typescript_commonjs_require/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter (RIPR-SPEC-0085 §PR6):

- finds the `formatAmount` owner in `src/format.ts`,
- finds the `require('../src/format')` destructuring in `tests/format.test.js`
  and treats it as an import resolving to `src/format`,
- selects `format.test.js` as a related test with an `ImportedOwnerCall`
  or `DirectOwnerCall` relation,
- extracts the `expect(formatAmount(1.5, 2)).toBe('1.50')` assertion.

## Must Not

- Ignore CommonJS `require()` destructured bindings.
- Claim `repair_packet_ready: true`.
- Use dynamic imports or factory-returns as ownership evidence.
