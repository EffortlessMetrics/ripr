# Handoff: Generated Evidence Discipline Closeout

Date: 2026-05-14
Branch / PR: `campaign-generated-evidence-closeout` / pending at authoring
Latest merged PR: #975 `docs: add generated evidence discipline` (commit `d9623085`)

## Current Work Item

`campaign/generated-evidence-discipline-closeout`

The generated-evidence discipline lane made RIPR's repo operating system safer
for agentic development. The shipped surface now separates authored
source-of-truth, generated evidence, deterministic repair, judgment-required
decisions, and review receipts.

This lane is repo-operations work. It does not reopen Lane 2 policy semantics
or change analyzer truth, evidence identity, recommendation ranking,
LSP/editor behavior, generated tests, provider behavior, mutation execution,
branch protection, default CI blocking, baseline adoption, suppression
creation, or preview-language promotion.

`.ripr/goals/active.toml` still records the last closed numbered campaign. This
closeout does not select or reorder the next numbered campaign.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Badge endpoint refresh has an owned automation path | #874 added the Badge Endpoints workflow, `cargo xtask badges`, committed Shields endpoint schema/docs, and the `automation/badge-endpoints` refresh path. |
| Ordinary PRs reject generated residue | #930 added `cargo xtask check-generated-clean`, wired it into PR gates, and documented cleanup for `badges/*.json`, `target/ripr/**`, and sample target residue. |
| Badge endpoint diffs have an ownership policy | #938 added `cargo xtask check-badge-diff-policy`, allowing README badge layout changes while rejecting ordinary `badges/*.json` endpoint diffs. |
| `check-pr` keeps badge checks non-mutating | #938 routes PR validation through non-mutating badge checks and generated-clean policy instead of refreshing committed endpoint JSON. |
| Worktree hygiene is mechanical | #941 added `cargo xtask worktree doctor` for dirty main, behind branches, generated residue, untracked sample targets, and broad-source-of-truth warnings. |
| Spec numbering is mechanical | #946 added `cargo xtask specs next` and `cargo xtask check-spec-numbering`, with PR gate wiring and docs. |
| Open PR board state is visible | #950 added `cargo xtask pr-triage-report`, producing board-level findings for duplicate families, stale drafts, behind branches, validation gaps, and sensitive surfaces. |
| Merge readiness is deterministic | #952 added `cargo xtask gh-pr-status --pr <number>`, reporting checks, reviews, mergeability, and safe next action. |
| Lane 2 reopening triggers are encoded | #958 documented that stricter policy, preview promotion, gate eligibility, baseline adoption, suppression semantics, calibration promotion, and runtime/static vocabulary changes require explicit policy work. |
| Campaign/source-of-truth checks are harder to bypass | #965 tightened campaign checks around focused trackers, done-item commands, spec-backed implementation entries, closeout capability pointers, and active-manifest boundaries. |
| Gate receipts exist for review packets | #55 previously added `cargo xtask receipts` / `receipts check`; this lane keeps those receipts as generated evidence under `target/ripr/receipts/`. |
| Critic report exists for reviewer focus | #84 previously added `cargo xtask critic`; this lane keeps it advisory and target-local. |
| Deterministic repair suggestions are generated, not guessed | #971 added `cargo xtask suggested-fixes`, writing `target/ripr/reports/suggested-fixes.{patch,md}` for safe allowlist ordering only. |
| Contributor docs explain generated evidence discipline | #975 added `docs/GENERATED_EVIDENCE.md` and linked it from `CONTRIBUTING.md`, `docs/PR_AUTOMATION.md`, and the documentation index. |

## PR Chain

- #874 `badge: add verification stack, xtask badge commands, and badge-endpoint CI/workflow`
- #930 `devex: reject generated residue in ordinary PRs`
- #938 `badge: enforce generated endpoint ownership`
- #941 `devex: add worktree doctor for agent PR hygiene`
- #946 `docs(spec): add next-spec and numbering guard`
- #950 `devex: add open PR triage report`
- #952 `devex: add gh PR status report`
- #958 `docs(policy): define lane 2 reopening triggers`
- #965 `devex: harden campaign source-of-truth checks`
- #971 `automation: add suggested fixes patch`
- #975 `docs: add generated evidence discipline`
- `campaign/generated-evidence-discipline-closeout`

Pre-existing but load-bearing:

- #55 `automation: add gate receipts`
- #84 `automation: add advisory critic report`

## Verification Run

Closeout validation before opening this PR:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-generated-clean
cargo xtask check-campaign
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in the generated-evidence discipline lane after this
closeout.

Likely future repo-operations work should open explicitly if maintainers need a
new guardrail for release publication, external credential policy, stronger
campaign selection automation, or CI merge queue ownership. Those are not
implied by this closeout.

## What Not To Do

- Do not refresh `badges/*.json` in ordinary PRs.
- Do not hand-edit RIPR badge numbers.
- Do not make `check-pr` mutate committed endpoint JSON or source files.
- Do not put generated target artifacts into review diffs.
- Do not apply `suggested-fixes.patch` for judgment-required decisions.
- Do not bless goldens, add policy exceptions, adopt baselines, create
  suppressions, change dependency exceptions, or promote preview-language
  evidence through deterministic repair tooling.
- Do not reopen Lane 2 policy semantics for repo-operations polish.
- Do not add analyzer, editor, provider, generated-test, mutation-execution,
  release, branch-protection, default-blocking, baseline, suppression, or
  preview-promotion work to this closeout.
