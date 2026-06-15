# RIPR-SPEC-0088: TypeScript repair-packet surface projection (0085 §PR8)

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None yet

Linked ADRs:

- ADR 0008 — Rust-native `oxc` parser adoption for the TypeScript adapter

Linked plan:

- `docs/IMPLEMENTATION_CAMPAIGNS.md` — TypeScript evidence-adapter wave

Linked specs:

- RIPR-SPEC-0085 — TypeScript Evidence Adapter Contract. **This spec is PR 8
  of the 0085 campaign.** It depends on RIPR-SPEC-0087 (§PR7), which flips
  `repair_packet_ready: true` by validating a `GapRecord` projected from a
  TypeScript finding. PR 7 computes the `GapRecord` and discards it after the
  boolean decision. PR 8 retains and emits that packet into the operator
  surfaces.

Linked PRs:

- #1150 — PR 7: TypeScript preview→actionable repair-packet (RIPR-SPEC-0087)

Support-tier impact:

- **No tier change.** TypeScript stays `preview_advisory_only`. Projecting the
  full work-packet does not promote the language. The packet carries
  `authority_boundary: "preview_advisory_only"` throughout. TypeScript's support
  tier remains governed by [Support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`, the spec index
  (`docs/specs/README.md`), and `.ripr/traceability.toml`.
- No new crates, binaries, or dependencies. No public-API symbol additions.
- This is an **additive, documented behavior change**: the human and JSON
  outputs grow a new `typescript_repair_packet` sub-section when
  `repair_packet_ready: true`; the LSP surfaces grow verify/receipt copy
  actions for actionable TypeScript findings. No existing field is removed or
  renamed. `schema_version` does **not** bump (additive JSON field in an
  existing preview surface; not a schema contract change).

---

## Problem

RIPR-SPEC-0087 §PR7 flips `repair_packet_ready: true` for a complete-contract
TypeScript finding but immediately discards the `GapRecord` it computed to
decide the flip. The full work-packet (verify_command, receipt_command,
allowed_edit_surface, must_not_change, canonical_gap_id, target test) is
therefore **never visible** to the operator in any surface. The repair-packet
field reads `true` but the work the operator needs to act on it is absent.

PR 8 closes this gap by projecting the `GapRecord` — already built by
`typescript_gap_record_for` — through the **same shared renderer** the Rust
packets use, emitting it into:

- the human-readable output (the `explain` command field-note and the
  `check` human output's TypeScript preview card section);
- the JSON output (`typescript_repair_packet` alongside
  `typescript_preview_card`);
- the LSP hover (a "Repair packet" section when `repair_packet_ready: true`);
- the LSP code-action surface ("Copy TypeScript repair packet" command).

For a **blocked** (non-actionable) finding the human output names the
limitation and why the packet is not emitted — no partial packet, no implied
readiness.

## Non-Goals

- No new language promotion. Preview stays `preview_advisory_only`.
- No mutation execution, no runtime confirmation.
- No new public symbols beyond those needed for the new `typescript_repair_packet`
  JSON field.
- No new validator or completeness check. The only authority is
  `validate_agent_gap_record_packet` (unchanged from §PR7).
- No change to `repair_packet_ready` semantics (§PR7 already set them).

---

## Behavior

### Actionable-case consistency (the `Preview actionability` and card blocks)

For an **actionable** finding (`repair_packet_ready: true`) the existing
`Preview actionability` block and the TypeScript preview card must NOT emit the
incomplete-packet messaging that was written for blocked findings, because it
directly contradicts the complete packet. Specifically, when actionable:

- the `why not actionable:` line is relabeled `why actionable:` and reads as a
  confirmation naming the resolved contract fields (package root, runner, owner,
  oracle target, verify, receipt, edit cage);
- the `repair route:` line is relabeled `repair action:` and names the actual
  repair (the suggested assertion shape / missing discriminator), not the
  blocked-case "project ... only after verify, receipt, evidence refs, and edit
  boundaries are available" text;
- the `evidence needed:` line is omitted entirely (nothing is needed);
- the `Next step` line drops the "; no actionable repair packet is emitted until
  ... available" tail and confirms the packet is complete and delegatable.

The blocked cases are unchanged: they keep `why not actionable`, `repair route`,
and `evidence needed`. The JSON `preview_actionability` object keeps its stable
keys (`why_not_actionable`, `repair_route`, `evidence_needed_to_promote`) for
schema compatibility, but their content reads as actionable when flipped:
`repair_route` is the repair action, `evidence_needed_to_promote` is the empty
string, and `why_not_actionable` is the actionable confirmation.

### Human output (explain + check human)

For an **actionable** TypeScript finding (`repair_packet_ready: true`):

The human output's TypeScript preview card section gains a
`Repair packet (advisory)` subsection:

```
TypeScript repair packet (advisory)
  canonical gap: gap:typescript:<family>:<fp8>
  source: <owner> at <file>:<line>
  related test: <test_file>::<test_name>
  oracle: <observed_expr> → expected <expected_value>
  verify: <verify_command>
  receipt: <receipt_command>
  edit surface: <test_file>
  must not change:
    - <boundary_item_1>
    - ...
  why actionable: complete-contract TypeScript finding — package root, runner,
    owner, oracle all resolved; no blocking limitation
  authority: preview_advisory_only
```

For a **blocked** finding (`repair_packet_ready: false`):

```
TypeScript repair packet (advisory)
  status: not actionable
  limitation: <why_not_actionable text>
  next capability needed: <evidence_needed_to_promote>
```

### JSON output

A new `typescript_repair_packet` key is added to the finding JSON when
`repair_packet_ready: true`:

```json
"typescript_repair_packet": {
  "schema_version": "0.3",
  "source": "typescript_preview_projection",
  "gap_id": "...",
  "canonical_gap_id": "gap:typescript:<family>:<fp8>",
  "language": "typescript",
  "language_status": "preview",
  "authority_boundary": "preview_advisory_only",
  "verify_command": "<verify_command>",
  "receipt_command": "<receipt_command>",
  "allowed_edit_surface": ["<test_file>"],
  "forbidden_files": ["<production_file>"],
  "must_not_change": ["...", "Do not treat preview-language evidence as gate authority."],
  "assertion_shape": "expect(<observed>).toBe(<expected>)",
  "repair_kind": "AddBoundaryAssertion",
  "target_test": "<test_file>::<test_name>",
  "missing_discriminator": "<discriminator>",
  "repair_route": { ... }
}
```

This field is **absent** when `repair_packet_ready: false` (invariant: only
complete packets are surfaced; no partial/implied packet).

### LSP hover

When `preview_actionability.repair_packet_ready == true`, the hover adds a
`## Repair packet` section:

```markdown
## Repair packet (TypeScript preview, advisory)
Verify: `<verify_command>`
Receipt: `<receipt_command>`
Edit surface: `<test_file>`
Must not change: <n items>
Canonical gap: `<canonical_gap_id>`
Authority: preview_advisory_only
```

### LSP code action

A new code action `"Copy TypeScript repair packet"` is added when the
diagnostic's `data.preview_actionability.repair_packet_ready == true`. It
copies the same verify/receipt/edit-surface payload to the clipboard (via the
existing `COPY_CONTEXT_COMMAND` mechanism).

### Route-quality inputs

The `canonical_gap_id` from the projected `GapRecord` is included in the JSON
output so it is available to route-quality analysis without requiring a
separate projection step. This is additive — the field already exists in
`typescript_repair_packet` above.

---

## Required Evidence

- `fixtures/ts_repair_packet_complete` golden (check.json + human.txt) containing the
  full `typescript_repair_packet` JSON field and the human field-note section with
  verify, receipt, edit surface, must_not_change, canonical_gap_id, and authority.
- At least one non-actionable golden (human.txt) with the named limitation section
  (`status: not actionable`, `limitation:`, `next capability needed:`).
- LSP hover integration evidence: `## Repair packet (TypeScript preview, advisory)`
  section visible when `repair_packet_ready: true`.
- LSP code-action evidence: "Copy TypeScript repair packet (advisory)" action present
  when `repair_packet_ready: true`.
- All four surfaces built from the same `GapRecord` returned by
  `typescript_gap_record_for` — no parallel projection.

## Reuse invariants

- The `GapRecord` is built once by `typescript_gap_record_for` (already in
  `output/typescript_packet_projection.rs`). It is NOT rebuilt or re-projected
  in a separate function for each surface.
- The human renderer calls shared `agent_seam_packets` helper functions
  (`allowed_edit_surface_for_gap_route`, `gap_record_packet_do_not_do`,
  `forbidden_files_for_gap_record`) — no forks.
- The JSON field `typescript_repair_packet` is built from the same `GapRecord`,
  not a parallel TypeScript-only structure.
- The LSP hover/action surfaces read from `preview_actionability` (already in
  diagnostic data) and from the projected `GapRecord` — no new LSP protocol
  changes.

---

## Fixtures

Reuse existing fixtures from §PR7:

- `fixtures/ts_repair_packet_complete` — adds `typescript_repair_packet` JSON
  field and the human-output `Repair packet (advisory)` section. Golden changes
  are **additive** (new section in human.txt, new JSON field in check.json);
  bless with reason `"RIPR-SPEC-0088 §PR8: actionable TS packet now surfaced"`.
- `fixtures/ts_static_limit`, `fixtures/ts_no_verify_command` (and other
  non-actionable fixtures) — human output adds
  `TypeScript repair packet (advisory)\n  status: not actionable\n...` for the
  blocked case. Golden changes bless with
  `"RIPR-SPEC-0088 §PR8: named limitation surfaced for blocked TS packet"`.

---

## Acceptance Examples

### Actionable TypeScript finding — human output

```text
input:  fixtures/ts_repair_packet_complete (repair_packet_ready: true)
output (human):
  TypeScript repair packet (advisory)
    canonical gap: gap:typescript:predicate:xxxxxxxx
    source: applyDiscount at src/pricing.ts:42
    related test: src/pricing.test.ts::applies discount at threshold
    oracle: expect(result).toBeGreaterThan(50)
    verify: npx jest src/pricing.test.ts --testNamePattern "applies discount"
    receipt: ripr outcome ... target/ripr/receipts/gap-typescript-predicate-xxxxxxxx.json
    edit surface: src/pricing.test.ts
    must not change:
      - src/pricing.ts (changed production file — test-only edits required)
    why actionable: complete-contract TypeScript finding — package root, runner,
      owner, oracle all resolved; no blocking limitation
    authority: preview_advisory_only
```

### Actionable TypeScript finding — JSON output

```json
{
  "typescript_repair_packet": {
    "schema_version": "0.3",
    "source": "typescript_preview_projection",
    "canonical_gap_id": "gap:typescript:predicate:xxxxxxxx",
    "language": "typescript",
    "language_status": "preview",
    "authority_boundary": "preview_advisory_only",
    "verify_command": "npx jest src/pricing.test.ts ...",
    "receipt_command": "ripr outcome ... target/ripr/receipts/...",
    "allowed_edit_surface": ["src/pricing.test.ts"],
    "forbidden_files": ["src/pricing.ts"],
    "must_not_change": ["src/pricing.ts (changed production file — test-only edits required)", "Do not treat preview-language evidence as gate authority."]
  }
}
```

### Blocked TypeScript finding — human output

```text
input:  fixtures/ts_static_limit (repair_packet_ready: false)
output (human):
  TypeScript repair packet (advisory)
    status: not actionable
    limitation: dynamic_dispatch — static limit
    next capability needed: no further capability information
```

### Blocked TypeScript finding — JSON output

```text
typescript_repair_packet field absent (invariant: no partial packet surfaced)
```

---

## Test Mapping

```text
crates/ripr/src/output/human/sections.rs::tests::
  ts_actionable_packet_renders_field_note_section
  ts_blocked_packet_renders_named_limitation
crates/ripr/src/output/json/report.rs::tests::
  ts_actionable_packet_emits_typescript_repair_packet_json_field
  ts_blocked_packet_omits_typescript_repair_packet_json_field
crates/ripr/src/lsp/hover.rs::tests::
  ts_actionable_packet_hover_includes_repair_packet_section
crates/ripr/src/lsp/actions.rs::tests::
  ts_actionable_packet_action_includes_copy_repair_packet
fixtures/ts_repair_packet_complete   — complete contract (additive golden)
fixtures/ts_static_limit             — named limitation (additive golden)
fixtures/ts_no_verify_command        — named limitation (additive golden)
```

## Implementation Mapping

```text
MOD  crates/ripr/src/output/human/sections.rs
     (push_typescript_preview_card: add repair-packet field-note subsection
      for actionable; named-limitation subsection for blocked)

MOD  crates/ripr/src/output/json/report.rs
     (finding_json_with_config_and_counts: add typescript_repair_packet field
      when repair_packet_ready: true)

MOD  crates/ripr/src/lsp/hover.rs
     (push_preview_actionability: add ## Repair packet section when actionable)

MOD  crates/ripr/src/lsp/actions.rs
     (push_gap_actions / preview_actionability path: add Copy TypeScript repair
      packet action when repair_packet_ready: true in diagnostic data)
```

## Metrics

- `typescript_repair_packet_projection_surfaces_count` — number of surfaces
  that project the actionable GapRecord. Currently 4: human field-note, JSON
  `typescript_repair_packet` field, LSP hover section, LSP copy code action.
- Binary golden contract: `fixtures/ts_repair_packet_complete` check.json contains
  `typescript_repair_packet` field; all non-actionable fixture human.txt files
  contain the named limitation section.
- Authority boundary unchanged: all four surfaces carry
  `authority_boundary: preview_advisory_only`; TypeScript remains `preview`.
