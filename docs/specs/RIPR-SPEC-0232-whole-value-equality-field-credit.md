# RIPR-SPEC-0232: Whole-value equality credit for constructed fields

Status: proposed

Owner: product / analysis

Created: 2026-10-04

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- No tier change. A Rust `field_construction` finding can read `exposed` when
  a related test compares the owner's whole result against a literal that
  names the changed field, and the type's equality is the derived one. The
  `whole_object_equality` oracle kind stops being assigned from a `{`
  character alone. Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No schema version bump.

## Problem

Comparing a whole returned value is the most common way Rust tests pin a
constructor:

```rust
pub fn build(n: u32) -> Config { Config { retries: n + 1, name: "x".into() } }

#[test]
fn builds() {
    assert_eq!(build(3), Config { retries: 4, name: "x".into() });
}
```

The diff changed `retries: n + 2` to `retries: n + 1`, and any wrong value
for `retries` fails this test. ripr reports one
`field_construction` finding and reads it `weakly_exposed`. The oracle is
classified `whole_object_equality` (strong), reveal matches the `retries`
token, and then activation keeps a `FieldValue` missing discriminator because
the assertion neither contains `retries: n + 1` nor reads `.retries` on the
owner result (`analysis/classify/activation.rs`,
`missing_field_value_discriminator` and `reads_owner_result_field`). The
evidence classifier downgrades the finding to weak. A real, ordinary pin is a
false gap. The "Fill spec-defined corpus cases" ledger crate reproduces it
with `Header { kind: 7, len: 3 }` against mutant truth.

RIPR-SPEC-0005 already says credit requires that "the oracle structurally
names the exact constructed field through a member access or record
field/pattern". A struct literal naming the field is a record field, so the
current behavior under-delivers that spec.

The opposite risk sits in the oracle classifier. Any `assert_eq!` or
`assert_ne!` line containing `{` is classified `whole_object_equality` with
strong strength (`analysis/extract/oracles/classify.rs` and
`patterns.rs`, `is_whole_object_equality_assertion`). A brace in a format
string or a closure qualifies. No spec governs that rule, and a derived
`PartialEq` is never checked, so a manual `impl PartialEq` that ignores the
changed field would be credited the moment field credit is added.

## Behavior

### Whole-value oracle

An assertion is `whole_object_equality` only when the parser shows an
`assert_eq!` or `assert_ne!` operand that is a struct literal, or a single
tuple-variant constructor (`Ok`, `Some`, `Err` or an enum tuple variant)
wrapping one struct literal. Braces anywhere else do not make the kind.
Assertions that no longer qualify fall to the kind their other text earns.

### Field credit

A changed `field_construction` probe for field `f` of type `T` in a struct
literal the owner returns is confirmed by a related test when all of these
hold:

1. One `assert_eq!` operand is the owner's call, admitted by the same call
   identity and execution gates as RIPR-SPEC-0197, or an immutable `let`
   binding in the same test whose initializer is that call, with no
   `&mut` borrow, shadowing or interior-mutability write of the binding
   between the call and the assertion. A `let mut` binding gives no credit.
2. The other operand is a struct literal of `T` (wrapped at most once as
   above, matching any wrapper on the owner's return; a wrapper other than
   `Ok`, `Some` or `Err` must itself be a workspace enum that meets rule 3)
   that names `f`
   explicitly with an independent expected value: an expression built only
   from literals, constants, standard-library or prelude constructors and
   conversions on them, and workspace struct, tuple-struct or enum-variant
   literals whose leaves are themselves admissible (`4`, `MAX`, `"x".into()`,
   `Some(4)`, `Retries(4)`, `Retries { n: Count(4) }`). An expected value
   that names any local binding or calls any function of the workspace
   (including a constructor function such as `Retries::new(4)`, and a
   `From`/`Into` conversion or `Default` whose impl is in the workspace)
   gives no credit, because it can carry the owner's result
   (`retries: c.retries`, `let r = c.retries; .. retries: r`,
   `retries: build(3).retries`). Functional update (`..base`) is not
   admissible at any nesting depth.
3. `T` is a workspace type with a visible `#[derive(PartialEq)]` and no
   manual `impl PartialEq for T` in the workspace.
4. The type of `f` compares by value: a primitive, `String`, `&str`, or a
   standard collection or `Option`/`Result` of such types, or a workspace type
   that itself meets rule 3 and whose own field types meet this rule,
   recursively. Any manual `PartialEq` anywhere in that type tree refuses
   credit. No attribute on `f`, `T` or a type in that tree changes equality
   (for example `educe` or `derivative` ignore attributes).

Then activation clears the `FieldValue` missing discriminator for `f` and the
finding may read `exposed`, subject to every other stage.

### No credit

The finding does not read `exposed` from whole-value equality when:

- the literal omits `f` through `..base` or `..Default::default()`;
- the expected value comes from a separate binding or a helper
  (`let e = Config { .. }; assert_eq!(c, e)`, `assert_eq!(build(3), expected())`);
  it stays `observation_unverified`;
- the compared call is not the owner;
- the assertion is `assert_ne!`: it keeps the `whole_object_equality` kind
  but gives no field credit, because inequality with one wrong value does
  not pin the right one;
- `T` or the type of `f` has a manual `PartialEq`, comes from outside the
  workspace, or its equality cannot be seen (aliases, generics,
  macro-generated types, equality-changing attributes).

A single-field read of the changed field (`assert_eq!(c.retries, 4)`) keeps
crediting as today. A read of a sibling field (`assert_eq!(c.name, "x")`)
keeps the `FieldValue` missing discriminator.

