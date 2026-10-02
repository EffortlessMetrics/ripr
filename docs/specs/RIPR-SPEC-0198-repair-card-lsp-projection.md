# RIPR-SPEC-0198: RepairCard projection through standard LSP

Status: proposed

Owner: product / LSP

Created: 2026-10-02

Linked issues:

- #4668 (this slice: standard LSP half; the MCP half is structurally deferred
  on the open #1898/#3089/#3090 authorities, see Non-Goals)
- #3166 (parent: repair packet card profile)
- #4663 (RepairCardV1; landed as PR #4979, RIPR-SPEC-0192)
- #4666 (bounded detail references; landed as PR #4980, RIPR-SPEC-0193)
- #4667 (CLI default handoff; landed as PR #4982, RIPR-SPEC-0194)
- #4669 (budget ratification; landed as PR #4983, RIPR-SPEC-0196)
- #1603 / #1895 (standard LSP projection authorities this slice consumes)
- #1898 / #3089 / #3090 (MCP evidence/repair authorities that own the deferred
  MCP half)

Support-tier impact:

- None. Both projected surfaces are advisory, read-only presentations of the
  existing card authorities; they accept no edit or execution authority and
  write nothing.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The projection reads
  the completed analysis snapshot, resolves the repository head with the same
  `git rev-parse HEAD` the CLI producer runs, and renders into existing LSP
  responses and the clipboard-copy command only.

## Problem

RIPR-SPEC-0194 made the compact `RepairCardV1` the default CLI agent handoff,
but the editor journey still ended at the witness diagnostic and the copy
packet/brief/command actions: an editor user who wanted the one bounded work
object had to leave the editor and run `ripr agent card`. #4668 requires the
same card state and canonical identity to be reachable through standard LSP
presentation, with transport-specific budgets, typed degradation, and no
adapter-local reconstruction — and without reviving a custom public `riprAgent`
LSP protocol.

## Behavior

`crates/ripr/src/lsp/repair_card.rs` projects one `RepairCardV1` per
classified seam through two standard surfaces; both are framing-only
projections over the shared application authorities:

- `seam_repair_card(entry, snapshot)` assembles the card through the exact
  same `app::repair_card_handoff::assemble_repair_card` pure projection the
  CLI consumes, with every fact bound from authorities the completed LSP
  snapshot already holds instead of re-running the check pipeline: the
  witness and finding ID through the shared owner-discriminated
  `witness_from_findings` binding (a gap id shared with a sibling owner never
  credits this seam), the live repository head through the same
  `git rev-parse HEAD` the CLI producer runs (the card's semantic identity
  binds the head, so a snapshot-cached spelling would remint the digest), the
  portable workspace identity through the same
  `workspace_identity_for` derivation, the most recent repair-attempt
  manifest through the same `latest_attempt_for_seam` inventory, the rendered
  canonical packet behind `PacketCommandContext::Standalone`, and the typed
  packet inspection command. When any producer fact cannot be bound the
  projection returns `None`: both surfaces fail closed to omission rather
  than shipping a weakened or partially invented card.
- The seam code actions gain one entry, "Agent handoff: copy repair card"
  (action identity `copy_repair_card`, kind `quickfix.ripr.inspect`), riding
  the already-advertised `ripr.copyContext` client command so no new client
  capability is required. Its target carries `label: "repair_card"`, the
  seam identity, and the complete wire card in `packet` — the versioned
  `repair_card.v1` document under the ratified default budget
  (RIPR-SPEC-0196), stable detail references included. The VS Code
  `copyContext` handler copies the card directly from the target without an
  LSP round trip, exactly like the existing direct packet labels.
- The seam hover gains one bounded section, `## Repair card`: the canonical
  card identity, the typed instruction state (its wire spelling, never
  re-interpreted), the next-action display or an explicit none-reason
  (`no producer-owned route` for an unavailable instruction, `route gate
  closed` for a gated one — the state itself is never weakened), and per-state
  detail-family availability counts (`current · stale · unavailable · other
  of N families`). The complete card with its stable detail references stays
  behind the copy action; hover never re-derives or enhances a field.

Degradation and suppression stay governed by the existing transport
authorities: the action and the hover section appear only where the seam
diagnostic itself is current against the published snapshot (a stale seam
diagnostic suppresses the whole seam action surface today and suppresses the
card with it); a client without the copy-command advertisement keeps the
legacy fail-closed omission through the unchanged client-command policy; and
capability or budget may omit detail on the wire card but cannot strengthen
readiness, actionability, or currentness — the card's own route gate
(`repair_card_route_exposable`) still decides whether a next action rides,
and the builder independently rejects one the gate would not expose. The
wire card keeps `next_action: null` with an unavailable instruction exactly
like the CLI surface.

`witness_from_findings`, `workspace_identity_for`, and `latest_attempt_for_seam`
are raised from private to `pub(crate)` in `app/repair_card_handoff.rs` with
the CLI gather path refactored onto `witness_from_findings`, so exactly one
derivation of each fact exists and the LSP is a consumer, not a second
producer.

## Required Evidence

- `cargo test -p ripr --lib lsp::repair_card` covers the bounded hover
  section: identity/state/next-action/detail-count projection, the
  unavailable-instruction none-reason, the closed-gate none-reason that keeps
  the state honest, and the wire spelling of the typed states.
