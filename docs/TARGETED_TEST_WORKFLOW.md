# Targeted Test Workflow

Use this workflow when you want to turn RIPR seam evidence into one focused
test and leave a reviewable receipt.

The loop is:

```text
before static seam evidence
-> pick one weak or missing discriminator
-> copy the targeted test brief or seam packet
-> add one focused test
-> after static seam evidence
-> targeted-test outcome receipt
-> optional SARIF, badge, and runtime calibration checks
```

RIPR is still a static test-grip tool. The receipt shows whether the rendered
static evidence improved. It does not claim runtime mutation confirmation.

## Inputs and Outputs

The workflow uses these artifacts:

| Artifact | Purpose |
| --- | --- |
| `repo-exposure.json` | Before/after classified seam evidence. |
| `repo-exposure.md` | Human-readable seam list for picking the next gap. |
| `agent-seam-packets.json` | Tool-readable work orders for targeted tests. |
| LSP seam diagnostics and hovers | Editor path for inspecting evidence and copying briefs. |
| `targeted-test-outcome.{json,md}` | Advisory receipt comparing before and after snapshots. |
| `repo-sarif` / `sarif-policy` | Optional CI/code-scanning projection with the same seam semantics. |
| repo badge artifacts | Optional public count projection under badge policy. |
| `mutation-calibration.{json,md}` | Optional runtime calibration join when cargo-mutants data exists. |
| [`fixtures/CALIBRATION_CORPUS.md`](../fixtures/CALIBRATION_CORPUS.md) | Controlled fixture index for trying the loop on known seam scenarios. |

`ripr check` commands work from an installed `ripr` binary. The receipt,
cockpit, badge-endpoint, and calibration summary commands shown here are
repo-local `cargo xtask` automation today.

Use a local scratch directory for before/after snapshots:

```bash
mkdir -p target/ripr/workflow
```

## 1. Capture the Before Snapshot

From an installed `ripr` binary:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json
ripr check --root . --mode ready --format repo-exposure-md > target/ripr/workflow/before.repo-exposure.md
ripr check --root . --mode ready --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json
```

From this repository, the same surfaces can be generated through Cargo:

```bash
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-md > target/ripr/workflow/before.repo-exposure.md
cargo run -p ripr -- check --root . --mode ready --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json
```

For normal repo-local dogfooding, the wrapper commands write the same report
families under `target/ripr/reports/`:

```bash
cargo xtask repo-exposure-report
cargo xtask agent-seam-packets .
```

Archive the JSON snapshot you want to compare before adding the test. The
targeted outcome report compares two files; it does not rerun analysis.

A small dogfood example lives in
[Targeted test boundary-gap case study](case-studies/TARGETED_TEST_BOUNDARY_GAP.md).
It shows a focused test that changed rendered static evidence while the seam
class stayed `weakly_gripped`.

## 2. Pick One Seam

Open `target/ripr/workflow/before.repo-exposure.md` or
`target/ripr/reports/repo-exposure.md`.

Pick one headline seam with a class such as:

- `weakly_gripped`
- `ungripped`
- `reachable_unrevealed`
- `activation_unknown`
- `observation_unknown`
- `discrimination_unknown`

Prefer the first seam where the missing discriminator is concrete:

- equality boundary value missing;
- exact error variant assertion missing;
- exact return value assertion missing;
- side-effect observer missing;
- field or object assertion missing.

Avoid starting with `opaque` unless the next task is to inspect a helper,
fixture, macro, or dynamic boundary that hides evidence from static analysis.

## 3. Copy the Work Order

### Editor Path

In VS Code, enable seam diagnostics through `ripr.toml`, initialization options,
or extension settings. Save the file before refreshing; the current LSP model is
saved-workspace analysis.

For the selected seam diagnostic:

- Hover the diagnostic to read the evidence path and classification reason.
- Use `Copy targeted test brief` for a human-readable work order.
- Use `Copy seam packet` when a coding agent wants structured JSON.
- Use `Copy suggested assertion` when a concrete assertion example exists.
- Use `Open best related test` to jump to the strongest imitation target.

The repo-local cockpit report verifies that the editor fixture exposes these
actions without opening VS Code:

```bash
cargo xtask lsp-cockpit-report
```

### CLI / Agent Path

Use `target/ripr/workflow/agent-seam-packets.json` or
`target/ripr/reports/agent-seam-packets.json` and match the selected
`seam_id`.

For a `write_targeted_test` packet, pass the whole packet or the targeted test
brief to the agent. It should use:

- `recommended_test` for suggested placement and name;
- `candidate_values` and `missing_discriminators` for the input to exercise;
- `assertion_shape` for the oracle style;
- `nearest_strong_test_to_imitate` and `patterns_to_imitate` for local style;
- `patterns_to_avoid` to avoid broad smoke-only assertions.

The agent must still derive expected values from code or product behavior. RIPR
does not invent expected outcomes.

## 4. Add One Focused Test

Keep the code change narrow:

- add one test or one small test case;
- imitate the best related test's layout and assertion style;
- exercise the missing discriminator directly;
- assert the affected value, variant, field, object, or side effect;
- avoid broad `is_ok()`, `is_err()`, snapshot-only, or smoke-only assertions
  unless the packet explicitly says that is the strongest visible oracle.

Do not edit production code in this step unless the test cannot be expressed
without a legitimate testability seam change. If production code changes too,
the before/after receipt may report new or removed seam IDs instead of a simple
movement for the original seam.

## 5. Capture the After Snapshot

After saving the new test, rerun the same repo exposure command:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json
ripr check --root . --mode ready --format repo-exposure-md > target/ripr/workflow/after.repo-exposure.md
```

