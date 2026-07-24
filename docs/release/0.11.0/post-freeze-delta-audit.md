# RIPR 0.11.0 post-freeze delta audit

Status: **in progress — not merge-ready**

Authority: `EffortlessMetrics/ripr#1510`, under the release decision in
`EffortlessMetrics/ripr#1509`.

## Exact audit identity

```text
old swarm candidate  3a80c634f2157dee0e7f74fc72b7975f07699143
old freeze ref        freeze/source-sync-2026-07-21
audit head            c86807ecdbf359594ef88c0ff38b10b446139dca
compare status        142 ahead, 0 behind
source parent         ec7dc6beaec8ad2efca4abd502f9253cdb4f939c
```

The audit head is fixed. Later `ripr-swarm/main` commits are outside this
denominator unless a supplemental range is explicitly recorded and reviewed.

## Current coverage

The machine-readable denominator is
[`post-freeze-delta-ledger.json`](post-freeze-delta-ledger.json).

```text
recorded     41
expected    142
remaining   101
complete     no
```

All 41 current records use exact merged `ripr-swarm/main` commit identities.
Their dispositions are **provisional** until the owning category review is
accepted.

| Category | Fragment | Records | Provisional posture |
| --- | --- | ---: | --- |
| Trust | [`post-freeze-trust-fragment.json`](post-freeze-trust-fragment.json) | 12 | 11 must-include; 1 operator decision |
| Editor | [`post-freeze-editor-fragment.json`](post-freeze-editor-fragment.json) | 16 | 15 must-include; 1 safe defer |
| Analyzer | [`post-freeze-analyzer-fragment.json`](post-freeze-analyzer-fragment.json) | 5 | 2 must-include; 2 operator decisions; 1 safe defer |
| Operations | [`post-freeze-ops-fragment.json`](post-freeze-ops-fragment.json) | 8 | 2 must-include; 1 operator decision; 3 safe defer; 2 structural |

Current aggregate provisional dispositions:

```text
must_include_0_11             30
needs_operator_decision        4
safe_defer_post_0_11           5
structural_no_semantic_delta   2
```

The audit remains fail-closed: missing records are not treated as deferred,
structural, or irrelevant, and this draft must not merge at partial coverage.

## Why the July 21 candidate cannot proceed unchanged by default

The frozen tree predates coherent groups that now protect the planned release
narrative, including:

- complete configured gate routes with producer-owned seam and command identity;
- provenance-bound analysis artifacts, assurance-state separation, canonical
  verify-input validation, and actual repository-HEAD receipt binding;
- partial-diff selection with explicit limited scope and policy ineligibility;
- saved-workspace currentness, dirty-buffer quarantine, bounded diagnostic
  delivery, push/pull selection parity, progress, deadlines, and transport
  resource limits;
- analyzer correctness and honesty fixes, including scope, relation, oracle,
  coordinate, and fail-closed behavior;
- package, dependency, schema, fixture, generated-artifact, workflow, and
  source-integration changes needed by those semantics.

Keeping the old candidate would require a large rollback of the proposed
`0.11.0` claims. The working recommendation is therefore **refreeze**, but the
final decision remains owned by `ripr#1509` after the denominator and all four
category reviews are complete.

## Category reviews

| Category | Issue | Primary authority |
| --- | --- | --- |
| Trust | `ripr-swarm#2350` | Gate, artifacts, commands, verification, and receipts |
| Editor | `ripr-swarm#2351` | LSP, protocol, VS Code, lifecycle, currentness, and editor UX |
| Analyzer | `ripr-swarm#2352` | Analysis, CLI, config, package, and user-visible output |
| Operations | `ripr-swarm#2353` | CI, dependencies, control plane, docs, release copy, and structural changes |

Every commit receives exactly one primary category owner. Secondary impacts
remain explicit in the record instead of duplicating the commit across
fragments.

## Required dispositions

```text
must_include_0_11
safe_defer_post_0_11
source_only_followup
release_infrastructure_only
structural_no_semantic_delta
superseded_or_reverted
needs_operator_decision
```

A disposition is not merely a label. Each row must name the affected claim,
consumer, package/schema/workflow impact, source-conflict risk, reason, and
exact non-claim.

## Coherent groups that must not be split accidentally

### Trust

1. Gate eligibility contract, typed seam identity, command-boundary behavior,
   and true blocking/nonzero-exit proof.
2. Typed `CommandSpec` production and consumer projection.
3. Artifact schema/repository/revision/input/content commitments.
4. Static-movement versus executed-verification vocabulary.
5. Receipt HEAD binding, canonical verify-input validation, portable paths,
   and checking/currentness behavior.

The open execution and `RepairReceiptV2` train (`ripr-swarm#2332`–`#2334`)
must be either included before refreeze or explicitly deferred. A deferral
requires release copy stating that RIPR does not yet execute verification
commands or issue `RepairReceiptV2`.

### Editor

1. Diagnostic budget enforcement, omission disclosure, legitimate-zero
   semantics, and one stored push/pull selection authority.
2. Debounce, saved-content deduplication, dirty-buffer quarantine, and committed
   current snapshot publication.
3. Real-binary lifecycle, progress, deadlines/cancellation, and one terminal
   typed outcome.
4. Framing/payload/concurrency/slow-reader bounds plus typed client capability,
   configuration, and degradation authority.
5. Versioned code-action data, resolve-time revalidation, command authority,
   and real-wire compatibility journeys.
6. Workspace Trust, multi-root/start, server resolution/download, and packaged
   extension proof.

### Analyzer

1. Diff parsing, path confinement, coordinate safety, and source-scope
   disclosure.
2. Partial-scope selection, identity, typed status, policy ineligibility, and
   recovery guidance.
3. Analyzer evidence-honesty fixes with their adversarial fixtures and corpus
   gates.
4. CLI/config/output contracts required by gate, receipt, and editor consumers.
5. Cache/artifact/input-identity reuse and performance changes only where
   semantic parity is proved.
6. Cargo/npm/package/lockfile changes required by selected production code.

### Operations

1. Generated schemas, fixtures, traceability, and docs required by included
   semantic changes.
2. CI reliability and source-integration corrections required to prove the
   candidate.
3. Source/swarm authority migration and deletion of obsolete scheduler paths.
4. Dependency and workflow changes not owned directly by production semantics.
5. Behavior-preserving decomposition, classified as structural only after
   parity proof and old-authority reachability checks.

## Completion algorithm

```text
exact 142-commit enumeration
→ one primary category per commit
→ category semantic review and disposition
→ reconcile dependencies, fixtures, schemas, docs, and supersession
→ validate 142/142 coverage and no duplicates
→ ripr#1509 keep/refreeze decision
→ ripr-swarm#2354 immutable replacement freeze, if selected
→ refreshed ripr#1478 preflight
→ ripr#1507 source editor integration proof
→ ripr#1508 final direct two-parent join
```

## Merge gate for this audit PR

This PR must remain draft and unmergeable by policy until:

- the ledger records exactly 142 commits;
- category counts sum to 142 with no duplicate SHA;
- every row has a reviewed disposition;
- every `must_include_0_11` row names the release claim it protects;
- every deferral names a follow-up issue and release non-claim;
- reverts and superseding commits are linked;
- required source-only survivors remain separate;
- the final canonical ledger commitment is recorded;
- `ripr#1509` records the reviewed candidate decision.

## Authority boundary

This audit does not create a freeze ref, merge source history, alter versions or
changelog metadata, authorize tags or publication, use release secrets, sign
artifacts, publish marketplace packages, or promote support tiers.
