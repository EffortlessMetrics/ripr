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

- Every case names one evidence class and one maturity scope.
- Every case has at least one expected claim and at least one
  `must_not_claim` guard.
- Runtime-only signals stay calibration evidence and must not create a static
  `evidence_record`.
- Line-movement cases preserve canonical gap identity while allowing raw seam
  line numbers to move.
- Static limitations remain analyzer limits until a supported fixture-backed
  pattern is added.
- Oracle cases must distinguish clear exact-value helpers from opaque helpers
  and must keep tautological equality assertions from claiming strong
  exact-value grip.
- Presentation text constants must not become user test debt from text alone.
  The benchmark requires visibility, actionability, recommended observer, and
  declaration-plus-literal grouping evidence before downstream surfaces can
  render one useful action.

Run:

```bash
cargo xtask check-fixture-contracts
```