For a repo checkout:

```bash
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-md > target/ripr/workflow/after.repo-exposure.md
```

## 6. Generate the Receipt

Compare before and after:

```bash
cargo xtask targeted-test-outcome \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json
```

Read:

```text
target/ripr/reports/targeted-test-outcome.md
target/ripr/reports/targeted-test-outcome.json
```

Interpretation:

| Bucket | Meaning |
| --- | --- |
| `moved` | Matched seam changed to an equal or stronger static grip class. |
| `unchanged` | Matched seam stayed in the same class. Inspect the evidence delta and the new test's oracle. |
| `regressed` | Matched seam moved to a lower-ranked static grip class. Review the test edit, config, and snapshot inputs. |
| `new` | A seam appears only in the after snapshot, usually because production code or analysis scope changed. |
| `removed` | A seam appears only in the before snapshot, usually because production code changed or seam identity changed. |

The cleanest receipt is one moved seam with evidence deltas such as:

- missing discriminator no longer reported;
- new observed value visible;
- stronger related oracle visible.

An unchanged seam is still useful evidence. It usually means the test missed the
original seam, the assertion remained too broad, the test reached through an
opaque helper, or the before/after snapshots did not use the same root, mode, or
config.

## 7. Align CI, SARIF, Badges, and Calibration

These checks are optional for the local targeted-test loop. They make the same
story visible in the surfaces a reviewer may see.

### SARIF / CI Policy

Generate repo-scoped seam SARIF:

```bash
ripr check --root . --mode ready --format repo-sarif > target/ripr/workflow/after.repo-sarif.json
```

Run the advisory policy report:

```bash
cargo xtask sarif-policy \
  --current target/ripr/workflow/after.repo-sarif.json \
  --mode advisory
```

Use `--mode baseline-check` or `--mode fail-on-new-warning` only when the
repository has deliberately adopted an opt-in baseline policy.

For a copyable GitHub Actions recipe, see
[CI strategy](CI.md#copyable-ripr-sarif-workflow).

### Badges

For this repository's checked-in badge endpoints:

```bash
cargo xtask repo-badge-artifacts
cargo xtask check-badge-endpoints
```

Badge counts are seam-native unresolved static gap counts under configuration.
They are not coverage or runtime mutation confirmation.

### Runtime Calibration

When cargo-mutants output exists, join it to the after static snapshot:

```bash
cargo xtask mutation-calibration . \
  --mutants-json target/ripr/workflow/cargo-mutants.json \
  --repo-exposure-json target/ripr/workflow/after.repo-exposure.json
```

The boundary-gap case study has a checked-in sample for trying this without
running mutation tests:

```bash
cargo xtask mutation-calibration . \
  --mutants-json fixtures/boundary_gap/calibration/runtime-mutants.json \
  --repo-exposure-json fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

Runtime mutation vocabulary belongs in calibration reports only. A static
movement receipt is a good reason to run less blind mutation work, not a
replacement for runtime confirmation.

The calibration report's agreement section distinguishes static gaps with
runtime gap signals, static gaps without runtime gap signals, runtime signals
without static gaps, static-clean/runtime-clean matches, and inconclusive
runtime labels. Treat those counts as tuning evidence for when to run runtime
mutation next, not as a replacement for the targeted-test receipt.

## Review Checklist

Before calling the targeted-test loop handled, make sure the PR or local change
has:

- a before `repo-exposure-json` snapshot;
- a named seam and `seam_id`;
- the targeted test brief or seam packet used as the work order;
- one focused test or test case;
- an after `repo-exposure-json` snapshot from the same root, mode, and config;
- a `targeted-test-outcome` receipt;
- optional SARIF, badge, and calibration artifacts only when they matter for
  the review surface.
