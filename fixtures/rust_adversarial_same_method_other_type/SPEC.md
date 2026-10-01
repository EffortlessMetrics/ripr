# Fixture: rust_adversarial_same_method_other_type (false-exposed guard — trait method identity)

Spec: RIPR-SPEC-0108

Corpus case: `rust_same_method_other_type` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #4760).

## Given

An adversarial **same-trait-method-name, different-impl-type** over-credit trap
— a live false-`exposed` found on itertools `WhileSome::size_hint` (accuracy
hunt 2026-09-29). The changed owner is `impl Iterator for WhileSome::size_hint`,
flipping `(0, None)` → `(0, Some(self.remaining))`. The only related test
constructs a **different** type `Combinations` and asserts that type's
`size_hint` under a strong exact-value oracle:

```rust
// changed owner: impl Iterator for WhileSome
fn size_hint(&self) -> (usize, Option<usize>) {
    (0, Some(self.remaining))
}

// the ONLY related test — a DIFFERENT impl, never constructs WhileSome
let it = Combinations { remaining: 3 };
assert_eq!(it.size_hint().1, Some(3));
```

The test linked to the owner only because `body_contains_owner_call` /
`tests_by_call_name` matched the bare method name `size_hint(` on *any*
receiver. Same-crate competing impls are not unique, so the uniqueness
bypass that gates cross-crate package-prefix filtering does not apply.
The strong `assert_eq!(it.size_hint().1, …)` then supplied the oracle.

## When

```bash
ripr check \
  --root fixtures/rust_adversarial_same_method_other_type/input \
  --diff fixtures/rust_adversarial_same_method_other_type/diff.patch
```

## Then

ripr classifies the change **below `exposed`**. The name-only relation is
`weak_token_substring`, not `direct_owner_call`. For a trait-impl method that
may be reached unseen, proximity keeps reach `weak` and the finding
`weakly_exposed`. `related_tests` lists the Combinations test once even though
it has several matching assertions.

**This fixture must NEVER read `exposed`.** Before the receiver-identity gate
it read `exposed` at confidence 1.0 — a silent over-credit: a test that never
constructs `WhileSome` was reported as discriminating that impl's changed
return. The companion `rust_same_method_owner_type_positive` pins the inverse
— a test that constructs the owner type keeps `direct_owner_call` / `exposed`.

## Must Not

- Credit `exposed` or `direct_owner_call` from a bare trait-method name when
  the receiver is another impl type or unresolved.
- Absorb #4478 (method-call confirmation pin), #4486 (proximity-only oracle),
  or #3727 (CallFact receiver fields).
- Use mutation-runtime outcome vocabulary.
