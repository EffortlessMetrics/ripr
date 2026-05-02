# PR Automation Operating Model

`ripr` uses repo automation to shape PRs before human review. The goal is not
more process. The goal is to remove deterministic cleanup from the review path
and turn judgment-required issues into precise repair briefs.

Core rule:

```text
Anything deterministic should be automated.
Anything judgment-based should produce a repair brief.
```

Humans and coding agents should spend attention on behavior, evidence,
exceptions, and public contracts. They should not spend attention on formatting,
allowlist order, report directory setup, generated indexes, or gate ordering.

Codex Goals consume this harness. The `/goal` loop may advance a multi-PR
campaign, but each work item should still leave the same shaped PR, reports, and
receipts described here.

## Current Commands

The current repo automation surface is:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
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

`pr-summary` writes `target/ripr/reports/pr-summary.md` from git diff and git
status. It classifies changed paths into production, evidence, docs, policy,
workflow, extension, and public-contract surfaces.

`precommit` is the cheap non-mutating guardrail. It checks formatting and the
policy surfaces that should fail quickly before review.

`check-pr` is the review-ready local gate. It runs the current fast CI lane,
then clippy, docs, and PR summary generation. It intentionally leaves
release/package verification to `ci-full` or release-specific workflows.

`fixtures` validates fixture contract shape, runs `ripr check` for fixture
directories when they exist, writes actual outputs under
`target/ripr/fixtures/<name>/`, compares stable expected outputs, and writes
`target/ripr/reports/fixtures.md`. It passes with a clear report when no
fixture directories exist yet.

`goldens check` runs fixtures and fails on drift between actual and expected
outputs without mutating checked-in files. `goldens bless <fixture> --reason
<reason>` records an explicit blessing reason, updates expected JSON and human
outputs, and appends the fixture expected-output changelog.

`ci-fast` is the current non-mutating local and CI check lane. It runs the Rust
checks plus the existing policy checks for static language, panic-family usage,
file policy, executable bits, workflow shell budgets, spec format, fixture
contracts, generated files, dependencies, process spawning, and network policy.
Those policy checks write Markdown pass/fail reports under
`target/ripr/reports`.

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
cargo xtask fixtures
cargo xtask goldens check
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
```

Local tools may fix. CI verifies.

### Reporting Commands

Reporting commands produce review artifacts under `target/ripr/reports`.

Current:

```bash
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
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
```

Planned:

```bash
cargo xtask dogfood
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

Future summary fields:

- likely missing evidence
- changed spec IDs
- fixture and golden changes
- capability status movement
- generated reports
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
file-surface checks, spec format, and fixture contract validation. It should not
run release packaging, marketplace packaging, real mutation work, or slow
full-matrix checks.

The current `precommit` command runs:

```bash
cargo fmt --check
cargo xtask check-static-language
cargo xtask check-no-panic-family
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
```

The hooks themselves should be generated locally by a future
`cargo xtask install-hooks` command instead of checked in as executable scripts.

## CI Reports

CI uploads review artifacts from the Rust workflow when reports are present:

```text
target/ripr/reports/
```

Expected reports as the automation matures:

```text
shape.md
fix-pr.md
pr-summary.md
static-language.md
no-panic-family.md
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
pr-shape.md
metrics.md
metrics.json
suggested-fixes.patch
```

For untrusted PRs, CI should not push fixes. It may upload a suggested patch for
safe deterministic changes so authors or agents can apply it locally.

## Future PR Sequence

The next automation path is trusted-change evidence:

| Order | PR | Purpose |
| ---: | --- | --- |
| 1 | `fixtures/first-two-goldens` | Add the first behavior fixtures and expected outputs. |
| 2 | `testing/test-oracle-report` | Measure the strength of `ripr`'s own test oracles. |
| 3 | `dogfood/static-self-check` | Generate focused `ripr`-on-`ripr` evidence reports. |
| 4 | `automation/gate-receipts-v1` | Write machine-readable receipts for gate runs. |
| 5 | `automation/critic-report` | Add an advisory adversarial review packet from existing reports. |
| 6 | `devex/onboard-doctor` | Report whether the local checkout and toolchain are ready to work. |
| 7 | `devex/install-hooks` | Generate local hooks without checking executable scripts into the repo. |

After those are in place, analyzer work can move through Codex Goals campaigns.
Each campaign may span multiple PRs, while each work item should still follow
the scoped PR contract.
