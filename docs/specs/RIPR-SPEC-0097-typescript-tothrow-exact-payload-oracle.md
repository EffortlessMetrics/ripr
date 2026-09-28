# RIPR-SPEC-0097: TypeScript toThrow Exact-Payload Oracle Upgrade

Status: accepted

## Problem

`toThrow('message')`, `toThrow({ code: 'X' })`, and `toThrow(ErrorClass)` all
assert the EXACT thrown error -- they discriminate the changed error variant.
Before this spec, ripr treated every `toThrow` form as a broad/smoke oracle
(`BroadError` / weak strength) regardless of whether a payload was present. A
test with `expect(() => fn()).toThrow('exact message')` was indistinguishable
from a bare `expect(() => fn()).toThrow()`, so a real discriminating test kept
a finding at `weakly_exposed` instead of promoting it to `exposed`.

This is a honesty gap: the upgrade from `weakly_exposed` to `exposed` is
HONESTY-SAFE because it only credits syntax-visible, in-source, exact payloads.
No runtime inference, no type inference, no behavior assumption.

## Behavior

### Exact-payload forms that upgrade to ExactErrorVariant / Strong

When a `toThrow` or `toThrowError` call carries exactly ONE argument and that
argument is a concrete, statically-resolvable payload, the oracle MUST be
classified as `ExactErrorVariant` / `Strong`:

**String literal**: `expect(() => fn()).toThrow("exact message")`

The argument is an `Argument::StringLiteral`. Record the string text as
`oracle_payload`. No dynamic evaluation, no variable resolution.

**All-literal object**: `expect(() => fn()).toThrow({ code: "ENOENT", message: "not found" })`

The argument is an `Argument::ObjectExpression` where EVERY property:
- has a static (non-computed) key,
- has no shorthand binding (`{ code }` is REJECTED -- fail-closed),
- has no spread element,
- has a value that is one of: string literal, number literal, boolean
  literal, or null literal.

**PascalCase constructor reference**: `expect(() => fn()).toThrow(TypeError)`

The argument is any expression that:
- parses as a safe JavaScript member path (identifier or dotted path, no
  computed access, no call expressions),
- and whose FIRST segment starts with an ASCII uppercase letter.

The uppercase-first gate is the fail-closed guard that prevents promoting
`.toThrow(message)` (a camelCase variable reference) to ExactErrorVariant.
We cannot distinguish class references from variable references via AST
alone; PascalCase is the conventional signal for error class names.

### Async variants (`rejects`)

The same three-form upgrade applies to `await expect(...).rejects.toThrow(arg)`
and `await expect(...).rejects.toThrowError(arg)`.

### Fail-closed cases that MUST stay BroadError / Weak

- Bare `.toThrow()` or `.toThrowError()` (no argument): stays `BroadError` / weak.
- `.toThrow(dynamicVar)` where `dynamicVar` is a camelCase identifier:
  stays `BroadError` / weak (cannot confirm it is a class, not a variable).
- `.toThrow({ code })` (shorthand property -- value not a literal):
  stays `BroadError` / weak.
- `.toThrow(someCall())` or any call-expression argument: stays `BroadError` / weak.
- `.toThrow(someVar ?? fallback)` or any composite expression: stays `BroadError` / weak.

### TypeScriptErrorPayloadKind variants added

- `ThrowsLiteral` (already existed): `.toThrow("string")`.
- `ThrowsObject` (new): `.toThrow({ key: "value" })` with all-literal properties.
- `ThrowsClass` (new): `.toThrow(PascalCaseRef)`.
- `RejectsThrowLiteral` (already existed): `.rejects.toThrow("string")`.
- `RejectsThrowObject` (new): `.rejects.toThrow({ key: "value" })`.
- `RejectsThrowClass` (new): `.rejects.toThrow(PascalCaseRef)`.

### Support tier

TS stays PREVIEW-tier (advisory). This spec changes the exposure CLASS
within preview (`weakly_exposed` → `exposed` when an exact `toThrow` payload
is present) but does NOT change the support tier or emit actionable repair
packets.

## Required Evidence

- Fixture `typescript_tothrow_exact_oracle`: owner changed, tests use all
  three exact-payload forms → must produce `exposed`,
  `oracle_kind: exact_error_variant`.
- Fixture `typescript_broad_tothrow` (existing control): bare `.toThrow()`
  → must stay `weakly_exposed` (BroadError / weak). Must NOT flip to exposed.
- Unit tests:
  - `extract_tests_maps_object_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_class_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_dotted_class_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_keeps_lowercase_ident_tothrow_broad` (fail-closed control)
  - `extract_tests_keeps_dynamic_object_tothrow_broad` (fail-closed control)
  - `extract_tests_maps_class_rejects_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_object_rejects_tothrow_to_exact_error_variant_oracle`

