# RIPR-SPEC-0027: TypeScript Preview Static Facts

Status: proposed

## Problem

The TypeScript preview adapter must emit RIPR static facts from
TypeScript/JavaScript source without depending on `tsc`, the TypeScript
language server, or any runtime tooling. Syntax-first facts are the
contract; semantic enrichment is explicitly deferred.

This spec defines the per-language behavior the adapter must produce. The
language-neutral boundary, the router, the output metadata, the opt-in
posture, and the cross-language non-goals live in
[RIPR-SPEC-0026: Language adapter contract](RIPR-SPEC-0026-language-adapter-contract.md).
The proposal context is
[RIPR-PROP-0001: Multi-Language Adapter Preview](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md).

## Behavior

The TypeScript preview adapter is enabled by repo configuration:

```toml
[languages]
enabled = ["rust", "typescript"]
```

When enabled, it routes `*.ts`, `*.tsx`, `*.js`, and `*.jsx` files. It
emits the same RIPR fact families as the Rust adapter and tags each
finding with `language = "typescript"` and `language_status = "preview"`.

The adapter is syntax-first. It must not depend on `tsc`, type checking,
or a build graph. When syntax-first analysis cannot classify, the adapter
emits an explicit `static_limit_kind` instead of silently coercing to
`no_static_path`.

## Inputs

- TypeScript or JavaScript source files routed to this adapter.
- Diff spans inside those files.
- Repo configuration including `[languages] enabled` and any future
  TypeScript-specific options layered on top of this spec.

The adapter does not read `tsconfig.json`, `package.json` dependencies,
node_modules contents, generated declaration files, sourcemaps, or
runtime test output.

## Owner Facts

Owners the adapter must recognise:

- `function` declarations (`function name(...) { ... }`)
- arrow functions assigned to a `const`/`let` (`const name = (...) => { ... }`)
- class declarations and class methods
- exported and default-exported variants of the above
- React-ish component functions when obvious (named PascalCase function
  declarations or PascalCase arrow consts returning JSX)
- module-scope `const` initializers that participate in changed behavior
  (when a probe attaches to the initializer expression)

Owner kinds emitted in output (per RIPR-SPEC-0026):

- `function`, `method`, `class_method`, `arrow_function`, `component`,
  `module_function`.

## Test and Assertion Facts

Test discovery:

- `test(...)`, `it(...)`, and `describe(...)` blocks, including nested
  `describe` for hierarchical naming
- Jest/Vitest `test.each`, `it.each`, and table-driven variants when
  syntactically identifiable
- top-level `expect(...)` calls when paired with a `test`/`it` block
- exported test files matched by configured patterns (default:
  `*.test.ts`, `*.test.tsx`, `*.spec.ts`, `*.spec.tsx`, and the
  corresponding `.js`/`.jsx` variants)

Assertions / oracles the adapter must recognise:

- `expect(actual).toBe(expected)` and `.toEqual` / `.toStrictEqual` →
  exact-value oracle
- `expect(...).toThrow(...)` → error-path oracle
- `expect(...).resolves.toBe(...)` and `.rejects.toThrow(...)` →
  async-aware exact/error oracle
- `expect(mockFn).toHaveBeenCalledWith(...)` and `toHaveBeenCalledTimes`
  → side-effect/call oracle
- `expect(...).toMatchSnapshot()` and `.toMatchInlineSnapshot()` →
  snapshot oracle (weak / static-limited)
- bare `expect(actual).toBeTruthy()` / `toBeFalsy()` /
  `toBeDefined()` → smoke oracle

Related-test heuristics mirror the Rust adapter: changed-owner name match,
import-reference match, file-path proximity, and call-graph proximity at
the syntax level. Direct owner-call matches must be token-aware: a top-level
function owner can match `applyDiscount(...)`, but string/comment mentions and
arbitrary object-method calls such as `order.applyDiscount(...)` must not make
the test related.

## Probe Facts

Probes the adapter must generate (syntax-first):

- predicate probes for changed `if`/`else if`/ternary boundary conditions
- return-value probes for changed `return` and tail expressions
- error-path probes for changed `throw` statements and `try`/`catch`/
  `Promise.reject` shapes
- field probes for changed object-literal or class-field assignments
- call probes for changed function and method calls including changes to
  arguments
- mock-interaction probes for changed call surfaces against identifiers
  resolved through a syntactic `vi.fn()` / `jest.fn()` initializer

When the adapter cannot classify, it emits one of the `static_limit_kind`
values defined in RIPR-SPEC-0026:

