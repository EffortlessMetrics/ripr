# ADR 0016: Editor Adoption Assurance Remains Read-Only

Status: proposed

Date: 2026-05-18

## Context

Lane 3 has closed the editor cockpit, preview routing, gap cockpit, first-run
diagnosis, first-repair packet, receipt projection, and first-pr bridge. The
editor can guide a saved-workspace user through one focused repair loop and
point to the first-pr packet that carries evidence forward.

The next adoption risk is setup and root ambiguity. Users and coding agents
need the editor to explain whether the active extension, server, workspace
root, artifacts, receipt, and first-pr packet are compatible and safe.

That does not make the editor an installer, analyzer, report producer, policy
surface, source editor, generator, provider client, mutation runner, or gate.

## Decision

Editor adoption assurance remains read-only and projection-only.

The editor may validate and display compatibility, root, artifact, receipt,
and first-pr packet state. It may show setup diagnosis, Show Status lines,
hover text, bounded code actions, refresh/setup/regeneration guidance, and
copyable commands when typed fields validate.

The editor must not:

- install, download, replace, or repair binaries;
- mutate repository config or user settings;
- rerun hidden analysis to make artifacts current;
- create first-pr packets, PR evidence, generated CI summaries, or PR comments;
- decide policy, gate, badge, waiver, baseline, or suppression state;
- infer action semantics from Markdown prose;
- invent analyzer facts, gap records, repair routes, or receipt movement;
- edit source or apply patches;
- generate tests;
- call providers or models;
- run mutation testing;
- claim runtime adequacy, mutation proof, gate success, policy eligibility, or
  merge readiness;
- add CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays in this campaign.

If compatibility, schema, root, freshness, identity, language, path, command,
receipt, or first-pr packet state cannot be validated, the editor fails
closed. It may explain the state and offer refresh, setup diagnosis, or
regeneration guidance, but it suppresses stronger repair actions and proof
claims.

## Consequences

Positive:

- First-time users get clearer setup and recovery state without hidden magic.
- Agents receive a narrower handoff: one root, one gap, one compatibility
  boundary, one verify command, one receipt command, and explicit stop
  conditions.
- Multi-root and wrong-root failures become safe no-action states instead of
  misleading repair work.
- Lane 3 stays aligned with upstream typed artifacts.

Negative:

- The editor cannot fix missing or incompatible binaries by itself.
- Users still need to run documented CLI or package-manager commands to
  refresh artifacts or repair setup.
- More validation state must be modeled before actions appear.

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| Auto-install or auto-update the server. | Installation and release have separate trust, network, and rollback requirements. |
| Pick the first workspace root in multi-root workspaces. | Wrong-root repair packets can be actively harmful; ambiguous roots must fail closed. |
| Let Markdown packets drive actions. | Human prose is not an action contract; typed JSON fields drive safety. |
| Add diagnostics for setup failures. | Setup state is not code evidence and should remain in status/diagnosis unless a typed artifact supports a diagnostic. |
| Add richer UI furniture now. | The adoption risk is compatibility and root clarity, not CodeLens, inlays, or inline patches. |

## Related Specs and Plans

- [RIPR-PROP-0012: Editor Adoption Assurance](../proposals/RIPR-PROP-0012-editor-adoption-assurance.md)
- [RIPR-SPEC-0054: Editor Adoption Assurance](../specs/RIPR-SPEC-0054-editor-adoption-assurance.md)
- [RIPR-SPEC-0052: Editor First-PR Packet Projection](../specs/RIPR-SPEC-0052-editor-first-pr-packet-projection.md)
- [RIPR-SPEC-0050: Editor First Repair Loop](../specs/RIPR-SPEC-0050-editor-first-repair-loop.md)
- [RIPR-SPEC-0049: Editor Setup Status](../specs/RIPR-SPEC-0049-editor-setup-status.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- [Editor Adoption Assurance implementation plan](../../plans/editor-adoption-assurance/implementation-plan.md)
