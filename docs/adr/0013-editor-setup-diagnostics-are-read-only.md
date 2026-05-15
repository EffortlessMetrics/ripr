# ADR 0013: Editor Setup Diagnostics Are Read-Only

Status: proposed

Date: 2026-05-15

## Context

Lane 3 owns RIPR's editor and LSP projection surfaces. The saved-workspace
editor cockpit, preview-language routing, and Editor Gap Cockpit are closed.
The editor can now project stable Rust evidence and opt-in preview evidence
from existing artifacts while preserving Rust defaults and preview boundaries.

The next usability problem is first-run uncertainty. Users need the editor to
explain whether the server started, which binary is active, which workspace is
being analyzed, which languages are enabled and available, why diagnostics are
absent, which artifacts are stale or missing, what action is safe, and whether
a receipt records movement for the current gap.

Those questions are setup and projection questions, not permission to turn the
editor into an analyzer, installer, config mutator, source editor, test
generator, provider client, mutation runner, policy engine, gate authority, or
PR publisher.

## Decision

Editor setup diagnostics and first-run repair guidance are read-only.

The editor may collect and display existing extension, server, workspace,
configuration, language, artifact, freshness, command, and receipt state. It
may show that state in `ripr: Show Status`, a setup diagnosis report, hover,
and bounded code actions. It may copy a first repair packet, verify command,
or receipt command when typed evidence supports those actions.

The editor must not:

- install or download a server binary as part of setup diagnosis;
- mutate repository configuration;
- rerun hidden analysis to make diagnostics appear;
- create analyzer facts, gap records, repair routes, or receipts;
- decide policy or gate outcomes;
- publish PR comments or generated CI summaries;
- edit source or apply patches;
- generate tests;
- call providers or models;
- run mutation testing;
- add CodeLens, inlay hints, semantic tokens, inline patches, or unsaved
  overlays in this campaign.

When setup state, artifacts, commands, receipts, paths, language availability,
or freshness cannot be validated, the editor fails closed: it may explain the
state and offer refresh/setup guidance, but it suppresses repair actions and
movement claims.

## Consequences

Positive:

- First-run users get useful setup and no-output explanations inside VS Code.
- The first Rust repair loop becomes easier to follow without changing
  analyzer, policy, PR/CI, release, or receipt ownership.
- Receipt status remains tied to existing artifacts and gap identity instead
  of editor-generated evidence.
- Preview-language boundaries remain explicit and static-limit bounded.
- Coding agents receive narrower packets with setup context, one gap, verify
  command, receipt command, and non-claims.

Negative:

- The editor cannot automatically fix missing binaries, missing config, or
  missing artifacts.
- Some users may still need to run CLI commands outside VS Code to create
  fresh evidence and receipts.
- More validation state is required before showing setup, receipt, or repair
  claims.

## Alternatives Considered

- **Auto-run analysis when diagnostics are absent.** Rejected because it
  creates a hidden analyzer path and weakens saved-workspace reproducibility.
- **Install or update `ripr` from Diagnose Setup.** Rejected because binary
  acquisition belongs to release/package flows; setup diagnosis should explain
  what is missing without mutating the machine.
- **Mutate config to enable languages from the editor.** Rejected because
  config changes are source changes and should remain explicit user actions.
- **Create receipts from the editor.** Rejected because receipts should come
  from existing receipt commands and artifacts; the editor may copy commands
  and show status.
- **Treat no diagnostics as clean.** Rejected because no-output can mean clean,
  stale, missing, disabled, unavailable, wrong-root, or server unavailable.
- **Add richer editor furniture in the same campaign.** Rejected because
  CodeLens, inlays, semantic tokens, inline patches, and unsaved overlays need
  separate contracts.

## Related Specs and Plans

- [RIPR-PROP-0008: Editor First-Run Usability](../proposals/RIPR-PROP-0008-editor-first-run-usability.md)
- [RIPR-SPEC-0048: Editor Setup Status](../specs/RIPR-SPEC-0048-editor-setup-status.md)
- [RIPR-SPEC-0049: Editor First Repair Loop](../specs/RIPR-SPEC-0049-editor-first-repair-loop.md)
- [RIPR-SPEC-0047: Editor Gap Projection](../specs/RIPR-SPEC-0047-editor-gap-projection.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- [Editor first-run usability implementation plan](../../plans/editor-first-run-usability/implementation-plan.md)