- `dynamic_dispatch` (e.g., dynamic calls through `obj[methodName]`)
- `metaprogramming` (e.g., decorators applied at the syntax site)
- `missing_import_graph` (the symbol is imported from a module the
  adapter cannot resolve syntactically)
- `mocked_module` (e.g., `vi.mock(...)` / `jest.mock(...)` indirection
  observed)
- `unsupported_syntax` (e.g., TypeScript syntax the parser flags as
  unsupported for this preview)

## Required Evidence

The TypeScript preview contract is supported only when the implementation
can show:

- a fixture corpus pinning at least one example per owner kind above
- a fixture corpus pinning at least one example per oracle kind above
- a fixture corpus pinning at least one example per probe kind above
- a fixture corpus pinning at least one example per `static_limit_kind`
- fixtures cover `*.ts`, `*.tsx`, `*.js`, and `*.jsx`
- a fixture proving `async` `test`/`it` resolves and rejects classify
  correctly
- a fixture proving snapshots are tagged as weak / static-limited
- generated CI fixtures cover TypeScript preview output visible only
  when `[languages]` declares `typescript`
- LSP protocol smoke covers a TypeScript seam diagnostic, hover, code
  action, and evidence packet
- VS Code e2e smoke covers opening a TypeScript file when the adapter is
  enabled
- `cargo xtask dogfood` records a checked TypeScript preview receipt
- the capability matrix gains `TypeScript preview static facts` at
  `alpha`, marked preview, with metrics from RIPR-SPEC-0026 plus
  TypeScript-specific counts where the language adds vocabulary

## Non-Goals

- No type checking, type narrowing, type inference, or `tsc` dependency.
- No `package.json` parsing or dependency-graph resolution.
- No source map consumption.
- No bundler integration (webpack, esbuild, vite, rollup, parcel).
- No Node test runner integration (mocha, jest, vitest) beyond syntax
  pattern recognition.
- No JSX semantic analysis beyond detecting the syntactic component
  shape.
- No mocking framework introspection beyond syntactic recognition.
- No automatic source edits, generated tests, or provider calls.
- No claims of parity with Rust evidence.

## Acceptance Examples

Function with boundary gap:

```ts
export function applyDiscount(amount: number, threshold: number) {
  if (amount >= threshold) {
    return amount * 0.9;
  }
  return amount;
}
```

Existing tests:

```ts
test('discount applies above threshold', () => {
  expect(applyDiscount(100, 50)).toBe(90);
});

test('no discount below threshold', () => {
  expect(applyDiscount(10, 50)).toBe(10);
});
```

Expected static evidence:

- owner: `applyDiscount` (`function`)
- predicate probe: `amount >= threshold`
- oracle: `exact-value` via `.toBe(90)` and `.toBe(10)`
- missing discriminator: `amount === threshold`
- finding emits `language = "typescript"`,
  `language_status = "preview"`, `owner_kind = "function"`

Snapshot oracle:

```ts
test('renders header', () => {
  expect(render(<Header user="ada"/>)).toMatchSnapshot();
});
```

Expected static evidence:

- oracle: `snapshot` (weak)
- finding records snapshot as a weak oracle with the existing snapshot
  exposure class

Dynamic dispatch limit:

```ts
function dispatch(actions: Record<string, () => void>, key: string) {
  actions[key]();
}
```

Expected static evidence:

- probe emits `static_limit_kind = "dynamic_dispatch"`; finding stays
  conservative.

## Test Mapping

Follow-up fixtures and tests cover the owner, test, assertion, related
test, probe, and static-limit cases listed under Required Evidence, plus
generated CI behavior and LSP smoke coverage.

## Implementation Mapping

Follow-up implementation belongs to Campaign 27 work item
`analysis/typescript-preview-adapter`. The boundary, router, repo config,
and additive output metadata land first under RIPR-SPEC-0026 work items.
This spec PR records the per-language contract; no analyzer behavior
changes in the spec PR.

## Metrics

In addition to the cross-language metrics in RIPR-SPEC-0026, the
TypeScript adapter contributes:

- `language_adapter_typescript_findings_preview`
- `language_adapter_typescript_owner_function`
- `language_adapter_typescript_owner_method`
- `language_adapter_typescript_owner_class_method`
- `language_adapter_typescript_owner_arrow_function`
- `language_adapter_typescript_owner_component`
- `language_adapter_typescript_oracle_exact_value`
- `language_adapter_typescript_oracle_error_path`
- `language_adapter_typescript_oracle_side_effect`
- `language_adapter_typescript_oracle_snapshot_weak`
- `language_adapter_typescript_oracle_smoke`
- `language_adapter_typescript_static_limit_mocked_module`
