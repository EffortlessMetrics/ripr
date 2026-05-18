# Defaults-First Example Corpus

This index is the public operator corpus for Campaign 7. It points at small
checked fixtures and expected artifacts that demonstrate the default loop:

```text
static seam evidence -> targeted test brief -> rerun ripr -> receipt -> optional calibration
```

It is an index, not a new runner surface. Every executable fixture still lives
under `fixtures/<name>/` with `SPEC.md`, `diff.patch`, and
`expected/check.json`.

## Scenario Map

| Scenario | Fixture | What it demonstrates | Expected artifacts |
| --- | --- | --- | --- |
| Boundary gap | `fixtures/boundary_gap` | A predicate changes from `>` to `>=`; related tests reach and observe the owner but miss the equality-boundary value. | CLI goldens: `expected/check.json`, `expected/human.txt`; LSP expectations: `expected/lsp-diagnostics.json`, `expected/lsp-code-actions.json`; editor-agent loop artifacts under `expected/editor-agent-loop/`; targeted-test receipt and calibration artifacts under `calibration/`. |
| Editor LSP workflow | `fixtures/editor_lsp_workflow` | The same equality-boundary seam is projected through the saved-workspace editor loop. | CLI goldens: `expected/check.json`, `expected/human.txt`; editor expectations: `expected/lsp-diagnostics.json`, `expected/lsp-code-actions.json`, `expected/lsp-hover.md`, `expected/vscode-status.json`, and `expected/first-useful-action-status.json`. |
| Editor gap cockpit workflow | `fixtures/editor_gap_cockpit` | Read-only gap records are projected into diagnostics, hover, status, and bounded actions for Rust, preview static limits, disabled languages, wrong-root reports, stale artifacts, and no-action states. | Manifest-only editor expectations under each case: `expected/lsp-diagnostics.json`, `expected/lsp-code-actions.json`, `expected/lsp-hover.md`, `expected/vscode-status.json`, and `expected/gap-projection.json`. |
| Editor first-pr bridge workflow | `fixtures/editor_first_pr_bridge` | Read-only first-pr packet state is projected into setup diagnosis, status, bounded packet actions, and receipt-aware PR handoff guidance. | Manifest-only editor expectations under each case: `expected/vscode-status.json`, `expected/setup-diagnosis.md`, `expected/lsp-diagnostics.json`, `expected/lsp-code-actions.json`, and `expected/first-pr-status.json`. |
| Missing equality boundary | `fixtures/boundary_gap` | The static evidence names `discount_threshold (equality boundary)` as the missing discriminator and suggests an exact returned-value assertion. | `expected/lsp-code-actions.json` contains the copyable targeted-test brief; `calibration/targeted-test-outcome.md` records the after snapshot gaining observed value `100`. |
| Weak oracle | `fixtures/weak_error_oracle` | A related test reaches an error path but only asserts `is_err()`, so the oracle is broad rather than exact. | CLI goldens: `expected/check.json`, `expected/human.txt`. |
| Exact error variant | `fixtures/weak_error_oracle`, `fixtures/strong_error_oracle` | The weak fixture shows `AuthError::RevokedToken` as missing; the strong fixture is the control where `matches!` asserts the exact variant. | CLI goldens for both fixtures pin the missing-vs-present exact-variant evidence. |
| Opaque fixture/builder | `fixtures/opaque_fixture_builder` | A related exact assertion reaches the owner, but fixture/builder input construction hides the boundary value from static inspection. | CLI goldens: `expected/check.json`, `expected/human.txt`; output includes `infection_unknown`, `fixture_opaque`, and the fixture/builder next step. |
| Optional calibration | `fixtures/boundary_gap/calibration` | A tiny imported cargo-mutants sample joins by `seam_id` after the targeted test is visible in static evidence. | `runtime-mutants.json`, `after-targeted-test.repo-exposure.json`, `mutation-calibration.json`, and `mutation-calibration.md`. |

## Operator Loop Artifacts

The boundary-gap case is the complete worked loop:

```bash
cargo run -p ripr -- check \
  --root fixtures/boundary_gap/input \
  --diff fixtures/boundary_gap/diff.patch \
  --mode ready \
  --format agent-seam-packets-json

cargo run -p ripr -- outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json

cargo run -p ripr -- calibrate cargo-mutants \
  --mutants-json fixtures/boundary_gap/calibration/runtime-mutants.json \
  --repo-exposure-json fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

Checked artifacts for that loop:

- `fixtures/boundary_gap/expected/lsp-code-actions.json` carries the targeted
  test brief, suggested assertion, and open-related-test action.
- `fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json` is the
  seam-scoped agent packet copied from the editor command path.
- `fixtures/boundary_gap/expected/editor-agent-loop/agent-brief.json` is the
  agent working-set brief for the same seam.
- `fixtures/boundary_gap/expected/editor-agent-loop/agent-verify.json` compares
  the checked before and after snapshots.
- `fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json` narrows
  the verify result to the top seam.
- `fixtures/boundary_gap/expected/editor-agent-loop/operator-cockpit.json` and
  `operator-cockpit.md` show the cockpit join over the editor-agent artifacts.
- `fixtures/boundary_gap/expected/llm-work-loop/` pins the artifact-only LLM
  work-loop matrix for happy, unchanged, regressed, missing-artifact,
  stale-artifact, configured-off, path-with-spaces, and Windows-separator
  review-summary states.
- `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json`
  is the static before snapshot.
- `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json`
  is the static after snapshot where the focused test value is visible.
- `fixtures/boundary_gap/calibration/targeted-test-outcome.json` and
  `targeted-test-outcome.md` are the public receipt.
- `fixtures/boundary_gap/calibration/mutation-calibration.json` and
  `mutation-calibration.md` are the optional runtime calibration import.

## Validation

Run:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-fixture-contracts
cargo xtask lsp-cockpit-report
cargo xtask check-output-contracts
cargo xtask check-static-language
```

These commands keep the executable fixtures, CLI goldens, LSP cockpit fixtures,
output schemas, and static language policy aligned.
