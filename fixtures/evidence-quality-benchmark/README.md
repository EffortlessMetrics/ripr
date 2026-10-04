# Lane 1 Evidence Quality Benchmark

This fixture corpus is the benchmark foundation for Lane 1 Evidence Quality
Leadership. It records evidence classes, expected claims, must-not-claim
guards, and audit or calibration signals before future analyzer repairs change
behavior.

The corpus is advisory maintainer data. It does not define a public output
schema, generate tests, edit source, run mutation testing, change gates, or
promote any capability globally.

## Files

- `corpus.json` - machine-readable benchmark manifest for RIPR-SPEC-0035.

## Rules

- The legacy `cases` collection requires unique IDs. Historical
  `semantic_oracle_controls` requires unique `(id, variant)` keys, so the two
  reviewed views may share one historical case ID.
- Every case names one evidence class and one maturity scope.
- Every case has at least one expected claim and at least one
  `must_not_claim` guard.
- Missing `semantic_oracle` means `unreviewed`, including legacy rows. An
  explicit `valid` or `invalid` expected-behavior label needs independent
  semantic review bound to the exact claim/basis/source/test/capture/verdict
  and all six retained native pairings described in
  RIPR-SPEC-0035. Passing tests or a strong static oracle cannot supply that
  label. The fixture-contract report discloses this separate axis; it does not
  change the Lane 1 scorecard or judged-panel calibration. Native artifacts
  held in external task evidence are explicitly `NOT_REVERIFIED` by ordinary
  source-only checks. Declared local artifact files must pass byte checks;
  missing local bytes cannot silently fall back to an external declaration.
- Runtime-only signals stay calibration evidence and must not create a static
  `evidence_record`.
- Optional `semantic_oracle_controls` holds two reviewed views of one historical
  Regex case separately from the 82 static/calibration rows. It verifies native,
  review and historical-static capture identities without prescribing a static
  classification. The weak test is only a removal control; capped reported
  related tests do not establish the complete internal selection.
- Line-movement cases preserve canonical gap identity while allowing raw seam
  line numbers to move.
- Static limitations remain analyzer limits until a supported fixture-backed
  pattern is added.
- Activation-value resolution must not invent synthetic values. Direct owner
  calls may satisfy activation for value-insensitive seams, including multiline
  no-argument calls, calls with opaque arguments, and one-hop same-file helpers
  that directly call the owner, while predicate-boundary checks still require
  concrete activation values.
- Same-test struct field projections are source-order scoped: literals are
  concrete only when visible before the owner call and not shadowed or mutated
  before the call.
- Oracle cases must distinguish clear exact-value helpers from opaque helpers
  and must keep tautological equality assertions from claiming strong
  exact-value grip.
- Presentation text constants must not become user test debt from text alone.
  The benchmark requires visibility, actionability, recommended observer, and
  declaration-plus-literal grouping evidence before downstream surfaces can
  render one useful action.
- Finding-alignment cases must preserve raw findings as supporting evidence
  while pinning one canonical evidence item, explicit `gap_state`,
  `actionability`, repair route, and count bucket for user-visible,
  already-observed, internal-only, static-limitation, and non-collision states.
- Config/policy constants must not become user test debt from metadata alone.
  The benchmark requires role, visibility, observer/discriminator,
  actionability, and named static-limitation evidence before downstream
  surfaces can render repair guidance.

Run:

```bash
cargo xtask check-fixture-contracts
```