## Non-Goals

- Upgrading `.toThrow(dynamicExpression)` or template-literal arguments.
- Type inference to distinguish class references from variable references.
- Runtime validation of whether the exact payload matches the actual thrown error.
- Emitting actionable repair packets from TypeScript preview findings
  (TS stays advisory-only, no repair packet regardless of oracle strength).
- Changing the TypeScript support tier (stays PREVIEW, advisory).
- Adding a JSON schema version bump (no new output fields; `exact_error_variant`
  is an existing `OracleKind` value).

## Honesty Bar

The upgrade is ONLY safe for in-source, syntax-visible exact payloads. The
three fail-closed gates (bare toThrow, camelCase identifier, shorthand object
property) ensure we never upgrade a case where the test does not visibly
specify the exact error variant.

## Acceptance Examples

### Exact string payload (must produce exposed)

```
owner:          parseUser in src/parser.ts, changes error-path condition
test:           expect(() => parseUser("")).toThrow("empty user")

Before spec:    weakly_exposed, BroadError / weak oracle
After spec:     exposed, ExactErrorVariant / strong oracle,
                oracle_payload: "empty user"
```

### Exact object payload (must produce exposed)

```
owner:          parseUser in src/parser.ts
test:           expect(() => parseUser("")).toThrow({ code: "EMPTY_INPUT" })

After spec:     exposed, ExactErrorVariant / strong oracle,
                oracle_payload: { code: "EMPTY_INPUT" }
```

### PascalCase class reference (must produce exposed)

```
owner:          parseUser in src/parser.ts
test:           expect(() => parseUser("")).toThrow(ParseError)

After spec:     exposed, ExactErrorVariant / strong oracle,
                oracle_payload: ParseError
```

### Bare toThrow control (must stay weakly_exposed)

```
owner:          parseUser in src/parser.ts
test:           expect(() => parseUser("")).toThrow()

After spec:     weakly_exposed, BroadError / weak oracle (unchanged)
```

### camelCase identifier control (must stay weakly_exposed)

```
owner:          parseUser in src/parser.ts
test:           expect(() => parseUser("")).toThrow(message)

After spec:     weakly_exposed, BroadError / weak oracle (unchanged)
                (cannot confirm `message` is a class, not a variable)
```

## Test Mapping

- `fixtures/typescript_tothrow_exact_oracle/` — golden fixture proving all three
  exact-payload forms produce `exposed` with `oracle_kind: exact_error_variant`.
- `fixtures/typescript_broad_tothrow/` — existing golden control: bare `.toThrow()`
  must stay `weakly_exposed` after this spec (classification must not change).
- `crates/ripr/src/analysis/language/typescript/tests.rs`:
  - `extract_tests_maps_object_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_class_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_dotted_class_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_keeps_lowercase_ident_tothrow_broad` (fail-closed control)
  - `extract_tests_keeps_dynamic_object_tothrow_broad` (fail-closed control)
  - `extract_tests_maps_class_rejects_tothrow_to_exact_error_variant_oracle`
  - `extract_tests_maps_object_rejects_tothrow_to_exact_error_variant_oracle`

## Implementation Mapping

- `crates/ripr/src/analysis/language/typescript/types.rs`:
  - `TypeScriptErrorPayloadKind::ThrowsObject` (new variant)
  - `TypeScriptErrorPayloadKind::ThrowsClass` (new variant)
  - `TypeScriptErrorPayloadKind::RejectsThrowObject` (new variant)
  - `TypeScriptErrorPayloadKind::RejectsThrowClass` (new variant)
  - `TypeScriptErrorPayload::oracle_text()` updated to cover all variants.
- `crates/ripr/src/analysis/language/typescript/oracle.rs`:
  - `safe_error_class_payload_text()` — new helper: extracts PascalCase member
    path from an identifier/dotted-member argument; returns `None` for camelCase
    (fail-closed).
  - `error_payload_from_assertion()` — expanded: tries string literal, then
    all-literal object, then PascalCase class ref (in that priority order)
    for both sync `toThrow` and async `.rejects.toThrow`.

## Metrics

- `tothrow_exact_payload_upgrades_to_exact_error_variant`: fixture
  `typescript_tothrow_exact_oracle` produces `exposed` with
  `observe.summary` containing `exact_error_variant` (validated by
  `cargo xtask fixtures typescript_tothrow_exact_oracle`).
- `tothrow_bare_stays_weakly_exposed`: fixture `typescript_broad_tothrow`
  produces `weakly_exposed` with `oracle_kind: broad_error` (validated by
  `cargo xtask fixtures typescript_broad_tothrow`).
