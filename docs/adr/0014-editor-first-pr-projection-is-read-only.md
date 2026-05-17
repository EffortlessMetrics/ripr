# ADR 0014: Editor First-PR Projection Is Read-Only

Status: proposed

Date: 2026-05-17

## Context

Lane 3 owns editor and LSP projection. The editor cockpit, gap cockpit, setup
diagnosis, receipt status, and first repair packet are now closed surfaces. The
first successful PR packet also exists as an xtask/report surface:

```text
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
```

The next Lane 3 problem is continuity. A user can complete a local repair loop
in VS Code, but the editor should also explain whether the first-pr packet that
carries that evidence forward exists, is fresh, belongs to the current
workspace, matches the current gap when applicable, and is safe to inspect.

That does not make the editor a PR/CI producer. The packet producer, PR/CI
rendering, policy authority, gates, and release/install rails remain owned by
their existing lanes.

## Decision

Editor first-pr projection is read-only.

The editor may validate and display existing first-pr packet artifacts. It may
show first-pr packet state in `ripr: Diagnose Setup`, `ripr: Show Status`,
hover, and bounded code actions. It may open a workspace-local Markdown packet
or copy typed packet text, verify commands, and receipt commands when JSON
fields validate.

The editor must not:

- create or regenerate first-pr packets;
- compose generated CI summaries;
- publish PR comments;
- decide policy or gate outcomes;
- claim PR merge readiness;
- infer action semantics from Markdown prose;
- invent analyzer facts, gap records, repair routes, or receipt movement;
- edit source or apply patches;
- generate tests;
- call providers or models;
- run mutation testing;
- install binaries or mutate config;
- add CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays in this campaign.

When packet state, schema, root, freshness, identity, paths, language, or
commands cannot be validated, the editor fails closed: it may explain the
state and offer refresh, setup diagnosis, or first-pr regeneration guidance,
but it suppresses open/copy repair actions and movement claims.

## Consequences

Positive:

- Users can move from editor repair to PR-facing evidence without learning the
  report graph.
- Coding agents receive a narrower handoff: one gap, one packet path, one
  verify command, one receipt command, explicit limits, and a stop condition.
- Lane 3 stays aligned with first successful PR artifacts without owning their
  production.
- Preview-language boundaries remain visible when first-pr packets carry
  preview evidence.

Negative:

- The editor cannot make missing or stale first-pr packets current by itself.
- Users still need to run the documented xtask or CLI command to create or
  refresh first-pr packets.
- More validation state is required before showing first-pr packet actions.

## Alternatives Considered

- **Generate first-pr packets from the editor.** Rejected because Lane 3 would
  become a report producer and duplicate xtask/Lane 4 ownership.
- **Publish PR comments from VS Code.** Rejected because PR publishing has
  separate review, cap, dedupe, and opt-in semantics.
- **Treat Markdown as the source of action truth.** Rejected because editor
  actions need typed fields, not prose parsing.
- **Treat a found first-pr packet as PR-ready.** Rejected because the packet is
  advisory unless an explicit gate-decision artifact provides authority.
- **Add richer UI furniture now.** Rejected because the bottleneck is
  validated continuity over existing artifacts, not CodeLens, inlays, semantic
  tokens, inline patches, or unsaved overlays.

## Related Specs and Plans

- [RIPR-PROP-0010: Editor First-PR Bridge](../proposals/RIPR-PROP-0010-editor-first-pr-bridge.md)
- [RIPR-SPEC-0052: Editor First-PR Packet Projection](../specs/RIPR-SPEC-0052-editor-first-pr-packet-projection.md)
- [RIPR-SPEC-0051: First Successful PR UX](../specs/RIPR-SPEC-0051-first-successful-pr-ux.md)
- [RIPR-SPEC-0050: Editor First Repair Loop](../specs/RIPR-SPEC-0050-editor-first-repair-loop.md)
- [RIPR-SPEC-0049: Editor Setup Status](../specs/RIPR-SPEC-0049-editor-setup-status.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- [Editor First-PR Bridge implementation plan](../../plans/editor-first-pr-bridge/implementation-plan.md)
