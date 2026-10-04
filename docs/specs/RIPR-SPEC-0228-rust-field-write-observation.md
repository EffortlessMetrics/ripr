# RIPR-SPEC-0228: Rust field writes are probed and observed, not unknown

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

- No tier change. A changed Rust field assignment (`recv.field = expr`,
  `recv.field op= expr`) gets a typed probe instead of `static_unknown`, and a
  test that calls the mutating owner and then reads that field exactly can
  confirm it. Python and TypeScript previews already probe field assignments
  (RIPR-SPEC-0027, RIPR-SPEC-0028); this closes the Rust asymmetry. Claim
  boundaries remain governed by [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No schema version bump. Two proposed `static_limit_kind` values are
  additive and would be documented in `docs/STATIC_LIMITS.md` and
  `docs/OUTPUT_SCHEMA.md` by the implementing PR.

## Problem

State changes through `&mut self` are a core Rust shape:

```rust
impl Counter {
    pub fn bump(&mut self) { self.count += 1; }   // changed from `+= 2`
}

#[test]
fn bumps() {
    let mut c = Counter::default();
    c.bump();
    assert_eq!(c.count, 1);
}
```

The parser emits no shape for assignments or compound assignments: a binary
expression counts only for predicate operators (`analysis/syntax/ra.rs`). The
lexical fallback needs a `:` without `::` to call a line a field
(`analysis/probes/lexical.rs`, `has_field_shape`). So the line becomes a
`static_unknown` probe and the finding reads `static_unknown` with stop
reason `static_probe_unknown` and no `static_limit_kind`
(`analysis/classify/decision.rs`).

Measured on origin/main with `ripr check --diff`, a direct field assertion, a
getter assertion and a test that reads nothing all produce the same
`static_unknown`. ripr cannot tell an observed write from an unobserved one.
`self.state = State::Done` and `*p = v` fall the same way. The "Fill
spec-defined corpus cases" ledger crate reproduces it: `self.balance +=
amount` abstains although an `assert_eq!` catches every mutant.

Neighbouring shapes are already typed: `self.label = compute(x)` reads as a
`call_deletion`, and `self.items.push(x)` reads as `side_effect` with sink
`state_write`. RIPR-SPEC-0094's direct collection observer covers a bare
receiver (`items.push`) and leaves field-path receivers on its Part C.
RIPR-SPEC-0001 says labels "should be available for state writes" without
defining them for assignments.

## Behavior

### Probe

A changed Rust statement `place = expr` or `place op= expr`, where `place` is a
field path (`self.f`, `recv.f`, `recv.a.b`), produces a `field_construction`
probe whose field identity is the full projection path after the receiver
(`count` for `self.count`, `child.count` for `self.child.count`) and whose
expression is the assignment's right-hand side and operator. Every rule below
matches that full path: a read of `r.count` never observes a write to
`r.child.count`. It does not produce a
`static_unknown` probe for the same span.

### Observation

The probe is confirmed by a related test that:

In this section `f` is the full projection path (`r.child.count`, never
`r.count`, for a write to `self.child.count`).

1. calls the owner method on a local binding `r` (the receiver), and the
   changed statement is not inside a loop or closure, and after it, on any
   path to the owner's return, the body uses `self` only in plain reads of
   fields whose values are copied, or compared with a built-in operator on
   a primitive type (`let n = self.limit;`, `self.count > 3`). A comparison
   that dispatches to a workspace `PartialEq` or `PartialOrd` impl counts as
   a call, because that impl could write interior-mutable state. Any other later use refuses credit, written or
   implicit: an assignment to `f` or a prefix (`self.child = ..`,
   `*self = ..`), a method call on `self` or on any field path
   (`self.reset()`, `self.child.reset()`, `self.count.set(0)` through a
   `Cell`), `self` or a field passed to a call (`zero(self)`,
   `mem::take(&mut self.child)`), a closure capturing `self`, a macro call,
   or a use of any binding that borrows `self` or one of its fields
   (`let p = &mut self.count;` before or after the change). So
   `self.count += 1; self.count = 0` and `self.count += 2; self.reset()`
   both give no credit, and
2. after that call, holds an admitted exact oracle that reads `r.f`
   (`assert_eq!(r.f, v)`, or a whole-value comparison of `r` that names `f`
   under RIPR-SPEC-0225); and
3. between the owner call and that read, nothing else can write `r.f`: no
   method call on `r` taking `&mut self`, no assignment to `r` or `r.f`, no
   `&mut r` passed to a call, and no method call on `r` at all when `f` is a
   `Cell`, `RefCell`, `Mutex`, `RwLock` or atomic (interior mutability).
   Otherwise the read does not confirm
   (`c.bump(); c.reset(); assert_eq!(c.count, 0)` hides the mutant).

A read of a sibling field keeps a `FieldValue` missing discriminator for `f`,
as for constructed fields (RIPR-SPEC-0005). A test that reaches the owner and
reads nothing after the call follows the constructed-field classes: no oracle
on the receiver reads `reachable_unrevealed`, a weak or sibling read reads
`weakly_exposed`. An enum-variant write (`self.state = State::Done`) applies
RIPR-SPEC-0094 Part B variant scoping to the read.

Collection writes on a field path (`self.items.push(x)`,
`self.map.insert(k, v)`) extend the direct-collection observer (#4575, today
limited to `push` on a bare identifier in
`analysis/classify/propagation_witness.rs`) to field-path receivers and to
`insert`, so `c.items` in the test is the observed subject for `self.items`
in the owner. This is new behavior, not reuse.

### Limits

- A read through a method (`r.count()`) confirms only when the method
  resolves to an inherent method of the receiver's own type (not a trait
  method, a `Deref` target or a same-name method on another type) whose body
  returns `self.f` directly (or a reference, copy or clone of it). Otherwise, when the test holds a strong oracle on the
  method result, the finding reads `static_unknown` with the proposed
  `static_limit_kind` `rust_state_read_path_unresolved`.
- A write through a dereferenced `&mut` parameter (`*p = v`) or through a
  `&mut` argument (`fill(&mut v, 7)` writing `vec.push(..)` in the owner)
  needs parameter-to-argument aliasing. Until that exists the finding reads
  `static_unknown` with the proposed `static_limit_kind`
  `rust_mut_reference_alias_unresolved`.

### Decisions for the owner

1. **Family.** Recommended: `field_construction`, matching the Python and
   TypeScript previews and reusing the `FieldValue` discriminator.
   Alternative: `side_effect` with sink `state_write`.
2. **Getter credit.** Recommended: resolved-body rule above, fail closed as
   `static_unknown`. Alternative: credit on a name match (`count()` for
   `count`), which a getter returning another field would fool.
3. **Unread writes.** Recommended: `reachable_unrevealed` when no oracle reads
   the receiver after the call, matching constructed fields. Alternative:
   keep them non-actionable until the read set is established another way.

## Required Evidence

- The reproduction reads `exposed` with a `field_construction` finding and no
  `static_unknown` finding on the line.
- A sibling-field control (`assert_eq!(c.label, "x")` only) reads
  `weakly_exposed` with a `FieldValue` missing discriminator.
- A no-read control (`c.bump();` and nothing after it) reads
  `reachable_unrevealed`.
- A getter control returning `self.count` reads `exposed`; a getter returning
  another field does not.
- A `*p = v` control reads `static_unknown` with
  `rust_mut_reference_alias_unresolved`.
- Golden drift: every finding that leaves `static_unknown` is listed with its
  new class; none reads `exposed` without a same-test read of the field.

## Non-Goals

- No interprocedural aliasing.
- No credit through `Debug`/`Display` output or logs.
- No change to `call_deletion` for `self.f = call(..)`.
- No new probe family.

## Acceptance Examples

`Counter { count: u32, label: String }`, owner `bump(&mut self)`, the diff
changes `self.count += 2` to `self.count += 1`.

1. `c.bump(); assert_eq!(c.count, 1);`: `exposed`.
2. `c.bump(); assert_eq!(c.count(), 1);` with `fn count(&self) -> u32 { self.count }`:
   `exposed`.
3. Same as 2 with `fn count(&self) -> u32 { self.other }`: not `exposed`.
4. `c.bump(); assert_eq!(c.label, "x");`: `weakly_exposed`, `FieldValue`
   missing discriminator for `count`.
5. `c.bump();` and no assertion: `reachable_unrevealed`.
6. `self.state = State::Done` (changed from `State::Idle`), test
   `c.finish(); assert_eq!(c.state, State::Done)`: `exposed`; a test
   asserting only `State::Idle` on another field: not `exposed`.
7. `fn set(p: &mut u32, v: u32) { *p = v; }` (changed from `v + 1`), test
   `set(&mut x, 5); assert_eq!(x, 5)`: `static_unknown`,
   `rust_mut_reference_alias_unresolved`.
8. `self.items.push(x)` with argument changed, test
   `c.add(4); assert_eq!(c.items, vec![4])`: `exposed` (measured on main with
   `ripr check --diff`, sink `state_write`).
9. `c.bump(); c.reset(); assert_eq!(c.count, 0)` with `reset` taking
   `&mut self`: not `exposed`.
10. `bump` writes `self.count += 1; self.count = 0;` (first statement
    changed from `+= 2`), test `c.bump(); assert_eq!(c.count, 0)`: not
    `exposed`.
11. `bump` writes `self.count += 2; self.reset();` (first statement
    changed from `+= 3`), with `reset` setting `count = 0`, test
    `c.bump(); assert_eq!(c.count, 0)`: not `exposed`.

## Test Mapping

- Existing: `fixtures/python_field_assignment_shape` (parity reference),
  `fixtures/observation_verified_side_effect`,
  `fixtures/observation_unverified_side_effect`.
- Planned: one fixture or verdict-corpus case per acceptance example.
- Planned: probe extraction unit tests for `=` and `op=` on field paths.

## Implementation Mapping

- `crates/ripr/src/analysis/syntax/ra.rs`: emit an assignment shape for field
  paths.
- `crates/ripr/src/analysis/probes/diff.rs` and `lexical.rs`: map it to
  `field_construction`; stop the `static_unknown` fallback for that span.
- `crates/ripr/src/analysis/classify/activation.rs`: receiver read after the
  owner call; `FieldValue` for sibling reads.
- `crates/ripr/src/analysis/classify/propagation_witness.rs`: field-path
  receivers for collection writes.

## Metrics

- `rust_field_write_static_unknown`: changed field assignments that read
  `static_unknown` without a named limit; must be zero.
- `rust_field_write_direct_read_credit`: field writes confirmed by a same-test
  exact read of the field.
