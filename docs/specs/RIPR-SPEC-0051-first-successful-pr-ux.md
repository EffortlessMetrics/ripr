# RIPR-SPEC-0051: First Successful PR UX

Status: accepted

## Problem

RIPR now has the repair-loop architecture needed for a useful Rust first run:
gap decision ledger, first useful action, PR repair cards, agent packets,
receipts, generated CI summaries, optional gates, and report packet navigation.
Those artifacts are correct but still require a new user to know too much
internal report topology.

A first-time adopter should be able to run one obvious path on one Rust PR and
answer:

```text
What happened?
What matters most?
What should I repair?
How do I verify movement?
Which artifact backs this?
What is advisory?
What, if anything, has pass/fail authority?
```

The first successful PR UX contract defines that front door. It does not add
analyzer truth. It composes explicit artifacts into a start-here packet for
manual pilot, coding-agent, and advisory-CI adoption.

## Behavior

RIPR should provide one documented first-run path for a Rust PR. The public
path is `ripr first-pr`; the repo-local `cargo xtask first-pr` command remains
a compatibility wrapper over the same first-run packet contract.

The target flow is:

```text
install or use local binary
-> run first PR path
-> read start-here packet
-> inspect top repairable Rust gap or no-action state
-> copy bounded repair packet
-> add one focused test or output proof outside RIPR
-> run verification command
-> keep receipt
-> decide whether to adopt advisory CI
```

The first-run path must compose existing explicit artifacts. It may call public
or documented commands to create missing first-run artifacts when the command
reports that work in the packet. It must not silently rerun hidden analysis,
invent analyzer evidence, edit source, generate tests, call providers, run
mutation testing, publish comments, or change gate policy.

### Start-here packet

The first-run path should write:

```text
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
```

An implementation may also mirror those files under
`target/ripr/first-pr/` for local pilot ergonomics, but
`target/ripr/reports/start-here.*` is the reviewer-facing packet location.

The packet must answer these reviewer questions in stable, user-facing terms:

| Question | Required answer |
| --- | --- |
| What happened? | First-run status and selected state. |
| What matters most? | One top repairable Rust gap, or a no-action/error state. |
| What should happen next? | One repair, refresh, retry, inspect, or no-action step. |
| Which artifact backs it? | Paths to the source gap ledger, repair card, agent packet, receipt, and gate decision when present. |
| Which command regenerates the missing piece? | A copyable command when the packet knows one. |
| What proves movement? | Verify and receipt commands, or a reason they are unavailable. |
| What has authority? | Advisory status plus explicit gate-decision authority when configured. |

The Markdown packet is the human first screen. The JSON packet is the stable
machine-readable form consumed by generated CI, LSP orchestration, and agents.

### Selection rules

The packet should select at most one top item for the first screen:

1. A PR-local, Rust, repairable, unsuppressed, unwaived, non-baseline gap with
   a repair route and verification command.
2. A missing required artifact state with a regeneration command.
3. A stale, wrong-root, malformed, timeout, or other blocked state with a retry
   or inspection command.
4. A no-action state such as empty diff, already observed, waived, suppressed,
   baseline-only, or report-only static limitation.

When multiple repairable gaps exist, the first-run path should prefer the same
top repair route used by the existing first useful action and PR repair-card
surfaces. It must not create a separate ranking policy in this spec.

### Top gap shape

A repairable top gap should include:

- gap ID and source artifact path;
- gap kind;
- changed behavior text when safe to display;
- why the gap matters;
- repair route;
- likely related test or output proof location when known;
- verification command;
- receipt command or receipt path when known;
- advisory and authority boundary;
- stable dedupe or anchor identity when available.

The packet must not present raw exposure class, static limitation, or numeric
confidence as the instruction. Raw evidence may appear under artifact links or
supporting context.

### No-action states

No-action states must be explicit. A first-run packet may select no-action when
the best available artifact says:

- empty diff;
- no projectable Rust gap;
- already observed;
- waived;
- suppressed;
- baseline-only;
- preview-only;
- report-only static limitation;
- language disabled or unsupported for this path.

No-action does not mean runtime adequacy, coverage adequacy, mutation adequacy,
or general correctness. The packet must preserve advisory language.

### Error and recovery states

Expected first-run failures must become useful packets, not stack traces, when
the process can still write output:

