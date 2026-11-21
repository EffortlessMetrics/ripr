# ADR 0017: Editor Gap Queue Is Read-Only

Status: accepted

Date: 2026-05-19

## Context

Lane 3 has closed the saved-workspace editor cockpit, preview-language routing,
editor gap cockpit, first-run repair loop, first-pr bridge, and adoption
assurance. Lane 1 now emits actionable gap queue artifacts that contain typed
repair information for the current workspace.

Users and coding agents need the editor to surface that current repair queue,
but the editor must not become a second analyzer, ranker, report producer,
policy engine, source editor, test generator, provider surface, mutation
runner, or gate.

## Decision

The editor may project actionable gap queue state from existing artifacts.

The editor may:

- validate `actionable-gaps` artifacts for schema, workspace root, freshness,
  identity, language, paths, commands, receipt state, first-pr state, and
  packet consistency;
- show bounded queue state in `ripr: Show Status`;
- copy a current repair packet when typed fields validate;
- copy a read-only repo gap map;
- show refresh, setup, and regeneration guidance;
- explain preview boundaries, static limits, report-only rows, and no-action
  states.

The editor must not:

- create, mutate, repair, or regenerate the `actionable-gaps` artifacts it
  consumes;
- create analyzer facts, evidence records, gap records, repair routes, receipt
  movement, first-pr packets, PR evidence, generated CI summaries, or PR
  comments;
- rank gaps independently of the upstream artifact;
- decide policy, gates, badges, waivers, baselines, suppressions, or default
  blocking;
- infer action semantics from Markdown prose;
- edit source or apply patches;
- generate tests;
- call providers or models;
- run mutation testing;
- claim runtime adequacy, mutation proof, gate success, policy eligibility, or
  merge readiness;
- add CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays in this campaign.

If schema, root, freshness, identity, language, path, command, receipt,
first-pr, or actionable-packet state cannot be validated, the editor fails
closed. It may explain the state and offer refresh, setup diagnosis, or
regeneration guidance, but it suppresses stronger repair actions and proof
claims.

## Consequences

Positive:

- Users get a local repair queue without learning the report graph first.
- Coding agents receive a bounded packet with one gap, one repair route, one
  verification command, receipt guidance, and stop conditions.
- Lane 3 stays aligned with upstream typed artifacts and avoids parallel
  schema or ranking truth.
- Unsafe artifact states become explainable no-action states instead of risky
  repair prompts.

Negative:

- The editor cannot fill missing queue fields by reading Markdown.
- Users must regenerate upstream artifacts outside the editor when the packet
  is missing, stale, wrong-root, malformed, or unsupported.
- Queue projection waits on upstream schema fields before actions can appear.

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| Let Lane 3 compute the queue from diagnostics and gap ledgers. | That creates parallel analyzer/ranking behavior and weakens the source-of-truth split. |
| Let Markdown packet text drive actions. | Prose is for humans; typed fields are the only safe action contract. |
| Show all report entries as repair actions. | Report-only and static-limit-only entries can orient users, but they must not interrupt without a repair route and verify command. |
| Add editor dashboard UI. | The current bottleneck is bounded work selection, not a new dashboard surface. |
| Allow automatic repairs or generated tests. | Lane 3 is an instrument panel and packet surface, not an agent or code generator. |

## Related Specs and Plans

- [RIPR-PROP-0013: Editor Actionable Gap Queue](../proposals/RIPR-PROP-0013-editor-actionable-gap-queue.md)
- [RIPR-SPEC-0055: Editor Actionable Gap Queue](../specs/RIPR-SPEC-0055-editor-actionable-gap-queue.md)
- [RIPR-SPEC-0054: Editor Adoption Assurance](../specs/RIPR-SPEC-0054-editor-adoption-assurance.md)
- [RIPR-SPEC-0052: Editor First-PR Packet Projection](../specs/RIPR-SPEC-0052-editor-first-pr-packet-projection.md)
- [RIPR-SPEC-0047: Editor Gap Projection](../specs/RIPR-SPEC-0047-editor-gap-projection.md)
- [Lane 3 editor/LSP tracker](../lanes/LANE_3_EDITOR_LSP.md)
- [Editor Actionable Gap Queue implementation plan](../../plans/editor-actionable-gap-queue/implementation-plan.md)
