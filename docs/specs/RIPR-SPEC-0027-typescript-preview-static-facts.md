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

When enabled, it routes `*.ts`, `*.tsx`, `*.mts`, `*.cts`, `*.js`, `*.jsx`,
`*.mjs`, and `*.cjs` files. It emits
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

Source reads use a no-follow open followed by opened-handle regular-file
validation. On supported Linux and macOS targets the open is nonblocking, so
a file replaced with a FIFO between path inspection and open cannot wait for
a writer. Windows opens the reparse point itself and validates the handle;
other targets refuse the guarded open. The FIFO regression must bound its
child process and verify that the intended child test actually executed.

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
- top-level CommonJS assignment exports whose value is a function or arrow
  (#4545): `exports.NAME = ...` and `module.exports.NAME = ...` (owner
  `NAME`), `module.exports = function NAME(...)` or an arrow (the module's
  default-export owner, `NAME` or `default`), and static-identifier function
  properties of `module.exports = { ... }`; non-function values, computed or
  string-literal keys, compound or chained assignments, and nested
  assignments yield no owner; a name that more than one CommonJS export in
  the same file defines (`module.exports = { f }` then
  `module.exports.f = ...`) yields no owner for any of them
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
- mocha BDD `specify(...)` and `context(...)`, and the mocha TDD / Vitest /
  `node:test` `suite(...)`, as test and describe roots with the same active
  modifiers (`.only`, `.concurrent`, `.sequential`); `.skip`, `xit`, and
  `xcontext` register nothing (#4548)
- an options object between the title and the callback
  (`it(name, { timeout }, fn)`, `describe(name, { concurrency }, fn)`); a
  trailing timeout (`test(name, fn, 5000)`) keeps argument 1 as the callback;
  an options object (before or after the callback) whose `skip` / `todo` /
  `fails` key holds anything but literal `false` / `undefined`, or that
  holds a spread, computed key, or method, registers nothing, exactly like
  `.skip`
- a `describe` / `context` / `suite` whose title is not a string literal
  (`describe(Div.name, fn)`, a template literal): its body is walked and the
  suite is named by the computed-title placeholder `<computed title, line N>`
- Jest/Vitest `test.each`, `it.each`, and table-driven variants when
  syntactically identifiable
- `test(...)`, `it(...)`, and `describe(...)` registered inside a `for`,
  `for...of`, or `for...in` loop body or a `.forEach(...)` callback, at file
  or `describe` level. The body is extracted once and relates to owners
  exactly as an ordinary test body does; the loop variables and callback
  parameters shadow every enclosing binding of the same name, including an
  imported owner or class name, so a call through a shadowing binding is not
  an owner call. A loop is walked only when it is known to run at least
  once: its iterable is a non-empty array or object literal,
  `Object.entries`/`keys`/`values` of one, or a `const` whose innermost
  binding in scope is a single declaration of one; a counted `for` needs a
  literal start and bound that admit the first iteration. A test inside any
  other loop (an empty, reassignable, imported or parameter iterable, a
  non-literal bound) could be credited without existing, so that loop is not
  walked and stays disclosed as `typescript_test_extraction_partial`, as does
  a `for...of`/`for...in` whose target is not a declaration.
- a computed title (a template literal with substitutions, a concatenation,
  an identifier) is never evaluated: the test is named by the enclosing
  `describe` names, the placeholder `<computed title, line N>`, and the line
  of the registration. A string literal or a substitution-free template keeps
  its text.
- top-level `expect(...)` calls when paired with a `test`/`it` block
- exported test files matched by configured patterns (default:
  `*.test.ts`, `*.test.tsx`, `*.spec.ts`, `*.spec.tsx`, and the
  corresponding `.js`/`.jsx` variants). Every routed source extension
  participates, so the modern ESM/CJS suffixes `.mts`, `.cts`, `.mjs`, and
  `.cjs` are test files by the same `.test`/`.spec` convention rather than
  an unrouted gap.

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
- assertion libraries reached through a binding the test file imports from
  `assert`, `node:assert`, `assert/strict`, `node:assert/strict`, or `chai`
  (ESM import, top-level `require(...)`, or `require('chai').expect`; #4547):
  `assert.strictEqual` / `deepStrictEqual` → exact-value oracle; `equal` /
  `deepEqual` → exact-value only in strict mode (bound from `assert/strict`,
  `node:assert/strict`, or the `strict` export) and relational under legacy
  `node:assert`, whose `==` comparison is loose; chai `assert.equal` (loose
  `==`) → relational and chai `assert.deepEqual` (strict deep equality) →
  exact-value; `notStrictEqual` / `notDeepStrictEqual` / `notEqual` /
  `notDeepEqual` / `match`, `node:assert` `doesNotMatch`, and chai `notMatch`
  / `include` / `notInclude` / `lengthOf` → relational; `ok`, chai `isTrue` /
  `isFalse` / `isOk` / `isNotOk` / `isNull` / `isUndefined` / `isDefined`,
  and the bare callable `assert(value)` → smoke; `throws` / `doesNotThrow`
  and `node:assert` `rejects` / `doesNotReject` → broad error-path oracle. A
  method the bound API does not have (a chai-only method on `node:assert`, or
  the reverse) is not credited. A named method import (`strictEqual(a, b)`)
  maps the same way as the module it comes from. A binding re-declared in the
  test body, as a test or describe callback parameter, or in an enclosing
  describe body is shadowed and not credited; a first test-callback
  parameter with that name is not read as an AVA / tape receiver either.
  chai
  `expect(x).to.equal(y)` / `.to.eql(y)` /
  `.to.deep.equal(y)` → exact-value (relational under `.not`);
  `.to.be.true` / `.false` / `.ok` / `.null` / `.undefined` → smoke;
  `.to.throw(...)` → broad error; `.include` / `.contain` / `.match` /
  `.above` / `.below` / `.lengthOf` → relational. The observed expression is
  the first (actual) argument. A same-named local helper and a Jest/Vitest
  `expect` are never read as these libraries, and unrecognised methods or
  chain words are not credited

The 0.8.1 Bun UB advisory lane also permits internal, evidence-only
TypeScript facts for the configured Bun bridge calibration routes: syntactic
`SharedArrayBuffer` construction, resizable `ArrayBuffer` construction through
`maxByteLength`, `ArrayBuffer.resize(...)`, typed-array/DataView views,
view-backed `Blob(...)` input, `blob.arrayBuffer()` observers, stable byte/text
assertions, weak byte/text smoke or snapshot oracles, byte/text read-only
mentions without assertions, and `maxByteLength` mention-only controls. A
bounded internal Bun Blob bridge profile may combine those facts into
evidence-only `configured_hint` / `bridge_unknown` advisory lines for
`Blob::from_js_without_defer_gc`, `copy_to_unshared`, and
`MarkdownObject::to_string` owners, together with their configured Rust
boundaries. A bounded cross-language preview projection may surface one
coherent configured profile in the advisory
TypeScript preview card as `rust_ungripped_ts_discriminated`,
`rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`, or
`bridge_unknown` state for the selected configured Bun route. When multiple
profiles share related evidence, the card must keep the hint, verdict, grip,
and placement from one profile rather than mix fields across profiles.
Missing-discriminator Bun bridge findings must fail closed as
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
import alias. Dependency injection, mocked modules, prototype aliases,
namespace chains, and dynamic property access remain advisory or unsupported.

A top-level function owner may also relate through a same-module entry: an
exported name of the owner's module whose code reaches the owner within three
call edges. An edge is a bare call to a top-level declaration name, and only
when every mention of that name in the enclosing function is a bare call: a
parameter (plain, destructured, or of a nested callback), a local
declaration, an assignment or a value use may rebind it, so no edge is
recorded. A function's own edges exclude the functions it directly returns.
A value built by a same-module factory (`export const defu = createDefu()`,
with `as`/`satisfies`/non-null/parentheses stripped) calls what the factory's
directly returned function calls, checked against the factory's whole text so
a captured factory parameter or local shadows too; calling the factory itself
does not reach them, and a factory imported from another module is not
followed. Named, listed (`export { local as name }`) and default exports,
including an anonymous default function or arrow, count; type-only exports and
re-exports from other modules do not. The test's call of
the entry must pass every identity gate a direct or imported owner call does.
The relation is `helper_owner_call` with `medium` confidence and is admitted
only when no owner-call, import-call, receiver, class-method, module-observer,
or re-export relation exists, so it never mixes with a relation that calls the
owner. Reach only through entries is at most `weakly_exposed`, and the
missing-evidence line names the entries the related tests call.

A test is related to an owner only when the test references the owner. When
no owner-call, import-call, receiver, class-method, module-observer, or
re-export relation exists, the same-stem, `describe(...)`-name, and
test-name heuristics may relate a test only if its body references the owner
without a recognized call shape: the bare owner identifier outside comments,
strings, object-literal keys, and member positions and not shadowed by a
local declaration; a named-import local of the owner; or a namespace member
`ns.owner` whose import resolves to the owner module. Such a link stays
`weakly_exposed` with `actionability_category = ambiguous_related_test`. A
test that only names the owner in its title, its `describe(...)` title, or its
file stem, or that only calls a sibling owner from the same module that is not
a same-module entry reaching the owner, is not related: when no test references the owner, the finding is `no_static_path`
with reach `no` and the missing reference named (`No test references
\`owner(\``).

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

Added lines that are blank, comment-only, or punctuation-only (`}`, `});`,
`)`) produce no probe. Neither do lines inside a top-level ambient
declaration found in the parsed syntax tree (`declare function`,
`export declare const`, `declare module 'x' { ... }`, `declare global`), or
any line of a `.d.ts`/`.d.mts`/`.d.cts` declaration file: these are type-only
and erased at compile time. Declaration files still count as changed files.
`declare` used as a runtime identifier, or at the start of a template-literal
line, is still probed. An unrecognised line keeps the non-specific predicate
fallback and is never classified `exposed`.

A changed line whose in-place removed counterpart differs only in erased
TypeScript type syntax also produces no probe (#4282). Both sides must parse
as a single-line function, method, or variable declaration, and their runtime
parts must match exactly: names, parameter patterns, default values, parameter
properties, `async`/generator, export shape, and body or initializer text. A
line with a decorator, a constructor line, and a method line in a file that
uses decorators keep their probe: under `emitDecoratorMetadata` decorated
parameter and return types are emitted as runtime metadata. Return types, parameter and variable annotations, optional
markers, generic parameter lists, and `this` parameters are ignored. Type
syntax inside an expression (`as`, `satisfies`, generic call arguments) and
multi-line signature fragments are not compared and keep their probe.

The opening line of a new function, method, or arrow owner (an added line with
no removed counterpart, starting an owner whose span holds another added,
probe-eligible line) produces no probe: its behavior is carried by the body
lines. A default value, destructuring default, parameter property, computed
key, decorator, constructor, one-line body, or changed signature keeps its
probe.

A changed predicate is `exposed` only when a strong, family-matching
assertion's observed expression (`expect(<expr>)`) calls the owner at the
changed boundary: an argument carries the literal operand (`total >= 50` needs
a call such as `shippingFee(50)`), or, when neither operand is a literal, the
call passes two identical arguments or an object-literal argument binds both
operands to the same value. Predicates without a comparable boundary (such as
`Number.isNaN(n)`) and boundaries reached only through locals or computed
values stay `weakly_exposed`; the literal or operand-pair boundary becomes the
missing discriminator when one is known.

Three one-hop input resolutions credit the idiom shapes that reach the same
boundary without changing the guards (#4104 E):

- **Local-binding idiom**: a bare-local `observed_expression` whose single
  `const`/`let` initializer IS the owner call (`const result =
  applyDiscount(100); expect(result).toBe(90)`) witnesses through the
  initializer's arguments — position, arity, standalone-literal, and
  anchoring guards unchanged. The declaration must be the binding visible at
  the assertion (the same lexical scope walk as the shadow guard: a
  top-level declaration binds the whole body; a declaration inside a nested
  block binds only that block, so an outer assertion is never attributed to
  a nested helper's same-named `const`). A wrapper around the call, a
  derived value, a `let` reassigned after the call (plain or compound
  assignment, or an increment/decrement), or a twice-declared name stays
  fail-closed.
- **Operand shapes the comparison parser previously rejected**: `?.`
  normalizes to member access and `??` reads as the nullish boundary
  comparison (quote-aware, so literal contents never normalize — and a
  quoted `??` never arms the nullish-input path); a `yield <comparison>`
  tail classifies as a predicate and strips the keyword. For a nullish
  boundary the nullish input (`null`/`undefined`) at the left operand's read
  position is the ONLY creditable boundary input: the right-hand fallback
  literal is never creditable (a non-nullish call never evaluates it and
  behaves identically before and after), and parenthesized compound forms
  and call-shaped tails stay fail-closed.
- **Named constants**: an UPPER_CASE comparison operand resolves through a
  single immutable integer module-level `const` in the owner's own module
  (the recorded owner span first, then the owner file through the workspace
  root; a same-name declaration at any non-top-level scope is a shadow the
  changed read may observe, so resolution fails closed — the operand keeps
  its name and the file-level fallback never runs), and an UPPER_CASE
  argument resolves through the test body or — via the import record — a
  single such declaration in the owner's own module, mirroring the Rust
  `value_resolution::named_constant` strictness. `let`/`var`, computed
  initializers, repeated declarations, off-value constants, and constants
  imported from a module the adapter cannot resolve to the owner stay
  fail-closed.
- **Boundary input evidence**: for a changed predicate with a named missing
  discriminator `<param> == <operand>`, the adapter emits
  `typescript_boundary_input: parameter=<param>;index=<i>;operand=<operand>;value=<v>`
  only when, on the oxc token stream of the owner's module, `<param>` is the
  owner's plain positional parameter at index `<i>` and every later
  occurrence in the owner is a plain read (no write, update, redeclaration,
  nested parameter, catch, loop, or destructuring binding; no `arguments`,
  `eval`, `with`, or escaped identifier), and `<operand>` is a plain decimal
  integer literal or an UPPER_CASE name bound exactly once, at the module top
  level, by a non-`declare` `const` with an integer literal initializer, whose
  every other occurrence in the module is a plain read. Anything else emits
  nothing.
- **Boundary parameters evidence** (#4759): when both sides of that
  discriminator are distinct plain positional parameters of the owner
  (`amount == threshold`), the adapter instead emits
  `typescript_boundary_parameters: parameter=<left>;index=<i>;operand=<right>;operand_index=<j>`
  under the same module rules, applied to each parameter: each is read-only
  in the owner, the changed line compares them as whole sides, and it runs on
  every call. No value is claimed; the call's own arguments at `<i>` and
  `<j>` decide whether it hits the boundary.

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
- fixtures cover `*.ts`, `*.tsx`, `*.js`, and `*.jsx`; the modern ESM/CJS
  suffixes (`.mts`, `.cts`, `.mjs`, `.cjs`) are covered by adapter routing and
  module-identity tests, plus end-to-end `analyze_diff` discovery and
  oracle-credit tests for `.mts` and `.cts` only — not by golden fixtures
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
- the preview repair-loop corpus can opt into semantic assertions that require
  an operator verify command and receipt command while also requiring the source
  fixture to withhold packet-ready verify evidence (`typescript_verify_command`
  and preview-card `verify.command`) until a complete repair-packet contract
  exists
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

Untested sibling owner in a tested module:

```ts
// src/pricing.ts
export function discountedTotal(amount: number) { /* tested */ }
export function loyaltyPrice(amount: number, years: number) {
  if (years >= 5) {
    return amount - Math.floor(amount / 20);
  }
  return amount;
}

// test/pricing.test.ts — never references loyaltyPrice
import { discountedTotal } from "../src/pricing";
it("no discount below threshold", () => {
  expect(discountedTotal(5000)).toBe(5000);
});
```

Expected static evidence:

- changed `loyaltyPrice` lines classify `no_static_path` with
  `reach no: 0 related test(s) found for owner \`loyaltyPrice\``;
- no related test is listed, and the missing line names the absent
  reference (`No test references \`loyaltyPrice(\``);
- the same file still relates both tests to `discountedTotal`, which they call.

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
