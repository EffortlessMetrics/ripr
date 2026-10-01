# RIPR-SPEC-0194: RepairCard as the default CLI agent handoff

Status: proposed

Owner: product / agent

Created: 2026-10-06

Linked issues:

- #4667 (this slice)
- #3166 (parent: repair packet card profile)
- #4663 (RepairCardV1; landed as PR #4979, RIPR-SPEC-0192)
- #4666 (bounded detail references; landed as PR #4980, RIPR-SPEC-0193)
- #4669 (measured budget ratification)
- #4330 (edit-cage authority)

Support-tier impact:

- None. `ripr agent card` is an advisory, read-only projection over the
  existing analysis authorities; it accepts no edit or execution authority
  and writes nothing.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The command runs the
  same check analysis the packet route already runs and renders to stdout
  only.

## Problem

RIPR-SPEC-0192 defined `RepairCardV1` and RIPR-SPEC-0193 kept it finite with
typed detail references, but no production surface assembled one: the card
builder, its budget, and its route gate sat behind staged `dead_code`
expectations. Meanwhile the ordinary agent handoff still required a consumer
to open the complete canonical packet — the unbounded document — to learn
what to do next. #4667 wires the first producer: the compact card becomes
the default `ripr agent` work handoff, projected verbatim from the shared
authorities (fix-instruction witness, repair-route readiness, packet
eligibility, the packet's own edit-cage derivation, the repair-attempt
inventory, and the typed command catalog), while the complete canonical
packet stays retrievable through the card's explicit detail route and the
compatibility `ripr agent packet` command remains byte-for-byte unchanged.

## Behavior

`ripr agent card --root PATH --seam-id ID [--json]` resolves one visible,
policy-admitted seam exactly like `ripr agent packet` (same inventory, same
not-found refusal naming `ripr pilot`, same `AgentBriefPolicy` omission
check), then assembles one `RepairCardV1`:

- `crates/ripr/src/app/repair_card_handoff.rs` gathers the producer facts:
  the packet eligibility/readiness authority
  (`analysis::repair_route::repair_packet_eligibility`), the witness of the
  check finding that names the seam's canonical gap (one `ripr check` pass
  with the default cooperative git deadline; the witness is the single
  instruction authority through `FixInstructionSummary::from_witness`, with
  `unavailable()` when no finding names the gap). Because the canonical gap
  id is content-derived and excludes the source location, sibling seams can
  share one id: the finding match requires the gap owner to equal the seam
  owner, so another seam's witness is never credited to this card. The
  repository head (`git rev-parse HEAD`), the portable
  workspace identity, the seam's most recent repair-attempt manifest by
  `created_unix_ms`, the rendered canonical packet behind
  `PacketCommandContext::Standalone`, and the typed packet inspection
  command. `assemble_repair_card` is the pure, unit-tested projection from
  those facts.
- Snapshot identity is producer-owned through admitted `TestTargetEvidence`
  (the selected Existing target's identity, else the first related test's
  admitted target identity). When nothing on the seam names a portable
  identity the command fails closed and names the packet route in the error;
  identity is never minted from a checkout path.
- The instruction summary, changed behavior (witness `changed_expression`,
  else the seam expression), the exact blocker (first witness
  missing-discriminator fact), the observer-setup flag, the
  suggested-assertion detail, and limitation lines are copied verbatim from
  the witness; with no witness the instruction is `unavailable`, the blocker
  and assertion detail are absent, and the changed behavior falls back to
  the seam expression.
- The edit cage reuses the packet authority's exact derivation
  (`task_for(entry) == TASK_WRITE_TARGETED_TEST` and
  `recommended_test_for(entry)`): allowed files carry the recommended test
  file, forbidden files carry the production file unless they coincide, and
  the two shared edit-cage stop conditions ride on the card unchanged. The
  card cannot promise a different edit surface than the attempt enforces.
- `done_when` is projection-only: static movement `closed_by_selected_route`,
  edit cage `compliant`, mutation confirmation `not_requested`, currentness
  `current`; focused test execution is `verified_pass` exactly when the
  packet task is the targeted-test task, else `explicitly_not_run`. The card
  never marks the work complete.
- The next action is the typed `ripr agent packet --seam-id ID --json`
  inspection command, and only when the shared fail-closed route gate opens
  (`repair_card_route_exposable(instruction.state, readiness.repair_ready)`
  plus packet eligibility). A card with no witness, a limited instruction
  state, or a not-ready seam carries `next_action: null` and the human
  summary names the explicit non-actionable reason (instruction state and
  the readiness flip), never a guessed route. The builder independently
  rejects a next command the gate would not expose.
