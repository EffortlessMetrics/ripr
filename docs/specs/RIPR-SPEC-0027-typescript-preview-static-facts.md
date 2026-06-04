# RIPR-SPEC-0027: TypeScript Preview Static Facts

Status: accepted

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

This accepted spec records the TypeScript-family preview contract. Campaign 27
implemented the first useful preview loop, and
[Lane 1 TypeScript Preview Completion](../lanes/LANE_1_TYPESCRIPT_PREVIEW_COMPLETION.md)
plus the
[TypeScript preview completion implementation plan](../../plans/typescript-preview-completion/implementation-plan.md)
track the remaining completion slices. Acceptance does not promote
TypeScript/JavaScript beyond opt-in preview and does not make preview evidence a
default gate, badge, baseline, or RIPR Zero input.

The TypeScript preview adapter is enabled by repo configuration:

```toml
[languages]
enabled = ["rust", "typescript"]
```

When enabled, it routes `*.ts`, `*.tsx`, `*.js`, and `*.jsx` files. It emits
the same RIPR fact families as the Rust adapter. TypeScript files are labeled
`language = "typescript"` and JavaScript files are labeled
`language = "javascript"`; both use `language_status = "preview"`.
JavaScript uses the TypeScript-family adapter implementation, but it remains a
separately labeled JavaScript preview surface unless a future promotion packet
changes that support tier.

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

## TypeScript-Family Fact Vocabulary

TypeScript/JavaScript preview findings and repair packets use these bounded
fields when the adapter has enough syntax-first evidence to populate them:

| Field | Values |
| --- | --- |
| `language` | `typescript`, `javascript` |
| `language_status` | `preview` |
| `owner_kind` | `function`, `method`, `class_method`, `arrow_function`, `component`, `module_function` |
| `test_kind` | `jest_test`, `vitest_test`, `describe_block`, `table_test`, `unknown_test` |
| `assertion_kind` | `exact_value`, `error_path`, `async_resolve`, `async_reject`, `mock_interaction`, `snapshot_weak`, `smoke_weak` |
| `probe_kind` | `predicate`, `return_value`, `error_path`, `field_construction`, `call_side_effect`, `mock_interaction` |
| `static_limit_kind` | `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`, `decorator_indirection`, `mocked_module`, `unsupported_syntax` |
| `repair_kind` | `add_boundary_assertion`, `add_exact_value_assertion`, `add_error_variant_assertion`, `add_mock_argument_assertion`, `strengthen_snapshot_oracle`, `name_static_limitation` |

Only `language`, `language_status`, `owner_kind`, and `static_limit_kind` are
currently language-adapter output fields in RIPR-SPEC-0026. The remaining
fields become public only through later output-schema or repair-packet PRs; they
must not be inferred by consumers before those PRs land.

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

Owner kinds emitted in TypeScript/JavaScript preview facts:

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
- bare `expect(...).toThrow()` / `.rejects.toThrow()` → broad
  error-path oracle
- literal `expect(...).toThrow("...")` / `.rejects.toThrow("...")`
  and safe `.rejects.toMatchObject({ ... })` payloads → exact
  error-variant oracle
- `expect(...).resolves.toBe(...)` → async-aware exact-value oracle
- `expect(mockFn).toHaveBeenCalledWith(...)` and `toHaveBeenCalledTimes`
  → side-effect/call oracle
- `expect(...).toMatchSnapshot()` and `.toMatchInlineSnapshot()` →
  snapshot oracle (weak / static-limited)
- bare `expect(actual).toBeTruthy()` / `toBeFalsy()` /
  `toBeDefined()` → smoke oracle

