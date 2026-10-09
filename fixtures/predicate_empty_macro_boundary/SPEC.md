# Fixture: predicate_empty_macro_boundary

Spec: RIPR-SPEC-0197

Owner: analysis-fixtures

Issue: #5027

## Given

A uniquely resolved local empty catch-all macro discards a boundary owner call.
A real equality on the same line observes the far input 90/100.
Both the correct >= owner and the wrong > owner compile and pass one test.

## When

`cargo xtask fixtures predicate_empty_macro_boundary` runs the public analyzer.
`empty_macro_arguments_cannot_activate_a_real_far_oracle` compares this case
with removal of the empty invocation and a real 100/100 boundary assertion.

## Then

One predicate remains weakly_exposed with weak infection and the missing
boundary discriminator. The real far assertion retains strong oracle strength,
Observe=yes and Discriminate=yes. The real boundary positive remains exposed
and rejects the wrong implementation.

## Must Not

- Promote discarded arguments through a retained mixed-line CallFact text.
- Remove the genuine far oracle or downgrade its observation/discrimination.
- Claim arbitrary macro expansion, raw non-equality oracle provenance or runtime adequacy.
