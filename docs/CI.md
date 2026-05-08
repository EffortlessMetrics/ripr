# CI Strategy

CI should protect correctness without making ordinary contribution slow or
noisy. Default CI is advisory for static exposure findings until calibration and
configuration are mature enough to support opt-in failure policies.

## Verification Economics Policy

CI is a product surface. A contributor should be able to tell what ran, why it
ran, what it cost, what it produced, and which explicit label or follow-up
artifact changes that behavior.

`ripr` uses **Local Evidence Minutes** (LEM) as the planning unit for CI cost.
One LEM is approximately one minute of hosted CI time on one normal GitHub
runner, including setup, toolchain/cache work, command runtime, report writing,
and artifact upload for that lane. LEM is intentionally approximate until
`target/ci/ci-actuals.json` exists; PRs should still estimate the order of
magnitude so reviewers can notice when a small docs change starts paying for a
release-style proof.

Budget bands:

| Band | Estimated cost | Expected posture |
| --- | ---: | --- |
| `small` | 0-5 LEM | docs, policy metadata, or focused code checks |
| `medium` | 6-20 LEM | ordinary product PR with Rust and policy gates |
| `large` | 21-60 LEM | multi-surface PR, extension checks, or broad evidence artifacts |
| `release` | 60+ LEM | explicit `release-check` or `full-ci` proof |

CI lanes are grouped by posture, not by how convenient they are to place in one
workflow file.

| Posture | Purpose | Examples | Default behavior |
| --- | --- | --- | --- |
| Required | Cheap merge-safety and policy invariants. | `fmt`, `cargo check`, clippy, focused tests, static-language, file/workflow/process/dependency policy, output-contract checks for schema/output changes. | Blocking on ordinary PRs that touch the relevant surface. |
| Advisory | Evidence that helps review but should not block routine work until calibrated. | coverage, Test Analytics, `ripr` self-dogfood, SARIF upload, agent-loop artifacts, Droid review, future Clippy lints, broad security posture scans. | Upload artifacts or comments; do not fail the PR by default. |
| On-demand / release | Expensive, slow, or release-bearing proof. | `cargo package`, `cargo publish --dry-run`, VSIX packaging, server archive checks, release readiness, full workspace proof. | Run on `main`, manual dispatch, `release-check`, or `full-ci`; avoid default PR blocking. |

The current `ci.yml` still carries some release-like proof in the primary Rust
job. Treat that as legacy posture while the CI split is rolled out. New CI work
should move toward small required gates at the front door, advisory evidence by
default, and label-gated release proof.

This section defines the target policy. It does not mean the current workflows
already implement PR planning, label-gated lane selection, CI actuals, or
budget enforcement. Until those later PRs land, the "Current Workflows" section
below remains the source of truth for what GitHub Actions runs today.

### PR Planning

Every pull request should eventually get a cheap CI forecast before heavier
lanes run. The planned `target/ci/ci-plan.json` artifact should record:

- changed files;
- detected risk packs;
- expected required, advisory, and on-demand lanes;
- estimated LEM;
- labels that changed lane selection;
- artifact families expected from each lane.

Example step summary:

```text
PR Plan
- Scope: Rust product + docs
- Required lanes: rust, policy, output-contracts
- Advisory lanes: coverage, ripr-self-dogfood
- Skipped by default: vscode, release package, future-clippy
- Estimated cost: 14 LEM
- To run all: add full-ci
```

The active PR Plan workflow is structural advisory today: it runs on opened,
synchronized, reopened, labeled, and unlabeled pull requests, uploads the
changed-file list, and writes a placeholder step summary. Until the numeric
planner exists, authors should still fill the PR template's CI economics
section for CI-affecting changes.

### Risk Packs

Risk packs are the planned machine-readable replacement for broad path guesses.
They map changed paths to lanes and artifacts. The first implementation should
live in policy files such as `policy/ci-risk-packs.toml` and should start
structural: validate that packs, lane names, and schema versions exist before
trying to infer perfect cost.

Initial pack shape:

```toml
[risk_pack.rust_product]
paths = ["crates/ripr/src/**"]
required = ["rust", "policy", "output-contracts"]
advisory = ["coverage", "ripr-self-dogfood"]

[risk_pack.vscode]
paths = ["editors/vscode/**"]
required = ["vscode-compile", "vscode-e2e"]
advisory = []

[risk_pack.docs_only]
paths = ["docs/**", "README.md", "CHANGELOG.md"]
required = ["docs", "static-language"]
advisory = []
```

Risk packs must stay explainable. If a lane runs because a pack matched, the PR
plan should name the pack and paths that triggered it.

The seed policy ledgers are machine-readable but non-enforcing:

- `policy/ci-budget.toml` records LEM bands, label effects, and default budget
  posture;
