# Fixture: typescript_adversarial_owner_identity_after_insertion (false-exposed guard — stale line/owner identity)

Spec: RIPR-SPEC-0108

Corpus case: `ts_owner_identity_after_insertion` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
TypeScript family "stale line/owner identity after an insertion").

## Given

An adversarial **stale line/owner identity** trap. The diff INSERTS an
interface and an exported const above the pre-existing functions in
src/pricing.ts, shifting every later line number down by six, and changes the
discount predicate in `applyDiscount`:

```ts
// inserted above (lines 1-6): FeeRule + DEFAULT_FEE_RULE — line shift
// changed owner (now at line 14, previously line 8):
if (amount >= threshold) {   // was: amount > threshold
```

A strong exact-value test exists for the **sibling** function `computeFee`,
which sits between the insertion and the changed owner:

```ts
import { computeFee } from '../src/pricing';
expect(computeFee(100)).toBe(3);
```

The tempting wrong relation is a stale pre-insertion line/owner mapping: if
the changed line were resolved against pre-insertion line numbers (or attached
to the nearest strong test), the probe would land on `computeFee` and borrow
its strong oracle.

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr keeps owner identity aligned to the POST-insertion file: the changed
predicate resolves to `applyDiscount`, whose only test link is the
same-file-stem proximity heuristic, so it classifies `weakly_exposed`. The
inserted module initializer `DEFAULT_FEE_RULE` has no observer and classifies
`no_static_path`. The `computeFee` exact-value oracle is never borrowed —
`computeFee` is unchanged, so it produces no finding at all.

**This fixture must NEVER read `exposed`.** Both findings must stay
non-promoted: the shifted owner must not inherit the sibling's strong oracle,
and the inserted initializer must not gain a fabricated observer.

## Must Not

- Resolve changed lines against pre-insertion line numbers or attach the
  changed probe to the sibling `computeFee` owner.
- Borrow the `computeFee` exact-value oracle as evidence about
  `applyDiscount`.
- Run any TypeScript runtime; static preview evidence only.
