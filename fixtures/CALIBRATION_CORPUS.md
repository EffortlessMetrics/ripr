# Calibration Corpus Index

This index maps existing executable fixtures to the calibration questions they
can answer. It is intentionally a catalog, not a new fixture runner surface.
Every directory directly under `fixtures/` remains an executable fixture with a
`SPEC.md`, `diff.patch`, and `expected/check.json`.

Use this file when choosing controlled scenarios for:

- before/after targeted-test outcome receipts;
- static/runtime mutation calibration imports;
- SARIF, badge, LSP, and report alignment checks;
- future bounded cargo-mutants artifacts.

For the public defaults-first example path that joins CLI, LSP,
targeted-test receipts, and optional calibration artifacts, see
[`EXAMPLE_CORPUS.md`](EXAMPLE_CORPUS.md).

## Scenario Set

| Scenario | Fixture | Static signal | Useful receipt |
| --- | --- | --- | --- |
| Boundary gap | `fixtures/boundary_gap` | Equality-boundary discriminator is missing from related tests. | `fixtures/boundary_gap/calibration/targeted-test-outcome.{json,md}` records the new observed boundary value; `fixtures/boundary_gap/calibration/runtime-mutants.json` and `mutation-calibration.{json,md}` show a runtime-clean calibration join for the after snapshot. |
| Strong boundary oracle | `fixtures/strong_boundary_oracle` | Exact boundary assertion is present. | Static-clean control for calibration agreement and badge/SARIF alignment. |
| Strong error oracle | `fixtures/strong_error_oracle` | Exact error variant oracle is present. | Static-clean control for calibration agreement and related-test ranking. |
| Weak error oracle | `fixtures/weak_error_oracle` | Related tests use broad error assertions without the exact variant. | Targeted-test receipt should show improvement when the exact variant assertion is added. |
| Snapshot oracle | `fixtures/snapshot_oracle` | Snapshot-style oracle is visible but broad. | Static-only weak-oracle control; runtime confirmation is optional and separate. |
| Token-only mention | `fixtures/unrelated_test_mentions_token` | Test text mentions changed tokens without a real owner call. | False-positive guard for static relation evidence. |
| Formatting-only diff | `fixtures/format_only_diff` | No behavior probe should be emitted for formatting churn. | Noise-control baseline for adoption docs and CI recipes. |
| Comment-only diff | `fixtures/comment_only_diff` | No behavior probe should be emitted for comment churn. | Noise-control baseline for adoption docs and CI recipes. |
| Import-only diff | `fixtures/import_only_diff` | No behavior probe should be emitted for import churn. | Noise-control baseline for adoption docs and CI recipes. |
| Syntax variants | `fixtures/boundary_gap_multiline_assert`, `fixtures/boundary_gap_nested_tests`, `fixtures/boundary_gap_reordered_tests`, `fixtures/weak_error_oracle_assert_matches` | Equivalent test evidence should stay stable across harmless layout variants. | Regression guard for refactors that touch syntax extraction or related-test ranking. |

## Runtime Calibration Artifacts

The corpus now includes one tiny checked-in runtime sample:

| Case | Static input | Runtime input | Command |
| --- | --- | --- | --- |
| Boundary gap after targeted test | `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json` | `fixtures/boundary_gap/calibration/runtime-mutants.json` | `cargo xtask mutation-calibration . --mutants-json fixtures/boundary_gap/calibration/runtime-mutants.json --repo-exposure-json fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json` |

The boundary-gap runtime sample imports one `caught` outcome for seam
`67fc764ba37d77bd`. It exists to exercise the calibration report path and to
show the honest disagreement case: the static after snapshot still says
`weakly_gripped`, while the supplied runtime data is clean for this mutant.
The checked `mutation-calibration.{json,md}` files pin the expected report
shape for the defaults-first example corpus.

## Missing Runtime Calibration Artifacts

Runtime mutation calibration still needs small checked-in sample artifacts for
these cases:

| Planned case | Purpose | Status |
| --- | --- | --- |
| `mock_expectation` | Show when a mock expectation observes a side effect strongly enough for a seam. | Planned. |
| `side_effect_observer` | Compare static side-effect evidence with a runtime signal. | Planned. |
| `opaque_dynamic_dispatch` | Keep static limitations explicit when runtime data sees behavior behind dynamic dispatch. | Planned. |
| `weak_snapshot_oracle` | Compare broad snapshot evidence with runtime mutation data without changing static language. | Planned. |
| `ambiguous_file_line_join` | Preserve ambiguous runtime joins without assigning a record to the first static seam. | Planned. |

Runtime artifacts should be tiny, deterministic samples checked in only after
their source and update command are documented. They should feed
`cargo xtask mutation-calibration`; they should not make fixture execution run
mutation testing.

## Operator Path

For a controlled calibration pass:

```bash
cargo xtask fixtures boundary_gap
cargo run -p ripr -- check --root fixtures/boundary_gap/input --diff fixtures/boundary_gap/diff.patch --format repo-exposure-json > target/ripr/before.json

# Add a focused test in a working copy or fixture variant.

cargo run -p ripr -- check --root fixtures/boundary_gap/input --diff fixtures/boundary_gap/diff.patch --format repo-exposure-json > target/ripr/after.json
cargo xtask targeted-test-outcome --before target/ripr/before.json --after target/ripr/after.json
```

When runtime mutation data is available, keep it in the calibration lane:

```bash
cargo xtask mutation-calibration . --mutants-json <mutants-json> --repo-exposure-json target/ripr/after.json
```

The targeted-test receipt remains a static evidence movement receipt. Runtime
mutation agreement appears only in `mutation-calibration.{json,md}`.