- All nine detail families from RIPR-SPEC-0193 are populated: the canonical
  packet, related-test candidates, and static movement name the packet
  route — the canonical packet is `current` only when the rendered envelope
  actually surfaces this seam in its `packets` array; a seam the packet
  queue omits records the exact unavailable reason `the canonical packet
  does not surface this seam under the current packet queue policy` instead
  of claiming a route the packet would not answer. With a witness, fix
  instruction, witness stage evidence, and
  limitation detail name the `ripr explain <finding_id>` route (stage
  evidence naming `ripr check --json`); without one all three record the
  exact unavailable reason `no witness-producing finding names this seam in
  the current analysis`. Repair-attempt status names
  `ripr agent status --json` when an attempt exists — `current` at the
  snapshot's head, `stale` when the manifest was recorded against another
  repository head — and records an exact
  unavailable reason otherwise. Focused-proof receipt and mutation
  calibration are always unavailable with their precise reasons. The default
  versioned budget applies unmodified (#4669 ratifies the numbers).
- `--json` emits the versioned `repair_card.v1` document (pretty JSON with a
  trailing newline). Without it, the same typed fields render as a compact
  human summary in card order: the renderer presents values verbatim, never
  re-derives, reorders, or enhances them, names the selected target
  (existing or proposed) when the card carries one, and ends with the
  explicit full packet line — the typed next action's display, which binds
  the portable root and is directly executable, when the route gate is open,
  else the rootless packet route exactly as the detail references name it.

`ripr agent packet` is unchanged and remains the compatibility path: same
envelope, same schema, same bytes. Shared field names keep one meaning
across the card, the packet, and the attempt manifest. The staged
`dead_code` expectations #4663/#4666 left on the card builder, the budget
constructors, the packet inspection command, and their helpers are removed
by this wiring; visibility is raised only where the producer is the first
live caller (`git_output`, `task_for`, `TASK_WRITE_TARGETED_TEST`).

## Required Evidence

- `cargo test -p ripr --lib repair_card` covers the pure projection: every
  family rides behind a typed reference with the packet route for the
  canonical packet, the route gate omits the next action (and the builder
  stays consistent) when the gate is closed, and the wire card stays within
  the default item/byte bounds.
- `cargo test -p ripr --lib agent_card` and `--lib agent` cover CLI parsing:
  `--seam-id` is required and non-empty, `--json` is opt-in (the human
  summary is the default), unknown arguments name `ripr agent card --help`,
  and the unknown-subcommand error lists `card`.
- `cargo test -p ripr --test cli_smoke -- agent_card` covers the journey on a
  committed boundary-gap fixture with a dirty worktree: `--json` emits
  `repair_card.v1` naming the seam, the committed HEAD, and a current
  canonical-packet detail route (`ripr agent packet --seam-id ... --json`)
  without embedding packet envelope content; the default human output names
  the seam, the next-action line, and the full packet route; a `probe:...`
  finding ID is refused with the same seam-ID source hint the packet surface
  names.

## Non-Goals

- No LSP or MCP card projection in this slice; editor surfaces keep their
  existing witness diagnostics.
- No deletion of the complete packet or any low-level expert command.
- No automatic edit, execution authority, or support-tier promotion; the
  card's one next action is an inspection route, and
  `CommandRole::Repair` remains future vocabulary.
- No first-useful-action or PR/CI summary card projection here; those
  surfaces cannot strengthen or rerank a card they do not project, and a
  later slice adopts them against measured use (#4669).
- No measured usability claim: the budget numbers stay provisional-but-
  versioned until #4669 ratifies them.

## Acceptance Examples

1. `ripr agent card --root . --seam-id ID` on a visible, policy-admitted seam
   prints the compact card: typed fields in card order, an explicit
   non-actionable reason when no bounded route is exposed, and a final
   `full packet:` line naming `ripr agent packet --seam-id ID --json`.
2. The same call with `--json` prints one `repair_card.v1` document whose
   canonical-packet detail reference is `current` and names the packet
   route; no packet envelope content appears on the wire card.
3. A seam hidden by `AgentBriefPolicy` is refused with the same
   omission reason and pilot hint `ripr agent packet` names.
4. An unknown seam ID — including a `probe:...` finding ID — is refused with
   the seam-ID source hint; no card is rendered.
5. A seam with no witness-producing finding still renders a card with an
   `unavailable` instruction, no next action, and the three witness-derived
   detail families recording the exact unavailable reason.
6. A seam nothing can name a portable workspace identity for fails closed
   with an error that names the packet route; no identity is invented.

## Test Mapping

- `crates/ripr/src/app/repair_card_handoff.rs` unit tests cover the pure
  assembly over producer-owned facts: the nine-family projection with the
  packet route, the closed-gate omission of the next action, the default
  budget bounds, the owner-discriminated gap match, the packet-queue
  omission failing closed on the canonical packet family, and the
  foreign-head attempt projecting `stale`.
- `crates/ripr/src/cli/agent.rs` unit tests cover option parsing and the
  updated unknown-subcommand listing.
- `crates/ripr/tests/cli_smoke.rs` `agent_card_hands_off_one_seam_as_the_default_repair_card`
  covers the JSON and human journeys plus the cold-agent refusal.
- `crates/ripr/tests/cli_help_hierarchy.rs` pins the new `help --all` row
  with its `[advanced]` marker; `command_metadata` agreement tests pin the
  catalog row, metadata row, and help-body registration.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/app/repair_card_handoff.rs` | producer fact gathering (witness check pass, git head, workspace identity, latest attempt, packet render, inspection command), pure `assemble_repair_card`, detail-source population, tests |
| `crates/ripr/src/cli/commands/agent_card.rs` | entry resolution, policy omission check, `--json` and compact human output |
| `crates/ripr/src/cli/agent.rs` | `AgentCardOptions`, `AgentCommand::Card`/`CardHelp`, parsing, unknown-subcommand listing |
| `crates/ripr/src/cli/commands/agent.rs` + `agent_dispatch.rs` | dispatch and help routing |
| `crates/ripr/src/cli/help/agent.rs` + `help.rs` | `AGENT_CARD_HELP`, `AGENT_HELP` listing, help-text registration, printer |
| `crates/ripr/src/cli/command_catalog.rs` + `command_metadata.rs` | `cmd:agent.card` catalog and metadata rows |
| `crates/ripr/src/agent/artifact.rs`, `output/agent_seam_packets.rs`, `agent/command_specs.rs`, `app.rs`, `repair_card_budget.rs` | staged `dead_code` expectation removal and first-live-caller visibility |

## Metrics

- `repair_card_schema_version`
- `repair_card_budget_version`
- `repair_card_violations`
