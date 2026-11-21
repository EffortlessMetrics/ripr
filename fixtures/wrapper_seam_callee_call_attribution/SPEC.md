# Fixture: wrapper_seam_callee_call_attribution

Spec: RIPR-SPEC-0106

Issue: #3714

Citation note: RIPR-SPEC-0106 owns the error-seam family this fixture
belongs to (its Fixture 5 documents the #3700 typed wrapper limitation);
the unwrap_err/expect_err binding shapes 0106 details are exercised by its
own fixtures. What THIS fixture adds is the #3714 call-based relation
attribution (round-1/round-2): a generically-named callee-calling test is
weakly related (`seam_callee_call`, medium) instead of being dropped at
`no_static_path`.

## Given

A wrapper error seam (`parse_summary(raw).map_err(|error| error.to_string().into())`
over the typed callee `try_parse_summary`), with three integration tests:

- a generically-named test (`observes_callee_outcome`) whose body calls only
  the seam's converted callee (`try_parse_summary("abc")`) — no owner call,
  no owner or probe-token affinity in its name;
- a positional-defeat witness (`calls_callee_before_shadow_binding`,
  #3728): its captured call precedes a same-named `let` binding later in
  the body, so the binding shadows only subsequent uses and the relation
  survives — the whole-body defeat this refinement replaced dropped it;
- an unrelated-named test (`unrelated_scope_has_no_call`) that calls nothing
  related to the seam.

The diff changes only the wrapper conversion line (a
concrete-error-preserving conversion to the stringified conversion — the
#3700 controlled-proof direction), so the wrapper seam probe fires on the
changed line. The conversion direction does not affect the attribution:
`wrapper_error_seam_expression` recognizes the top-level `.map_err(..)`
conversion in either form.

## When

```bash
cargo xtask fixtures wrapper_seam_callee_call_attribution
```

or:

```bash
ripr check --root fixtures/wrapper_seam_callee_call_attribution/input \
           --diff fixtures/wrapper_seam_callee_call_attribution/diff.patch \
           --mode fast
```

## Then

The generically-named callee-calling tests are related with
`relation_reason: seam_callee_call` (captured `CallFact` name match on the
converted callee, confidence medium) — including
`calls_callee_before_shadow_binding`, whose captured call precedes the
body's same-named `let` binding (#3728 positional shadow defeat) — and the
seam classifies `weakly_exposed` — the relation is weak: no exact-variant
credit is claimed for the wrapper conversion
(`wrapper_error_binding_unresolved` stays the honest outcome for the
conversion's variant identity per #3700).

The unrelated-named test produces no related-test entry: a name that shares
nothing with the probe and calls nothing the seam touches establishes no
relation.

## Must Not

- Leave a direct callee-calling test unrelated (`no_static_path`) because
  its name carries no affinity — name affinity ranks, it does not gate.
- Credit `seam_callee_call` relations with exact-variant discrimination:
  the relation is weak; variant identity for wrapper conversions remains
  the typed `wrapper_error_binding_unresolved` limitation (#3700).
- Relate tests whose bodies define or bind a same-named local (`fn <callee>`
  defeats every call — fn items hoist; `let <callee> =` defeats captured
  calls at or after the binding's line). Shadow-shaped text inside comments
  or string literals defeats nothing (#3728 masked scans).
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
