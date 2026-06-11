# RIPR-SPEC-0069: LSP / Agent Feedback Use Case

Status: proposed

Owner: product / swarm

Created: 2026-06-06

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- plans/use-case-specs/implementation-plan.md (planned)

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec writes the user-facing contract for the existing
  `ripr lsp --stdio` surface and its agent-facing packets. It
  promotes no language, surface, or evidence class to a stronger
  support tier. Preview evidence shown through the LSP stays
  advisory under the canonical boundary in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Nothing new beyond the spec itself: no new crate, binary,
  dependency, parser, runtime executor, LSP server, artifact type,
  or workflow.

## Problem

The LSP sidecar already exists as mechanism: `serve_stdio` in
`crates/ripr/src/lsp.rs`, workspace diagnostics
(`lsp/diagnostics.rs`, `workspace_diagnostic_batches`), code actions
(`lsp/actions.rs`), gap-artifact validation (`lsp/gap_artifacts.rs`),
hover (`lsp/hover.rs`), and snapshot state (`lsp/state.rs`). What is
missing is the product contract that says what this surface is for
and what it must refuse to offer.

The framing decision this spec records: the LSP surface is an
**agent cockpit first, human editor decoration second**. Squiggles
and hovers are projections; the product is the bounded, copyable
packet an agent can act on safely.

The user question this surface owns:

```text
What is the first safe bounded action
an agent should take in this workspace?
```

Without a written contract, the LSP can drift into decoration-only
output (pretty diagnostics with no bounded next step) or, worse,
into offering repair actions whose edit boundaries were never
established.

## Behavior

### Surface

`ripr lsp --stdio` serves a saved-workspace model over
`tower-lsp-server`. Diagnostics come in three closed kinds:

