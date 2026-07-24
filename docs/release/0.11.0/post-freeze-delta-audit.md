# RIPR 0.11.0 post-freeze delta audit

Status: **142/142 coverage complete; category review pending — not merge-ready**

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

## Coverage and provisional result

The machine-readable denominator is
[`post-freeze-delta-ledger.json`](post-freeze-delta-ledger.json).

```text
recorded     142
expected     142
remaining      0
coverage complete  yes
category review    pending
candidate decision pending
```

Every record names an exact merged `ripr-swarm/main` SHA, one primary category,
a provisional candidate disposition, the release claim it protects, an exact
non-claim, secondary impacts, and source-conflict risk.

The denominator was reconciled against the GitHub compare count and exact
post-freeze commit-date windows. Automated duplicate/content-commitment
validation remains required before this draft may become merge-ready.

| Category | Owning issue | Records |
| --- | --- | ---: |
| Trust | `ripr-swarm#2350` | 19 |
| Editor | `ripr-swarm#2351` | 36 |
| Analyzer | `ripr-swarm#2352` | 31 |
| Operations | `ripr-swarm#2353` | 56 |
| **Total** | | **142** |

Current **provisional** dispositions:

```text
must_include_0_11             88
needs_operator_decision        6
release_infrastructure_only   13
safe_defer_post_0_11          15
source_only_followup           1
structural_no_semantic_delta  19
                              ---
total                         142
```

Coverage completion is not disposition acceptance. The four category owners
must review the fragments and the six operator-decision groups must be settled.

## Current recommendation: refreeze

The old candidate explicitly deferred partial-scope behavior and predates a
large portion of the current trust floor:

- complete configured gate routes with producer-owned seam, gap, and command
  identity plus command-level blocking proof;
- provenance-bound analysis artifacts, assurance-axis separation, canonical
  verify-input validation, actual repository-HEAD receipt binding, and portable
  receipt paths;
- partial-diff and repository-scope limits that remain visible and policy
  ineligible rather than becoming false completeness;
- saved-workspace currentness, dirty-buffer quarantine, bounded delivery,
  push/pull diagnostic and action parity, progress, deadlines, transport bounds,
  and one negotiated client-feature authority;
- analyzer correctness and honesty fixes across owner, route, assertion,
  cross-language, scope, Git-input, and failure-disclosure paths;
- security dependency updates and generated-CI behavior required by the selected
  extension and configured gate narrative.

Keeping the July 21 tree would require a broad rollback of the planned
`0.11.0` claims. The working recommendation is therefore **refreeze**, subject
to category acceptance and the six explicit decisions below.

## Operator decisions still required

1. **Executed-verification contract:** ship the landed
   `VerificationExecutionResultV1` contract while deferring
   `ripr-swarm#2332`–`#2334`, or complete the execution/`RepairReceiptV2`
   train before refreeze. A deferral requires explicit release copy saying
   RIPR does not execute verification commands or issue `RepairReceiptV2`.
2. **Check-artifact reuse group:** include or defer together the accepted design,
   implementation, wire/currentness fixes, domain-boundary correction, and
   worktree extension. Do not ship a half-contract.
3. **Rayon parsing:** include when required for the documented cold/editor
   latency boundary; otherwise defer with the dependency and performance claim.
4. **Mixed audit-head cleanup:** retain the receipt-help correction required by
   the selected trust path while treating its static-language maintenance
   portion as non-semantic.

The check-artifact item accounts for multiple provisional rows but is one
coherent product decision.

## Category review law

Every commit has one primary category owner. Secondary impacts are recorded in
the row instead of duplicating the SHA across fragments.

### Trust — `ripr-swarm#2350`

Review gate eligibility, canonical gap/seam/command identity, artifact
provenance, static-versus-executed assurance, receipt paths/currentness, and
legacy-baseline compatibility.

### Editor — `ripr-swarm#2351`

Review diagnostic budget and action parity, saved-workspace currentness,
lifecycle/deadlines, transport and capability bounds, command/action authority,
Workspace Trust, multi-root, server resolution, and packaged-extension proof.

### Analyzer — `ripr-swarm#2352`

Review diff and repository scope, parser/owner/oracle/route correctness,
cross-language honesty, CLI/config/output contracts, check-artifact and cache
identity, public API, performance, and package-required dependencies.

### Operations — `ripr-swarm#2353`

Review CI reliability, generated workflows, dependencies, retained authority
and release copy, package evidence, and behavior-preserving decomposition.
Pure moves remain structural only where parity and old-path reachability are
explicit.

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

A disposition is not merely a label. Each accepted row must retain the affected
claim, consumer, package/schema/workflow impact, source-conflict risk, reason,
and exact non-claim.

## Completion and release sequence

```text
142/142 exact SHA denominator                         complete
→ automated uniqueness and content-commitment check  pending
→ four category reviews and disposition acceptance   pending
→ operator decisions and source-only survivor audit  pending
→ ripr#1509 records keep/refreeze decision            pending
→ ripr-swarm#2354 creates immutable replacement freeze, if selected
→ refresh ripr#1478 preflight
→ prove source editor integration in ripr#1507
→ build the final direct two-parent join in ripr#1508
→ merge source promotion under ripr#1465
→ continue metadata/readiness/qualification under ripr#1463
```

## Merge gate for this audit PR

This PR must remain draft until:

- fragment counts and unique SHAs mechanically reconcile to 142;
- every provisional row is accepted or amended by its category owner;
- every `must_include_0_11` row names the release claim it protects;
- every deferral names a follow-up issue and exact release non-claim;
- superseding and reverted rows are linked where applicable;
- source-only `0.10.1` survivors and source workflow/settings authority are
  represented separately;
- the final canonical ledger commitment is recorded;
- `ripr#1509` records the reviewed candidate decision.

## Authority boundary

This audit does not create a freeze ref, merge source history, alter versions or
changelog metadata, authorize tags or publication, use release secrets, sign
artifacts, publish marketplace packages, or promote support tiers.
