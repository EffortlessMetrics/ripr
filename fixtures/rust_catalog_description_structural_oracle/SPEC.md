# Fixture: rust_catalog_description_structural_oracle

Spec: RIPR-SPEC-0108

## Given

`default_check_description` renders the description text for the `check`
catalog entry; `build_catalog` stores it in the asserted `Catalog`.
`entry_description_violations` checks structural consistency only
(duplicate/empty names and descriptions) and never inspects the wording.
One test reaches the changed owner directly and asserts zero violations
with a strong exact oracle; the assertion shares a probe-expression token
(`catalog`), so the observation guard clears on coincidence.

## When

```bash
cargo xtask fixtures rust_catalog_description_structural_oracle
```

The diff changes the rendered wording (`"checks catalog consistency"` ->
`"verifies catalog consistency"`).

## Then

Current residual (p1745 catalog-description survivor replicated): the wording
row is `exposed` on structural evidence alone. Pinned as a control until the
specificity retry lands; the retry must flip this row to non-promotion while
keeping exact-sink oracles exposed.

## Must Not

- Treat this control as adequate discrimination: a wording-only change
  passes the asserting test.
- Claim runtime mutation adequacy.
