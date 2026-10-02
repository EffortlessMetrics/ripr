# ADR 0023: Consumer evidence state is distinct from analysis outcome

- Status: Accepted
- Date: 2026-09-26
- Related: #1146, #2955, #2956, #2957, #2958

## Context

`AnalysisOutcome` describes what the producer could analyze and why it stopped.
The canonical evidence record decides what a consumer can present about a gap.
Its five `gap_state` values had been inline strings at the decision point.
Combining those values with producer limitations would make a review-card state
appear to describe an analysis stage. Several other fields named `status` and
`run_status` also describe different scopes and cannot safely be renamed by
text substitution.

## Decision

`domain::EvidenceState` owns the five consumer gap states: `actionable`,
`already_observed`, `internal_only`, `static_limitation`, and `unknown`.
`output::evidence_record::evidence_state_for` makes the decision once from
classified seam and actionability facts. Its `gap_state_for` adapter retains
the established strings for existing JSON, LSP, review, and packet consumers.
Only `Actionable` grants the consumer an actionable state; `Unknown` does not
mean a clean result. Producer `AnalysisOutcome`, language-specific limitations,
receipt lifecycle, and transport currentness keep their own authorities.

## Migration and consequences

This first slice does not rename a public field or change its version. Follow-up
consumer migrations replace string comparisons with the typed state at their
own semantic boundaries. A later public `gap_state` to `evidence_state` rename
requires a versioned schema migration, updated readers/writers and fixtures,
and an explicit compatibility decision; merely emitting both fields would
create two competing authorities. Other `status`/`run_status` fields require
separate per-surface mapping and must not be silently recast as gap states.

The enum's serialization preserves the five current wire values and rejects
an unrecognized value. Future states require an explicit typed addition and
consumer policy, rather than a new unreviewed string literal.
