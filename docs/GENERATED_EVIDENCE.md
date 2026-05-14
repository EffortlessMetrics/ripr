# Generated Evidence Discipline

Generated evidence is not authored copy. It exists to make review and
automation trustworthy, so ordinary PRs must not hand-edit generated counts,
reports, receipts, or local build output.

This page defines the boundary between authored source-of-truth, generated
evidence, deterministic repair, and judgment-required decisions.

## Ownership Model

| Surface | Owner | Rule |
| --- | --- | --- |
| README badge links and layout | Human-authored docs | May be edited in docs or README PRs. |
| `badges/*.json` | Badge generation | Generated only by `cargo xtask badges` or the Badge Endpoints workflow. |
| `target/ripr/**` | Local or CI report generation | Never committed; upload or inspect as artifacts. |
| `crates/ripr/examples/sample/target/**` | Local sample build output | Never committed. |
| Specs, ADRs, plans, trackers, capability rows, traceability | Authored source-of-truth | Edited deliberately with the owning work item. |
| `target/ripr/reports/suggested-fixes.patch` | Deterministic repair hints | Generated only; authors may inspect or apply locally. |

The short rule:

```text
Authored truth lives in docs, specs, plans, policy, metrics, and source.
Generated evidence lives under target/ripr or explicit generated endpoints.
Deterministic repair is suggested by xtask.
Judgment-required decisions stop and name the owner.
```

## Badge Endpoints

Public badge endpoint JSON is generated evidence. Do not manually edit badge
numbers.

For a repo-scoped badge refresh:

```bash
cargo xtask badges
```

For a policy-backed `GapRecord` badge refresh:

```bash
cargo xtask badges --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

or trigger the Badge Endpoints workflow and review the generated
`badge: refresh public endpoints` PR.

Ordinary docs, README, implementation, and refactor PRs must not carry
`badges/*.json` diffs. If validation creates local badge endpoint diffs as a
side effect, remove them from the PR unless the work item is explicitly a badge
refresh.

Use:

```bash
cargo xtask badges --check
cargo xtask badges --check --gap-ledger target/ripr/reports/gap-decision-ledger.json
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
```

`cargo xtask check-badge-diff-policy` enforces the endpoint ownership boundary.
`cargo xtask check-generated-clean` rejects generated residue in ordinary PRs.

## Generated Report Artifacts

PR-scoped evidence belongs under `target/ripr/`:

```text
target/ripr/reports/
target/ripr/receipts/
target/ripr/fixtures/
target/ripr/dogfood/
```

These files can be uploaded by CI, attached to a PR, or inspected locally. They
must not be committed as source-of-truth.

Common generated reports:

```bash
cargo xtask pr-summary
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
cargo xtask critic
cargo xtask suggested-fixes
```

`cargo xtask check-pr` refreshes the review packet and target-local reports,
but it must not leave tracked source diffs. If it reports generated residue,
follow the repair instruction from the report instead of editing counts or
reports by hand.

## Deterministic Repair

Safe deterministic repair can be automated or represented as a patch:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask suggested-fixes
```

Current deterministic repair includes formatting, allowlist ordering, report
directory setup, PR summary generation, and narrow suggested patches for
allowlist ordering.

Suggested patches must never include:

```text
badge value edits
golden blessings
policy exceptions
baseline adoption
suppression creation
dependency exceptions
schema version changes
branch protection changes
preview-language promotion
```

Those require explicit authored decisions in the owning source-of-truth file.

## Judgment-Required Decisions

When automation cannot safely act, it should stop and name the owning surface.

| Decision | Owning surface |
| --- | --- |
| Golden output acceptance | Fixture expected outputs and blessing reason. |
| Policy exception | Policy allowlist or policy tracker with owner/reason/scope. |
| Baseline adoption or refresh | Baseline ledger and RIPR Zero workflow. |
| Suppression creation | `.ripr/suppressions.toml` with owner, reason, scope, and optional expiry. |
| Spec ID allocation | `cargo xtask specs next`, `docs/specs/README.md`, and traceability. |
| Capability status movement | `metrics/capabilities.toml` and `docs/CAPABILITY_MATRIX.md`. |
| Preview evidence promotion | Explicit Lane 2 policy tracker/spec and promotion packet. |
| Badge endpoint refresh | Badge Endpoints workflow or explicit badge refresh work item. |

Do not turn these into suggested patches or silent fixups.

## Standard PR Flow

For ordinary work:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
```

For repo hygiene and review context:

```bash
cargo xtask worktree doctor
cargo xtask pr-triage-report
cargo xtask gh-pr-status --pr <number>
```

For spec and source-of-truth movement:

```bash
cargo xtask specs next
cargo xtask check-spec-numbering
cargo xtask check-campaign
cargo xtask check-traceability
cargo xtask check-capabilities
```

A small PR means one semantic production delta, not low line count. Large
fixtures, docs, schemas, receipts, or traceability updates are acceptable when
they make that one delta reviewable.

## Cleanup Rules

Before committing an ordinary PR:

```bash
cargo xtask check-generated-clean
git diff --check
```

If generated residue is present, remove it from the PR rather than editing it:

```bash
git restore -- badges/*.json
```

Remove local target output with normal filesystem cleanup, for example:

```powershell
Remove-Item -Recurse -Force target\ripr
Remove-Item -Recurse -Force crates\ripr\examples\sample\target
```

Only run destructive cleanup against the intended generated directories.
