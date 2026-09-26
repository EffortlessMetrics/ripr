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
| `release` | 61+ LEM | explicit `release-check` or `full-ci` proof |

CI lanes are grouped by posture, not by how convenient they are to place in one
workflow file.

| Posture | Purpose | Examples | Default behavior |
| --- | --- | --- | --- |
| Required | Cheap merge-safety and policy invariants. | `fmt`, `cargo check`, clippy, focused tests, static-language, file/workflow/process/dependency policy, output-contract checks for schema/output changes. | Blocking on ordinary PRs that touch the relevant surface. |
| Advisory | Evidence that helps review but should not block routine work until calibrated. | coverage, Test Analytics, `ripr` self-dogfood, SARIF upload, agent-loop artifacts, Droid review, future Clippy lints, broad security posture scans. | Upload artifacts or comments; do not fail the PR by default. |
| On-demand / release | Expensive, slow, or release-bearing proof. | `cargo package`, `cargo publish --dry-run`, VSIX packaging, server archive checks, release readiness, full workspace proof. | Run only from each lane's wired `main`, manual dispatch, `release-check`, or `full-ci` trigger; avoid default PR blocking. |

The current `ci.yml` still carries some release-like proof in the primary Rust
job. Treat that as legacy posture while the CI split is rolled out. New CI work
should move toward small required gates at the front door, advisory evidence by
default, and label-gated release proof.

This section defines the target policy. It does not mean the current workflows
already implement PR planning, label-gated lane selection, CI actuals, or
budget enforcement. Until those later PRs land, the "Current Workflows" section
below remains the source of truth for what GitHub Actions runs today.

## Codex CI-Efficiency Compatibility Invariants (Hard Guardrails)

When asking Codex or other automation agents to "make CI cheaper," treat this
section as strict compatibility policy for CI-efficiency PRs. It constrains
future workflow and planner changes, but it does not override the current
workflow behavior documented in [Current Workflows](#current-workflows).

### 1) Concurrency semantics for heavy/core PR workflows

- Do **not** change heavy/core workflow concurrency as a generic efficiency
  edit. The PR must name the current behavior and the intended behavior.
- Treat both models as product decisions:
  - no-cancel preserves a running expensive job and allows only one pending
    replacement;
  - synchronize-cancel favors the latest commit and may abandon near-complete
    work.
- Any switch to or from `cancel-in-progress` must document the affected
  workflows, rollback path, cost tradeoff, and review impact.
- Cheap metadata-only workflows may use `cancel-in-progress: true`, but only as
  an explicit documented exception.

### 2) Change classification before lane selection

- Do **not** build a future planner that routes all changed files through Rust
  CI.
- Treat metadata/control-plane surfaces as candidates for light paths unless
  mixed with real Rust/build/test changes, including:
  - `docs/**`, markdown-only edits, `README*`, `CHANGELOG*`, `SECURITY*`,
    `CONTRIBUTING*`;
  - `policy/**`, `plans/**`, `badges/**`, `AGENTS.md`;
  - `.github/CODEOWNERS`, `.github/dependabot.yml`,
    `.github/pull_request_template.md`, `.github/PULL_REQUEST_TEMPLATE/**`;
  - `docs/tracking/**`, `ci/hardware/**` receipt files, `.rails/**`,
    `.uselesskey/**`.
- Treat `.github/workflows/**` as a special workflow-change surface:
  - not docs-light;
  - route to minimal hosted workflow safety/validation unless policy requires
    more.
- Until a classifier is implemented and validated, the active workflows remain
  the source of truth. Do not claim a docs-only PR skipped Rust proof unless the
  check run evidence shows that it did.

### 3) Target PR planner policy

The target planner should classify first, then choose the cheapest truthful
lane:

- docs/control-plane-only -> no Rust compile;
- workflow-only -> hosted YAML/workflow validation, no full Rust by default;
- Rust source/build/test touched -> routed Rust-small;
- hardware/GPU/receipt-only -> syntax/receipt validation only;
- mixed or unknown -> Rust-small, not full CI.

Full CI is opt-in by policy trigger (label/manual dispatch/main push/release/
schedule/merge queue), not the default PR path.

### 4) Hosted fallback boundary

- Do **not** remove, narrow, or relabel the current protected GitHub-hosted
  Rust-small fallback without updating the routed-runner docs, active-goal
  state, and branch-protection contract in the same PR.
- The current routed Rust-small workflow may select GitHub-hosted when a PR is
  untrusted, runner state cannot be read, or no idle image-ready CX53/CX43
  runner is available. That result remains a valid protected-path success while
  the CX53/CX43 proof closeout is blocked.
- Do **not** replace a targeted Rust-small fallback with broader hosted full CI
  unless the PR names the cost, proof obligation, and explicit trigger such as
  `full-ci`, `release-check`, or `ci-budget-ack`.

### 5) Artifact policy

- Avoid default-path artifact uploads with `if: always()` unless merge policy
  requires them and they are tiny.
- Prefer upload-on-failure with short retention (3-7 days) for diagnostics.
- Keep receipt uploads minimal, especially for docs/control-plane-only paths.

### 6) Required validation for CI-only efficiency PRs

Every CI-efficiency PR should show evidence appropriate to the edited surface:

- `git diff --check`;
- docs/policy-only changes: the relevant docs, policy, and static-language
  checks, plus an explicit claim boundary;
- workflow changes: YAML/workflow validation and a note confirming the current
  and intended concurrency semantics;
- classification logic changes: dry-run or unit checks covering:
  - docs-only;
  - `.rails/**`;
  - `.uselesskey/**`;
  - workflow file change;
  - Rust file change;
  - mixed docs + Rust.

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
synchronized, reopened, labeled, and unlabeled pull requests, writes the
changed-file list locally, and writes a placeholder step summary. To avoid
routine artifact churn, it uploads the changed-file list only on failure or
when the PR is labeled `full-ci`. Until the numeric planner exists, authors
should still fill the PR template's CI economics section for CI-affecting
changes.

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

For repo-local operator work, run:

```bash
cargo xtask cockpit
cargo xtask pr-ready
cargo xtask check-pr
```

`cockpit` is the repo-level advisory front panel for board state,
source-of-truth checks, generated-evidence rails, badge ownership, and command
catalog coverage. `pr-ready` is the active-branch pre-review packet. Neither
command changes badge endpoint JSON, branch protection, baseline state,
suppressions, generated tests, or policy authority.
Use [Merge freshness and watcher policy](MERGE_WATCH_POLICY.md) when watching a
specific PR through hosted checks, branch freshness, Droid/advisory status, and
merge execution.

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
| `release-check` | Run the currently wired release-surface proof without opting into every `full-ci` lane: package list, publish dry-run, and release-readiness. |
| `vscode` | Run editor extension lanes even when no editor path changed. |
| `coverage` | Run coverage lanes and upload coverage artifacts. |
| `ripr-waive` | Acknowledge a soft static exposure finding for this PR. Does not skip CI and does not apply when `full-ci` is present. |
| `ci-budget-ack` | Acknowledge that this PR intentionally exceeds the expected LEM band. |
| `clippy-future` | Run future or candidate Clippy lint lanes in advisory mode. |
| `windows-ci` | Run the advisory Windows lane on a pull request without opting into every `full-ci` lane. |

New labels that affect CI must update this table, the PR template, and the
budget/risk-pack policy files in the same PR.

These labels are the documented target vocabulary. Today, `release-check` and
`full-ci` activate the Rust workflow's package list, publish dry-run, and
release-readiness steps on pull requests. Other label effects remain target vocabulary until a later PR
wires them into a PR plan or workflow condition. The GitHub Settings App
contract in `.github/settings.yml` codifies these label names, descriptions,
and colors so the reviewable vocabulary does not drift in the GitHub UI.

### Advisory Windows Lane

This repository is developed on Windows and Linux but was only continuously
validated on Linux. Platform-specific test-harness bugs therefore shipped and
stayed green on `main`: four separate classes were found only once someone ran
the suite on Windows by hand — #2390 (path interpolated into JSON), #2391 (host
separator compared against server output), #2409 (fixture swallowed a git
failure), #2429 (host separator in a portable output contract).

