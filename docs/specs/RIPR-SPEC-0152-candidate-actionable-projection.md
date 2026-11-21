# RIPR-SPEC-0152: Candidate-actionable projection authority

Status: proposed

Issue: #3281 (parent #3212; builds on RIPR-SPEC-0151)

## Problem

Every actionability-bearing surface — CLI counts, JSON, SARIF, gap ledger,
gate decisions, baselines, RIPR Zero, badges, review guidance, repair
cards, editor diagnostics, agent packets — derived its own notion of which
findings are current repair obligations. After #3280 typed source
currentness, a `base_deleted` finding could still be counted as a blocking
gap by one surface while guidance named nothing actionable: exactly the
#3212 incident shape.

## Behavior

`Finding::is_candidate_actionable` is the single eligibility authority: a
finding may drive current candidate-side obligations only when its
producer resolved `source_currentness == candidate_current`. `base_deleted`
and `moved_or_renamed` are base-side evidence; `unresolved_subject` is not
proven current. Classification, severity, and repair readiness never
upgrade a non-current finding.

Preview producers complete their resolution in this slice: TypeScript,
JavaScript, and Python probes are seeded from head-side added lines and
resolve `candidate_current` through the delta rule; the Bun cross-language
bridge tags changed Rust head-side lines the same way. Perl's fact-packet
path carries no diff evidence and stays the explicit `unresolved_subject`;
its projections were already advisory-only on every authority flag.

Routed surfaces:

- Badge exposure-gap candidates and unknowns count candidate-actionable
  findings only; the analyzed denominator keeps every finding.
- Finding-alignment items form only from candidate-actionable findings;
  the summary denominator keeps every finding.
- Gap records projected from check-output findings exclude proven
  base-side evidence (`base_deleted`, `moved_or_renamed`); legacy payloads
  without the field and the explicit unknown keep forming bounded,
  inspectable records. Gap records carry `source_currentness`, and the
  authority projections (`agent_packet`, `pr_comment`, `gate_candidate`,
  `ripr_zero_count`, `ripr_plus_count`, `lsp_diagnostic`) require
  `candidate_current` — markdown advisory visibility does not. Alignment
  items earn `candidate_current` only when every raw finding they grouped
  is candidate-current in the same payload; repo-exposure seams are
  candidate-current by construction (current-tree analysis).
- The human "Start here" triage names a current obligation only; the
  hidden-count denominator keeps every non-suppressed finding, and the
  `human-full` surface lists base-side findings with an explicit revision
  label (the bounded default view omits lower-priority findings exactly as
  before).
- The LSP actionable profile shows candidate-actionable findings only;
  the full profile keeps base-side evidence visible as history.
- SARIF results and GitHub annotations — current CI diagnostics — carry
  candidate-actionable findings only; the check JSON and human output
  remain the visibility surfaces for base-side evidence.
- PR evidence severe-gap counts and targeted-mutation candidates recompute
  from candidate-actionable findings when the findings array is present,
  falling back to the classification summary only when it is absent.
- The context packet carries `source_currentness` so agent consumers can
  refuse non-current edit targets.

Review comments, gate decisions, baselines, RIPR Zero, RIPR+, agent
packets, and repair cards inherit the authority transitively through
`projection_eligible` and the packet validator — no consumer may read the
eligibility map directly.

## Required Evidence

- A recurrence test: a `base_deleted` weakly_exposed finding yields zero
  badge gaps, zero alignment items, zero gap records, no actionable LSP
  diagnostic, no SARIF result, no GitHub annotation, and no PR severe gap,
  while the same finding shape marked `candidate_current` retains them.
- Removing the predicate from one representative projection (badge) makes
  the recurrence test fail.
- Existing candidate-current goldens remain stable except the preview
  `source_currentness` value completing to `candidate_current` and the
  human revision label on base-side findings.
- The evidence-promotion corpus pins that deleted/unresolved records
  never generate repair/gate/agent eligibility.

## Required guards

- No renderer may re-derive actionability from class or severity instead
  of the shared predicate.
- Base-side evidence stays visible on surfaces that explicitly support
  historical context (check JSON, human output with the revision label,
  LSP full profile), labelled non-actionable.
- Denominators never lose findings: badge `analyzed_findings`,
  alignment total, and classification summary keep counting every finding.
- Unknown is not actionable: findings without the currentness field (old
  artifacts, hand-built payloads) produce no obligations.

## Acceptance Examples

- Accept: one `base_deleted` weakly_exposed finding plus one
  `candidate_current` weakly_exposed finding in one diff — badge shows one
  gap, alignment one item, guidance names the current finding, the
  base-side finding renders labelled.
- Accept: Perl advisory findings remain visible and advisory with no
  eligibility change.
- Reject: a `moved_or_renamed` or `unresolved_subject` finding becoming a
  gap record, diagnostic, annotation, or repair target.

## Test Mapping

`crates/ripr/src/domain/probe.rs` pins the predicate and the delta rule;
`crates/ripr/src/output/badge/tests.rs` the recurrence test; projection
tests in `output/gap_decision_ledger.rs`, `output/first_pr.rs`,
`output/json/finding_alignment.rs`, `lsp/diagnostics.rs`, `output/sarif.rs`,
`output/github.rs`, and `app/pr_evidence.rs` pin each routed surface with
`candidate_current` fixtures; re-blessed goldens carry the completed
preview dispositions.

## Non-Goals

- No change to classification, stage evidence, confidence, or finding
  identity.
- No re-coordination of recorded locations (deferred with #3280).
- No rename-map retention; no Perl producer resolution; no schema-version
  bump (additive fields only).
- No consumer-side suppression-policy change.

## Implementation Mapping

- `crates/ripr/src/domain/probe.rs` — `is_candidate_actionable`,
  `permits_candidate_action`, `from_probe_delta`.
- Preview producers: `analysis/language/python.rs`,
  `analysis/language/typescript/{classifier,parse,bun_bridge}.rs`.
- Routed surfaces: `output/badge/summaries.rs`,
  `output/json/finding_alignment.rs`, `output/gap_decision_ledger.rs`,
  `output/human/{triage,sections}.rs`, `lsp/diagnostics.rs`,
  `output/sarif.rs`, `output/github.rs`, `app/pr_evidence.rs`,
  `domain/context_packet.rs` + `output/json/context_packet.rs`.

## Metrics

No new metric; existing actionable counts now share one denominator of
meaning. Promotion and corpus gates (`check-evidence-promotion-honesty`)
pin the non-promotion invariants.
