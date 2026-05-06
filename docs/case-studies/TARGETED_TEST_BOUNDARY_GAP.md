# Targeted Test Case Study: Boundary Gap

This case study dogfoods the targeted-test workflow on
`fixtures/boundary_gap`. It is intentionally small: one weak seam, one focused
test, one before/after receipt.

The checked public corpus index is
[`fixtures/EXAMPLE_CORPUS.md`](../../fixtures/EXAMPLE_CORPUS.md). The
boundary-gap receipt and optional calibration outputs are checked in under
`fixtures/boundary_gap/calibration/`.

## Operator Loop

Scratch workspace:

```text
target/ripr/case-study/boundary-gap
```

Before snapshot:

```bash
cargo run -p ripr -- check \
  --root target/ripr/case-study/boundary-gap \
  --diff target/ripr/case-study/boundary-gap/diff.patch \
  --mode ready \
  --format repo-exposure-json \
  > target/ripr/case-study/boundary-gap/reports/before.repo-exposure.json
```

Work order:

```bash
cargo run -p ripr -- check \
  --root target/ripr/case-study/boundary-gap \
  --diff target/ripr/case-study/boundary-gap/diff.patch \
  --mode ready \
  --format agent-seam-packets-json \
  > target/ripr/case-study/boundary-gap/reports/agent-seam-packets.json
```

RIPR selected seam:

```text
seam_id: 67fc764ba37d77bd
kind: predicate_boundary
file: src/lib.rs:2
before class: weakly_gripped
missing discriminator: discount_threshold (equality boundary)
suggested assertion shape: exact returned value assertion at the equality boundary
```

Focused test added in the scratch copy:

```diff
+#[test]
+fn equality_boundary_discounts() {
+    assert_eq!(discounted_total(100, 100), 90);
+}
```

After snapshot:

```bash
cargo run -p ripr -- check \
  --root target/ripr/case-study/boundary-gap \
  --diff target/ripr/case-study/boundary-gap/diff.patch \
  --mode ready \
  --format repo-exposure-json \
  > target/ripr/case-study/boundary-gap/reports/after.repo-exposure.json
```

Receipt:

```bash
cargo run -p ripr -- outcome \
  --before target/ripr/case-study/boundary-gap/reports/before.repo-exposure.json \
  --after target/ripr/case-study/boundary-gap/reports/after.repo-exposure.json
```

Checked corpus receipt:

```bash
cargo run -p ripr -- outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

## Receipt Result

The receipt stayed advisory and reported:

```text
moved: 0
unchanged: 1
regressed: 0
new: 0
removed: 0
```

The unchanged seam still carried a useful evidence delta:

```text
67fc764ba37d77bd src/lib.rs:2 weakly_gripped -> weakly_gripped
- new observed value: 100
```

## Interpretation

The focused test was visible to RIPR: the after snapshot gained the observed
value `100` and the related test list included `equality_boundary_discounts`.
The seam class did not move because the current repo seam evidence still
reports the equality-boundary discriminator as missing.

That is an acceptable dogfood receipt. It tells the operator:

- the targeted test reached the intended seam;
- the rendered evidence changed;
- the current static classifier still did not close the grip class;
- runtime calibration was not run for this case.

This is the useful product behavior: the receipt records what improved and what
did not, without claiming runtime confirmation or hiding a static-model gap.

## Optional Runtime Calibration Sample

The fixture also includes a tiny imported runtime sample:

```bash
ripr calibrate cargo-mutants \
  --mutants-json fixtures/boundary_gap/calibration/runtime-mutants.json \
  --repo-exposure-json fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

Expected agreement summary:

```text
static_gap_and_runtime_signal: 0
static_gap_without_runtime_signal: 1
runtime_signal_without_static_gap: 0
static_clean_and_runtime_clean: 0
runtime_inconclusive: 0
```

That is the calibration value of this case. The static after snapshot still
reports a gap, while the supplied runtime data reports the imported mutant as
`caught`. The report keeps those facts separate instead of upgrading the static
classification.

Checked corpus calibration outputs:

- `fixtures/boundary_gap/calibration/mutation-calibration.json`
- `fixtures/boundary_gap/calibration/mutation-calibration.md`
