# Handoff: First-Run UX and Adoption Hardening Closeout

Date: 2026-05-16
Branch / PR: `codex/first-run-ux-closeout` / pending at authoring
Latest merged PR: #1070 `docs(quickstart): compress first-hour paths`
(commit `cf7a339c`)

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

## What Shipped

| Surface | Evidence |
| --- | --- |
| Product intent | `RIPR-PROP-0009` records why the first run needs one front door over existing artifacts. |
| Behavior contract | `RIPR-SPEC-0051` defines `start-here.{json,md}`, no-action states, recovery states, repair packets, and authority boundaries. |
| Local first-run packet | `cargo xtask first-pr --root .` composes explicit artifacts into the first-run packet. |
| Start-here report | `target/ripr/reports/start-here.{json,md}` is the reviewer-facing first screen. |
| Recovery states | Empty, missing, stale, wrong-root, malformed, timeout, blocked, and no-action states produce packets instead of vague failure. |
| Fixture corpus | `fixtures/first_successful_pr/` pins boundary-gap, output-contract, empty-diff, and blocked-ledger cases. |
| Dogfood receipts | `cargo xtask dogfood` validates the first successful PR corpus and reports first-run receipt status. |
| PR repair-card copy | PR comment bodies now use bounded repair-card language over gap records. |
| Editor orchestration | VS Code exposes `ripr: Start Current Repair` over existing diagnostics and repair actions. |
| Agent packet copy | Gap-ledger agent packets include pasteable Task, Context, Repair, Verification, Stop Conditions, Do Not Do, and Authority sections. |
| Generated CI summary | Generated GitHub CI shows first-run status, top gap or no-action state, repair route, verify command, artifacts, and advisory gate boundary. |
| Gate adoption checklist | Blocking readiness now requires repair-loop evidence before moving beyond advisory or `visible-only`. |
| Public adoption copy | README leads with the repair loop, and Quickstart routes users through CLI, PR, and editor/agent first-hour paths. |

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
- `campaign/first-run-ux-hardening-closeout`

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
cargo xtask check-pr
git diff --check
```

The final README and Quickstart PRs also passed hosted GitHub `rust`, `msrv`,
`vscode`, `cargo-deny`, Dependency Review, CodeQL, coverage, Test Analytics,
Droid review, Codecov patch, GitGuardian, and PR Plan checks before merge.

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
maintainers want a packaged demo repository, release framing for 0.6.0,
stricter adoption metrics, or external adopter validation.

## What Not To Do

- Do not add hidden analysis reruns to first-run packets.
- Do not make `start-here` a new evidence source.
- Do not make generated CI blocking by default.
- Do not ask users to learn the report graph before they can repair one gap.
- Do not promote preview-language evidence into Rust repair-loop authority.
- Do not treat static receipts as runtime mutation, coverage, or correctness
  proof.