- `policy/ci-lane-whitelist.toml` records allowed target lane IDs and artifact
  families;
- `policy/ci-risk-packs.toml` maps changed path families to required,
  advisory, and on-demand lane IDs;
- `policy/ci-whitelist-exceptions.toml` records current workflow behavior that
  intentionally differs from the target policy while the split rolls out.

`cargo xtask check-ci-lane-whitelist` validates these files structurally:
schema version, lane IDs, label IDs, artifact family IDs, owners, and reasons.
It does not fail a PR because a risk pack matched or an estimate changed.

### Artifact Families

Generated artifacts should have predictable paths and one index. Planned CI
artifacts are grouped by family:

| Family | Expected paths |
| --- | --- |
| `ci-plan` | `target/ripr/reports/pr-plan-changes.txt`, `target/ci/ci-plan.json`, `target/ci/ci-actuals.json` |
| `ripr-evidence` | `target/ripr/reports/index.md`, `target/ripr/reports/repo-exposure.json`, `target/ripr/reports/repo-sarif.json` |
| `editor-agent-loop` | `target/ripr/reports/operator-cockpit.{json,md}`, `target/ripr/reports/agent-receipt.json`, `target/ripr/workflow/agent-seam-packets.json`, `target/ripr/workflow/workflow.json`, `target/ripr/workflow/commands.md`, `target/ripr/workflow/agent-status.{json,md}`, `target/ripr/workflow/agent-review-summary.{json,md}`, `target/ripr/workflow/agent-packet.json`, `target/ripr/workflow/agent-brief.json`, `target/ripr/workflow/agent-verify.json`, plus compatibility copies under `target/ripr/agent/` |
| `release-readiness` | package lists, publish dry-run transcript, VSIX package proof, server archive proof |

The report index should be the front door for artifact discovery. CI should not
require reviewers to inspect raw job logs to find the packet that justifies a
decision.

The `pr-plan-changes.txt` file is the current structural advisory artifact;
the `target/ci/ci-plan.json` forecast remains planned. The `editor-agent-loop`
paths reflect the current split between the local bulk packet envelope
(`agent-seam-packets.json`) and generated CI's focused agent artifacts under
`target/ripr/agent/`.

### Label Policy

Labels are policy inputs, not folklore. Each supported label must have one
documented effect:

These label effects are the target policy. Active workflow switches are called
out below; remaining label effects stay documented until follow-up PRs
implement and validate the lane-selection logic.

| Label | Effect |
| --- | --- |
| `full-ci` | Run required, advisory, and release-like lanes. Demotes `ripr-waive` for this PR. Expected to cost more. |
| `release-check` | Run package, publish dry-run, VSIX package, server archive, and release-readiness proof where applicable. |
| `vscode` | Run editor extension lanes even when no editor path changed. |
| `coverage` | Run coverage lanes and upload coverage artifacts. |
| `ripr-waive` | Acknowledge a soft static exposure finding for this PR. Does not skip CI and does not apply when `full-ci` is present. |
| `ci-budget-ack` | Acknowledge that this PR intentionally exceeds the expected LEM band. |
| `clippy-future` | Run future or candidate Clippy lint lanes in advisory mode. |

New labels that affect CI must update this table, the PR template, and the
budget/risk-pack policy files in the same PR.

These labels are the documented target vocabulary. Today, `release-check` and
`full-ci` activate the Rust workflow's package list and publish dry-run steps
on pull requests. Other label effects remain target vocabulary until a later PR
wires them into a PR plan or workflow condition. The GitHub Settings App
contract in `.github/settings.yml` codifies these label names, descriptions,
and colors so the reviewable vocabulary does not drift in the GitHub UI.

### Cheaper Signal First

When adding CI coverage for a failure mode, prefer the cheapest stable signal
that catches the issue:

1. static policy check;
2. focused unit test;
3. fixture or golden output;
4. integration smoke;
5. advisory report;
6. release-style proof.

Do not add a broad required workflow when a local `xtask` checker or focused
test can catch the same failure earlier with clearer repair instructions.

### CI Actuals

Forecasts should become measurable. Planned lane actuals should emit
`target/ci/ci-actuals.json` with one record per lane:

```json
{
  "schema_version": "0.1",
  "workflow": "ci",
  "job": "rust",
  "status": "success",
  "duration_seconds": 212,
  "runner": "ubuntu-latest",
  "estimated_lem": 8,
  "actual_lem": 9,
  "cache_hit": true
}
```

Budget guards should remain advisory until the repo has enough actuals to
separate normal variance from waste.

### Rollback

Every CI-affecting PR should describe how to back out the change without
weakening branch safety. Examples:

- remove a new advisory workflow without changing required gates;
- revert a risk pack while keeping the old required lane;
- disable an artifact upload while keeping the underlying local report command;
- move a release proof back to manual dispatch if it proves too costly.

