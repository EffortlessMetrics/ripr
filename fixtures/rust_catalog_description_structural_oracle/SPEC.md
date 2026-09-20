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

The wording row must not promote on structural evidence alone: with only the
coincidental `catalog` token shared between oracle and changed expression,
the row withholds at `weakly_exposed` (delta-token confirmation, #1748).
An oracle quoting the changed word would confirm and expose.

## Must Not

- Treat this control as sufficient discrimination: a wording-only change
  passes the asserting test.
- Claim runtime mutation adequacy.
