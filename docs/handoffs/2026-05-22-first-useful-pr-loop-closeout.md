# Handoff: First Useful PR Loop Closeout

Date: 2026-05-22

Branch / PR: `campaign-first-useful-pr-loop-closeout` / pending at authoring

Latest merged PR: #188 `campaign: advance surface convergence work`
(commit `f4dd66aa2d008adaa727bd2edf5107f76dd57b5f`)

## Current Work Item

`campaign/first-useful-pr-loop-closeout`

Campaign 28 made the first useful PR loop the obvious front door for a changed
Rust PR:

```text
ripr first-pr
-> one top gap or no-action state
-> changed behavior
-> missing discriminator
-> focused proof intent
-> verify command
-> receipt command and receipt path
-> ripr outcome receipt
```

The campaign stayed advisory, static, and read-only. It did not add default
blocking, source edits, generated tests, provider/model calls, mutation
execution, runtime adequacy claims, coverage adequacy claims, public badge
semantic changes, or preview-language promotion.

## What Landed

| Surface | Evidence |
| --- | --- |
| Repo-native context | `docs/source-of-truth/`, `docs/agent-context/CONTEXT_SYSTEM.md`, `docs/ROADMAP.md`, and `.ripr/goals/active.toml` tie the control-plane language to existing RIPR source-of-truth files instead of a runner-local namespace. |
| Active-goal freshness | `cargo xtask goals next` rejects stale closed campaigns unless a successor or `no_current_goal = true` is recorded. |
| First-pr front door | `ripr first-pr --root . --base origin/main --head HEAD` performs read-only preflight for root, Git refs, diff, Cargo workspace, config/defaults, output paths, mode, and next-command guidance. |
| One-screen recommendation | `target/ripr/reports/start-here.{json,md}` names top gap/no-action, changed behavior, current evidence strength, missing discriminator, focused proof intent, verify command, receipt command, receipt path, and static-advisory boundary. |
| Reviewer-native receipt | `ripr outcome` records before flags, focused proof signals, movement after verification, remaining weak/unknown seams, and reviewer claim boundaries. |
| Demo story | `fixtures/first_successful_pr/boundary-gap/` documents and validates the before -> first-pr -> focused external proof -> outcome -> receipt path. |
| Generated CI/editor/agent convergence | Generated CI summary, VS Code first-pr handoff, and agent copyable packets now mirror the same repair unit and receipt boundary. |
| Campaign state | `.ripr/goals/active.toml` is closed with `no_current_goal = true`, and the closed manifest is archived at `.ripr/goals/archive/2026-05-22-first-useful-pr-loop.toml`. |

## PR Chain

- #119 `sync: import first useful PR loop campaign`
- #122 `docs: record ripr end goal`
- #123 `goals: reject stale closed active manifests`
- #170 `fix: fail closed on invalid first-pr roots`
- #171 `cli: add first-pr front-door preflight`
- #172 `fix: preflight first-pr git refs`
- #176 `ux: clarify first-pr recommendation proof rails`
- #177 `xtask: validate active goal artifact references`
- #178 `campaign: advance first useful PR loop`
- #179 `first-pr: provide receipt command in recommendation`
- #181 `outcome: add reviewer-native receipts`
- #182 `campaign: advance outcome receipt work`
- #183 `docs: align outcome receipt workflow copy`
- #184 `fixtures: add first-pr boundary demo story`
- #185 `test: cover first-pr demo fixture contract`
- #187 `surfaces: align first-pr handoff wording`
- #188 `campaign: advance surface convergence work`
- `campaign: close first useful PR loop`

## Proof Executed

Closeout validation for this PR:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

The final surface-convergence PR also passed:

```bash
cargo test -p ripr --lib agent_seam_packets
cargo test -p ripr --lib generated_github_workflow
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Claim And Support-Tier Changes

No support-tier promotion landed in this closeout. The user-facing claim remains
bounded to advisory static evidence: RIPR can orient a reviewer or agent toward
one focused proof path, but it does not prove runtime adequacy, coverage
adequacy, mutation results, correctness, gate approval, or merge approval.

## Policy Ledger Updates

No policy exception changed. The only policy-adjacent update is lifecycle state:
the active campaign manifest is closed, marked with `no_current_goal = true`,
and archived for history.

## Remaining Limits

- Rust remains the stable first-pr path.
- TypeScript, JavaScript, and Python remain preview evidence outside this
  campaign's first useful PR promise.
- Start-here, generated CI, editor handoff, and agent packets are advisory
  orientation surfaces, not gate authority.
- Receipts record static artifact relationships and observed movement, not
  mutation proof.
- No successor campaign is selected in `.ripr/goals/active.toml`.

## Artifacts

- `.ripr/goals/archive/2026-05-22-first-useful-pr-loop.toml`
- `docs/handoffs/2026-05-22-first-useful-pr-loop-closeout.md`
- `plans/first-useful-pr-loop/implementation-plan.md`
- `docs/IMPLEMENTATION_CAMPAIGNS.md`
- `target/ripr/reports/start-here.md`
- `target/ripr/reports/check-pr.md`

## Next Recommended Goal

Select a successor campaign from the roadmap or an accepted source-of-truth
stack before starting more behavior-bearing work. Until then,
`.ripr/goals/active.toml` intentionally records `no_current_goal = true` so a
cold-start agent does not continue from a closed campaign.

## What Not To Do

- Do not keep working from Campaign 28 after this closeout.
- Do not infer a successor campaign from chat history.
- Do not promote first-pr advice into default blocking or gate authority.
- Do not claim runtime, coverage, mutation, correctness, merge, or support-tier
  proof from the static first-pr loop.
- Do not add source edits, generated tests, provider calls, or mutation
  execution to the first-pr front door.