If rollback requires branch-protection changes, the PR must say so explicitly
and should usually be split.

## Current Workflows

The Rust workflow currently runs:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-readme-state
cargo xtask markdown-links
cargo xtask check-campaign
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

On pushes to `main` or `master`, and on pull requests labeled `release-check`
or `full-ci`, the Rust workflow also runs the release-surface package checks:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

The CI workflow also has an explicit MSRV job that pins Rust `1.93.1` and runs:

```bash
cargo check --workspace --all-targets
```

The main Rust job stays on `stable` so routine CI also proves the current stable
toolchain, while the MSRV job proves the declared workspace baseline.

Local shaping commands are intentionally separate from CI because they mutate
the worktree:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
```

They are safe to run before checks. `shape` runs `cargo fmt`, sorts allowlists,
ensures `target/ripr/reports`, and writes a local report. `fix-pr` currently
runs `shape`, refreshes `pr-summary`, and writes a local fix-pr report.
`pr-summary` writes `target/ripr/reports/pr-summary.md` from git diff/status.
`precommit` is the cheap non-mutating local guardrail. `check-pr` is the
review-ready local gate and intentionally does not run package or publish
dry-run checks. `fixtures` and `goldens check` validate the current fixture and
expected-output scaffolding without accepting output drift. `golden-drift`
writes advisory Markdown and JSON summaries of semantic expected-output drift
for reviewers. `test-oracle-report` writes an advisory baseline for the strength
of `ripr`'s own Rust test oracles. `dogfood` writes a non-blocking
`ripr`-on-`ripr` report from stable fixture diffs. `critic` writes an advisory
adversarial review packet from the current diff, reports, and receipts.
`reports index` writes a reviewer front door for generated reports.
`receipts` writes machine-readable gate evidence under `target/ripr/receipts`,
and `receipts check` validates the receipt set.

The fuller automation model is documented in [PR automation](PR_AUTOMATION.md).
Deterministic shaping should happen locally; CI should verify the committed
tree and upload reports when available.

Codex Goals runs should treat CI artifacts as campaign receipts. A campaign can
advance through multiple work items, but each scoped PR should leave the same
shape/check/report artifacts that CI uploads for human review.

Current policy checks write Markdown reports to `target/ripr/reports` when they
run. The Rust workflow generates `target/ripr/reports/index.md`, writes it to
the GitHub Actions job summary when present, and uploads the report and receipt
directories as the `ripr-pr-reports` artifact.

Local policy checks can also be run directly:

```bash
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-readme-state
cargo xtask markdown-links
cargo xtask check-campaign
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-supply-chain
cargo xtask check-process-policy
cargo xtask check-network-policy
```

Fixture and golden scaffolding checks can be run directly with:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
```

The VS Code workflow currently runs:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
xvfb-run -a npm run test:e2e
```

The `test:e2e` step launches a headless VS Code instance via `@vscode/test-electron`, activates the extension in a fixture Rust workspace, and runs the smoke test suite. `xvfb-run` provides the virtual display required on Linux CI runners.

The VS Code extension build and extension publish workflows use Node 24. This
is separate from the VS Code extension-host compatibility declared in
`editors/vscode/package.json`.

The coverage workflow currently runs:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

It uploads `lcov.info` as the `rust-lcov` GitHub Actions artifact and uploads
the same file to Codecov with the `rust` flag and `rust-workspace` upload name.

Codecov uses the repository `CODECOV_TOKEN` secret. Codecov upload failures are
blocking for trusted coverage runs: pushes and same-repository pull requests.
Fork pull requests still generate `lcov.info` and upload the `rust-lcov`
GitHub Actions artifact, but skip the Codecov upload because repository secrets
are unavailable to those runs.

Codecov project and patch status checks are not yet branch-protection gates.
After the emitted status names and baseline are stable, a later scoped PR can
ratchet Codecov status requirements and branch protection separately.

**Coverage Baseline Calibration**

As of 2026-05-04, the main branch coverage baseline is stable at **75.5%**
(product crate: 94.8%, automation: 59%). The project target of 75% in
`codecov.yml` is appropriate for this baseline.

Codecov now tracks product and automation coverage separately to prevent
automation code from obscuring product quality:

- **Product crate** (crates/ripr/src/): target 94% (project), 94% (patch), threshold 1%/3%
- **Automation** (xtask/src/): target 59% (project), 75% (patch), threshold 1%/10%
  The automation project target aligns with the current 59% baseline, allowing ratchet
  growth as xtask debt is paid down. The patch threshold of 10% provides initial ratchet
  tolerance for the large, unevenly-tested xtask main.rs.

The component split uses Codecov's path-based named statuses. Future coverage
ratchets should follow the [calibration strategy](IMPLEMENTATION_CAMPAIGNS.md).

The Test Analytics workflow currently runs:

```bash
cargo nextest run --workspace --all-features --profile ci
cargo test --workspace --doc
```

It uploads the JUnit XML as the `rust-junit` GitHub Actions artifact and uploads
the same file to Codecov Test Analytics only when `CODECOV_TOKEN` is available
on trusted runs. Fork pull requests still run tests and upload the artifact, but
skip the Codecov test-results upload because repository secrets are unavailable.

## SARIF and Policy Contract

Campaign 5B SARIF work is governed by
[RIPR-SPEC-0008](specs/RIPR-SPEC-0008-sarif-ci-policy.md). The contract is
advisory by default: generating SARIF must not make ordinary pull requests
block unless an explicit baseline policy mode is requested.

The defaults-first adoption contract in
[RIPR-SPEC-0009](specs/RIPR-SPEC-0009-defaults-first-adoption.md) keeps that
stance for first-run CI recipes: copyable or generated GitHub Actions should
upload review guidance by default, not fail CI unless the repository opts into
a baseline policy.

SARIF artifact commands:

```bash
cargo run -p ripr -- check --format sarif > target/ripr/reports/ripr-findings.sarif.json
cargo run -p ripr -- check --format repo-sarif > target/ripr/reports/ripr-seams.sarif.json
```

SARIF consumes configured severity from `ripr.toml`:

| Config severity | SARIF behavior |
| --- | --- |
| `warning` | `level: "warning"` |
| `info` | `level: "note"` |
| `note` | `level: "note"` |
| `off` | omitted |

The opt-in baseline policy compares current SARIF against a checked-in baseline
using `ruleId` plus `partialFingerprints.riprFingerprintV1`.

The local policy command writes `target/ripr/reports/sarif-policy.{json,md}`:

```bash
cargo xtask sarif-policy \
  --current target/ripr/reports/ripr-seams.sarif.json \
  --baseline .ripr/sarif-baseline.json \
  --mode baseline-check