- finding-based diagnostics (always on);
- seam grip-class diagnostics (configurable via
  `enable_seam_diagnostics`, default on
  (`DEFAULT_LSP_SEAM_DIAGNOSTICS = true` in
  `crates/ripr/src/config.rs`); opt out via the
  `seamDiagnostics: false` initialization option or repo config; a
  seam-walk failure downgrades to "no seam diagnostics this
  refresh", never a hard failure);
- `GapRecord` projections from validated gap artifacts.

The exposed command vocabulary is closed (`crates/ripr/src/lsp.rs`):

- `ripr.copyContext`
- `ripr.copyAgentPacketCommand`
- `ripr.copyAgentBriefCommand`
- `ripr.copyAfterSnapshotCommand`
- `ripr.copyAgentVerifyCommand`
- `ripr.copyAgentReceiptCommand`
- `ripr.copySuggestedAssertion`
- `ripr.copyTargetedTestBrief`
- `ripr.collectContext`
- `ripr.collectEvidenceContext`
- `ripr.openRelatedTest`
- `ripr.refresh`

No command outside this vocabulary may ship without amending this
spec. Every command is read-only or copy-to-clipboard; none edits
source.

First-useful-action integration is projection-only:
`target/ripr/reports/first-useful-action.json` is consumed
read-only. The LSP server validates the artifact
(`lsp/gap_artifacts.rs`) and projects it into hover and
diagnostics; the status-bar rendering is the VS Code extension
client's surface (`editors/vscode/src/client.ts`), not the LSP's.
Neither generates a new report nor performs source edits.

```text
The user should be able to answer:
- What is the first safe bounded action here?
   -> one repair packet with edit boundaries, or one named
      limitation when no repair is safe.
- Is this analysis current?
   -> runtime status; stale snapshots say so before any
      repair work is assigned.
- What may I touch, and what must I not change?
   -> allowed_edit_surface and must_not_change on every
      actionable packet.
- How do I verify and receipt the attempt?
   -> copyable verify command and receipt command.
```

### What the LSP must expose

For the agent-cockpit contract, the surface must expose:

- the first useful repair packet (when one is safely derivable);
- the top named limitation when no repair is safe;
- runtime status (fresh versus stale snapshot; stale status routes
  to `ripr.refresh` before repair work is assigned);
- `allowed_edit_surface` on every actionable packet;
- `must_not_change` on every actionable packet;
- a verify command;
- a receipt command;
- a copyable packet (`ripr.copyAgentPacketCommand` and
  `ripr.copyAgentBriefCommand`) so the agent leaves the editor with
  the full bounded brief, not a paraphrase.

"Repair packet" here is the canonical RIPR-SPEC-0061 contract, not
a separate LSP shape. A complete packet carries the full
RIPR-SPEC-0061 field list — `packet_id`, `canonical_gap_id`,
`repair_kind`, `target_test_shape`, `related_test_or_observer`,
`verify_command`, `receipt_command`, `confidence`,
`must_not_change[]`, `allowed_edit_surface[]`, and structured
`raw_evidence_refs[]`. The bullets above name the fields this
surface enforces on every offer; the packet contract itself is
owned by RIPR-SPEC-0061 and is not restated or narrowed here.

`lsp/gap_artifacts.rs` already validates that an actionable packet
must carry `allowed_edit_surface` and `must_not_change`; this spec
makes that the product rule for every action the LSP offers, not
just artifact ingestion.

### Fail closed

- No complete packet -> show the named limitation, not a repair
  action. Missing fields are listed by name (for example:
  `missing_actionability_fields: verify_command, receipt_command,
  must_not_change`).
- Preview evidence (TypeScript/Bun, Perl, cross-language) ->
  advisory only; no agent repair packet, no edit surface, no
  receipt synthesis.
- Missing edit surface -> no action. A packet without
  `allowed_edit_surface` and `must_not_change` is context, never an
  instruction.
- Stale or absent snapshot -> refresh-only guidance; a diagnostic
  without a current snapshot offers `ripr.refresh`, nothing else.
- Invalid or unvalidated gap artifact -> rejected with the named
  validation failure; never projected as an actionable diagnostic.

### Required and forbidden wording

Required wording examples:

- "First safe action: add a boundary assertion near
  `tests/pricing.rs::discount_above_threshold`. Allowed edit
  surface: `tests/pricing.rs`. Must not change:
  `src/pricing.rs`. Verify: `cargo test -p ripr ...`."
- "No safe bounded action: cross-language test target unresolved.
  Route: analysis/cross-language-target-resolution."
- "Editor status is stale; refresh analysis before assigning repair
  work."

Forbidden wording examples:

- "Apply this fix" with no edit boundary attached.
- "This change is fully tested" or any runtime-adequacy claim from
  static evidence.
- Presenting a preview-language hover as a repair instruction.

### Non-claims

The LSP surface does not claim analyzer authority of its own: every
diagnostic, hover, action, and packet is a projection of canonical
actionability (RIPR-SPEC-0061) plus runtime completeness. It does
not re-derive state from raw findings, and an empty diagnostic set
is a scope statement, never an all-clear.

## Non-Goals

- No autonomous edits. The LSP never modifies source; every command
  is read-only or copy-only.
- No generated test patches. Suggested assertion shapes are
  guidance text, not applied edits.
- No provider integration: no model calls from the LSP, and no
  packet field that requires one.
- No second analyzer: the LSP projects existing reports and
  snapshots; it adds no new analysis truth.
- No new report generation from the first-useful-action
  integration; it remains a read-only projection.
- No change to the existing default-on seam diagnostics posture in
  this lane; `enable_seam_diagnostics` stays default on with
  opt-out, owned by its existing config contract.

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- Existing LSP tests (`crates/ripr/src/lsp/tests.rs`,
  `lsp/gap_artifacts.rs` validation tests) mapped to the packet
  contract as implementation slices land.
- Fixture-backed examples for: a complete actionable packet, a
  named-limitation state, a stale-snapshot refresh-only state, and
  a preview-evidence advisory state.

Fail-closed verifier reject list — the surface must refuse to render
these states as an actionable offer:

- a repair action without both `allowed_edit_surface` and
  `must_not_change`;
- an actionable packet missing a verify command or receipt command
  (must surface as `missing_actionability_fields: ...`);
- a repair action derived from preview evidence
  (`language_status = "preview"` or
  `authority_boundary = "preview_advisory_only"`);
- a repair action offered against a stale or absent snapshot
  (refresh-only guidance is the only allowed offer);
- a gap artifact that fails validation projected as a diagnostic;
- a first-useful-action status item synthesized without the
  underlying `target/ripr/reports/first-useful-action.json`;
- an empty diagnostic set presented as "workspace clean" instead of
  a scope statement;
- a command outside the closed command vocabulary.

## Acceptance Examples

- An agent connects to `ripr lsp --stdio`, queries diagnostics, and
  invokes `ripr.copyAgentPacketCommand` on the top diagnostic. The
  copied packet names the gap, the allowed edit surface, the
  must-not-change set, the verify command, and the receipt command.
  The agent edits only within the surface, runs verify, then
  receipts.
- The same workspace with an unresolved cross-language target
  yields a hover that names the limitation and its route; no copy
  command produces a repair instruction for it.
- A diagnostic raised before the snapshot was refreshed offers only
  `ripr.refresh`; after refresh the full action set returns.
- A human in VS Code sees the first-useful-action title in the
  status bar (rendered by the extension client, sourced read-only
  from `target/ripr/reports/first-useful-action.json`); opening it
  routes to the same bounded packet the agent would copy.
- An actionable-gaps artifact with an empty `allowed_edit_surface`
  is rejected at validation with the named error
  ("actionable packet must carry allowed_edit_surface") and never
  becomes a diagnostic.

## Test Mapping

- None yet. This spec is docs-only; traceability entries are added
  when the implementation slices land tests against the packet
  contract, the closed command vocabulary, and the reject list
  above.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0069-lsp-agent-feedback-use-case.md — this
  document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "LSP agent packet" slice: make every offered action carry the
  full bounded packet (edit surface, must-not-change, verify,
  receipt) or degrade to a named limitation, and add the
  reject-list checks to LSP tests.
- Existing mechanism: `crates/ripr/src/lsp.rs` (`serve_stdio`,
  command vocabulary), `crates/ripr/src/lsp/diagnostics.rs`,
  `crates/ripr/src/lsp/actions.rs`,
  `crates/ripr/src/lsp/gap_artifacts.rs`,
  `crates/ripr/src/lsp/hover.rs`, `crates/ripr/src/lsp/state.rs`.

## Metrics

- Packet completeness rate: share of offered actions carrying edit
  surface, must-not-change, verify, and receipt (target: 100% by
  construction once the reject list is enforced).
- Limitation honesty: share of no-action states that name a
  limitation and route rather than rendering nothing.
- Stale-snapshot safety: zero repair actions offered against stale
  snapshots in tests.
- Agent outcome quality (with RIPR-SPEC-0073, a sibling proposed
  spec in this use-case stack): receipt closure or improvement rate
  for packets copied from the LSP surface.
- Promotion rule: move this spec to `accepted` when the closed
  command vocabulary, the packet-completeness rule, and the
  reject-list checks are enforced by LSP tests, and the linked plan
  slice is complete.

## Failure Modes

- Decoration drift: diagnostics ship without bounded packets — the
  packet-completeness metric and reject list make this a named
  defect, not a style choice.
- Boundary erosion: an action offers an edit without
  `allowed_edit_surface` / `must_not_change` — validation in
  `gap_artifacts.rs` plus the reject list fail closed.
- Preview leakage: preview evidence reaches an agent as a repair
  instruction — non-claim fields and the advisory-only rule keep it
  context.
- Stale authority: an agent acts on an outdated snapshot — runtime
  status plus refresh-only guidance route the agent to
  `ripr.refresh` first.
- Vocabulary creep: new commands appear without a spec change — the
  closed command list in this spec is the review checkpoint.
