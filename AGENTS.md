# Agent Instructions

This repository is the product repo for `ripr`: a static mutation-exposure
analyzer for Rust/Cargo workspaces.

## Product Contract

`ripr` answers this question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Keep all work aligned with that contract. Do not turn `ripr` into a full
mutation engine, a coverage dashboard, a proof system, a second rust-analyzer,
or a generic test generator.

## Language Rules

Static findings must use conservative language:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Do not claim:

- `killed`
- `survived`
- `untested`
- `proven`
- `adequate`

Real mutation testing confirms later. `ripr` gives draft-mode exposure evidence
and targeted test intent.

## Architecture Rules

Keep the public surface as one published package:

```text
Package: ripr
Binary:  ripr
Library: ripr
Automation: xtask, unpublished
```

Do not split into `ripr-core`, `ripr-cli`, `ripr-lsp`, `ripr-engine`, or
`ripr-schema` until there is a real external contract.

The current internal shape is:

- `domain`: probe, RIPR evidence, oracle strength, exposure classification
- `app`: use-case orchestration and public library API
- `analysis`: diff loading, syntax indexing, probe generation, classification
- `output`: human, JSON, and GitHub annotation rendering
- `cli`: command-line adapter
- `lsp`: experimental sidecar adapter

## Rust Baseline

- Edition: Rust 2024
- Minimum Rust version: 1.95
- Keep `unsafe_code = "forbid"`

## Rust-First File Policy

Rust is the default implementation language for repo automation, production
logic, test harnesses, fixture runners, release checks, and policy checks.

Do not add shell, Python, JavaScript, TypeScript, or other programming files
outside approved surfaces. Prefer `cargo xtask` for repo automation. If a
non-Rust file is necessary, update `policy/non-rust-allowlist.toml` and explain
the exception in the PR.

The VS Code extension, GitHub Actions declarations, fixture inputs,
documentation examples, generated outputs, and assets are explicit exceptions
when covered by policy metadata.

## Required Gates

Run these before claiming the branch is ready:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask pr-triage-report
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask metrics
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-droid-review-config
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
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
cargo xtask check-proof-packs
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

