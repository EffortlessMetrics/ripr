# Fixture: infect_value_returned

Spec: RIPR-SPEC-0096

## Given

Production code changes so that a function result is stored in a named
binding `let result = helper(amount * MULTIPLIER)` which is subsequently
used in a tail expression `result + 1`.  The changed value IS read into the
returned value — this is a genuine infecting change.

A test with an exact-value assertion that references the changed token
(`MULTIPLIER`) exercises `score`:

```rust
#[test]
fn score_uses_multiplier() {
    assert_eq!(score(10), 10 * MULTIPLIER / 10 + 1);
}
```

## When

```bash
cargo xtask fixtures infect_value_returned
```

or:

```bash
ripr check --root fixtures/infect_value_returned/input --diff fixtures/infect_value_returned/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` (with `infect: yes`) for the named binding probe
`let result = helper(amount * MULTIPLIER)`.  A named binding (`let result =
…`) is NOT a discard — the value flows into `result + 1`.  This is the
**control** for fix A in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md):
the wildcard-discard predicate must NOT fire for `let _name = …`.  The
`infect: yes` on this probe is the load-bearing signal — it proves fix A's
predicate is inert on named bindings.

## Must Not

- Classify `let result = helper(…)` as `infection_unknown`.
- Set `infect` to `unknown` for a named binding used in a subsequent expression.
- Use mutation-runtime outcome vocabulary.
