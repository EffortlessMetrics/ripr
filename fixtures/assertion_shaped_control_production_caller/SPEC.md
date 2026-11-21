# Fixture: assertion_shaped_control_production_caller

Spec: RIPR-SPEC-0133

## Given

An assert-dominated helper (`check_score_invariants` in `src/lib.rs`) that DOES
have a production caller (`validate_score`). The caller rule keeps it out of
the assertion-shaped class: guidance for it must stay in the code-under-test
voice.

The diff changes a predicate computed inside the helper:

```rust
let clamped = if value > 0 { value } else { 0 };
```

to:

```rust
let clamped = if value >= 0 { value } else { 0 };
```

## When

```bash
cargo xtask fixtures assertion_shaped_control_production_caller
```

or:

```bash
ripr check --root fixtures/assertion_shaped_control_production_caller/input --diff fixtures/assertion_shaped_control_production_caller/diff.patch --mode fast
```

## Then

Control: the finding evidence contains no `owner_shape:` line and the guidance
is the standard text for the emitted class. A non-test caller — even a
bare-name match — must block the oracle reframe (fail-closed).

## Must Not

- Treat an assert-heavy function with production callers as assertion-shaped.
- Change the exposure class or finding set relative to the pre-RIPR-SPEC-0133
  behavior for this shape.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