- `cargo test -p ripr --lib lsp::tests` covers the action: fail-closed
  omission outside a git workspace, the assembled wire card inside a real
  repository (schema version, seam subject, live-head binding matching
  `git rev-parse HEAD`, nine detail references, unavailable instruction with
  `next_action: null`, and the ratified item/byte bounds), the
  witness-bound card (finding identity, fix-site instruction, `ripr explain`
  detail route, and the shared next-action gate), stale-diagnostic
  suppression, and the bounded hover section in both repository and
  non-repository workspaces.
- `npm --prefix editors/vscode test` covers the `repair_card` direct-copy
  label: the card copies from the target without an LSP fallback and names
  the repair card in the confirmation.

## Known Limitations / Residuals

- **Live-head binding window.** The card's semantic identity binds the head
  resolved at request time, but the completed snapshot does not retain the
  analyzed HEAD as producer-owned provenance (its input identity records the
  resolved *base*, not the analyzed head). Between a HEAD move and the
  completed refresh (`.git/HEAD` is already a watched diagnostics input,
  #4896) the card can therefore bind new-head identity to snapshot evidence.
  Closing this exactly needs analyzed-head provenance plumbed through the
  refresh pipeline — a new snapshot field and a suppression comparison —
  recorded here as residual for that authority change, not papered over by a
  renderer-side guess.
- **Witness scope: editor vs CLI input sets.** Both surfaces bind the witness
  through the same owner-discriminated `witness_from_findings` matcher, but
  the input sets differ by design: the LSP snapshot's findings come from the
  worktree-aware saved-workspace check, while the CLI card gathers its
  witness from the committed-diff check. A worktree-only finding that names
  the seam can therefore produce an editor witness the CLI card would not.
  The matcher is one authority; aligning the input sets (or recording the
  scope on the wire card) is residual work owned by the witness producer.
- **Request-path assembly cost.** Card assembly resolves the repository head
  and the attempt inventory synchronously. The code-action path already
  clones the snapshot `Arc` and releases the state lock before building
  actions; the hover path now does the same (the lock is not held across the
  Git/attempt I/O). Moving assembly onto a blocking task or caching it keyed
  by snapshot + head + attempt state remains available headroom, not a
  correctness gap.

## Non-Goals

- No MCP card projection in this slice. ADR 0022 bounds the MCP adapter to
  the static startup workspace status (`ripr_workspace_status` /
  `ripr://workspace/status`), and the MCP gap/detail/prepare-repair
  authorities (#1898/#3089/#3090) that would own analysis-producing MCP
  tools are still open with no landed `mcp::gaps`/`mcp::repair` module;
  building an analysis-running MCP tool here would create a parallel
  authority instead of a projection. The MCP half of #4668 lands with those
  authorities, at which point the deferred acceptance predicates there are
  re-evaluated.
- No new custom public `riprAgent` LSP request, and no change to the
  experimental sidecar protocol: the journey completes on standard
  diagnostics, hover, and code actions.
- No new client command or capability advertisement: the card rides the
  existing `ripr.copyContext` command and `quickfix.ripr.inspect` kind.
- No diagnostics-payload change in this slice: the bounded diagnostic/hover
  route for a minimal client stays the existing witness/fix-instruction
  presentation; the card is one action (and one hover section) away.
- No deletion or weakening of any existing packet/brief/command action.
- No edit, execution authority, or support-tier promotion.

## Acceptance Examples

1. On a current seam diagnostic in a repository, the quick-fix list contains
   "Agent handoff: copy repair card"; running it copies one `repair_card.v1`
   document naming the seam, the live `git rev-parse HEAD`, nine typed detail
   references, and the next action only when the shared route gate is open.
2. The same seam's hover shows `## Repair card` with the canonical card
   identity, the instruction state, an explicit next-action line or
   none-reason, and per-state detail availability counts.
3. Outside a repository (or whenever any producer fact cannot be bound) both
   surfaces are absent; no weakened card is ever presented.
4. A stale seam diagnostic offers neither surface, consistent with the
   existing seam-action suppression.
5. The wire card never exceeds the ratified default item/byte bounds and
   never carries a next action the route gate would refuse.

## Test Mapping

- `crates/ripr/src/lsp/repair_card.rs` unit tests cover the hover-section
  rendering contract over an assembled card fixture.
- `crates/ripr/src/lsp/tests.rs`
  `seam_code_actions_include_the_assembled_repair_card_in_a_git_workspace`,
  `seam_repair_card_binds_a_finding_witness_in_a_git_workspace`,
  `repair_card_action_fails_closed_outside_a_git_workspace`,
  `repair_card_action_suppressed_for_stale_seam_diagnostic`,
  `seam_hover_projects_bounded_repair_card_section_in_a_git_workspace`, and
  `seam_hover_omits_repair_card_section_outside_a_git_workspace` cover the
  projection, its fail-closed degradation, its stale suppression, and the
  witness-bound card with the shared next-action gate.
- `editors/vscode/test/suite/extension.test.ts`
  `copyContext copies repair cards without LSP fallback for active workspace
  file` covers the direct-copy label.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/lsp/repair_card.rs` | snapshot-authority card assembly (fail-closed), bounded hover section, tests |
| `crates/ripr/src/lsp/actions.rs` | `copy_repair_card` action construction over the advertised `ripr.copyContext` command |
| `crates/ripr/src/lsp/hover.rs` | seam-hover `## Repair card` section wiring |
| `crates/ripr/src/app/repair_card_handoff.rs` | `witness_from_findings` extraction (CLI path refactored onto it), `workspace_identity_for` / `latest_attempt_for_seam` crate visibility |
| `editors/vscode/src/client.ts` | `repair_card` direct-copy label in `copyContext` |

## Metrics

- `repair_card_schema_version`
- `repair_card_budget_version`
