# ADR 0011: Editor Preview Routing Is Projection-Only

Status: proposed

Date: 2026-05-12

## Context

Campaign 27 extends RIPR from stable Rust evidence toward opt-in preview
TypeScript-family and Python evidence. Lane 3 owns the editor and LSP
projection surfaces: diagnostics, hover evidence, code actions, context
packets, status, related-test opening, command payloads, VS Code behavior, and
`lsp-cockpit-report`.

That editor cockpit is useful because it projects saved-workspace evidence
into bounded local action. It is not trusted because it is clever; it is trusted
because it is rooted, current, pasteable, fixture-pinned, and fail-closed on
wrong-root, stale, missing, malformed, or unsupported artifacts.

Preview language routing creates a tempting shortcut: start an analyzer from
the extension, inspect unsaved buffers, generate tests, add provider-backed
suggestions, or treat preview evidence as policy-ready because it is visible
in the editor. Those shortcuts would bypass the evidence spine and make preview
findings look more mature than they are.

This ADR records the architecture boundary before editor routing work starts.

## Decision

The editor remains a projection surface over existing RIPR artifacts and
saved-workspace analysis. Editor preview routing consumes language metadata,
preview status, static limits, related-test facts, command payloads, and
freshness/root state produced by the analyzer and output layers.

The editor must not become:

- a second analyzer;
- a policy engine;
- a test generator;
- a source editor;
- a runtime mutation runner;
- a provider or model integration point;
- an unsaved-buffer overlay engine;
- a separate preview-only command system.

Preview routing must fail closed when evidence is stale, malformed,
unsupported, missing, or for another workspace. Rust saved-workspace defaults
remain unchanged. Preview language routing is opt-in and advisory.

## Consequences

Positive:

- Rust editor behavior stays boringly stable while preview routing is added.
- Analyzer truth remains in Campaign 27 adapter work, not duplicated in LSP or
  VS Code code.
- Policy and gate semantics remain outside Lane 3.
- Static limits and preview labels can be projected consistently from the same
  artifacts used by CLI, reports, agents, and fixtures.
- Wrong-root, stale, missing, malformed, and unsupported report behavior keeps
  one fail-closed path.
- Future editor affordances need explicit editor campaigns instead of arriving
  as side effects of routing.

Negative:

- The editor cannot make preview evidence appear before the analyzer and output
  layers emit projectable artifacts.
- Some useful editor ideas, such as unsaved-buffer overlays or generated test
  drafts, remain intentionally blocked until a later campaign opens that scope.
- Static-limit presentation depends on upstream artifacts; when
  `static_limit_kind` is absent, the editor may display stable limit text but
  cannot safely branch behavior on parsed prose.

## Alternatives Considered

- **Run preview analysis from the extension.** Rejected because it creates a
  second analysis path, weakens saved-workspace reproducibility, and complicates
  wrong-root and stale-state handling.
- **Inspect unsaved buffers for preview diagnostics.** Rejected because the
  current cockpit is saved-workspace only; unsaved-buffer behavior needs its own
  contract, fixtures, and failure model.
- **Generate tests from preview findings.** Rejected because RIPR supplies test
  intent and bounded packets, not generated source edits.
- **Add provider-backed editor suggestions.** Rejected because provider calls
  are outside RIPR's local evidence loop and would blur the source of a finding.
- **Treat visible preview evidence as policy-ready.** Rejected because policy
  readiness is owned outside Lane 3; preview evidence remains visible advisory
  evidence until an explicit promotion policy says otherwise.
- **Create a separate preview editor UI.** Rejected because it would split the
  cockpit and duplicate command behavior instead of sharpening labels in the
  existing flow.

## Related Specs and Campaigns

- [RIPR-PROP-0003: Editor Preview Routing](../proposals/RIPR-PROP-0003-editor-preview-routing.md)
- [RIPR-SPEC-0036: Editor Preview Routing](../specs/RIPR-SPEC-0036-editor-preview-routing.md)
- [RIPR-SPEC-0037: Editor Preview Static-Limit Projection](../specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- Campaign 27: Language Adapter Preview in
  [Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md)
