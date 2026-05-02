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

## Current Commands

The current repo automation surface is:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
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

`ci-fast` is the current non-mutating local and CI check lane. It runs the Rust
checks plus the existing policy checks for static language, panic-family usage,
file policy, executable bits, workflow shell budgets, spec format, fixture
contracts, generated files, dependencies, process spawning, and network policy.

## Command Lanes

`ripr` automation is split into three lanes.

### Mutating Shape Commands

Mutating commands are allowed to change files, but only for deterministic local
normalization.

Current:

```bash
cargo xtask shape
cargo xtask fix-pr
```

Future:

```bash
cargo xtask metrics --write
cargo xtask docs-index --write
cargo xtask capability-matrix --write
cargo xtask goldens bless <fixture> --reason "..."
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
```

Planned:

```bash
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-architecture
cargo xtask check-pr-shape
```

Local tools may fix. CI verifies.

### Reporting Commands

Reporting commands produce review artifacts under `target/ripr/reports`.

Current:

```bash
cargo xtask pr-summary
```

Planned:

```bash
cargo xtask metrics
cargo xtask dogfood
```

Reports should be useful to both humans and agents. A failed check should name
the path, explain why the rule exists, classify the fix kind, provide exact
commands to rerun, and include an exception template when a policy exception is
appropriate.

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
cargo xtask check-generated
```

The hooks themselves should be generated locally by a future
`cargo xtask install-hooks` command instead of checked in as executable scripts.

## CI Reports

CI should upload review artifacts from:

```text
target/ripr/reports/
```

Expected reports as the automation matures:

```text
shape.md
fix-pr.md
pr-summary.md
static-language.md
panic-family.md
file-policy.md
workflow-policy.md
generated.md
dependencies.md
process-policy.md
network-policy.md
spec-format.md
fixture-contracts.md
pr-shape.md
metrics.json
suggested-fixes.patch
```

For untrusted PRs, CI should not push fixes. It may upload a suggested patch for
safe deterministic changes so authors or agents can apply it locally.

## Future PR Sequence

The remaining automation path is:

| Order | PR | Purpose |
| ---: | --- | --- |
| 1 | `guided-check-reports` | Refactor existing checks to write Markdown repair briefs. |
| 2 | `ci-report-artifacts` | Upload `target/ripr/reports` from CI. |
| 3 | `fixture-golden-scaffolding` | Add fixture and golden command scaffolding. |
| 4 | `traceability-spec-id-checks` | Validate behavior manifests, spec IDs, and drift warnings. |
| 5 | `capability-metrics-report` | Generate metrics and capability reports from machine-readable sources. |
| 6 | `architecture-guard` | Add workspace shape, module boundary, and public API checks. |

After those are in place, analyzer work can move in goal mode with one scoped
capability per PR.