```

To make new warning-level results blocking, opt in explicitly:

```bash
cargo xtask sarif-policy \
  --current target/ripr/reports/ripr-seams.sarif.json \
  --baseline .ripr/sarif-baseline.json \
  --mode fail-on-new-warning
```

Missing baselines remain advisory by default. Use `--missing-baseline error`
only when the repository has deliberately adopted a required SARIF baseline.

Policy modes:

| Mode | Default? | Behavior |
| --- | --- | --- |
| `advisory` | yes | Emit reports and exit successfully. |
| `baseline-check` | no | Report new configured-warning results relative to a baseline. |
| `fail-on-new-warning` | no | Exit non-zero when new configured-warning results appear. |

### Copyable RIPR Advisory Workflow

External repositories can start with a non-blocking pull-request workflow that
installs `ripr`, runs the defaults-first pilot loop, writes repo report and
badge artifacts, uploads them for review, and optionally publishes SARIF to
GitHub code scanning:

```bash
ripr init --ci github
```

The generated workflow matches the recipe below. It uploads the pilot, report,
and agent artifact directories; if the repository is the RIPR source tree, it
also renders the repo-local operator cockpit through xtask. The official GitHub
SARIF upload documentation uses `github/codeql-action/upload-sarif@v4`; keep
the RIPR job, artifact upload, and optional SARIF steps advisory until the
repository has chosen a baseline policy.

For a CI-first user, the useful output is the artifact packet:

- `target/ripr/pilot/` - first-screen pilot summary, repo exposure snapshot,
  and agent seam packets;
- `target/ripr/workflow/` - selected-seam workflow manifest, commands,
  status JSON/Markdown, review summary JSON/Markdown, and agent packet,
  brief, and verify JSON when a top seam is available;
- `target/ripr/agent/` - compatibility copies of packet, brief, verify, and
  receipt JSON for the top seam when one is available;
- `target/ripr/reports/` - targeted-test outcome, SARIF files when enabled,
  repo badge JSON, `agent-receipt.json`, and any repo-local cockpit output.
- `target/ripr/review/` - PR test guidance JSON and Markdown when
  `ripr review-comments` runs on pull requests.

The workflow also writes a `RIPR advisory summary` step summary. It includes
the top recommendation, the agent review packet when present, artifact links,
SARIF and badge status, known limits, and PR guidance annotation counts when
`target/ripr/review/comments.json` exists. On pull requests, the generated
workflow writes that report before emitting changed-line check annotations by
default without posting inline review comments.

See [LLM operator guide](LLM_OPERATOR_GUIDE.md) for the same status, workflow
packet, verify, receipt, and reviewer-summary loop outside CI. See
[PR review guidance](PR_REVIEW_GUIDANCE.md) for the PR-facing annotation
contract and review workflow.

### PR Test Guidance Annotations

RIPR-SPEC-0012 defines the pinned planning contract for the PR-facing
projection of the same evidence packet. The default CI surface is a GitHub job
summary plus check annotations. Inline PR review comments should
remain opt-in because they create durable review-thread noise when ranking or
placement is wrong.

The generated workflow runs the pure renderer on pull requests:

```bash
ripr review-comments \
  --root . \
  --base "$GITHUB_BASE_SHA" \
  --head "$GITHUB_SHA" \
  --out target/ripr/review/comments.json
