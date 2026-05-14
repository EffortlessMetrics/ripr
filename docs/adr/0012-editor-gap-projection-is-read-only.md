# ADR 0012: Editor Gap Projection Is Read-Only

Status: proposed

Date: 2026-05-14

## Context

Lane 3 owns RIPR's editor and LSP projection surfaces. Campaign 27 closed the
language adapter preview routing slice, so the editor can now project stable
Rust evidence and opt-in preview TypeScript, JavaScript, and Python evidence
while preserving Rust defaults.

RIPR also has richer gap-oriented artifacts: evidence records, gap records,
gap decision ledgers, first-useful-action reports, repair cards, verify
commands, receipts, and preview static-limit metadata. Those artifacts can make
the editor a better local repair cockpit, but they also create a boundary risk.

If the editor starts creating gap records, deciding policy, composing PR
comments, generating tests, editing source, calling providers, running
mutation, or rerunning hidden analysis, it stops being a trustworthy projection
surface over saved-workspace evidence.

## Decision

Editor gap projection is read-only and saved-workspace first.

The editor consumes existing RIPR artifacts and projects them as diagnostics,
hover, status, code actions, related-test opening, repair packets, verify
commands, receipt commands, and refresh guidance. It must validate root,
freshness, schema, identity, language enablement, path safety, and command
payloads before projecting stronger gap actions.

The editor must not become:

- an analyzer;
- a policy engine;
- a gate authority;
- a PR comment publisher;
- a generated CI summary composer;
- a test generator;
- a source editor;
- a provider or model client;
- a runtime mutation runner;
- an unsaved-buffer overlay engine;
- a CodeLens, inlay, or semantic-token feature surface for this campaign.

Other lanes own analyzer truth, evidence schema, policy, gates, PR/CI reports,
receipts, release, and security. Lane 3 consumes those artifacts only after
they exist and fail closed when they are stale, malformed, unsupported,
wrong-root, disabled, unavailable, or unsafe to reference from the workspace.

## Consequences

Positive:

- Rust editor behavior remains stable while gap projection becomes richer.
- Preview evidence stays visibly advisory and static-limit bounded.
- Editor actionability comes from typed gap and repair-route artifacts, not
  prose parsing.
- Agents receive narrower packets with one gap, one repair route, explicit
  limits, verify command, receipt command, and a stop condition.
- Wrong-root, stale, malformed, missing, unsupported, and disabled states keep
  one fail-closed model.
- Future editor features such as CodeLens, inlays, semantic tokens, inline
  patches, and unsaved-buffer overlays require explicit campaigns.

Negative:

- The editor cannot project a repair route until upstream artifacts provide
  validated identity and command payloads.
- Some useful ideas, such as automatic test generation or provider-backed
  suggestions, remain intentionally blocked.
- More validation code is required before showing actions, so early PRs may
  look like guardrail work rather than visible UI.

## Alternatives Considered

- **Let VS Code rerun analysis for gap actions.** Rejected because it creates a
  hidden analyzer path and weakens saved-workspace reproducibility.
- **Infer repair routes from diagnostic prose.** Rejected because actionability
  should come from typed artifacts, not text parsing.
- **Generate or apply tests from repair packets.** Rejected because RIPR
  provides bounded test intent and verification commands, not generated source
  edits.
- **Publish PR comments from editor actions.** Rejected because PR/CI surfaces
  own publishing and review composition.
- **Expose preview evidence as the same confidence as Rust.** Rejected because
  preview evidence is syntax-first, opt-in, advisory, and static-limit bounded.
- **Add richer editor furniture in the same campaign.** Rejected because
  CodeLens, inlays, semantic tokens, inline patches, and unsaved overlays need
  separate contracts and fixtures.

## Related Specs and Plans

- [RIPR-PROP-0007: Editor Gap Cockpit](../proposals/RIPR-PROP-0007-editor-gap-cockpit.md)
- [RIPR-SPEC-0047: Editor Gap Projection](../specs/RIPR-SPEC-0047-editor-gap-projection.md)
- [RIPR-SPEC-0046: Gap Decision Ledger](../specs/RIPR-SPEC-0046-gap-decision-ledger.md)
- [RIPR-SPEC-0020: First Useful Action Report](../specs/RIPR-SPEC-0020-first-useful-action-report.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- [Editor gap cockpit implementation plan](../../plans/editor-gap-cockpit/implementation-plan.md)
