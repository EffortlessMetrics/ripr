# RIPR-PROP-0008: First-Run UX and Adoption Hardening

Status: proposed

Owner: ripr maintainers

Created: 2026-05-15

Target campaign: First-run UX and adoption hardening

Linked specs:

- `RIPR-SPEC-0048`: First successful PR UX contract, planned
- `RIPR-SPEC-0046`: Gap decision ledger
- `RIPR-SPEC-0020`: First useful action report
- `RIPR-SPEC-0023`: PR review front panel report
- `RIPR-SPEC-0024`: Report packet index
- `RIPR-SPEC-0025`: PR inline comment publisher
- `RIPR-SPEC-0038`: Generated PR CI review workflow

Linked ADRs:

- None planned for the proposal slice.

Linked work items:

- Campaign plan and active manifest are planned follow-ups.

## Problem

RIPR now has the structural pieces for a usable Rust repair loop: gap decision
ledger, PR repair cards, first useful action, PR review cockpit, LSP/editor
projection, agent packets, badges, optional gates, receipts, support tiers,
and closeouts. The remaining adoption problem is not missing evidence
infrastructure. It is first-run friction.

A new team still has to know too much report topology before they can answer
the adoption question:

```text
Can I run ripr on one real Rust PR, fix one useful gap, verify movement, and
trust the recommendation as advisory evidence?
```

The current [first successful PR workflow](../FIRST_PR_WORKFLOW.md) explains
the path, but the product path is still spread across multiple commands and
artifacts. First-run UX should compress the shipped repair loop into one clear
front door:

```text
one PR
-> one start-here summary
-> one top repairable gap or no-action state
-> one repair route
-> one verification command
-> one agent packet
-> one receipt path
```

Without that front door, users can still experience correct artifacts as a
pile of reports. The next campaign should make the first real PR feel obvious,
useful, and safe without weakening the gap decision boundary.

## Users and surfaces

- Staff engineers evaluating RIPR on a first Rust PR.
- Reviewers reading generated CI summaries and PR repair cards.
- Coding agents consuming bounded repair packets.
- Maintainers deciding whether to add advisory generated CI.
- Platform owners separating advisory evidence from optional gate authority.
- Contributors proving first-run regressions with fixtures and receipts.

Primary surfaces:

- CLI or xtask first-run workflow packet;
- `target/ripr/reports/start-here.{json,md}`;
- generated CI job summary and uploaded report packet;
- PR repair cards;
- LSP/editor repair orchestration;
- agent repair packets;
- first-run fixtures, dogfood receipts, and adoption docs.

## Success criteria

- One documented command path produces a first-run packet for a Rust PR.
- A `start-here` artifact answers what happened, what matters most, what to do
  next, what shows movement, and what has authority.
- The first-run path selects one top repairable gap or explains a no-action
  state such as empty diff, already observed, waived, suppressed, or report-only
  static limitation.
- Every user interruption names a gap, repair route, and verification command.
- Missing, stale, malformed, wrong-root, timeout, and empty-diff states produce
  clear packets with regeneration or retry commands.
- First-run fixtures cover at least missing boundary assertion, missing
  output/golden proof, report-only static limitation, already observed
  no-action, and waived or suppressed non-blocking state.
- PR repair-card copy stays boring and direct: no raw classifier label as the
  instruction, no generic confidence field, and no default mutation-testing
  escalation.
- Editor and agent surfaces expose one obvious repair-start path over the
  existing gap records rather than a parallel evidence source.
- Generated CI stays advisory by default and keeps gate authority separate.
- Adoption metrics track repair success, unchanged-after-attempt, report-only
  no-action, false interruption, and time to first useful action.

## Proposed shape

Add a first-run orchestration layer that composes existing artifacts instead of
creating new analyzer truth:

```text
gap ledger
+ first useful action
+ PR repair comments
+ PR evidence summary
+ agent packet
+ gate decision
+ badge status
+ receipt state
-> start-here summary
-> first-run packet
```

The first public behavior spec should define the first successful PR UX
contract. It should state how a first-run command or xtask wrapper creates a
packet, how `start-here` selects the top repairable gap, how no-action states
are represented, and how every missing artifact reports the command that
regenerates it.