### Decisions for the owner

1. **Credit whole-value literals at all.** Recommended: yes, as above. This is
   what RIPR-SPEC-0005's "record field/pattern" wording implies.
2. **Wrapper depth.** Recommended: one level (`Ok(T { .. })`). Alternative:
   none, which keeps `parse()`-style tests as gaps.
3. **Equality gate.** Recommended: derived `PartialEq` on `T` and on the
   field's type, fail closed otherwise. When the gate refuses, the finding
   keeps its `FieldValue` missing discriminator and stays a gap
   (`weakly_exposed`); the #5416 unknown-not-a-gap rule 3 does not withhold
   it, because a missing discriminator is named. Alternative: a new
   withholding route with its own `static_limit_kind`, so a refused equality
   gate reads as an analyzer limit instead of a gap.

## Required Evidence

- The reproduction above reads `exposed`.
- The manual-`PartialEq` control (equality ignores `retries`) does not read
  `exposed`.
- The `..Default::default()` control, the separate-binding control and the
  non-owner control do not read `exposed`.
- A brace inside a format string in an `assert_eq!` message does not produce
  `whole_object_equality`.
- Golden drift: every finding that gains `exposed` names a literal with the
  changed field; every finding that loses `whole_object_equality` is listed
  with the kind it now carries.

## Non-Goals

- No reasoning about `PartialEq` bodies.
- No credit for `Debug`, `Display` or snapshot comparisons (other authorities).
- No change to `return_value` owner-return pins (RIPR-SPEC-0197).
- No credit for nested struct literals more than one level deep.

## Acceptance Examples

The diff changes `retries: n + 2` to `retries: n + 1` in `build`; `Config` derives
`PartialEq` unless stated.

1. `assert_eq!(build(3), Config { retries: 4, name: "x".into() })`: `exposed`.
2. Same, with `impl PartialEq for Config { fn eq(&self, o: &Self) -> bool { self.name == o.name } }`:
   not `exposed`.
3. `assert_eq!(parse("3"), Ok(Config { retries: 4, name: "x".into() }))`,
   `parse` the owner: `exposed`.
4. `let c = build(3); assert_eq!(c, Config { retries: 4, name: "x".into() })`: `exposed`.
5. `let c = build(3); let e = Config { retries: 4, name: "x".into() }; assert_eq!(c, e)`:
   `weakly_exposed`, `observation_unverified`.
6. `assert_eq!(build(3), expected())`: `weakly_exposed`.
7. `assert_eq!(build(3), Config { name: "x".into(), ..Config::default() })`: not `exposed`.
8. `assert_eq!(other(), Config { retries: 4, name: "x".into() })`, `other` not the owner: not `exposed`.
9. `let c = build(3); assert_eq!(c.name, "x")`: `weakly_exposed`, `FieldValue`
   missing discriminator kept.
10. `assert_ne!(build(3), Config { retries: 9, name: "x".into() })`: `weakly_exposed`, no field credit.
11. The owner writes `retries: Retries(n + 1)`; `retries` of type `Retries(u32)` with `impl PartialEq for Retries { fn eq(&self, _: &Self) -> bool { true } }`,
    test `assert_eq!(build(3), Config { retries: Retries(4), name: "x".into() })`:
    not `exposed`.
12. `let c = build(3); assert_eq!(c, Config { retries: c.retries, name: "x".into() })`:
    not `exposed`.
13. `let mut c = build(3); c.retries = 4; assert_eq!(c, Config { retries: 4, name: "x".into() })`:
    not `exposed`.
14. The changed literal is in `wrap(n)`, the owner, which returns
    `Wrapped::One(Config { retries: n + 1, name: "x".into() })`; `Wrapped`
    has `impl PartialEq for Wrapped { fn eq(&self, _: &Self) -> bool { true } }`;
    test `assert_eq!(wrap(3), Wrapped::One(Config { retries: 4, name: "x".into() }))`:
    not `exposed`.
15. `let c = build(3); let r = c.retries; assert_eq!(c, Config { retries: r, name: "x".into() })`:
    not `exposed`.
16. The owner writes `retries: Retries { n: Count(n + 1) }`; `retries` of type `Retries { n: Count }` where only
    `Retries` derives `PartialEq` and `Count(u32)` has the manual
    `impl PartialEq for Count { fn eq(&self, _: &Self) -> bool { true } }`,
    test `assert_eq!(build(3), Config { retries: Retries { n: Count(4) }, name: "x".into() })`:
    not `exposed`. The same test with `#[derive(PartialEq)]` on `Count` in place of the manual impl reads
    `exposed`.

## Test Mapping

- Existing: `fixtures/observation_verified_field_construction`,
  `fixtures/observation_unverified_field_construction`,
  `fixtures/rust_field_construction_token_coincidence`.
- Planned: one fixture or verdict-corpus case per acceptance example.
- Planned: oracle classifier unit tests for braces outside struct-literal
  operands.

## Implementation Mapping

- `crates/ripr/src/analysis/extract/oracles/classify.rs` and `patterns.rs`:
  parser-backed `whole_object_equality`.
- `crates/ripr/src/analysis/classify/activation.rs`: clear `FieldValue` for a
  named field in an admitted whole-value literal.
- `crates/ripr/src/analysis/classify/owner_pin.rs`: reuse call identity and
  execution admission.

## Metrics

- `field_construction_whole_value_credit`: findings confirmed by a whole-value
  literal naming the changed field.
- `whole_object_equality_lexical_only`: assertions that earned the kind from a
  brace alone; must be zero.