The drift runs in both directions. #2337 is the inverse case — goldens blessed
on Windows that fail on Linux. Each platform was blind to the other's breakage,
and single-platform CI was the root cause enabling both.

`.github/workflows/windows-advisory.yml` (#2442) closes the gap:

- **Scope.** `cargo test --workspace`, twice. No test or golden `cargo xtask`
  gates and no blessing — a Windows lane that blessed or checked goldens would
  institutionalize the #2337 drift it exists to catch. There is one required
  `cargo xtask` step, `windows-advisory-summary`, which validates the two run
  logs and their captured exit statuses; it is evidence validation, not a test
  gate, and it fails the job when the evidence is missing or unusable.
- **Two samples per run.** The suite runs twice on purpose: one sample cannot
  separate a reproducible failure from a load-dependent flake. The summarizer
  tracks three states per test (failed, observed pass, not observed) and reports
  `masked_unknown` rather than guessing about a test it never saw. Two failures
  are reported as `repeated_failure`, not "deterministic" — a shared race can
  reproduce twice.
- **Advisory outcomes, non-advisory evidence.** A failing test does not fail the
  job. A missing log, a missing captured exit status, or a summarizer crash
  does. A lane that reports success while proving nothing is exactly the
  false-confidence condition it exists to prevent.
- **Selection.** A daily schedule for standing signal, `workflow_dispatch`, and
  pull requests labeled `windows-ci` or `full-ci`.

Promotion to required is gated on #2430 and on stability across repeated runs on
hardware that reproduces the failures — the hosted runner does not reproduce the
parallel-load class in #2419 at all, so green runs there prove nothing about it.
Promoting a lane that is already red converts a useful signal into background
noise reviewers learn to skip, which is the false-confidence failure mode in
reverse.

Measured on `7b7d1322`: 3781 library tests and 150 CLI smoke tests pass on
Windows. The original count of 14 known failures was reduced by #2417, #2416,
and #2431. One failure remains — #2430,
`lsp_lifecycle::compat_journey_collect_workspace_status_over_real_wire`, where
`ripr.refresh` exceeds the harness's 15s response budget. Unlike the #2419
class, it reproduces on the hosted runner: two independent `windows-latest` runs
produced the identical `timed out waiting for response id 3`, and it reproduces
5 of 5 on a Windows developer host. That is the lane doing its job — a platform
question that could not be settled from one machine, settled by CI.

### Advisory Specification Maintenance Digest

The Source of Truth workflow owns the advisory spec maintenance digest
(#3467). `cargo xtask specs digest` runs the same inventory pipeline as
`specs maintenance` (one scan, one DTO) and writes the full
`spec-maintenance.{json,md}` reports plus a short bounded
`spec-maintenance-digest.md` for the step summary; the full report is
retained as the `ripr-spec-maintenance` artifact.

- **Triggers.** A weekly schedule (no more frequent than weekly), explicit
  `workflow_dispatch`, and pull requests that touch spec governance paths
  (`docs/specs/**`, `docs/templates/**`, `.ripr/traceability.toml`,
  `.allow/spec-system/**`, `docs/status/SUPPORT_TIERS.md`, or the workflow
  itself). Source-only PRs do not start it.
- **Three states.** Candidates found (`maintenance_status:
  attention_required`) and no candidates (`clean`) are both successful
  observations; neither count changes the exit status. A structurally blind
  scan (the spec index absent and nothing scanned) is `attention_required`,
  not `clean`. An instrument failure (unreadable spec, serialization error)
  exits nonzero with no digest written, the step fails visibly, and the
  `if: always()` summary annotates `maintenance_status: instrument_failure`
  — a failed advisory observation, not a failed required gate.
- **Non-blocking by construction.** The digest job is `continue-on-error`
  and is not in `.github/settings.yml` required contexts; a scheduled run
  queues behind an active one (`cancel-in-progress: false`) so a nearly
  complete inventory is not discarded. The lane creates no issues,
  comments, labels, or branch-protection mutation.

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

### Swarm Routed Rust

`ripr-swarm` adds `.github/workflows/routed-rust.yml` as the development-trunk
Rust gate. It exposes one branch-protection-facing check:

```text
Ripr Rust Small Result
```

The implementation jobs are conditional and should not be required directly:

```text
Route Ripr Rust Small
Ripr Rust Small on CX53
Ripr Rust Small on CX43
Ripr Rust Small on GitHub Hosted
```

Routing policy:

```text
trusted same-repo PR or push:
  CX43 if idle
  CPX42 if idle
  CX53 if idle
  GitHub-hosted otherwise

fork or otherwise untrusted PR:
  GitHub-hosted only
```

The router uses the repository or organization `EM_RUNNER_READ_TOKEN` secret
when available. It selects a self-hosted runner only when the runner is idle and
has both the host label (`CX43`, `CPX42`, or `CX53`) and the `em-ci-rust-1.95`
runner-image/toolchain readiness label. If runner state cannot be read, or a
runner is idle but not image-ready, the workflow fails closed to GitHub-hosted
rather than selecting a self-hosted runner by guesswork.

The route job and protected result summaries include count-only runner
diagnostics so operators can separate missing host runners, busy runners, and
missing `em-ci-rust-1.95` readiness labels without exposing runner names,
registration tokens, secrets, or full label inventories. The protected result
job also receives those values as environment variables, so downloaded result
logs are sufficient for issue proof.

The copyable self-hosted proof runbook is in
[`docs/swarm-development.md`](swarm-development.md#self-hosted-proof-runbook).
Use it to record CX43 primary proof, CPX42/CX53 fallback proof, or the bounded
runner availability blocker without exposing runner tokens or secrets.

The routed lane runs the existing Rust/product command surface without release
package or publish dry-run steps. Each required lane invokes the shared
`cargo xtask precommit` gate table after the cargo build/test steps and keeps
only the lane-only gates enumerated (`check-evidence-promotion-honesty`,
`check-dependencies`, `check-process-policy`, `check-network-policy`,
`goldens check`, `fixtures`); the docs-gate job runs the same precommit table
for docs-only pull requests. It keeps advisory evidence artifacts
non-blocking and uploads the normal `target/ripr` report packet when present.

The legacy `CI` workflow (`.github/workflows/ci.yml`) no longer runs the
workspace test suite; the routed `Ripr Rust Small` lane owns ordinary
merge-safety, and its runner contract is in
[PRODUCT_GATE_PLAN.md](ci/PRODUCT_GATE_PLAN.md) (#3825). The legacy workflow's
`Perl and release proof` job runs on pushes to `main` or `master`, manual
dispatches, and pull requests labeled `release-check` or `full-ci`, and keeps
only the proof unique to it, the non-default `lang-perl` feature:

```bash
cargo check -p ripr --features lang-perl
cargo test -p ripr --features lang-perl --lib analysis::language::perl
```

On pushes to `main` or `master` and on pull requests labeled `release-check`
or `full-ci`, the same job also runs the release-surface package checks:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
release_version="$(cargo pkgid -p ripr | sed 's/.*#//')"
cargo xtask release-readiness --version "$release_version"
```

The CI workflow also has an explicit MSRV job that pins Rust `1.95.0` and runs:

```bash
cargo check --workspace --all-targets
```

The `release-proof` job pins the declared `1.95.0` toolchain; the MSRV job
duplicates that baseline and runs only on manual dispatch or `full-ci` pull
requests.

The legacy workflow's `release-proof` and `msrv` jobs run on `ubuntu-latest`.
They carry release-surface and baseline proof and must not
depend on self-hosted runner capacity when preparing a source release. The
routed Rust-small workflow remains the swarm development lane that selects
self-hosted runners when available and falls back to hosted capacity.

Local shaping commands are intentionally separate from CI because they mutate
the worktree:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask commands
cargo xtask pr-summary
cargo xtask pr-ready
cargo xtask pr-triage-report
cargo xtask gh-pr-status --pr <number>
cargo xtask suggested-fixes
cargo xtask check-command-catalog
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
`commands` writes the xtask mutability catalog under `target/ripr/reports/` so
agents can distinguish mutating commands, non-mutating checks, report-only
commands, external-state reads, external-state mutations, and
argument-dependent commands.
`check-command-catalog` verifies that the help catalog and mutability catalog
stay aligned, that write surfaces are documented, and that external-state
mutations remain judgment-required.
`pr-summary` writes `target/ripr/reports/pr-summary.md` from git diff/status.
`pr-ready` writes `target/ripr/reports/pr-ready.md` and
`target/ripr/reports/pr-ready.json` by composing worktree doctor, command
catalog, PR summary, critic, receipts check, suggested fixes, generated-clean,
and badge diff policy into one advisory local operator packet.
`pr-triage-report` writes the advisory open-board hygiene report as Markdown
and JSON.
`gh-pr-status --pr <number>` writes a read-only merge-readiness packet for one
PR as Markdown and JSON, including merge state, required check status when
GitHub exposes it, reviews, Droid status, and the next safe action.
`suggested-fixes` writes a deterministic repair patch and companion report
under `target/ripr/reports/`; it suggests allowlist ordering fixes and docs
index table ordering for specs and ADRs, plus traceability behavior block
ordering by spec ID and capability block ordering by spec ID and capability ID.
It never writes badge values, baselines, suppressions, goldens, dependency
exceptions, or schema changes.
`precommit` is the cheap non-mutating local guardrail. `check-pr` is the
review-ready local gate and intentionally does not run package or publish
dry-run checks. `check-badge-diff-policy` fails ordinary PRs that carry
generated badge endpoint diffs, while `check-generated-clean` fails generated
target/sample build residue and shares the same badge endpoint boundary.
`fixtures` and
`goldens check` validate the current fixture and
expected-output scaffolding without accepting output drift. `golden-drift`
writes advisory Markdown and JSON summaries of semantic expected-output drift
for reviewers. `test-oracle-report` writes an advisory baseline for the strength
of `ripr`'s own Rust test oracles. If no tests are selected, both report formats
use status `not_run` and explain that oracle evidence was not established; this
is distinct from a nonempty all-strong `pass` and remains advisory. `dogfood` writes a non-blocking
`ripr`-on-`ripr` report from stable fixture diffs. `critic` writes an advisory
adversarial review packet from the current diff, reports, and receipts.
`reports index` writes a reviewer front door for generated reports and includes
the repo-ops packet statuses for command mutability, PR-ready, worktree doctor,
PR triage, per-PR merge readiness, generated-clean, badge diff policy, command
catalog coverage, critic, receipts, suggested fixes, and `check-pr`.
`receipts` writes machine-readable gate evidence under `target/ripr/receipts`,
and `receipts check` validates the receipt set.

The fuller automation model is documented in [PR automation](PR_AUTOMATION.md).
Deterministic shaping should happen locally; CI should verify the committed
tree and upload reports when available.

Codex Goals runs should treat CI artifacts as campaign receipts. A campaign can
advance through multiple work items, but each scoped PR should leave the same
shape/check/report artifacts that CI uploads for human review.

Current policy checks write Markdown reports to `target/ripr/reports` when they
run. The Rust workflow generates `target/ripr/reports/index.md` and writes it
to the GitHub Actions job summary when present. To keep ordinary PR CI cheap,
it uploads the report and receipt directories as the `ripr-pr-reports` artifact
only on failure or when the PR is labeled `full-ci`.

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
cargo xtask check-spec-numbering
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
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-command-catalog
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
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

### Self-Hosted Runner Placement

The everyday required Rust gate routes through `routed-rust.yml`
(`CX43 -> CPX42 -> CX53 -> GitHub-hosted` fallback, shared `/mnt/ci-cache`, disk guards,
and scratch cleanup) and exposes the single branch-protection check
`Ripr Rust Small Result`. That lane is the migrated reference and is not changed
by routine runner-placement edits.

The remaining (non-required) self-hosted lanes route to the smallest safe EM
shared self-hosted tier by actual workload, each with explicit
`group` + `labels` and a per-job `timeout-minutes` hang guard. Queueing on
these groups is acceptable backpressure and these lanes are advisory or
label/push gated, so they do not block merge. The VS Code e2e lane is the
exception: it runs on GitHub-hosted Ubuntu because it installs `xvfb` for
headless extension tests and must not depend on privileged package installs on
EM self-hosted runners.

| Workflow / job | Group | Tier label |
| --- | --- | --- |
| `ci.yml` `rust` | `em-ci-small` | `rust-medium` |
| `ci.yml` `msrv` | `em-ci-small` | `rust-small` |
| `ci.yml` `vscode` | GitHub-hosted | `ubuntu-latest` |
| `coverage.yml` | `em-ci-small` | `rust-heavy-medium` |
| `test-analytics.yml` | `em-ci-small` | `rust-medium` |
| `future-clippy.yml` | `em-ci-small` | `rust-medium` |
| `security.yml` `cargo-deny` | `em-ci-tiny` | `rust-tiny` |
| `source-of-truth.yml`, `badge-endpoints.yml` | `em-ci-tiny` | `rust-tiny` |
| `security.yml` `dependency-review` | `em-ci-nano` | `policy-nano` |
| `pr-plan.yml` | `em-ci-nano` | `workflow-nano` |
| `droid-review`, `droid`, `droid-security-scan` | `em-ci-review` | `droid-review` |

All self-hosted lanes carry the `trusted-pr` label and keep their existing
fork/untrusted-PR `if:` guards, so fork code cannot reach trusted self-hosted
runners. `rust-large` is intentionally not used here; it is reserved org-wide
for the single heaviest lane. Build-heavy lanes still use `Swatinem/rust-cache`;
moving them onto the shared `sccache`/`/mnt/ci-cache` path used by
`routed-rust.yml` is a tracked follow-up rather than part of this placement
change.

Release and publish workflows (`publish-extension.yml`,
`release-server-binaries.yml`) and branch protection (`.github/settings.yml`)
are intentionally out of scope for this placement change.

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

Copy the generated file, not a workflow from this page. Run
`ripr init --ci github --dry-run` to print it without writing anything. It
uploads the pilot, report, and agent artifact directories. The official GitHub
SARIF upload documentation uses `github/codeql-action/upload-sarif@v4`; keep
the RIPR job, artifact upload, and optional SARIF steps advisory until the
repository has chosen a baseline policy.

For a CI-first user, the useful output is the artifact packet:

- `target/ripr/pilot/` - first-screen pilot summary, repo exposure snapshot,
  and agent seam packets;
- `target/ripr/workflow/` - selected-seam workflow manifest, commands,
  status JSON/Markdown, review summary JSON/Markdown, the before snapshot,
  and agent packet and brief JSON when a top seam is available;
- `target/ripr/agent/` - compatibility copies of packet and brief JSON for
  the top seam when one is available;
- `target/ripr/reports/` - SARIF files when enabled, repo badge JSON,
  `gap-decision-ledger.{json,md}`,
  `assistant-loop-health.{json,md}`, `first-useful-action.{json,md}`,
  `pr-review-front-panel.{json,md}`, `start-here.{json,md}`,
  `waiver-aging.{json,md}`, `suppression-health.{json,md}`,
  `policy-readiness.{json,md}`, and `index.{json,md}`.
- `target/ripr/review/` - PR test guidance JSON and Markdown when
  `ripr review-comments` runs on pull requests.

CI prepares the before side of the repair loop only. There is no test edit
between two snapshots of one CI checkout, so the workflow writes no after
snapshot, verify JSON, agent receipt, or targeted-test outcome. The
`ripr agent repair --root . --seam-id <seam-id> --phase before` command the
summary leads with starts the repair where the test edit happens; the
`--attempt ... --phase after` command it prints runs verify and writes the
receipt. The summary labels the low-level verify and receipt commands as steps
that run after the test edit.

The workflow also writes a `RIPR advisory summary` step summary. It starts with
the `start-here` first-run packet when `ripr first-pr` can compose one from
explicit artifacts, then includes the PR review front panel, first useful
action fallback, a language preview grouping section when `[languages]` enables
TypeScript or Python, policy readiness, waiver aging, and suppression health
when their input artifacts exist, assistant-loop health when proof artifacts
exist, the report packet index when any indexed artifact exists, the top
recommendation, the agent review packet when present, artifact links,
SARIF and badge status, known limits, and PR guidance annotation counts when
`target/ripr/review/comments.json` exists. On pull
requests, the generated workflow writes that report before emitting
changed-line check annotations by default without posting inline review
comments.

See [LLM operator guide](LLM_OPERATOR_GUIDE.md) for the same status, workflow
packet, verify, receipt, and reviewer-summary loop outside CI. See
[PR review guidance](PR_REVIEW_GUIDANCE.md) for the PR-facing annotation
contract and review workflow. See
[PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) for the
first-screen summary that composes PR guidance, first useful action, assistant
proof, assistant-loop health, ledger, baseline, gate, calibration,
coverage/grip, and receipt artifacts.

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
  test file, related test to imitate, and the repair start
  (`ripr agent repair --root . --seam-id <seam-id> --phase before`) when the
  seam is repair-ready. Check annotations carry only the reason and that
  repair start, so they never name a path inside the CI runner's checkout.

The LLM guidance in annotations is bounded handoff material. It should ask for
one focused test, avoid production edits unless explicitly requested, and point
to `ripr agent verify` after the edit. It must not ask an LLM to decide which
diff regions matter, run mutation testing, or claim runtime confirmation.

The generated workflow includes the optional inline review-comment publisher,
but keeps it disabled by default. Set `RIPR_COMMENT_MODE=plan` to upload and
summarize the read-only publish plan, or `RIPR_COMMENT_MODE=inline` to publish
same-repository changed-line comments only when the plan reports safe
operations. The publisher posts only from `comments[]`, targets changed lines
only, caps comment count, deduplicates by `dedupe_key`, and leaves gate
decisions as the separate pass/fail authority.

See [PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md)
for rollout guidance, publish-plan review, fork and permission behavior,
dedupe/upsert expectations, and rollback.

The generated workflow also captures existing RIPR inline-comment metadata,
checks the publish plan's `safe_to_publish` result, and only calls GitHub for
safe create/update operations in explicit `inline` mode. Read those steps in
the output of `ripr init --ci github --dry-run`; this page does not keep a copy
because a copy drifts from what the command writes.

One step is kept here because a test holds it byte-equal to the template: the
pull request diff capture, which pins the diff presentation so ambient Git
configuration cannot change the bytes RIPR analyzes (#4005).

```yaml
      - name: Capture pull request diff
        if: github.event_name == 'pull_request'
        run: |
          mkdir -p target/ripr/reports
          # Pinned diff contract (#4005): the same presentation pins as the
          # production loaders. Ambient external-diff, textconv, color,
          # context, path-quoting, and side-prefix configuration must not
          # change the bytes RIPR analyzes.
          base_ref="origin/${{ github.base_ref }}"
          base_sha="$(git rev-parse --verify "${base_ref}^{commit}")" || { echo "ripr: cannot resolve base ref $base_ref" >&2; exit 1; }
          head_sha="$(git rev-parse --verify "HEAD^{commit}")" || { echo "ripr: cannot resolve HEAD" >&2; exit 1; }
          git -c core.quotePath=true diff --binary --no-ext-diff --no-textconv --no-color --src-prefix=a/ --dst-prefix=b/ --unified=3 --inter-hunk-context=0 "${base_sha}...${head_sha}" > target/ripr/reports/pr.diff || { echo "ripr: git diff failed for ${base_sha}...${head_sha}" >&2; exit 1; }
          byte_count="$(wc -c < target/ripr/reports/pr.diff | tr -d ' ')"
          digest="$(sha256sum target/ripr/reports/pr.diff)" || {
            echo "ripr: failed to compute SHA-256 for patch" >&2
            exit 1
          }
          digest="${digest%% *}"
          jq -n --arg base_ref "$base_ref" --arg base_sha "$base_sha" --arg head_sha "$head_sha" --argjson byte_count "$byte_count" --arg digest "$digest" '{tool:"ripr",kind:"pr-diff-receipt",base_ref:$base_ref,base_sha:$base_sha,head_sha:$head_sha,byte_count:$byte_count,sha256:$digest}' > target/ripr/reports/pr-diff.receipt.json
          if [ "$byte_count" -eq 0 ]; then
            name_list="$(mktemp)" || { echo "ripr: cannot create temp file for path inventory" >&2; exit 1; }
            git -c core.quotePath=true diff --name-only -z "${base_sha}...${head_sha}" > "$name_list" || { echo "ripr: git diff --name-only failed for ${base_sha}...${head_sha}" >&2; exit 1; }
            changed_paths="$(tr -cd '\0' < "$name_list" | wc -c | tr -d ' ')"
            rm -f "$name_list"
            if [ "$changed_paths" -ne 0 ]; then
              echo "ripr: empty patch but $changed_paths changed path(s); refusing an absent result" >&2
              exit 1
            fi
          fi

      - name: Run RIPR PR guidance report
        # ... the remaining steps are in `ripr init --ci github --dry-run`
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
exist. When `RIPR_GATE_BASELINE` is set and gate evaluation writes
`target/ripr/reports/gate-decision.json`, the workflow also runs
`ripr baseline diff`, then `ripr zero status`, and includes:

- `target/ripr/reports/baseline-debt-delta.json`;
- `target/ripr/reports/baseline-debt-delta.md`;
- `target/ripr/reports/ripr-zero-status.json`;
- `target/ripr/reports/ripr-zero-status.md`.

The baseline debt delta is advisory debt-movement evidence. It is summarized in
the job summary and feeds the RIPR Zero status summary, but `ripr gate
evaluate` remains the only generated-workflow pass/fail authority. The RIPR
Zero section reports visible unresolved debt, new policy-eligible debt,
acknowledgements, suppressions, baseline metadata health, top debt area, top
repair route, and trend availability as advisory progress evidence. Use
[RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md) for how to read
the status, refresh stale baseline metadata, and route repair packets. The repo
badge files in that artifact are:

- `target/ripr/reports/repo-ripr-badge.json`, the native public badge payload
  using the actionable canonical repair basis;
- `target/ripr/reports/repo-ripr-badge-shields.json`, the Shields projection.

Seam-native inventory is not the public badge headline. Use `cargo xtask
badge-basis --include-seam-classes` or repo-exposure reports when internal seam
pressure is needed.

The generated workflow sets `RIPR_UPLOAD_SARIF` to `"true"` so first-run
repositories get code-scanning guidance. Set it to `"false"` in the copied
workflow to keep the report artifact path while skipping SARIF rendering and
upload. This is useful for repositories that do not want GitHub code scanning
permissions or want to review the report artifacts before enabling annotations.

Calibrated gates are opt-in. Leave `RIPR_GATE_MODE` unset for the default
advisory posture. The generated workflow already reads repository variables, so
teams should adopt gates by setting variables rather than editing the workflow
for each mode.

### Gate Adoption Examples

Use these repository-variable examples with the generated workflow.

Default advisory mode:

```text
# Leave both repository variables unset.
RIPR_GATE_MODE=
RIPR_GATE_BASELINE=
```

This preserves first-run behavior: PR guidance, SARIF when enabled, badges,
agent packets, review packets, and artifacts remain advisory.

Visible decision report:

```text
RIPR_GATE_MODE=visible-only
RIPR_GATE_BASELINE=
```

`visible-only` writes `target/ripr/reports/gate-decision.{json,md}` and appends
an at-a-glance gate section plus the Markdown decision report to the job summary
without making a RIPR finding block the PR. If `RIPR_GATE_BASELINE` is also
set and the gate decision exists, the workflow writes
`target/ripr/reports/baseline-debt-delta.{json,md}` as a non-blocking debt
movement report. The first-screen summary names the mode, status, decision
counts, active and acknowledgement labels, applied waiver label, baseline
input, calibration inputs/effects, blocking reason, baseline debt movement, and
gate and delta artifact paths.

On pull requests where `ripr review-comments` writes
`target/ripr/review/comments.json`, generated CI also writes and uploads
`target/ripr/reports/pr-evidence-ledger.json` and
`target/ripr/reports/pr-evidence-ledger.md`. The ledger joins PR guidance,
optional gate decision, baseline debt delta, RIPR Zero status, recommendation
calibration, agent receipt, optional coverage summary, labels, and optional
history into one advisory PR movement card. The job summary shows new
policy-eligible gaps, baseline debt still present, baseline debt resolved,
acknowledged and suppressed counts, blocking candidates, visible unresolved
gaps, the top repair route, verify command, agent command, coverage/grip
frontier status, and history trend when available. The ledger is evidence only;
`ripr gate evaluate` remains the pass/fail authority for configured gate modes.
See [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) for how to
read the ledger as waiver aging, baseline burn-down, repair receipts, and
coverage/grip frontier evidence.

When the PR evidence ledger exists, generated CI also writes and uploads
`target/ripr/reports/waiver-aging.json` and
`target/ripr/reports/waiver-aging.md`. The report is advisory only: repeated
waiver remains a visible signal for focused-test or suppression review, not a
failure and not an automatic durable exception.

Generated CI also writes and uploads
`target/ripr/reports/suppression-health.{json,md}` and
`target/ripr/reports/policy-readiness.{json,md}` as advisory readiness
projection artifacts. The job summary names suppression-health metadata gaps
directly, and policy readiness composes existing gate, baseline, calibration,
waiver-aging, and suppression-health reports when present, then summarizes the
safest current policy mode. It does not run a gate, change baseline state, post
comments, create required checks, or add pass/fail authority beyond an
explicitly configured `ripr gate evaluate`.

When policy readiness exists, generated CI also writes and uploads
`policy-operations.{json,md}`, `policy-history.{json,md}`,
`policy-promotion-*.{json,md}`, and configured preview-language
`preview-promotion-*.{json,md}` packets. These reports are advisory operator
packets: they summarize the current policy ceiling, history trend, manual
promotion readiness, and preview-promotion evidence gaps. They do not mutate
`ripr.toml`, baselines, suppressions, workflows, branch protection, CI defaults,
history ledgers, or preview-language eligibility.

See [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
for how to read the proof report, warnings, static movement, optional CI
projection, and advisory limits.

When the full assistant-loop artifact chain is present, generated CI also
writes and uploads `target/ripr/reports/test-oracle-assistant-proof.json` and
`target/ripr/reports/test-oracle-assistant-proof.md`. This step is advisory and
runs only after PR guidance, the editor/agent brief, before/after static
evidence, the agent receipt, and the PR evidence ledger already exist. The job
summary appends the proof report and an at-a-glance card with the selected seam,
missing discriminator, placement state, static movement, receipt path, optional
gate input, optional coverage/grip frontier input, and warning count. If the
required inputs are absent, generated CI skips the proof projection instead of
printing a placeholder or changing pass/fail behavior.

When the proof report exists, generated CI also writes and uploads
`target/ripr/reports/assistant-loop-health.json` and
`target/ripr/reports/assistant-loop-health.md`. This step reads the existing
proof artifact only. It summarizes proof completeness, missing required and
optional inputs, static movement, warning groups, and repair queue counts as
advisory operating health over assistant-loop proof packets. It does not rerun
analysis, grade an agent, or change pass/fail authority.

See [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) for how
maintainers and coding agents read completeness, missing inputs, unchanged
movement, repair queue entries, and advisory limits.

Generated CI also projects the first useful action when at least one explicit
input artifact is already present. It runs `ripr first-action --root .` with
existing PR guidance, assistant proof, PR evidence ledger, baseline delta,
agent receipt, gate decision, coverage/grip frontier, and editor context
inputs when those files exist, then writes and uploads
`target/ripr/reports/first-useful-action.json` and
`target/ripr/reports/first-useful-action.md` with the normal report packet. The
job summary appends a first-run status card near the top of the advisory
summary, then the recommended next test at a glance plus the Markdown report.
The first-run card names the selected gap or no-action fallback, repair target,
agent packet command, verify command, receipt command, artifact paths, and the
advisory gate boundary. If no inputs exist, the step logs that no
first-useful-action inputs were available, the summary shows the regeneration
command, and CI pass/fail behavior is unchanged.

See [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) for how
developers, reviewers, and coding agents should read that summary, act on the
selected action, verify static movement, and emit receipts.

Generated CI also renders the first-run start-here packet. It first renders
`target/ripr/reports/gap-decision-ledger.{json,md}` from
`repo-exposure.json` when that snapshot exists, then runs `ripr first-pr --root
.` with the gap ledger, first useful action, PR repair cards, agent packet, and
gate decision paths. The command writes and uploads
`target/ripr/reports/start-here.json` and
`target/ripr/reports/start-here.md`. The job summary opens with this packet,
showing status, top gap or no-action/blocked state, canonical gap identity,
language/status, repair route, repair target, related test, static limit,
verify command, receipt command, receipt state, next regeneration command,
artifacts, and the gate authority boundary. If the packet is missing, the
summary shows the exact `ripr first-pr` regeneration command and leaves CI
pass/fail behavior unchanged.

Generated CI also projects the PR review front panel when at least one explicit
front-panel input artifact is already present. It runs
`ripr pr-review front-panel --root .` with existing PR guidance, first useful
action, assistant proof, assistant-loop health, PR evidence ledger, baseline
delta, RIPR Zero status, gate decision, recommendation calibration, imported
mutation calibration, coverage/grip frontier, and receipt inputs when those
files exist, then writes and uploads
`target/ripr/reports/pr-review-front-panel.json` and
`target/ripr/reports/pr-review-front-panel.md` with the normal report packet.
The job summary appends the PR review at-a-glance fields plus the Markdown
report. If no inputs exist, the step logs that no PR review front-panel inputs
were available and leaves CI pass/fail behavior unchanged.

See [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) for how
reviewers, maintainers, developers, and coding agents should read the
first-screen PR story, follow repair routes, inspect receipts, and preserve
the advisory gate boundary.

Generated CI also projects the report packet index when at least one indexed
artifact is already present. It runs `ripr reports index --root .` with the
explicit report, review, receipt, workflow, agent, pilot, and CI artifact
directories, then writes and uploads `target/ripr/reports/index.json` and
`target/ripr/reports/index.md` with the normal report packet. The job summary
renders index status, available and missing expected counts, start-here path,
gate authority path, missing surfaces, warning kinds, and the Markdown index.
If no indexed artifacts exist, the step logs that no report-packet index
inputs were available and leaves CI pass/fail behavior unchanged.
See [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md) for how to
read the grouped packet map, regenerate missing surfaces, and preserve gate
authority.

For every configured gate mode, the generated workflow behavior is:

1. capture active PR labels into `target/ci/labels.json`;
2. render `target/ripr/review/comments.json` before gate evaluation;
3. run `ripr gate evaluate` only when `RIPR_GATE_MODE` is set;
4. run `ripr baseline diff` only when `RIPR_GATE_BASELINE` is set and
   `gate-decision.json` exists;
5. run `ripr zero status` only when `baseline-debt-delta.json` exists;
6. render the at-a-glance gate section from `gate-decision.json`;
7. render the baseline debt movement section from
   `baseline-debt-delta.json` when present;
8. render the RIPR Zero at-a-glance section from `ripr-zero-status.json` and
   append `ripr-zero-status.md` when present;
9. run `ripr pr-ledger record` on pull requests when `comments.json` exists;
10. render the PR movement section from `pr-evidence-ledger.json` and append
   `pr-evidence-ledger.md` when present;
11. run `ripr assistant-loop proof` only when the required assistant-loop
   artifacts exist;
12. render the assistant proof section from `test-oracle-assistant-proof.json`
   and append `test-oracle-assistant-proof.md` when present;
13. run `ripr assistant-loop health` when `test-oracle-assistant-proof.json`
   exists;
14. render the assistant-loop-health section from `assistant-loop-health.json`
   and append `assistant-loop-health.md` when present;
15. run `ripr reports gap-ledger` from `repo-exposure.json` when present;
16. run `ripr first-action` when explicit first-action inputs exist;
17. render the First Useful Action section from `first-useful-action.json` and
   append `first-useful-action.md` when present;
18. run `ripr pr-review front-panel` when explicit front-panel inputs exist;
19. run `ripr first-pr` to compose `start-here.{json,md}` from explicit
   artifacts;
20. render the start-here packet first in the job summary when present;
21. render the PR review front-panel section from
   `pr-review-front-panel.json` and append `pr-review-front-panel.md` when
   present;
22. run `ripr reports index` when explicit indexed artifacts exist;
23. render the report-packet index section from `index.json` and append
   `index.md` when present;
24. append the detailed `gate-decision.md`, `baseline-debt-delta.md`, and
   `ripr-zero-status.md` reports when present;
25. upload gate, baseline delta, RIPR Zero, PR evidence ledger,
   test-oracle assistant proof, assistant-loop health, first useful action, and
   PR review front-panel plus start-here and report-packet index artifacts with
   the normal `ripr-reports` artifact packet;
26. fail only when the explicit gate mode returns `blocked` or `config_error`.

The generated workflow reads the configured baseline for gate, delta, and RIPR
Zero reports only. It must not run `ripr baseline update`, pass
`--remove-resolved`, accept or synthesize `--adopt-new`, or write the configured
baseline path. Baseline changes are repository changes that require a reviewed
PR.

Acknowledgeable policy:

```text
RIPR_GATE_MODE=acknowledgeable
RIPR_GATE_BASELINE=
```

`acknowledgeable` requires a visible acknowledgement such as the `ripr-waive`
label for policy-eligible findings. The finding stays in the gate decision; the
label records an acknowledged outcome rather than hiding the recommendation.

Baseline-aware policy:

```text
RIPR_GATE_MODE=baseline-check
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

`baseline-check` is for repos with an explicit checked-in baseline. Use it only
after reviewing the baseline file; missing baseline input is reported as a
configuration problem instead of being treated as clean evidence. When the
baseline is readable and the gate decision is produced, generated CI also
uploads `baseline-debt-delta.json` and `baseline-debt-delta.md` and summarizes
still-present, resolved, new policy-eligible, acknowledged, suppressed, stale,
invalid, and missing-input counts in the job summary. The delta report remains
advisory movement evidence; `ripr gate evaluate` is still the pass/fail owner.
When the baseline delta exists, generated CI also writes and uploads
`ripr-zero-status.json` and `ripr-zero-status.md`, then summarizes RIPR 0 state,
visible unresolved debt, metadata health, top debt area, and top repair route
as advisory adoption progress.

Calibrated gate:

```text
RIPR_GATE_MODE=calibrated-gate
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

`calibrated-gate` is the narrowest stricter mode. Use it only when the repo has
reviewed baseline behavior and the available recommendation or imported
mutation-calibration inputs support the same candidate class. Missing or
ambiguous calibration stays visible as unknown confidence; it must not be
treated as high confidence.

The SARIF baseline policy implementation still lives in `cargo xtask`. The
generated workflow above does not block pull requests by default; gate blocking
requires an explicit `RIPR_GATE_MODE` configuration.

See [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) for the operating model
covering modes, waivers, baseline inputs, calibration evidence, generated CI
behavior, and static/runtime vocabulary boundaries.

### Waiver And Label Workflows

Use waiver labels when a team wants a visible PR-time acknowledgement, not when
it wants to hide a finding. The default waiver label is `ripr-waive`, and the
generated workflow already captures pull-request labels into
`target/ci/labels.json` before running `ripr gate evaluate`.

Label setup:

```text
Label: ripr-waive
Meaning: acknowledge a soft RIPR static exposure finding for this PR
Effect: changes an eligible blocking candidate into an acknowledged decision
Scope: this PR only
```

In this repository, `.github/settings.yml` manages the label name, color, and
description. In another repository using the generated workflow, create the
same label before enabling `acknowledgeable` mode so reviewers do not have to
guess which label the gate evaluator expects.

Recommended acknowledgement workflow:

1. Start with `RIPR_GATE_MODE=visible-only` until reviewers are familiar with
   `gate-decision.md`.
2. Move to `RIPR_GATE_MODE=acknowledgeable` when the team wants policy-eligible
   gaps to require either a focused test or an explicit PR label.
3. When the gate reports a policy-eligible gap, review the job summary,
   `target/ripr/reports/gate-decision.md`, and the PR guidance packet.
4. If the finding is acceptable for this PR, add `ripr-waive`.
5. Let the labeled PR workflow rerun. The next gate decision should say
   `Decision: acknowledged`, list `ripr-waive`, and keep the candidate visible.
6. If a focused test is added instead, remove `ripr-waive` and rerun the gate so
   the receipt records the current evidence without an acknowledgement label.

The expected acknowledged summary looks like:

```text
Decision: acknowledged
Mode: acknowledgeable
Blocking: 0
Acknowledged: 1

Acknowledged:
- src/pricing.rs:88 weakly_gripped - policy-eligible gap acknowledged by ripr-waive
```

The machine-readable report keeps the same fact trail:

```json
{
  "status": "acknowledged",
  "mode": "acknowledgeable",
  "inputs": {
    "labels": ["ripr-waive"],
    "labels_json": "target/ci/labels.json"
  },
  "summary": {
    "acknowledged": 1,
    "blocking": 0
  }
}
```

Reviewers should be able to audit an acknowledgement from artifacts alone:

- `target/ci/labels.json` records the PR labels observed by the workflow.
- `target/ripr/reports/gate-decision.json` records the matching label and
  candidate decision.
- `target/ripr/reports/gate-decision.md` keeps the acknowledged finding in the
  job summary.
- `target/ripr/review/comments.json` keeps the underlying recommendation
  packet when PR guidance was produced.

Waivers and suppressions are separate controls:

| Control | Where it lives | Use for | Visibility |
| --- | --- | --- | --- |
| `ripr-waive` | PR label | Accept this visible PR-time finding for this review. | The finding remains in gate decision JSON/Markdown as `acknowledged`. |
| `.ripr/suppressions.toml` | Repository policy file | Record accepted debt or a durable exception before PR-time policy. | Suppressed/configured-off candidates cannot block and should be counted as suppressed or not applicable when present in inputs. |
| Baseline | Checked-in gate baseline | Avoid punishing historical debt while identifying new gaps. | Baseline state remains policy evidence; it does not hide new policy-eligible findings. |

Do not use `ripr-waive` as a substitute for adding a focused test when the
recommendation is correct and the PR is still changing the relevant behavior.
Do not add a suppression just to make one PR pass; suppressions are durable
repository policy and should carry owner, reason, and review intent.

### Gate Baseline Workflow

Use a gate baseline when a repository wants to adopt RIPR policy without
punishing historical behavioral test debt on every pull request. A baseline is
a checkpoint of visible existing findings. It is not a suppression file, not a
waiver label, and not evidence that the finding is acceptable forever.

The full command-by-command adoption workflow lives in
[Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md). This section keeps the
generated-CI summary and repository-variable shape close to the workflow
reference.

The adoption model is:

```text
show the full RIPR picture
-> checkpoint existing policy-eligible gaps as baseline debt
-> block or acknowledge only new policy-eligible gaps
-> add focused tests
-> remove resolved identities from the baseline
-> move toward RIPR 0 under the configured scope
```

`RIPR 0` means there are no visible unresolved behavioral test-grip gaps under
the configured scope and policy. It does not mean the test suite is perfect or
that RIPR has runtime mutation confirmation.

Recommended baseline creation workflow:

1. Run the generated workflow with `RIPR_GATE_MODE=visible-only`.
2. Download or inspect `target/ripr/reports/gate-decision.json` and
   `target/ripr/reports/gate-decision.md`.
3. Review the visible recommendations. Do not baseline malformed inputs,
   suppressed findings, configured-off findings, or items the team plans to fix
   in the same adoption PR.
4. Create `.ripr/gate-baseline.json` from reviewed current findings.
5. Commit the baseline in its own PR with the generated CI mode still
   `visible-only` or `baseline-check`.
6. Switch to `RIPR_GATE_MODE=baseline-check` only after the baseline PR is
   reviewed and merged.

Baseline ledger shape:

```json
{
  "schema_version": "0.1",
  "kind": "gate_baseline",
  "reviewed": false,
  "summary": {
    "entries": 1
  },
  "entries": [
    {
      "identity": {
        "seam_id": "8f7fa8644fd12280",
        "source_id": "ripr-review-8f7fa8644fd12280"
      },
      "decision": "advisory",
      "review": {
        "reviewed": false,
        "reason": "initial adoption baseline"
      }
    }
  ]
}
```

Use the stable identities already present in `gate-decision.json`. `ripr
baseline create` checkpoints current advisory, acknowledged, and blocking
decisions into a candidate file for review and refuses to overwrite an existing
baseline unless `--force` is passed:

```bash
ripr baseline create \
  --from target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/gate-baseline.candidate.json
```

Review that candidate before copying it into `.ripr/gate-baseline.json`.
Baselining everything blindly makes the file less useful as a debt ledger.

After the baseline is reviewed, compare it with current gate evidence to see
debt movement before changing policy:

```bash
ripr baseline diff \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/baseline-debt-delta.json \
  --out-md target/ripr/reports/baseline-debt-delta.md
```

The delta report is advisory. It shows still-present baseline debt, resolved
entries, new policy-eligible findings, acknowledged findings, suppressed
findings, stale baseline entries, invalid baseline entries, and missing current
inputs. Generated CI writes the same
`target/ripr/reports/baseline-debt-delta.{json,md}` artifacts automatically
when `RIPR_GATE_BASELINE` is set and a gate decision exists. `ripr gate
evaluate` remains the pass/fail authority.

Generated CI never adopts new baseline entries. It does not invoke
`ripr baseline update`, it does not pass `--remove-resolved`, and it does not
write `.ripr/gate-baseline.json`. New policy-eligible debt must stay visible as
new debt until a maintainer repairs it, acknowledges it for the PR, or creates a
separate reviewed baseline change.

`ripr gate evaluate` indexes identities from the new `entries[].identity`
ledger shape. For compatibility with existing fixtures and reviewed hand-built
baselines, it also accepts identities from `decisions`, `comments`,
`summary_only`, and `suppressed` arrays when those fields are present in the
baseline file. For each entry, it indexes `seam_id`, `id`, and `dedupe_key`
when present. Keep the baseline small and reviewable; do not check in an
uninspected copy of every PR guidance artifact.

Baseline review checklist:

- Every entry came from current `gate-decision.json` or PR guidance evidence.
- The entry represents existing debt, not a finding introduced by the adoption
  PR.
- The finding remains visible in summaries or artifacts after being baselined.
- The baseline PR explains the configured scope and why blocking is not enabled
  yet, if the repo is still in `visible-only`.
- The baseline PR explains the adoption date, reviewed artifact source, and
  owner for future refreshes.
- The baseline file is checked in at the same path configured by
  `RIPR_GATE_BASELINE`.

After the baseline PR is reviewed, set repository variables:

```text
RIPR_GATE_MODE=baseline-check
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

Expected behavior:

```text
Existing baseline identity: visible and non-blocking
New policy-eligible identity: blocking in baseline-check
Missing or invalid baseline: config_error
```

Refresh the baseline after focused tests move static evidence. The safe refresh
rule is remove identities that no longer appear in current gate output; do not
add new identities during a shrink refresh. New identities should go through the
normal review path as new policy-eligible debt.

Refresh workflow:

1. Add one or more focused tests.
2. Rerun PR guidance and gate evaluation.
3. Confirm the agent receipt or targeted-test outcome shows the expected static
   movement when those artifacts are available.
4. Compare the old `.ripr/gate-baseline.json` to the new
   `gate-decision.json`.
5. Remove baseline entries that no longer appear.
6. Keep the gate summary visible so reviewers can see which debt was removed.

Policy modes with a baseline:

| Mode | Baseline role |
| --- | --- |
| `visible-only` | Baseline is optional context; findings stay advisory. |
| `baseline-check` | Existing baseline identities stay visible and non-blocking; new policy-eligible identities can block. |
| `calibrated-gate` | Baseline identity must be new, policy-eligible, and supported by calibration before it can block. |

Baseline, waiver, and suppression controls have different jobs:

| Control | Good use | Bad use |
| --- | --- | --- |
| Baseline | Mark reviewed historical debt so stricter modes can focus on new gaps. | Add every new blocking finding to avoid fixing or acknowledging it. |
| `ripr-waive` | Acknowledge a visible finding for one PR. | Make a recurring gap disappear across future PRs. |
| `.ripr/suppressions.toml` | Record durable accepted debt or configured-off policy with owner and reason. | Replace baseline review or PR acknowledgement for convenience. |

Do not use a baseline to hide new findings. Do not move an uncomfortable
recommendation from a PR into the baseline without review. If the team accepts
one PR-time exception, use `ripr-waive`; if the team accepts durable debt, use
the baseline or a reasoned suppression depending on whether the finding should
remain part of the burn-down ledger.

When moving from `baseline-check` to `calibrated-gate`, keep the same baseline
discipline. Calibration can raise confidence for new, matching candidates; it
does not make stale baseline entries stronger or turn missing calibration into a
blocking signal.

### Blocking Readiness

Use [RIPR blocking readiness](BLOCKING_READINESS.md) before promoting a gate
mode. The guide explains when to stay advisory, when to require `ripr-waive`,
when a reviewed baseline is enough for `baseline-check`, and when
`calibrated-gate` has enough local evidence to block. Its
[Gate Adoption Checklist](BLOCKING_READINESS.md#gate-adoption-checklist) should
be complete before a repository moves from visible evidence to optional
blocking. Default generated CI still stays non-blocking unless
`RIPR_GATE_MODE` is explicitly configured.

The security workflow currently runs:

```bash
cargo-deny check advisories licenses bans sources
```

The coverage and Test Analytics workflows run on `ubuntu-latest`. Both lanes
remain advisory, but release candidates need an observable green boundary; these
lanes must not depend on self-hosted runner availability just to prove that the
current workspace can produce coverage and JUnit artifacts. The jobs still run
only on pull requests, pushes to `main`/`master`, and manual dispatches, and
Codecov upload remains non-blocking.

Future Clippy also runs on `ubuntu-latest`. It remains advisory and never fails
the branch; the hosted runner keeps deferred-lint readiness visible without
blocking release proof on self-hosted runner availability.

It installs `cargo-deny` as a normal command-line binary before running the
check, so self-hosted runners do not need Docker just to execute the security
workflow. The job also installs the Rust toolchain because `cargo-deny` shells
out to `cargo metadata` while evaluating the workspace. It uses `deny.toml` to
enforce RustSec advisories, license policy, banned crates, and approved
dependency sources. Duplicate dependency findings are warnings while the
`ra_ap_syntax` dependency graph is being baselined.

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
- the branch is current enough for repository freshness policy, or a maintainer
  has approved an explicit freshness exception using
  [Merge freshness and watcher policy](MERGE_WATCH_POLICY.md)
- docs and changelog are updated for user-visible changes
- static output language rules are preserved
- spec-test-code traceability is present for behavior changes

Local `--allow-dirty` packaging checks are useful during review but are not a
substitute for plain package and publish dry-run checks on the final committed
branch.