The campaign should then implement the smallest executable path, fixture it,
dogfood it, and polish projection copy across PR comments, editor actions,
agent packets, CI summaries, README, and quickstart. This proposal does not
choose whether the first command lands as public CLI or xtask; the spec and
implementation plan should decide that based on existing command ownership.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep improving individual reports only. | Correct reports still force new users to learn the artifact graph before they get a first useful repair. |
| Put all first-run behavior in generated CI. | Manual pilot and local agent workflows need the same start-here packet before CI rollout. |
| Make `pilot-summary.md` the only first screen. | Pilot is useful, but the new repair loop also has gap ledger, repair cards, agent packets, gate boundaries, badges, and receipts. |
| Add more analyzer classification before first-run UX. | Rust gap projection is already usable; the adoption blocker is choreography and copy, not more analyzer truth. |
| Make first-run CI blocking. | First-run adoption must remain advisory until the repository explicitly opts into gates. |
| Promote preview-language evidence into the first-run path. | TypeScript, JavaScript, and Python remain preview and advisory; this campaign is Rust adoption hardening. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0048`: First successful PR UX contract.
- Update `RIPR-SPEC-0020` if start-here selection changes first useful action
  behavior or inputs.
- Update `RIPR-SPEC-0023`, `RIPR-SPEC-0024`, and `RIPR-SPEC-0038` if generated
  CI summary or packet index gains a start-here projection.
- Update `RIPR-SPEC-0025` only if repair-card copy or eligibility changes.
- Update `RIPR-SPEC-0046` only if the gap ledger needs new first-run metadata.

## Architecture decisions needed

No ADR is required for the proposal slice. Add an ADR only if the campaign
changes durable command ownership, creates a persisted first-run state model,
or changes where generated reports live. The default architecture is an
additive orchestration layer over explicit artifacts under `target/ripr/`.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/first-run-ux-hardening-proposal`
2. `docs/first-successful-pr-ux-contract`
3. `workflow/first-pr-packet`
4. `report/start-here-pr-repair-summary`
5. `ux/first-run-state-packets`
6. `fixtures/first-successful-pr-corpus`
7. `dogfood/first-run-receipts`
8. `comments/repair-card-copy-audit`
9. `lsp/start-current-repair`
10. `agent/repair-packet-copy-audit`
11. `ci/advisory-first-run-path`
12. `docs/gate-adoption-checklist`
13. `docs/readme-repair-loop-opener`
14. `docs/quickstart-first-hour-compression`
15. `campaign/first-run-ux-hardening-closeout`

Keep the slices projection-first. Do not mix analyzer changes, schema
additions, CI behavior, editor commands, public docs, and closeout in one PR.

## Evidence plan

- First-run UX spec defines start-here behavior, no-action states, authority
  boundaries, regeneration commands, and acceptance examples.
- Fixture corpus covers missing boundary assertion, missing output/golden
  proof, report-only static limitation, already observed behavior, waived
  state, suppressed state, empty diff, stale ledger, malformed artifact, wrong
  root, and timeout packet.
- Golden outputs pin `start-here.{json,md}`, repair card copy, agent packet
  copy, and receipt instructions.
- CLI or xtask tests prove the first-run wrapper composes explicit artifacts
  and does not rerun hidden analysis.
- PR comment snapshots prove reviewer copy avoids raw classifier instructions,
  generic confidence fields, duplicate multi-line literal comments, and default
  mutation-testing escalation.
- LSP tests prove one orchestration command can find the nearest projected gap,
  copy the repair packet, open the related test, and copy verification or
  receipt commands without parsing prose.
- Dogfood receipts show that the selected top gap led to a focused test or
  output proof, movement was verified, and no-action states did not interrupt.
- Metrics track top gap selected, repair attempted, repair kind, movement
  result, receipt presence, unchanged reason, user override, false
  interruption, and time to first useful action.
- Docs link README, quickstart, support tiers, first successful PR workflow,
  PR review guidance, and gate adoption checklist.

## Risks

- First-run UX could become a second evidence source. Mitigation: start-here
  must compose explicit artifacts and link back to the gap ledger, repair card,
  agent packet, receipt, and gate decision.
- The first command could hide important advisory boundaries. Mitigation: every
  packet states advisory status and gate authority separately.
- Error handling could become more confusing than the current report graph.
  Mitigation: empty, missing, stale, malformed, wrong-root, and timeout states
  get explicit packet shapes and retry commands.
- PR comments could over-polish away repair detail. Mitigation: each repair
  card still includes changed behavior, why it matters, repair route, verify
  command, and stable fingerprint.
- Editor orchestration could become a new diagnostics model. Mitigation: LSP
  commands consume projected gap records and existing actions only.
- Adoption metrics could drift back to finding counts. Mitigation: the campaign
  measures repair attempts, movement, receipts, and no-action outcomes.

## Non-goals

- No analyzer rewrite.
- No new analyzer truth.
- No runtime mutation execution.
- No coverage or general correctness claim.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No default CI blocking change.
- No branch protection change.
- No preview-language promotion.
- No public badge semantics change beyond linking to first-run guidance.
- No replacement of the gap decision ledger, PR cockpit, or report packet
  index.
- No hidden artifact discovery; missing artifacts must report regeneration
  commands.

## Exit criteria

This proposal can move to `accepted` when:

- the first successful PR UX spec lands;
- one command path or documented wrapper creates a first-run packet;
- `start-here.{json,md}` exists and selects a top repairable gap or clear
  no-action state;
- first-run fixture and golden coverage pins repair, no-action, missing, stale,
  malformed, and timeout states;
- PR repair cards, agent packets, LSP orchestration, generated CI summary, and
  docs use the same first-run repair language;
- dogfood receipts show one selected gap moving through repair and verification;
- README, quickstart, support tiers, and gate adoption docs describe the
  advisory rollout path without overclaiming authority;
- closeout records what shipped, what stayed advisory, and what remains for
  future lanes.

