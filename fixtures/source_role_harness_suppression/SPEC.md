# Fixture: source_role_harness_suppression

Spec: RIPR-SPEC-0155

## Given

A production owner `price` with a boundary predicate, and one changed
Cargo integration test under `tests/` whose body uses the full harness
plumbing vocabulary: `Result<()>` plumbing with `?`, `map_err` chains,
`Ok(())` terminals, an Err-return guard, a harness-only `.contains()`
output check, and assertion-driver helper functions.

## When

```bash
cargo xtask fixtures source_role_harness_suppression
```

or:

```bash
ripr check --root fixtures/source_role_harness_suppression/input --diff fixtures/source_role_harness_suppression/diff.patch --mode fast
```

## Then

The harness plumbing creates zero production obligations: no probe, no
finding, and no repair route for `?`, `map_err`, `Ok(())`, the
`.contains()` check, or the helper control flow. The changed test stays
in changed-file accounting and its evidence remains available to the
production owner: the exact `assert_eq!` boundary assertion credits the
`price` predicate seam, so the production gap count reflects only the
production owner's real discriminator state (#3213 closeout matrix rows
1, 4, 5).

## Must Not

- Seed a production probe from any tests/ harness shape in this diff.
- Drop the changed test from changed-file accounting or from the index.
- Weaken or drop the production owner's ordinary classification.
- Credit the harness-only `.contains()` or Err-guard as exact oracles.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
