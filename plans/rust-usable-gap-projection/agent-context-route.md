# Rust Gap Projection Agent Route

Status: complete route seed

This route is the bounded work packet future agents should read before touching
Rust gap projection, gap-ledger consumers, repair-card projection, badge/gate
targets, LSP gap diagnostics, or agent packet repair flows.

The repo does not yet have `.ripr/context/*.toml` routing manifests or
`cargo xtask context packet`. Until that tooling lands, this file is the
human-readable route seed for the same data.

## Route ID

`rust-usable-gap-projection`

## Triggers

- `GapRecord`
- `gap decision ledger`
- `review-comments --gap-ledger`
- `gate evaluate --gap-ledger`
- `zero status --gap-ledger`
- `agent packet --gap-ledger`
- `MissingOutputContract`
- `AddOutputGolden`
- repair card
- repo badge gap target
- LSP gap diagnostic

## Read First

1. [RIPR-PROP-0006](../../docs/proposals/RIPR-PROP-0006-rust-usable-gap-projection.md)
2. [RIPR-SPEC-0045](../../docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
3. [RIPR-SPEC-0046](../../docs/specs/RIPR-SPEC-0046-gap-decision-ledger.md)
4. [RIPR-SPEC-0047](../../docs/specs/RIPR-SPEC-0047-editor-gap-projection.md)
5. [Output schema](../../docs/OUTPUT_SCHEMA.md)
6. [First successful PR workflow](../../docs/FIRST_PR_WORKFLOW.md)
7. [Support tiers](../../docs/status/SUPPORT_TIERS.md)
8. [Traceability](../../.ripr/traceability.toml)
9. [Closeout](../../docs/handoffs/2026-05-15-rust-usable-gap-projection-closeout.md)

## Primary Surfaces

| Surface | Paths |
| --- | --- |
| Gap ledger model and renderer | `crates/ripr/src/output/gap_decision_ledger.rs` |
| CLI command parsing | `crates/ripr/src/cli/commands.rs`, `crates/ripr/src/cli/help.rs` |
| Review repair cards | `crates/ripr/src/output/review_comments.rs` |
| First action and PR ledger | `crates/ripr/src/output/first_useful_action.rs`, `crates/ripr/src/output/pr_evidence_ledger.rs` |
| Badges and RIPR Zero | `crates/ripr/src/output/badge.rs`, `crates/ripr/src/output/ripr_zero_status.rs` |
| Gates | `crates/ripr/src/output/gate.rs` |
| Agent packets | `crates/ripr/src/output/agent_seam_packets.rs`, `crates/ripr/src/cli/agent.rs` |
| LSP projection | `crates/ripr/src/lsp/diagnostics.rs`, `crates/ripr/src/lsp/hover.rs`, `crates/ripr/src/lsp/actions.rs` |
| Fixture contract | `fixtures/gap-decision-ledger/` |
| Output contract | `docs/OUTPUT_SCHEMA.md` |
| Capability and support status | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `docs/status/SUPPORT_TIERS.md` |

## Work Packet Shape

Objective:
Make one projection surface consume explicit gap records without inventing
actionability from raw findings.

Current state:
Gap ledger records are the shared decision object. Existing consumers should
read `projection_eligibility`, `repair_route`, `anchor`,
`verification_commands`, `policy_state`, `language`, `language_status`, and
`authority_boundary`.

Target state:
The changed surface either projects one repairable Rust gap with a bounded
repair route or reports why the record is summary-only, report-only, waived,
suppressed, baseline, preview, missing, or not policy targeted.

Required production delta:
Touch only the selected consumer or ledger derivation path.

Required evidence delta:
Add or update the focused unit test, fixture/golden, output schema text,
traceability entry, capability entry, and docs that correspond to the changed
surface.

## Validation Commands

Choose the narrow commands that match the changed surface, then run
`check-pr` before opening a PR.

```bash
rtk cargo test -p ripr gap_decision_ledger --lib
rtk cargo test -p ripr reports_gap_ledger --lib
rtk cargo test -p ripr review_comments --lib
rtk cargo test -p ripr gate --lib
rtk cargo test -p ripr lsp --lib
rtk cargo xtask check-output-contracts
rtk cargo xtask check-traceability
rtk cargo xtask check-capabilities
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

## Stop Conditions

Stop and write a blocked report if the work would:

- make raw `ExposureClass`, raw finding count, or generic confidence text drive
  a PR comment, badge, gate, LSP diagnostic, or agent packet;
- parse prose to decide PR-comment or editor actions;
- make `static_unknown` interrupting without a configured repair route;
- make preview-language evidence count toward RIPR Zero, public badges, or
  default gates;
- change default CI blocking or branch protection;
- run mutation testing, run external providers, generate tests, or edit source;
- remove the advisory/static evidence boundary from user-facing output.

## PR Summary Seed

```text
Production delta:
<selected gap-ledger consumer or derivation path>

Evidence delta:
<tests / fixture / output schema / traceability / capability docs>

Acceptance:
The surface projects only explicit GapRecord eligibility and keeps authority
with policy/gate artifacts.

Non-goals:
No analyzer truth changes, no default blocking, no preview promotion, no source
edits, no generated tests, no provider calls, no mutation execution.

Validation:
<commands run>
```