| State | Required behavior |
| --- | --- |
| Empty diff | Emit schema-valid no-action packet with no repair interruption. |
| Missing artifact | Name the missing artifact and regeneration command when known. |
| Stale artifact | Suppress repair interruption and show refresh command. |
| Wrong root | Fail closed and explain the expected and observed roots when known. |
| Malformed artifact | Fail closed and name the artifact and parser error summary. |
| Timeout | Emit advisory timeout packet with retry command and artifact paths. |
| Missing git base | Explain the missing base and show the next safe command or config. |

These states may appear in CI summaries and report packets. They must not be
treated as waived, suppressed, improved, clean, or gate-passing states.

### Repair packet

The first-run path should point to or produce one bounded repair packet for the
selected gap. The packet should include:

- task;
- context;
- related test or output proof target;
- repair route;
- verification command;
- receipt command;
- stop conditions;
- explicit "do not do" guidance.

The repair packet is guidance for a developer or coding agent. RIPR still must
not edit source or generate tests.

### Authority boundary

The first-run packet, PR summary, repair card, LSP action, and agent packet are
advisory projections. Pass/fail authority remains with explicit gate-decision
artifacts when configured.

If a gate decision exists, the packet may link to it and summarize its state.
If no gate decision exists, the packet must say that no gate authority was
provided. The packet itself must not become a gate.

### Surface alignment

First-run projection should reuse the same gap and repair vocabulary across:

- start-here packet;
- generated CI summary;
- PR repair cards;
- LSP "start current repair" orchestration;
- agent repair packet;
- receipt instructions;
- first successful PR docs.

Those surfaces may choose different levels of detail, but they should not
disagree about the selected gap, repair route, verification command, or
authority boundary.

## Required Evidence

An implementation of this spec must provide:

- a first-run command path or documented wrapper that writes
  `start-here.{json,md}`;
- JSON and Markdown packet outputs with schema version, status, selected item,
  artifact links, commands, and authority boundary;
- fixtures for repairable, no-action, and expected error states;
- tests proving the first-run path composes explicit artifacts and does not
  use hidden analyzer truth;
- tests or fixture checks for missing, stale, wrong-root, malformed, timeout,
  and empty-diff states;
- repair-card, agent-packet, generated-CI, and LSP alignment tests when those
  projections consume the start-here packet;
- dogfood receipts showing selected gap, repair attempt, verification, receipt,
  and movement;
- docs that explain first-run adoption without requiring report graph
  knowledge.

## Inputs

The first-run path may consume explicit artifacts and configuration:

- gap decision ledger;
- first useful action report;
- PR repair-card or review-comment artifacts;
- PR evidence ledger;
- agent packet;
- receipt and movement artifacts;
- gate decision;
- RIPR Zero or badge status;
- report packet index;
- repository configuration;
- git base/head metadata supplied by the user or CI.

If an implementation runs lower-level commands to create these artifacts, the
start-here packet must show what was run or give the equivalent regeneration
command.

## Outputs

The first-run path should output:

- `target/ripr/reports/start-here.json`;
- `target/ripr/reports/start-here.md`;
- optional `target/ripr/first-pr/start-here.{json,md}` mirror;
- links to gap ledger, repair card, agent packet, receipt instructions, gate
  decision, and report packet index when present;
- no-action or recovery packets when first-run evidence is unavailable.

It must not output new analyzer facts, source edits, generated tests, provider
results, mutation results, branch-protection changes, or gate decisions.

## Non-Goals

- No analyzer behavior changes.
- No new analyzer truth.
- No mutation execution.
- No runtime test execution by default.
- No coverage or general correctness claim.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No default CI blocking change.
- No branch protection change.
- No preview-language promotion.
- No hidden artifact discovery.
- No replacement of the gap decision ledger, PR review front panel, report
  packet index, first useful action report, or gate decision.
- No PR comment publishing by the first-run command.
- No LSP/editor behavior change in the spec PR.

## Acceptance Examples

Repairable Rust boundary gap:

- Given a PR-local Rust `MissingBoundaryAssertion` gap with repair route,
  anchor, related test, verification command, and no waiver or suppression, the
  start-here packet selects it as the top gap.
- The Markdown first screen shows status, why it matters, repair route, verify
  command, artifact links, and advisory status.
- The JSON packet links to the source gap ledger record.

Missing output proof:

- Given a supported `MissingOutputContract` gap, the start-here packet selects
  output-proof repair language such as `AddOutputGolden`.
- It shows the output/golden verification command instead of a generic
  classifier label or mutation-testing escalation.

Empty diff:

- Given an empty PR diff, the first-run path emits a schema-valid no-action
  packet.
- It does not run a long analysis path, emit a repair card, or imply runtime
  adequacy.