```

That renderer writes JSON and Markdown under `target/ripr/review/` and does
not post to GitHub by itself. The generated workflow then:

- appends the Markdown summary to `$GITHUB_STEP_SUMMARY`;
- emits check annotations from changed-line entries;
- uploads the JSON and Markdown as artifacts;
- keeps inline PR review comments disabled by default.

Selection and placement must stay conservative:

- comment only when production Rust changed and a visible actionable seam maps
  to the changed region or owner function;
- skip recommendations when a nearby test changed in the pull request;
- target only changed lines, otherwise fall back to summary-only guidance;
- cap inline review comments to three by default;
- include the missing discriminator, suggested assertion shape, recommended
  test file, related test to imitate, and `ripr agent brief` command when
  available.

The LLM guidance in annotations is bounded handoff material. It should ask for
one focused test, avoid production edits unless explicitly requested, and point
to `ripr agent verify` after the edit. It must not ask an LLM to decide which
diff regions matter, run mutation testing, or claim runtime confirmation.

The generated workflow does not include an inline review-comment publisher.
Teams that add one in their own workflow must make it explicit opt-in, post
only from `comments[]`, target changed lines only, cap comment count, and
deduplicate by `dedupe_key`.

```yaml
name: RIPR

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  security-events: write

env:
  RIPR_UPLOAD_SARIF: "true"
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Install ripr
        run: cargo install ripr --locked

      - name: Generate RIPR pilot packet
        continue-on-error: true
        run: |
          ripr pilot \
            --root . \
            --out target/ripr/pilot \
            --mode ready \
            --max-seams 5

      - name: Prepare RIPR editor-agent artifacts
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports target/ripr/agent target/ripr/workflow
          if [ -f target/ripr/pilot/repo-exposure.json ]; then
            cp target/ripr/pilot/repo-exposure.json target/ripr/reports/repo-exposure.json
            cp target/ripr/pilot/repo-exposure.json target/ripr/workflow/before.repo-exposure.json
          fi
          if [ -f target/ripr/pilot/agent-seam-packets.json ]; then
            cp target/ripr/pilot/agent-seam-packets.json target/ripr/workflow/agent-seam-packets.json
          fi
          if [ -f target/ripr/pilot/pilot-summary.json ]; then
            top_seam_id="$(jq -r '.top_actionable_seams[0].seam_id // empty' target/ripr/pilot/pilot-summary.json 2>/dev/null || true)"
            if [ -n "$top_seam_id" ] && [ "$top_seam_id" != "null" ]; then
              echo "RIPR_TOP_SEAM_ID=$top_seam_id" >> "$GITHUB_ENV"
            fi
          fi

      - name: Generate RIPR agent loop artifacts
        if: always() && env.RIPR_TOP_SEAM_ID != ''
        continue-on-error: true
        run: |
          ripr agent start \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --out target/ripr/workflow
          ripr agent packet \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --json \
            > target/ripr/workflow/agent-packet.json
          cp target/ripr/workflow/agent-packet.json target/ripr/agent/agent-packet.json
          cp target/ripr/workflow/agent-brief.json target/ripr/agent/agent-brief.json
          ripr check \
            --root . \
            --mode ready \
            --format repo-exposure-json \
            > target/ripr/workflow/after.repo-exposure.json
          cp target/ripr/workflow/after.repo-exposure.json target/ripr/pilot/after.repo-exposure.json
          ripr agent verify \
            --root . \
            --before target/ripr/workflow/before.repo-exposure.json \
            --after target/ripr/workflow/after.repo-exposure.json \
            --json \
            > target/ripr/workflow/agent-verify.json
          cp target/ripr/workflow/agent-verify.json target/ripr/agent/agent-verify.json
          ripr agent receipt \
            --root . \
            --verify-json target/ripr/workflow/agent-verify.json \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --json \
            --out target/ripr/reports/agent-receipt.json
          cp target/ripr/reports/agent-receipt.json target/ripr/agent/agent-receipt.json
          ripr outcome \
            --before target/ripr/workflow/before.repo-exposure.json \
            --after target/ripr/workflow/after.repo-exposure.json \
            --format json \
            --out target/ripr/reports/targeted-test-outcome.json

      - name: Capture pull request diff
        if: github.event_name == 'pull_request'
        run: |
          mkdir -p target/ripr/reports
          git diff --binary "origin/${{ github.base_ref }}...HEAD" > target/ripr/reports/pr.diff

      - name: Run RIPR PR guidance report
        if: github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          mkdir -p target/ripr/review
          ripr review-comments \
            --root . \
            --base "origin/${{ github.base_ref }}" \
            --head HEAD \
            --out target/ripr/review/comments.json

      - name: Capture RIPR gate labels
        if: always() && github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          mkdir -p target/ci
          jq -c '{labels: [.pull_request.labels[]?.name]}' "$GITHUB_EVENT_PATH" > target/ci/labels.json

      - name: Render diff SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          ripr check \
            --root . \
            --diff target/ripr/reports/pr.diff \
            --format sarif \
            > target/ripr/reports/ripr-findings.sarif

      - name: Render repo seam SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true'
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-sarif \
            > target/ripr/reports/ripr-seams.sarif

      - name: Render RIPR repo badge artifacts
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-json \
            > target/ripr/reports/repo-ripr-badge.json
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-shields \
            > target/ripr/reports/repo-ripr-badge-shields.json

      - name: Render RIPR operator cockpit
        if: always() && hashFiles('crates/ripr/Cargo.toml') != '' && hashFiles('xtask/src/reports/operator.rs') != ''
        continue-on-error: true
        run: cargo xtask operator-cockpit

      - name: Evaluate RIPR gate decision
        if: always() && env.RIPR_GATE_MODE != '' && hashFiles('target/ripr/review/comments.json') != ''
        run: |
          mkdir -p target/ripr/reports
          gate_args=(
            gate evaluate
            --root .
            --pr-guidance target/ripr/review/comments.json
            --mode "$RIPR_GATE_MODE"
            --out target/ripr/reports/gate-decision.json
            --out-md target/ripr/reports/gate-decision.md
          )
          if [ -f target/ripr/reports/repo-exposure.json ]; then
            gate_args+=(--repo-exposure target/ripr/reports/repo-exposure.json)
          fi
          if [ -f target/ci/labels.json ]; then
            gate_args+=(--labels-json target/ci/labels.json)
          fi
          if [ -f target/ripr/workflow/agent-verify.json ]; then
            gate_args+=(--agent-verify target/ripr/workflow/agent-verify.json)
          fi
          if [ -f target/ripr/reports/agent-receipt.json ]; then
            gate_args+=(--agent-receipt target/ripr/reports/agent-receipt.json)
          fi
          if [ -n "${RIPR_GATE_BASELINE:-}" ]; then
            gate_args+=(--baseline "$RIPR_GATE_BASELINE")
          fi
          ripr "${gate_args[@]}"

      - name: Render RIPR LLM work-loop summaries
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/workflow
          ripr agent status \
            --root . \
            --json \
            > target/ripr/workflow/agent-status.json
          ripr agent status \
            --root . \
            > target/ripr/workflow/agent-status.md
          ripr agent review-summary \
            --root . \
            --json \
            > target/ripr/workflow/agent-review-summary.json
          ripr agent review-summary \
            --root . \
            > target/ripr/workflow/agent-review-summary.md

      - name: Emit RIPR PR guidance annotations
        if: always() && hashFiles('target/ripr/review/comments.json') != ''
        continue-on-error: true
        run: |
          escape_github_message() {
            local value="$1"
            value="${value//'%'/'%25'}"
            value="${value//$'\r'/'%0D'}"
            value="${value//$'\n'/'%0A'}"
            printf '%s' "$value"
          }

          escape_github_property() {
            local value="$1"
            value="${value//'%'/'%25'}"
            value="${value//$'\r'/'%0D'}"
            value="${value//$'\n'/'%0A'}"
            value="${value//':'/'%3A'}"
            value="${value//','/'%2C'}"
            printf '%s' "$value"
          }

          jq -r '.comments[]? | select(.placement.path and .placement.line) | [.placement.path, (.placement.line | tostring), (.reason // "RIPR targeted test guidance"), (.llm_guidance.command // "")] | @tsv' target/ripr/review/comments.json \
            | while IFS="$(printf '\t')" read -r path line reason command; do
                message="$reason"
                if [ -n "$command" ] && [ "$command" != "null" ]; then
                  message="$message Command: $command"
                fi
                annotation_path="$(escape_github_property "$path")"
                annotation_line="$(escape_github_property "$line")"
                annotation_title="$(escape_github_property "RIPR targeted test guidance")"
                message="$(escape_github_message "$message")"
                echo "::warning file=$annotation_path,line=$annotation_line,title=$annotation_title::$message"
              done

      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        run: |
          {
            echo '## RIPR advisory summary'
            echo
            echo "RIPR is advisory static evidence. It does not edit source, generate tests, or run mutation testing."
            echo
            echo '### Top recommendation'
            if [ -f target/ripr/pilot/pilot-summary.md ]; then
              cat target/ripr/pilot/pilot-summary.md
            else
              echo "Pilot summary was not generated. Inspect the uploaded artifact packet and job logs."
            fi
            echo
            echo '### Agent review packet'
            if [ -f target/ripr/workflow/agent-review-summary.md ]; then
              cat target/ripr/workflow/agent-review-summary.md
            else
              echo 'Agent review summary was not generated. Run `ripr agent status --root .` locally or inspect uploaded workflow artifacts.'
            fi
            echo
            echo '### Artifact packet'
            echo '- Pilot reports: `target/ripr/pilot/`'
            echo '- Agent workflow: `target/ripr/workflow/`'
            echo '- Agent compatibility copies: `target/ripr/agent/`'
            echo '- Repo reports, badges, SARIF, and receipts: `target/ripr/reports/`'
            echo '- CI labels and plan inputs: `target/ci/`'
            if [ -d target/ripr/review ]; then
              echo '- PR test guidance report: `target/ripr/review/`'
            else
              echo "- PR test guidance report: not generated yet"
            fi
            echo
            echo '### Gate decision'
            if [ -f target/ripr/reports/gate-decision.md ]; then
              cat target/ripr/reports/gate-decision.md
            else
              echo 'Gate decision was not run. Set `RIPR_GATE_MODE` to `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate` to opt in.'
            fi
            echo
            echo '### SARIF and badge status'
            if [ "${RIPR_UPLOAD_SARIF:-}" = "true" ]; then
              if [ -f target/ripr/reports/ripr-findings.sarif ]; then echo "- Diff SARIF: generated"; else echo "- Diff SARIF: missing or skipped"; fi
              if [ -f target/ripr/reports/ripr-seams.sarif ]; then echo "- Repo seam SARIF: generated"; else echo "- Repo seam SARIF: missing or skipped"; fi
            else
              echo '- SARIF upload: disabled by `RIPR_UPLOAD_SARIF`'
            fi
            if [ -f target/ripr/reports/repo-ripr-badge.json ]; then echo "- Badge JSON: generated"; else echo "- Badge JSON: missing or skipped"; fi
            if [ -f target/ripr/reports/repo-ripr-badge-shields.json ]; then echo "- Badge Shields JSON: generated"; else echo "- Badge Shields JSON: missing or skipped"; fi
            echo
            echo '### PR guidance annotations'
            if [ -f target/ripr/review/comments.json ]; then
              comments="$(jq -r '.summary.comments // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              summary_only="$(jq -r '.summary.summary_only // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              suppressed="$(jq -r '.summary.suppressed // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              echo "- Changed-line annotations emitted: $comments"
              echo "- Summary-only recommendations: $summary_only"
              echo "- Suppressed recommendations: $suppressed"
            else
              echo 'No PR test guidance report was generated. When `ripr review-comments` writes `target/ripr/review/comments.json`, this workflow emits changed-line check annotations by default.'
            fi
            echo
            echo '### Known limits'
            echo "- Advisory static evidence only; review the named seam and write one focused test."
            echo "- No automatic source edits or generated tests."
            echo "- No runtime mutation execution is performed by this workflow."
          } >> "$GITHUB_STEP_SUMMARY"

      - name: Upload RIPR report artifacts
        if: always()
        continue-on-error: true
        uses: actions/upload-artifact@v7
        with:
          name: ripr-reports
          path: |
            target/ripr/pilot
            target/ripr/agent
            target/ripr/workflow
            target/ripr/reports
            target/ripr/review
            target/ci
          if-no-files-found: ignore
          retention-days: 14

      - name: Upload RIPR diff findings
        if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request' && hashFiles('target/ripr/reports/ripr-findings.sarif') != ''
        continue-on-error: true
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-findings.sarif
          category: ripr-findings

      - name: Upload RIPR repo seams
        if: env.RIPR_UPLOAD_SARIF == 'true' && hashFiles('target/ripr/reports/ripr-seams.sarif') != ''
        continue-on-error: true
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-seams.sarif
          category: ripr-seams
```

For a first rollout, treat code-scanning annotations as review guidance. Do not
make the job blocking until the repository has reviewed its initial SARIF
baseline, tuned `ripr.toml`, and decided which configured-warning results should
fail CI. The `cargo xtask sarif-policy` baseline modes shown above are
repo-local automation today; a public package-level policy command is a future
adoption surface.

The generated workflow always uploads `target/ripr/pilot`,
`target/ripr/workflow`, `target/ripr/agent`, `target/ripr/reports`,
`target/ripr/review`, and `target/ci` as a `ripr-reports` artifact when files
exist. The repo badge files in that artifact are:

- `target/ripr/reports/repo-ripr-badge.json`, the seam-native native badge
  payload;
- `target/ripr/reports/repo-ripr-badge-shields.json`, the Shields projection.

The generated workflow sets `RIPR_UPLOAD_SARIF` to `"true"` so first-run
repositories get code-scanning guidance. Set it to `"false"` in the copied
workflow to keep the report artifact path while skipping SARIF rendering and
upload. This is useful for repositories that do not want GitHub code scanning
permissions or want to review the report artifacts before enabling annotations.

Calibrated gates are opt-in. Leave `RIPR_GATE_MODE` unset for the default
advisory posture. To evaluate the explicit gate decision layer, set repository
variables such as:

```text
RIPR_GATE_MODE=visible-only
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

`visible-only` writes `target/ripr/reports/gate-decision.{json,md}` without
blocking. `acknowledgeable`, `baseline-check`, and `calibrated-gate` can return
a blocking exit only when that explicit mode is configured. A `ripr-waive`
pull-request label remains visible in the decision report as an acknowledged
outcome rather than hiding the recommendation.

The SARIF baseline policy implementation still lives in `cargo xtask`. The
generated workflow above does not block pull requests by default; gate blocking
requires an explicit `RIPR_GATE_MODE` configuration.

See [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) for the operating model
covering modes, waivers, baseline inputs, calibration evidence, generated CI
behavior, and static/runtime vocabulary boundaries.

The security workflow currently runs:

```bash
cargo deny check advisories licenses bans sources
```

It uses `deny.toml` to enforce RustSec advisories, license policy, banned
crates, and approved dependency sources. Duplicate dependency findings are
warnings while the `ra_ap_syntax` dependency graph is being baselined.

Pull requests also run GitHub Dependency Review for high-severity vulnerability
alerts and denied license families. Dependency Graph is enabled for the
repository, so Dependency Review is a blocking security gate.

## GitHub Actions Runtime Policy

GitHub-hosted action majors should use Node-24-backed releases where official
releases exist. `cargo xtask check-workflows` rejects old action refs such as
`actions/checkout@v4`, `actions/setup-node@v4`, artifact v4 actions, and
`codecov/codecov-action@v4`.

`actions/dependency-review-action@v4` is temporarily allowlisted in
`policy/workflow_action_runtime_allowlist.txt` because the official Dependency
Review action still declares a Node 20 runtime and no Node-24-backed major is
available. Keep Dependency Review enabled until a supported replacement exists.

The same cargo-deny check can be run locally with:

```bash
cargo xtask check-supply-chain
```

Dependabot is configured in `.github/dependabot.yml` for Cargo dependencies,
the VS Code extension npm package, and GitHub Actions. Routine version-update
PRs are limited to minor and patch updates. Major updates should be deliberate,
scoped PRs because they often change toolchain, release, or runtime behavior.
Dependabot PRs are not auto-merged; they must pass the normal CI, coverage,
security, and `xtask` checks before merge.

GitHub-hosted security settings are tracked in
[Repository settings](REPO_SETTINGS.md). Dependency Graph, Dependabot alerts,
Dependabot security updates, secret scanning, push protection, and private
vulnerability reporting are settings, not workflow files. Keep that document
updated when repository settings change.

Release workflows handle extension publishing and server binary releases.

## Principles

- Fast gates first: formatting, check, clippy, and tests should fail early.
- Packaging gates matter: crates.io packaging catches missing files and metadata
  drift.
- Extension gates stay separate: Node setup should not slow Rust-only PRs.
- Policy gates should be mechanical and allowlisted while existing debt is paid
  down.
- Rust-first file policy keeps repo automation in `xtask` instead of ad hoc
  scripts.
- Blocking `ripr` findings remain opt-in. Use `cargo xtask sarif-policy` with
  an explicit baseline and failure mode only after the repository has adopted
  that gate.
- CI changes require documentation updates.

## Future Improvements

Planned CI work:

- cache Cargo and npm dependencies without hiding stale-lockfile failures
- decide whether CI should call `check-pr` directly or keep the current
  explicit workflow steps
- add markdown/link checks for docs-heavy PRs
- add README capability snapshot consistency checks
- add README state and Markdown link checks
- ratchet Codecov project and patch status requirements after the first stable
  coverage baseline
- decide when duplicate dependency findings should become blocking after the
  cargo-deny baseline is stable
- add SARIF schema validation for generated artifacts
- decide when to promote the opt-in SARIF baseline policy into repository
  workflows

## Merge Criteria

A branch is ready to merge when:

- required gates for touched areas pass on a committed tree
- docs and changelog are updated for user-visible changes
- static output language rules are preserved
- spec-test-code traceability is present for behavior changes

Local `--allow-dirty` packaging checks are useful during review but are not a
substitute for plain package and publish dry-run checks on the final committed
branch.