The 0.8.1 Bun UB advisory lane also permits internal, evidence-only
TypeScript facts for the Blob / ArrayBuffer calibration route: syntactic
`SharedArrayBuffer` construction, resizable `ArrayBuffer` construction through
`maxByteLength`, `ArrayBuffer.resize(...)`, typed-array/DataView views,
view-backed `Blob(...)` input, `blob.arrayBuffer()` observers, stable byte/text
assertions, weak byte/text smoke or snapshot oracles, byte/text read-only
mentions without assertions, and `maxByteLength` mention-only controls. A
bounded internal Bun Blob bridge profile may combine those facts into
evidence-only `configured_hint` / `bridge_unknown` advisory lines for
`Blob::from_js_without_defer_gc` and the
`array_buffer.shared || array_buffer.resizable` Rust boundary. A bounded
cross-language preview projection may surface those lines in the advisory
TypeScript preview card as `rust_ungripped_ts_discriminated`,
`rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`, or
`bridge_unknown` state for the configured Bun Blob route. Missing-discriminator
Bun Blob bridge findings must fail closed as
`cross_language_oracle_visibility_unresolved` until the binding route, external
callsite, and external oracle are proven; their suggested test file remains
`not_applicable` instead of selecting a TypeScript or Rust repair target. These
facts, bridge hints, limitation routes, and card fields do not promote
TypeScript preview evidence into repair packets, generated tests, source edits,
gates, badges, baselines, RIPR Zero, or support-tier claims.
Repositories may opt into a Bun stable-byte review profile with
`[profiles.bun_ub]` in `ripr.toml`; that profile records TypeScript-family
`test_roots` and a repo-relative `bridge_hints` file for advisory operator
workflow only. The profile does not enable TypeScript by itself, does not add a
separate `javascript` language key, and does not execute `tsc`, `tsserver`,
Bun/Jest/Vitest, mutation, generated tests, source edits, gates, badges,
baselines, RIPR Zero, or support-tier promotion.

Related-test heuristics mirror the Rust adapter: changed-owner name match,
import-reference match, file-path proximity, and call-graph proximity at
the syntax level. Direct owner-call matches must be token-aware: a top-level
function owner can match `applyDiscount(...)`, but string/comment mentions and
arbitrary object-method calls such as `order.applyDiscount(...)` must not make
the test related. Method owners may use a bounded receiver relation only when
the test constructs a local receiver with `new ClassName(...)` or a named import
alias for that class and then calls `receiver.method(...)`. Static class-method
owners may use a bounded direct class member relation only when the test calls
`ClassName.method(...)` through the same-file class name or an unshadowed named
import alias. Factory returns, dependency injection, mocked modules, prototype
aliases, namespace chains, and dynamic property access remain advisory or
unsupported.

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

- `dynamic_dispatch` (e.g., computed member calls such as `obj[methodName]` followed by invocation)
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
generated CI behavior and LSP smoke coverage. The Bun Blob / `ArrayBuffer`
preview calibration corpus is also summarized by
`xtask/src/main.rs::tests::bun_ub_calibration_report_summarizes_calibrated_states`
and
`xtask/src/main.rs::tests::bun_ub_calibration_command_writes_markdown_and_json`,
which keep TS-discriminated, missing-discriminator, mention-only, and
bridge-unknown cases advisory and non-repair-ready.

## Implementation Mapping

The first implementation landed under Campaign 27 work item
`analysis/typescript-preview-adapter`. Follow-up completion work belongs to the
TypeScript preview completion lane and should use
`plans/typescript-preview-completion/implementation-plan.md` as the PR queue. No
follow-up may change Rust behavior, preview advisory status, default gates,
badges, baselines, RIPR Zero eligibility, provider behavior, generated tests,
source-edit behavior, or runtime mutation execution unless a later accepted
contract explicitly changes that boundary. `cargo xtask bun-ub-calibration`
is an xtask-only operator report over the existing Bun calibration corpus; it
does not promote TypeScript/JavaScript preview evidence or run TypeScript,
JavaScript, Bun, mutation, provider, generated-test, or source-edit workflows.

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
- `language_adapter_typescript_related_imported_owner_call`
- `language_adapter_typescript_related_method_receiver_call`
- `language_adapter_typescript_probe_predicate`
- `language_adapter_typescript_probe_return_value`
- `language_adapter_typescript_probe_error_path`
- `language_adapter_typescript_probe_field_construction`
- `language_adapter_typescript_probe_call_side_effect`
- `language_adapter_typescript_probe_mock_interaction`
- `language_adapter_typescript_probe_ambiguous_suppressed`
- `language_adapter_typescript_static_limit_mocked_module`
- `language_adapter_typescript_bun_ub_array_buffer_facts`
- `language_adapter_typescript_bun_ub_stable_byte_oracle_facts`
- `language_adapter_typescript_bun_ub_bridge_hints`
- `language_adapter_typescript_bun_ub_cross_language_grip_states`
