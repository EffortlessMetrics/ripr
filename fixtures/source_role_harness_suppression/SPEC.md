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
production owner: the earlier `assert_eq!(value, 50)` retains strong oracle
credit. The later equality after a possible return is unestablished; the
Err-return guard retains its separate weak relational meaning. Classification
remains `propagation_unknown` at advisory confidence 0.66. Boundary
discrimination is weak: the admitted exact oracle observes `price(50, 100)`,
while the later equality on `price(100, 100)` follows a possible return and
has no established execution witness. The raw later assertion cannot restore
boundary-pairing credit after reveal refuses it. The original propagation
limitation remains separate, and the production gap count still reflects only
the production owner (#3213 closeout rows 1, 4, 5).

The shared statement-prefix query (#5027, RIPR-SPEC-0197) must not reject an
earlier assertion merely because a return occurs later. Six successfully
compiled one-test runtime subjects independently establish this positive:

| Implementation | First equality retained | First equality removed |
|---|---|---|
| Correct source | pass | pass |
| `>=` changed to `>` | fails via `boundary mismatch` Err | same failure |
| Else result `amount` changed to `amount + 1` | fails, 51 vs 50 | passes |

These controls use the exact integration-test source with rustc 1.95.0, plus
separately compiled correct/wrong libraries. Removing only the first equality
disables the asserted-value discriminator. This is fixture runtime evidence,
not a claim of general Result CFG interpretation or full static propagation.
The Err guard really rejects the executed boundary mutant; its retained weak
static oracle classification does not mean that the test is ineffective.

## Must Not

- Seed a production probe from any tests/ harness shape in this diff.
- Drop the changed test from changed-file accounting or from the index.
- Weaken or drop the production owner's ordinary classification.
- Credit the harness-only `.contains()` or Err-guard as exact oracles.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
