# Fixture: error_variant_boxed_wrapper_fail_closed

Spec: RIPR-SPEC-0106

Issue: #3700

## Given

Two boxed-error wrappers over typed inner errors, each observed only by a
downcast witness whose variant pin is NOT bound to the callee:

- `checksum_summary` (`try_checksum_summary(payload).map_err(Into::into)`,
  callee produces only `BadChecksum`) observed by a witness that pins
  `ChecksumError::MalformedPayload` — a sibling the callee never produces;
- `render_summary` (`try_render_summary(raw).map_err(Into::into)`) observed
  by a witness that pins `ParseSummaryError::MalformedSource` — a variant of
  an unrelated enum.

Neither witness calls the callee whose error the wrapper converts, so nothing
establishes a wrapper-to-variant binding for either seam.

## When

```bash
cargo xtask fixtures error_variant_boxed_wrapper_fail_closed
```

or:

```bash
ripr check --root fixtures/error_variant_boxed_wrapper_fail_closed/input \
           --diff fixtures/error_variant_boxed_wrapper_fail_closed/diff.patch \
           --mode fast
```

## Then

Every finding stays `weakly_exposed` and carries the typed static
limitation `static_limit_kind: wrapper_error_binding_unresolved`. The
wrapper seams carry no parseable variant, and whether the boxed conversion
carries the callee's error variant is not statically establishable, so the
witnesses' strong-shaped assertions never confirm observation — token
overlap between the seam expression and the witness message text (the
parameter names, the callee names, or `Into::into`) upgrades nothing.

This fixture is the corpus source for
`rust_boxed_wrapper_wrong_sibling_no_credit` and
`rust_boxed_wrapper_unrelated_enum_no_credit` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json`: an over-credit
regression that re-promotes either shape to `exposed` must fail
`cargo xtask check-evidence-promotion-honesty`.

## Must Not

- Upgrade either wrapper seam to `exposed` from token overlap alone.
- Credit a downcast witness that pins a variant without calling the callee
  whose error the wrapper converts, whatever enum the pin names.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
