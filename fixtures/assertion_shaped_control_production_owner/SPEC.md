# Fixture: assertion_shaped_control_production_owner

Spec: RIPR-SPEC-0133

## Given

A normal production owner (`discounted_total` in `src/lib.rs`) whose body is
not assertion-shaped, covered by a test that reaches it through a builder
helper.

The diff changes the discount predicate:

```rust
if amount > 100 {
```

to:

```rust
if amount >= 100 {
```

## When

```bash
cargo xtask fixtures assertion_shaped_control_production_owner
```

or:

```bash
ripr check --root fixtures/assertion_shaped_control_production_owner/input --diff fixtures/assertion_shaped_control_production_owner/diff.patch --mode fast
```

## Then

Control: the standard code-under-test guidance is emitted unchanged (the
`weakly_exposed` boundary-test advice), and the finding evidence contains no
`owner_shape:` line. The assertion-shaped reframe must not leak onto ordinary
production owners.

## Must Not

- Reframe guidance for an owner that is not assertion-shaped.
- Change the exposure class or finding set relative to the pre-RIPR-SPEC-0133
  behavior for this shape.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