Missing artifact:

- Given no gap ledger and no equivalent source artifact, the packet reports the
  missing artifact and the command that regenerates it when known.
- The state is not counted as clean, waived, suppressed, improved, or passing.

Stale or wrong-root artifact:

- Given a stale or wrong-root gap ledger, the packet suppresses repair
  interruption and shows a refresh or rerun command.

Static limitation only:

- Given report-only static limitation evidence with no bounded repair route,
  the packet selects a no-action or inspection state.
- It does not render a PR repair instruction.

Configured gate:

- Given a gate-decision artifact, the packet links to it as authority and
  summarizes the state.
- Without that artifact, the packet says the first-run summary is advisory and
  has no pass/fail authority.

Receipt movement:

- Given a selected gap with a matching receipt that records improved movement,
  the packet links to the receipt and shows the movement state.
- Given unchanged-after-attempt movement, the packet preserves the next repair
  or inspection route and does not claim success.

## Test Mapping

Follow-up implementation should add or update:

- unit tests for first-run selection over explicit packet inputs;
- JSON and Markdown golden tests for `start-here.{json,md}`;
- fixture cases for boundary gap, output proof, static limitation, already
  observed, waived, suppressed, baseline-only, empty diff, missing artifact,
  stale artifact, wrong root, malformed input, timeout, and configured gate;
- CLI or xtask tests proving no hidden analysis path is used without reporting
  the command;
- generated CI tests proving summary alignment and advisory authority;
- PR comment snapshot tests proving repair-card vocabulary alignment;
- LSP tests for future `Start Current Repair` orchestration over the same
  packet fields;
- agent packet tests for pasteable task, context, repair, verification, stop
  conditions, and "do not do" sections;
- dogfood receipt checks for detect gap, repair, verify, movement, and
  no-action states.

This spec PR does not add production code or output fields.

## Implementation Mapping

Planned slices:

1. `docs/spec-first-successful-pr-ux-contract` defines this behavior contract.
2. `workflow/first-pr-packet` adds the first-run command path and keeps the
   repo-local wrapper aligned with existing artifacts.
3. `report/start-here-pr-repair-summary` renders `start-here.{json,md}`.
4. `ux/first-run-state-packets` standardizes empty, missing, stale,
   wrong-root, malformed, and timeout packet states.
5. `fixtures/first-successful-pr-corpus` adds first-run fixture and golden
   coverage.
6. `dogfood/first-run-receipts` records repair and movement receipts.
7. `comments/repair-card-copy-audit` aligns PR repair-card copy with this
   vocabulary.
8. `lsp/start-current-repair` projects one editor orchestration command over
   the same fields.
9. `agent/repair-packet-copy-audit` makes repair packets directly pasteable.
10. `ci/advisory-first-run-path` surfaces first-run status in generated CI.
11. `docs/gate-adoption-checklist` documents safe optional gate adoption.
12. `docs/readme-repair-loop-opener` and
    `docs/quickstart-first-hour-compression` make the first path public.
13. `campaign/first-run-ux-hardening-closeout` records proof, limits, and next
    lanes.

Likely implementation surfaces:

- `crates/ripr/src/cli/commands.rs`;
- `crates/ripr/src/output`;
- `xtask/src/main.rs`;
- `fixtures/first_successful_pr/`;
- `docs/FIRST_PR_WORKFLOW.md`;
- `docs/QUICKSTART.md`;
- `README.md`;
- `.ripr/traceability.toml`;
- `metrics/capabilities.toml`.

## Metrics

The dogfood report emits first-run adoption counters for the checked
`fixtures/first_successful_pr/` corpus. These counters measure whether the
first screen selected a repairable gap, produced a no-action state, or produced
a blocked recovery state; they are adoption evidence, not runtime mutation or
coverage adequacy claims.

- `first_run_packets_total`;
- `first_run_top_gap_selected_total`;
- `first_run_no_action_total`;
- `first_run_blocked_total`;
- `first_run_missing_artifact_total`;
- `first_run_stale_artifact_total`;
- `first_run_wrong_root_total`;
- `first_run_malformed_artifact_total`;
- `first_run_timeout_total`.

Future implementation may add repair and movement counters only when backed by
code and traceable tests. Candidate follow-up metrics:

- `first_run_repair_attempted_total`;
- `first_run_receipt_present_total`;
- `first_run_movement_improved_total`;
- `first_run_movement_unchanged_total`;
- `first_run_false_interruption_total`;
- `first_run_time_to_first_useful_action_seconds`.
