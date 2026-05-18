# Handoff: First-Run UX and Adoption Hardening Closeout

Date: 2026-05-17
Branch / PR: `codex/first-run-adoption-closeout` / #1114
Latest merged PR: #1112 `release(ux): verify first-run install surfaces`
(commit `c08447eb`)

## Current Work Item

`campaign/first-run-ux-hardening-closeout`

First-Run UX and Adoption Hardening compressed the shipped Rust repair loop
into an adopter-first path:

```text
one PR
-> one start-here summary
-> one top repairable Rust gap or clear no-action state
-> one repair route
-> one verification command
-> one agent packet
-> one receipt path
-> advisory CI or explicit gate authority boundary
```

This campaign stayed choreography-first. It composed explicit artifacts from
the Rust gap repair loop; it did not add analyzer truth, source edits,
generated tests, provider calls, mutation execution, preview-language
promotion, branch protection, or default CI blocking.

Durable restart context:

- [proposal](../proposals/RIPR-PROP-0009-first-run-ux-adoption-hardening.md)
- [behavior spec](../specs/RIPR-SPEC-0051-first-successful-pr-ux.md)
- [first successful PR workflow](../FIRST_PR_WORKFLOW.md)
- [quickstart](../QUICKSTART.md)
- [support tiers](../status/SUPPORT_TIERS.md)
- [installation verification](../INSTALLATION_VERIFICATION.md)
- [release checklist](../RELEASE.md)

## What Shipped

| Surface | Evidence |
| --- | --- |
| Product intent | `RIPR-PROP-0009` records why the first run needs one front door over existing artifacts. |
| Behavior contract | `RIPR-SPEC-0051` defines `start-here.{json,md}`, no-action states, recovery states, repair packets, and authority boundaries. |
| Public first-run packet | `ripr first-pr` / `ripr start-here` composes explicit artifacts into the first-run packet. |
| Local first-run wrapper | `cargo xtask first-pr --root .` delegates to the same public implementation for repo-local adoption checks. |
| Start-here report | `target/ripr/reports/start-here.{json,md}` is the reviewer-facing first screen. |
| Recovery states | Empty, missing, stale, wrong-root, malformed, timeout, blocked, and no-action states produce packets instead of vague failure. |
| Fixture corpus | `fixtures/first_successful_pr/` pins boundary-gap, output-contract, empty-diff, and blocked-ledger cases. |
| Dogfood receipts | `cargo xtask dogfood` validates the first successful PR corpus and reports first-run receipt status. |
| PR repair-card copy | PR comment bodies now use bounded repair-card language over gap records. |
| Editor orchestration | VS Code exposes `ripr: Start Current Repair` over existing diagnostics and repair actions. |
| Agent packet copy | Gap-ledger agent packets include pasteable Task, Context, Repair, Verification, Stop Conditions, Do Not Do, and Authority sections. |
| Generated CI summary | Generated GitHub CI renders the gap ledger and `start-here.{json,md}`, then opens the job summary with first-run status, top gap or no-action state, repair route, verify command, artifacts, and advisory gate boundary. |
| Gate adoption checklist | Blocking readiness now requires repair-loop evidence before moving beyond advisory or `visible-only`. |
| Public adoption copy | README leads with the repair loop, and Quickstart routes users through CLI, PR, and editor/agent first-hour paths. |
| Product demo | `docs/demo/first-successful-pr.md` turns the fixture corpus into a visible before/gap/repair/receipt story. |
| Adoption metrics | `cargo xtask dogfood` reports first-run packet, selected-gap, no-action, blocked, missing, stale, wrong-root, malformed, and timeout counters. |
| Install verification | `cargo xtask release-readiness --version <version>` verifies installed `ripr first-pr --help`, generated-CI start-here markers, and the VS Code `ripr: Start Current Repair` command contribution. |

## PR Chain

- #1013 `docs(proposal): add first-run UX hardening proposal`
- #1021 `docs(spec): define first successful PR UX contract`
- #1024 `cli/xtask: add first-pr workflow packet`
- #1059 `report: write first-pr start-here under reports`
- #1061 `ux: classify first-pr recovery states`
- `fixtures(ux): add first successful PR start-here corpus`
- `dogfood(ux): record first successful PR receipts`
- #1064 `comments(ux): polish repair-card copy`
- #1065 `lsp(ux): add start current repair command`
- #1066 `agent(ux): make gap repair packets pasteable`
- #1067 `ci(ux): add advisory first-run summary path`
- #1068 `docs(policy): add gate adoption checklist`
- #1069 `docs(readme): lead with repair loop`
- #1070 `docs(quickstart): compress first-hour paths`
- #1099 `cli(ux): add public first-pr command`
- #1105 `docs(demo): add first successful PR walkthrough`
- #1107 `metrics(ux): add first-run adoption counters`
- #1108 `lsp(first-pr): add bounded packet actions`
- #1110 `fixtures(editor): add first-pr bridge fixtures`
- #1111 `ci(ux): make first-run card the generated summary front door`
- #1112 `release(ux): verify first-run install surfaces`
- #1114 `campaign(ux): refresh first-run adoption closeout`

## Validation Run

Representative validation across the final shipped slices:

```bash
cargo xtask check-readme-state
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-workflows
cargo xtask dogfood
cargo test -p xtask release_readiness --bin xtask
cargo test -p xtask dogfood_generated_ci --bin xtask
cargo xtask check-pr
git diff --check
```

The final public CLI, generated-CI, and release/install verification PRs also
passed hosted GitHub `rust`, `msrv`, `vscode`, `cargo-deny`, Dependency Review,
CodeQL, coverage, Test Analytics, Droid review, Codecov patch, GitGuardian, and
PR Plan checks before merge.

## What Did Not Change

- No analyzer classification or ranking change.
- No new analyzer truth.
- No default generated CI blocking.
- No branch protection change.
- No source edits or generated tests.
- No provider or model calls.
- No mutation execution.
- No preview-language promotion.
- No public badge semantics change.
- No replacement of the gap decision ledger, PR review front panel, report
  packet index, first useful action report, or gate decision.

## Remaining Limits

- First-run UX is a usable adoption loop, not a stability or runtime-adequacy
  claim.
- The first-run packet composes explicit artifacts. Missing or stale artifacts
  still need regeneration before a repair should be assigned.
- Static movement remains advisory; runtime mutation testing remains the
  execution-backed confirmation step.
- Generated CI remains advisory unless a repository explicitly configures gate
  mode.
- TypeScript, JavaScript, and Python evidence remains preview, opt-in,
  visibly advisory, and not default gate-eligible.

## Next Work Item

No ready work item remains in First-Run UX and Adoption Hardening after this
closeout. Future work should open a new proposal or scoped campaign if
maintainers want a packaged demo repository, stricter adoption thresholds,
runtime-calibrated repair-success measurement, or external adopter validation.

## What Not To Do

- Do not add hidden analysis reruns to first-run packets.
- Do not make `start-here` a new evidence source.
- Do not make generated CI blocking by default.
- Do not ask users to learn the report graph before they can repair one gap.
- Do not promote preview-language evidence into Rust repair-loop authority.
- Do not treat static receipts as runtime mutation, coverage, or correctness
  proof.