`cargo xtask shape` is allowed to make safe local edits: run `cargo fmt`, sort
policy allowlists, ensure `target/ripr/reports`, and write a shape report.
`cargo xtask pr-summary` writes a local reviewer packet from git diff/status.
`cargo xtask pr-triage-report` writes an advisory open-PR board hygiene report.
`cargo xtask gh-pr-status --pr <number>` writes a read-only merge-readiness
packet for one PR after it exists.
`cargo xtask ci-budget [--workflow <name>] [--limit <n>] [--input <path>]`
writes an advisory CI budget and merge-queue hygiene report that separates
disk-guard infrastructure tempfails (issue #1058) from product failures; it
reads recent routed-workflow runs through `gh` (or a supplied `--input` JSON
file) and changes no CI behavior.
`cargo xtask fix-pr` runs safe shaping and then refreshes the PR summary.
`cargo xtask precommit` is the cheap non-mutating guardrail.
`cargo xtask worktree doctor` reports dirty main, branches behind main,
generated residue, and broad untracked scope before PR work proceeds.
`cargo xtask check-pr` is the review-ready non-release gate.

See `docs/PR_AUTOMATION.md` for the shape/check/guide model, current automation
entrypoint, and repo-ops report packets.

Large-repo RIPR scans are build-heavy in this repo. Prefer `repo-badge-json`,
generated receipts, an explicit gap ledger, or
`cargo xtask repo-exposure-summary-report` for ordinary summary counts; do not
use full `repo-exposure-json` for normal badge, receipt, top-file, or packet
queue paths. Run at most one no-ledger repo-wide RIPR scan at a time, scope
`RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` only to intentional full refreshes, and
clean up ad-hoc large JSON outputs after inspection.

Useful runtime checks:

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
```

Editor extension checks:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.7.0.vsix --force
```

The extension should resolve the server in this order:

```text
ripr.server.path
bundled server binary
downloaded cached server binary
verified first-run download
ripr on PATH
actionable error
```

Do not make `cargo install ripr` a requirement for the normal editor install
path. It is a fallback for offline, pinned, or controlled environments.

## Implementation Bias

Prefer small, high-signal changes:

- Changed behavior first, not whole-repo abstract adequacy.
- Evidence paths before scores.
- Unknown is valid and should be explicit.
- Human output should be actionable.
- JSON output should be stable and versioned.
- Agent context should state the exact missing discriminator.

Do not add deep semantic dependencies, persistent databases, or broad LSP
features unless the basic CLI, schema, packaging, and tests remain green.

## PR Scope Doctrine

Do not optimize PRs for low line count. Optimize for narrow production risk and
complete evidence.

A large fixture, golden-output, spec, docs, ADR, metrics, or traceability diff
is welcome when it makes one production behavior reviewable. A small code diff
is not acceptable if it changes multiple contracts without a spec-test-code
trail.

Every material behavior change should preserve this chain:

```text
spec -> test or fixture -> code -> output contract -> metric
```

Make production delta, evidence delta, acceptance criterion, and non-goals
explicit in PRs and planning docs.

## Commit, PR, and Merge Boundary

Do not pause merely to commit, push, open a PR, update a PR, or merge a clean
PR.

For scoped implementation, docs, tests, and refactors, use this default flow:

```text
review -> improve -> validate -> commit -> push -> open/update PR -> merge when ready
```

A PR is ready when the branch is current, required checks pass, real review
findings are addressed, the diff matches the stated scope, and repo policy does
not require a different sequence.

`stackable = false` means do not build the next dependent work item on top of
the current branch. It does not create an approval gate.

`blocked_by` is a dependency rule. If a work item depends on another item, wait
until that dependency is landed or explicitly update the manifest. Do not invent
a separate merge rule.

Ask before proceeding only when continuing would change public schema, output
contracts, security/workflows/secrets, dependencies, release or publish
behavior, architecture boundaries, campaign ordering, or duplicate-PR
selection.

## Review posture

Automated review comments are primarily consumed by follow-up coding agents.
Do not optimize for a human reading every comment. Optimize for concrete,
structured, actionable findings that another agent can fix.

A clean review must still document what was inspected.
Do not treat "LGTM" as a useful review result. If there are no actionable
findings, produce a short inspection record that names:

- changed surfaces inspected;
- risks considered;
- repo invariants checked;
- validation signals;
- residual assumptions.

When reviewing or repairing code, read these files first:

- `.factory/skills/review-guidelines/SKILL.md`
- `.factory/rules/rust.md`
- `.factory/rules/github-actions.md`
- `.factory/rules/security.md`
- `docs/agent-context/repo-map.md`
- `docs/agent-context/review-invariants.md`
- `docs/agent-context/validation.md`

## Orchestration Operating Model

Orchestrated work is a staged pipeline, not one monolithic session:

```text
cheap discovery
-> cheap independent verification
-> written issue/spec/plan
-> focused implementation
-> targeted proof
-> stronger review only where needed
-> cleanup
```

The main session does synthesis and judgment. File searches, CI logs, PR
diffs, and failed hypotheses belong in subagents that return structured
summaries, not in the main context.

Route work by cost:

- Use cheap read-only scout agents (Haiku-class, Explore-style) for repo
  inventory, PR review sweeps, diffstat and changed-surface mapping,
  spec/schema surface mapping, validation-log summaries, claim checks, and
  cleanup audits. Scouts return structured tables (item, files touched,
  claim made, evidence found, missing proof, risk, next action), not prose.
- Before expensive or risky action (closing a PR, editing release claims,
  re-pinning a freeze candidate), run a second cheap adversarial pass:
  assume the first report is wrong, return only concrete discrepancies
  with file or PR references.
- Use implementation-grade agents (Sonnet-class) for code changes, test
  and fixture updates, conflict resolution, schema/doc alignment, and
  turning scout inventories into coherent PRs, with a bounded plan and a
  small working set.
- Escalate to top-tier judgment (Opus-class) only for high-risk release,
  security, or architecture decisions, or after two failed correction
  cycles. Using top-tier capacity to discover which files changed is an
  orchestration failure.
- Use broad parallel workflows (Ultracode-style fanouts) only for
  queue-scale uncertainty: open-PR reconciliation audits, release-claim
  audits across changelog/specs/schema, spec-surface inventories, CI
  failure taxonomies, cross-repo contract reviews. Never for one narrow
  edit.

Shift verification left so mistakes stay cheap:

```text
before implementation: scout inventory + adversarial check + filed issue/plan
before push:           focused proof + local preflight + cleanup audit
before release:        release-claim audit + package dry-run + non-claim review
before closing a PR:   supersession verified against the diff, not a summary
```

File or update the ripr-swarm issue or repo spec before implementation.
Repo specs are durable truth; issues track execution state; chat history is
not the plan. Use worktrees for risky or parallel branches. Every pass ends
with cleanup: worktrees, branches, stashes, `target/ripr` cache growth,
temp files, generated artifacts, cargo/npm churn, rescue leftovers, and
local-only files.

## Long-Context Agent Workflow

This repo is intentionally organized so agents can resume long-running goals
from repository artifacts instead of chat history.

When picking up work:

- start from `docs/ROADMAP.md` and `docs/IMPLEMENTATION_PLAN.md`
- use `docs/IMPLEMENTATION_CAMPAIGNS.md` and `.ripr/goals/active.toml` when
  working through a Codex Goals campaign
- use `docs/CAPABILITY_MATRIX.md` to identify current capability status
- use `docs/PR_AUTOMATION.md` to understand local shaping and PR reports
- use `docs/CODEX_GOALS.md` for the multi-PR campaign model
- use `docs/SCOPED_PR_CONTRACT.md` for one work item's PR-sized evidence bar
- use `docs/specs/` and `.ripr/traceability.toml` to map spec -> tests -> code
- choose the smallest vertical slice with one production delta and one evidence
  package
- update `docs/LEARNINGS.md` when repo knowledge or blockers should survive

See `docs/AGENT_WORKFLOWS.md` for the detailed handoff model.
