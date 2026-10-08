# Fixture: infect_wildcard_discard

Spec: RIPR-SPEC-0096

## Given

Production code adds calls whose return values are bound to the wildcard
discard patterns `let _ : i32 = compute_fee(amount * 9)` and
`let _= compute_fee(amount * 9)`. The caller function returns `amount`
unchanged — the changed values cannot reach any observable sink.

A test with a strong exact-value assertion covers `process`:

```rust
#[test]
fn process_returns_amount_unchanged() {
    assert_eq!(process(42), 42);
}
```

## When

```bash
cargo xtask fixtures infect_wildcard_discard
```

or:

```bash
ripr check --root fixtures/infect_wildcard_discard/input --diff fixtures/infect_wildcard_discard/diff.patch --mode fast
```

## Then

Both wildcard-discard statements must remain visible, at lines 2 and 3,
with family/class `static_unknown` and an `unknown` infection stage.
The bounded probe extractor does not assign a supported value/effect family
to these call initializers; its generic syntax limitation remains authoritative.
The original golden already recorded this conservative family/class. The
corrected head-side diff adds the previously missing second discard subject.
The unchanged return statement is retained at line 4 as `propagation_unknown`.

RIPR-SPEC-0096's discard-specific infection refusal applies to supported
probe families; it does not override `static_unknown` classification. The
previous fixture prose incorrectly promised `infection_unknown` and its
specific reason for these unsupported initializer subjects. Discarded-value
runtime mutations leave the caller's exact return assertion passing; a changed
caller return fails it. Neither outcome establishes general side-effect purity.

## Must Not

- Use mutation-runtime outcome vocabulary.
- Report `exposed` when the changed value is bound to a wildcard discard.
- Downgrade `let _name = ...` (named bindings) — those may still be used.
