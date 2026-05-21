# PR Automation Operating Model

`ripr` uses repo automation to shape PRs before human review. The goal is not
more process. The goal is to remove deterministic cleanup from the review path
and turn judgment-required issues into precise repair briefs.

Core rule:

```text
Anything deterministic should be automated.
Anything judgment-based should produce a repair brief.
Generated evidence is not authored copy.
```

Humans and coding agents should spend attention on behavior, evidence,
exceptions, and public contracts. They should not spend attention on formatting,
allowlist order, report directory setup, generated indexes, or gate ordering.

Codex Goals consume this harness. The `/goal` loop may advance a multi-PR
campaign, but each work item should still leave the same shaped PR, reports, and
review artifacts described here. Machine-readable receipts record which gates
and report commands ran so agents and reviewers can inspect evidence without
reading raw logs.

## Current Commands

The current repo automation surface is:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask commands
cargo xtask cockpit
cargo xtask pr-ready
cargo xtask pr-summary
cargo xtask pr-triage-report
cargo xtask gh-pr-status --pr <number>
cargo xtask suggested-fixes
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask check-test-oracles
cargo xtask dogfood
cargo xtask lsp-cockpit-report
cargo xtask targeted-test-outcome --before <path> --after <path>
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
cargo xtask doctor
cargo xtask specs next
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-droid-review-config
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-command-catalog
cargo xtask check-supply-chain
cargo xtask ci-fast
```

`shape` is the safe local normalizer. It can mutate local files only when the
mutation is deterministic and reversible by normal version control review.

Current `shape` responsibilities:

- run `cargo fmt`
- sort `.ripr/*.txt` and `policy/*.txt` allowlist files
- ensure `target/ripr/reports`
- write `target/ripr/reports/shape.md`

`fix-pr` is the contributor and agent entrypoint for safe repair. It runs
`shape`, refreshes the PR summary, and writes
`target/ripr/reports/fix-pr.md`.

`commands` writes `target/ripr/reports/commands.md` and
`target/ripr/reports/commands.json`. The catalog classifies xtask commands as
`mutating`, `non_mutating_check`, `report_only`, `external_state_read`,
`external_state_mutating`, or `argument_dependent`, and flags commands that
require judgment before use.

`check-command-catalog` writes `target/ripr/reports/command-catalog.md` and
fails when the help catalog and mutability catalog drift apart, when a command
uses an unknown mutability class, when mutating commands omit their write
surface, when external-state mutations are not judgment-required, or when an
argument-dependent command does not explain when it writes.

`pr-summary` writes `target/ripr/reports/pr-summary.md` from git diff and git
status. It classifies changed paths into production, evidence, docs, policy,
workflow, extension, and public-contract surfaces.

Top-level `plans/` files are documentation evidence and campaign-planning
inputs. They appear in the docs, evidence/support, and campaign-planning
sections of the reviewer packet without being treated as production behavior.

`precommit` is the cheap non-mutating guardrail. It checks formatting, spec
format and numbering, and the policy surfaces that should fail quickly before
review.

`doctor` is the shortest agent hygiene entry point and is equivalent to
`worktree doctor`. It reports dirty `main` worktrees, branches behind
`origin/main`, generated badge/target residue, and broad source-of-truth diffs
that lack an obvious work item marker. Its report also includes a short
next-action queue so agents can move from diagnosis to the right cleanup or
follow-up validation command without reverse-engineering the findings.

`pr-triage-report` is the open-board hygiene report. It reads open PR metadata
through GitHub CLI and writes `target/ripr/reports/pr-triage.md` plus
`target/ripr/reports/pr-triage.json`. It flags same-title families, identical
changed-file sets, stale drafts, branches behind main, incomplete validation
signals, and policy/gate/generated workflow surfaces. It also emits an
advisory queue disposition for each PR — `merge_candidate`, `needs_rebase`,
`needs_review`, `close_duplicate`, `superseded`, `needs_fresh_validation`,
`needs_owner_decision`, or `do_not_touch_wrong_lane`. It is advisory and never
updates, closes, merges, or comments on PRs.

`gh-pr-status --pr <number>` is the per-PR merge-readiness packet. It reads
GitHub CLI PR status, branch-protection required contexts when available,
latest reviews, and Droid-related checks, then writes
`target/ripr/reports/gh-pr-status.md` and
`target/ripr/reports/gh-pr-status.json` with the merge state, outstanding
checks, failed checks, behind-main state, review status, Droid status, and a
safe next action: `wait`, `rebase`, `inspect failure`, or `merge`. It is
advisory and never updates the branch, comments, approves, or merges.
Use [Merge freshness and watcher policy](MERGE_WATCH_POLICY.md) for polling
cadence, branch-refresh decisions, REST status fallback, Droid/advisory-check
handling, and local worktree merge limitations.

`suggested-fixes` writes `target/ripr/reports/suggested-fixes.patch` and
`target/ripr/reports/suggested-fixes.md` with safe deterministic repair
suggestions. The patch covers allowlist ordering under `.ripr/*.txt` and
`policy/*.txt`, docs index table ordering for specs and ADRs, and traceability
behavior block ordering by spec ID, plus capability block ordering by spec ID
and capability ID, and command catalog ordering by xtask help order. It never
generates badge endpoint values,
golden blessings, baselines, suppressions, dependency exceptions, or
schema-version changes.
The generated-vs-authored boundary is documented in
[Generated evidence discipline](GENERATED_EVIDENCE.md).

`check-pr` is the review-ready local gate. It runs the current fast CI lane,
then clippy, docs, and PR summary generation. It intentionally leaves
release/package verification to `ci-full` or release-specific workflows.
Its fast policy lane includes `check-badge-diff-policy`, which rejects
generated badge endpoint diffs in ordinary PRs, and `check-generated-clean`,
which rejects generated target/sample build residue. Before writing the final
report index, it also refreshes the deterministic suggested-fixes patch under
`target/ripr/reports/`.

`fixtures` validates fixture contract shape, runs `ripr check` for fixture
directories when they exist, writes actual outputs under
`target/ripr/fixtures/<name>/`, compares stable expected outputs, and writes
`target/ripr/reports/fixtures.md`. It passes with a clear report when no
fixture directories exist yet.

`goldens check` runs fixtures and fails on drift between actual and expected
outputs without mutating checked-in files. It also writes
`target/ripr/reports/golden-drift.md` and
`target/ripr/reports/golden-drift.json` so reviewers can inspect semantic drift
before any blessing. `goldens bless <fixture> --reason <reason>` records an
explicit blessing reason, updates expected JSON and human outputs, and appends
the fixture expected-output changelog.

`golden-drift` writes the same advisory drift reports without failing merely
because output drift exists. It still reports fixture execution errors as
command failures.

`test-oracle-report` writes an advisory baseline of `ripr`'s own Rust test
oracle strength to `target/ripr/reports/test-oracles.md` and
`target/ripr/reports/test-oracles.json`. `check-test-oracles` is currently an
alias that produces the same non-blocking report.

`test-efficiency-report` writes an advisory per-test evidence ledger to
`target/ripr/reports/test-efficiency.md` and
`target/ripr/reports/test-efficiency.json`. It records apparent owner calls,
oracle kind and strength, observed literal values, static limitations, and
advisory reason counts for low-discriminator signals. The report is a review
aid and does not block CI.

`dogfood` runs `ripr check --mode fast` against stable in-repo fixture diffs,
writes actual outputs under `target/ripr/dogfood/`, and writes advisory
Markdown and JSON reports under `target/ripr/reports/`. It also dogfoods gate
adoption by running `ripr gate evaluate` over checked boundary-gap PR guidance
and calibration evidence for `visible-only`, `acknowledgeable`,
`baseline-check`, and `calibrated-gate` modes. Those gate adoption receipts
are compared against `fixtures/boundary_gap/expected/gate-adoption/` and
written under `target/ripr/dogfood/gate-adoption/`; the dogfood report records
that default generated CI still does not block unless `RIPR_GATE_MODE` is
explicitly configured. It also checks first useful action receipts from
`fixtures/boundary_gap/expected/first-useful-action/` for actionable,
baseline-only, stale, missing-required-artifact, unchanged-after-attempt, and
no-actionable-seam routes.

`lsp-cockpit-report` reads committed LSP fixture expectations plus the VS Code
e2e smoke test file and writes `target/ripr/reports/lsp-cockpit.md` and
`target/ripr/reports/lsp-cockpit.json`. It summarizes which fixtures produce
editor diagnostics, which code actions are exposed, which context/action fields
are present, and which VS Code commands are covered by e2e tests.

`repo-exposure-latency-report` builds the local debug `ripr` binary, runs
repo-exposure formats under a bounded timeout, captures opt-in analyzer phase
trace lines, and writes `target/ripr/reports/repo-exposure-latency.md` and
`target/ripr/reports/repo-exposure-latency.json`. It is a diagnostic report for
cache and warm-path work, including file-fact cache hit/miss counters; it does
not change repo-exposure JSON/Markdown.

`release-readiness --version <version>` writes
`target/ripr/reports/release-readiness.md` and
`target/ripr/reports/release-readiness.json`. It path-installs the local
`ripr`, checks that `pilot`, `outcome`, `calibrate cargo-mutants`, and
`agent verify`/`agent receipt` are exposed, runs the boundary-gap
pilot/outcome/agent-verify/agent-receipt fixtures, refreshes repo-exposure
latency and LSP cockpit reports, inspects the advisory GitHub workflow dry-run,
and checks VSIX and known-limit docs. Package list and publish dry-run checks
record `not_run` until the requested version matches `crates/ripr` and the tree
is clean, so release prep can rerun them on the version-bump branch.

`targeted-test-outcome` compares two `repo-exposure-json` artifacts and writes
`target/ripr/reports/targeted-test-outcome.md` and
`target/ripr/reports/targeted-test-outcome.json`. It matches seams by
`seam_id`, summarizes before/after grip-class counts, and reports moved,
unchanged, new, removed, and regressed seams as an advisory targeted-test
receipt. It does not run mutation testing and does not block CI.

The installed CLI exposes the same receipt loop as `ripr outcome --before
<path> --after <path>` so users do not need this repository checked out. The
xtask command remains the repo-local report writer for automation packets under
`target/ripr/reports/`.

`critic` writes an advisory adversarial review packet to
`target/ripr/reports/critic.md` and `target/ripr/reports/critic.json`. It reads
the current diff plus generated reports and receipts, then flags likely missing
evidence such as analyzer changes without fixture/golden evidence, output
changes without output-contract evidence, campaign movement without campaign
reports, fixture output drift without blessing reasons, policy changes without
process docs, and extension changes that still need npm compile/package proof.
It does not fail CI.

`reports index` writes `target/ripr/reports/index.md` and
`target/ripr/reports/index.json` as a reviewer front door. It summarizes the
active campaign, available reports, missing expected reports for the changed
surface, advisory reports, and suggested next commands. The index also carries
repo-ops packet status for command mutability, the repo cockpit, PR-ready,
worktree doctor, PR triage, per-PR merge readiness, generated-clean, badge diff
policy, critic, receipts, suggested fixes, and `check-pr` artifacts so agents
can consume the operating packet as JSON instead of scraping prose. The command
catalog check packet is included next to the catalog itself so catalog drift is
visible in the same front-door index.

The index also carries a Lane 1 Evidence Readiness section for the report chain
that supports actionable canonical-gap counts and badge-readiness decisions:
`evidence-health`, `lane1-evidence-audit`, `actionable-gaps`,
`evidence-quality-scorecard`, `evidence-quality-trend`, and `badge-basis`.
Missing, warning, or failing artifacts keep the index in a warning state and
add the relevant regeneration command. The index only checks existing artifact
paths; it does not run those expensive reports or infer evidence from source.

`cockpit` writes `target/ripr/reports/cockpit.md` and
`target/ripr/reports/cockpit.json`. It is the repo-level maintainer front door:
it composes worktree doctor, command mutability, command-catalog coverage, spec
numbering, campaign/source-of-truth checks, open PR triage, generated-clean, and
badge diff policy into one advisory action queue. It reads GitHub PR metadata
through `pr-triage-report`, writes local report packets, and does not close
PRs, update branches, edit badge endpoint JSON, mutate source, or change
policy authority.

`pr-ready` writes `target/ripr/reports/pr-ready.md` and
`target/ripr/reports/pr-ready.json`. It composes the local repo-ops checks that
an agent should run before opening or updating a PR: worktree doctor, command
mutability catalog, PR summary, critic, receipts check, suggested fixes,
generated-clean, and badge diff policy. The command is advisory front-door
metadata; it does not replace `check-pr`.

The CLI front doors use the same start-here wording. `safe next action` means
repair one named gap, regenerate missing evidence, or stop on no-action.
`missing artifact`, `stale evidence`, `wrong root`, and `malformed artifact`
are fail-closed states. `preview-limited evidence` remains syntax-first and
advisory. `verify command`, `receipt command`, and `receipt path` are the static
movement proof rail, not runtime adequacy, mutation proof, or gate approval.

`receipts` writes machine-readable gate receipts under `target/ripr/receipts/`
for shape, fix-pr, ci-fast, check-pr, fixtures, goldens, test-oracle, dogfood,
and metrics runs. `receipts check` validates the required receipt files and
writes `target/ripr/reports/receipts.md`. `check-pr` refreshes receipts before
the final report index.

`check-allow-attributes` rejects guarded Rust lint suppressions such as
panic-family, unsafe-code, dead-code, unused-code, and broad warning
suppression attributes unless they are narrowly allowlisted in
`.ripr/allow-attributes.txt`. It writes
`target/ripr/reports/allow-attributes.md`.

`check-local-context` rejects committed local machine paths, Codex memory or
sandbox references, uploaded-file/chat citation artifacts, and runtime/session
state files. It writes `target/ripr/reports/local-context.md` and
`target/ripr/reports/local-context.json`. Narrow generic examples must use
`policy/local_context_allowlist.txt`.

`check-supply-chain` runs `cargo deny check advisories licenses bans sources`
using `deny.toml` and writes `target/ripr/reports/supply-chain.md`. It is a
local and CI security preflight; duplicate dependency findings are warnings
until the dependency graph baseline is stable.

`ci-fast` is the current non-mutating local and CI check lane. It runs the Rust
checks plus the existing policy checks for static language, panic-family usage,
lint-suppression bypasses, local context leaks, file policy, executable bits,
workflow shell budgets and action runtime policy, Droid workflow invariants,
spec format, fixture contracts, generated files, dependencies, process
spawning, and network policy.
The workflow check rejects avoidable Node-20-backed action majors and requires
Node 24 for extension build and publish workflows. Those policy checks write
Markdown pass/fail reports under `target/ripr/reports`.

## Command Lanes

`ripr` automation is split into three lanes.

### Mutating Shape Commands

Mutating commands are allowed to change files, but only for deterministic local
normalization.

Current:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask goldens bless <fixture> --reason "..."
```

Future:

```bash
cargo xtask metrics --write
cargo xtask docs-index --write
cargo xtask capability-matrix --write
```

Safe default mutations:

- formatting
- allowlist sorting
- policy manifest sorting
- generated docs/spec/ADR indexes
- generated capability matrix from machine-readable source
- generated metrics reports
- generated PR summary
- report directory creation

Not safe by default:

- accepting golden output changes
- adding policy exceptions
- adding dependency exceptions
- changing output schemas
- changing public contract versions
- adding suppressions

Those require an explicit command, a reason, or a manual reviewed edit.

### Non-Mutating Check Commands

Check commands verify the committed shape and must not modify the worktree.

Current:

```bash
cargo xtask ci-fast
cargo xtask precommit
cargo xtask check-pr
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-droid-review-config
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask check-test-oracles
cargo xtask dogfood
cargo xtask lsp-cockpit-report
cargo xtask targeted-test-outcome --before <path> --after <path>
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-traceability
cargo xtask metrics
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
cargo xtask check-supply-chain
```

Local tools may fix. CI verifies.

### Reporting Commands

Reporting commands produce review artifacts under `target/ripr/reports` and
machine-readable receipts under `target/ripr/receipts`.

Current:

```bash
cargo xtask pr-summary
cargo xtask commands
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask check-test-oracles
cargo xtask dogfood
cargo xtask check-droid-review-config
cargo xtask targeted-test-outcome --before <path> --after <path>
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
cargo xtask check-traceability
cargo xtask metrics
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
cargo xtask check-supply-chain
```

Reports should be useful to both humans and agents. A failed check should name
the path, explain why the rule exists, classify the fix kind, provide exact
commands to rerun, and include an exception template when a policy exception is
appropriate.

`check-pr-shape` is advisory. It writes `target/ripr/reports/pr-shape.md` and
warns when a diff shape suggests missing evidence, such as analyzer code
without specs/tests/fixtures, output code without output contract evidence, or
policy changes without process docs.

## Fix Kinds

Every check should classify failures into one of four fix modes.

| Fix kind | Meaning | Example response |
| --- | --- | --- |
| `auto_fixable` | The repo can normalize this safely. | Run `cargo xtask shape`. |
| `author_decision_required` | The author must explain or adjust the change. | Update dependency policy with reason and owner. |
| `reviewer_decision_required` | The change may be acceptable, but it changes a contract. | Update schema docs, goldens, changelog, and compatibility notes. |
| `policy_exception_required` | The default policy rejects the change unless an exception is justified. | Prefer moving logic into `xtask`, or add an allowlist entry with owner and reason. |

The failure text should answer:

- what failed
- why it matters
- what can be auto-fixed
- what requires judgment
- which file to edit
- which template to use
- which command to rerun

## Repair Brief Format

Policy checks should converge on this Markdown shape:

````md
# check-name

Status: fail

## Violation

Path:

```text
path/to/file
```

Problem:

```text
short description
```

Why this matters:

```text
repo-specific reason
```

Fix kind:

```text
policy_exception_required
```

Recommended fixes:

```text
1. Move the logic into xtask.
2. Or add an allowlist entry if this surface is truly necessary.
```

Then run:

```bash
cargo xtask shape
cargo xtask ci-fast
```
````

## PR Summary

The PR summary is the reviewer packet. It should become the first file a
reviewer opens for any non-trivial PR.

Current summary fields:

- production delta
- evidence and support delta
- detected surfaces
- public contracts touched
- policy exceptions
- suggested reviewer focus
- follow-up commands

Next summary fields:

- machine-readable receipt links
- warning-only drift checks

The summary should classify large evidence-heavy PRs correctly. A large fixture,
docs, and golden diff can be scoped when it supports one narrow production
change. A small code diff can still be unscoped when it mixes unrelated
contracts.

## Pre-Commit Shape

Local hooks are optional. CI is the source of truth.

The desired local hook behavior is:

```bash
cargo xtask shape --precommit
cargo xtask precommit
```

`precommit` should stay cheap. It should prefer formatting, policy checks,
file-surface checks, spec format, fixture contract validation, and Droid
workflow invariant checks. It should not run release packaging, marketplace
packaging, real mutation work, or slow full-matrix checks.

The current `precommit` command runs:

```bash
cargo fmt --check
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-droid-review-config
cargo xtask worktree doctor
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
cargo xtask check-campaign
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
```

Install local git hooks with:

```bash
cargo xtask install-hooks
```

This writes a ripr-managed `.git/hooks/pre-commit` hook that runs
`cargo xtask precommit` and keeps hook scripts out of version control. If a
pre-existing hook is already ripr-managed, the command updates it idempotently.
If a pre-existing hook is not ripr-managed, the command refuses to overwrite it
so local user hooks are not clobbered.

## CI Reports

CI uploads review artifacts from the Rust workflow when reports are present:

```text
target/ripr/reports/
target/ripr/receipts/
```

CI also writes `target/ripr/reports/index.md` into the GitHub Actions job
summary when the index exists. The report index lists available receipts when
`target/ripr/receipts/` has been generated.

Expected reports as the automation matures:

```text
shape.md
fix-pr.md
pr-summary.md
static-language.md
no-panic-family.md
allow-attributes.md
local-context.md
local-context.json
file-policy.md
executable-files.md
workflows.md
generated.md
dependencies.md
process-policy.md
network-policy.md
spec-format.md
fixture-contracts.md
traceability.md
capabilities.md
workspace-shape.md
architecture.md
public-api.md
output-contracts.md
doc-index.md
readme-state.md
markdown-links.md
campaign.md
goals.md
goals-next.md
pr-shape.md
fixtures.md
goldens.md
goldens-bless.md
golden-drift.md
golden-drift.json
test-oracles.md
test-oracles.json
dogfood.md
dogfood.json
critic.md
critic.json
index.md
index.json
receipts.md
pr-shape.md
metrics.md
metrics.json
release-readiness.md
release-readiness.json
suggested-fixes.md
suggested-fixes.patch
```

For untrusted PRs, CI should not push fixes. It may upload a suggested patch for
safe deterministic changes so authors or agents can apply it locally. Suggested
patches are repair hints, not policy exceptions: they must not carry badge
counts, golden blessings, baselines, suppressions, dependency exceptions, or
schema changes.

## Current Automation Queue

Campaign 1 and Campaign 2 are complete. Campaign 3 is active, and
`.ripr/goals/active.toml` plus `cargo xtask goals next` are the source of truth
for product work. The next automation path should improve trusted-change
evidence without delaying Campaign 3:

| Order | PR | Purpose |
| ---: | --- | --- |
| 1 | `devex/onboard-doctor` | Report whether the local checkout and toolchain are ready to work. |
| 2 | `devex/install-hooks` | Generate local hooks without checking executable scripts into the repo. |
| 3 | `xtask/command-registry` | Make the growing command surface self-describing. |

Analyzer work can now move through Codex Goals campaigns. Each campaign may span
multiple PRs, while each work item should still follow the scoped PR contract.

## Source-Of-Truth PR Body Scaffold

Use the active goal manifest to draft a PR body for one bounded work item:

```bash
cargo xtask pr-body --work-item <id>
```

The command writes:

```text
target/ripr/reports/source-of-truth-pr-body.md
```

The scaffold links the active goal, work item, proposal/spec/plan references
when present, acceptance text, non-goals, and proof commands. It deliberately
leaves support-tier and policy impact checkboxes unchecked because those claims
must be reviewed from the actual diff and proof, not inferred from active-goal
metadata.
