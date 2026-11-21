# Fixture: error_variant_boxed_wrapper_downcast_witness

Spec: RIPR-SPEC-0106

Issue: #3700

## Given

A boxed-error wrapper seam in the shape reported on #3700
(`perl-lsp-swarm` `parse_perl_summary`, lib.rs:532): a public owner returns
`Result<T, Box<dyn Error>>` by converting an inner typed error through
`try_parse_summary(raw).map_err(Into::into)`, next to a typed callee whose
early-error branches construct `ParseSummaryError::MalformedSource`. Two
existing tests observe the exact variant: a typed sibling test (whose name
contains the wrapper owner, calls the callee, and pins
`Err(ParseSummaryError::MalformedSource)` via `matches!` with a `return Err`
on mismatch) and a boxed downcast witness (extracts the error with
`.err().ok_or(...)?` and returns `Err` unless
`matches!(error.downcast_ref::<ParseSummaryError>(), Some(ParseSummaryError::MalformedSource))`).

Five companion wrappers pin the fail-closed side: a wrong-sibling downcast
pin (`checksum_summary`, pins `MalformedPayload` which the callee never
produces), an unrelated-enum downcast pin (`render_summary`, pins
`ParseSummaryError::MalformedSource` against a different enum), a broad
`is_err()`-only observer (`length_summary`), a stringified conversion
(`glyph_summary`, `map_err(|error| error.to_string().into())`), and an
ignored `matches!` result (`theme_summary`).

The diff changes the typed error construction inside `try_parse_summary`
(both early-error branches construct `MalformedSource`) and each wrapper's
conversion line, so one `error_path` probe lands on the parseable-variant
typed seam and one on each wrapper conversion.

## When

```bash
cargo xtask fixtures error_variant_boxed_wrapper_downcast_witness
```

or:

```bash
ripr check --root fixtures/error_variant_boxed_wrapper_downcast_witness/input            --diff fixtures/error_variant_boxed_wrapper_downcast_witness/diff.patch            --mode fast
```

## Then

The typed parseable-variant seam (`return
Err(ParseSummaryError::MalformedSource);`) classifies `exposed` with
`exact_error_variant` / `strong` — the pre-existing variant-bound credit path
(RIPR-SPEC-0106), no wrapper heuristics involved.

Every `map_err(Into::into)` wrapper seam — including the one the downcast
witness reaches — classifies `weakly_exposed` and carries the typed static
limitation `static_limit_kind: wrapper_error_binding_unresolved`: whether the
boxed conversion faithfully carries the callee's error variant is not
statically establishable, so lexical confirmation is refused by construction.
The downcast witness and the typed sibling test remain listed as related, but
the emitted missing/recommendation text names the limitation instead of
prescribing an assertion the suite may already contain. This is the
fail-closed outcome the issue sanctions; crediting a faithful typed
conversion requires modeling `Into`/`From` through `Box` (follow-up slice).

The fail-closed companions all stay `weakly_exposed` for their own reasons:
no variant pin (broad `is_err()`), a conversion that destroys the typed
identity (stringified), an ignored `matches!` result, and pins that are not
bound to the converted callee (wrong sibling, unrelated enum).

## Must Not

- Classify a wrapper `map_err(Into::into)` seam as `exposed` from lexical
  heuristics: every overlap between the seam expression and witness text is
  token coincidence, and the conversion's variant binding is not statically
  establishable.
- Prescribe an exact-variant assertion for a wrapper seam whose witnesses
  already downcast-and-pin the variant; emit the typed limitation instead.
- Report the typed parseable-variant seam as a gap while its exact-variant
  witnesses exist.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
