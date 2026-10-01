# Fixture: rust_same_method_owner_type_positive (positive control — true receiver identity keeps exposed)

Spec: RIPR-SPEC-0108

Corpus case: `rust_same_method_owner_type_positive_control` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #4760,
required positive control: exact same-entity relation MUST still fire for the
true owner).

## Given

The mirror image of `fixtures/rust_adversarial_same_method_other_type`.
The changed owner is again `impl Iterator for WhileSome::size_hint`, flipping
`(0, None)` → `(0, Some(self.remaining))`, but this time the test provides
genuine receiver identity: it constructs `WhileSome` and asserts the method's
exact return value on that receiver:

```rust
let it = WhileSome { remaining: 2 };
assert_eq!(it.size_hint().1, Some(it.remaining));
```

A second impl of `size_hint` (`Combinations`) exists in the same crate so the
receiver-identity gate is active; uniqueness cannot bypass it.

## When

```bash
ripr check \
  --root fixtures/rust_same_method_owner_type_positive/input \
  --diff fixtures/rust_same_method_owner_type_positive/diff.patch
```

## Then

ripr credits the receiver-owner relation (`let it = WhileSome { … }` +
`it.size_hint(…)`) as `direct_owner_call` and classifies the return-value
change `exposed` with an `exact_value`/`strong` oracle.

**This control must NEVER lose `exposed`.** It proves the same-method-other-type
guard did not degenerate into "disable all impl-method relations": the true
owner, reached with real receiver identity and observed by a strong oracle,
keeps its `exposed` classification.

## Must Not

- Downgrade a same-entity receiver relation with a strong exact-value oracle
  below `exposed`.
- Absorb #4478, #4486, or #3727.
- Use mutation-runtime outcome vocabulary.
