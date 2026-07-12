# Rust Evidence-Bound Repair Trust and Adoption Plan

Status: active  
Owner: product-swarm  
Plan artifact: RIPR-PLAN-0062  
Linked goal: `.ripr/goals/active.toml`  
Linked issues: #1423, #1424, #1425, #1427, #1440, #1543, #1560
Starting PRs: #1489, #1487, #1483

## Outcome

Make RIPR the default evidence-to-repair protocol for Rust pull requests: one
changed behavior becomes one exact, safe, test-only repair and a current-state
before/after receipt, while unsupported cases remain explicit limitations rather
than plausible-looking recommendations. The accepted targeted-rerun contract is
now regression-protected infrastructure inside this trust and adoption goal,
not the long-range destination.

This supersedes RIPR-PLAN-0061 as the active execution sequence. Its accepted
contracts remain useful historical source, but its pending statuses and
maintainer-fixed order are not the current queue.

## Spec-system authority

Cargo-allow's opt-in `spec-system` profile is the authoritative structural
validator for governed artifact paths, kinds, lifecycle states, and graph links.
RIPR xtask remains the repo-facing proof executor and may invoke cargo-allow,
but must not independently reimplement the same graph rules. Every campaign
control-plane PR records cargo-allow doctor, audit, and worklist outputs.

The profile is advisory while its findings are made low-noise. The sole RIPR
execution manifest at `.ripr/goals/active.toml` is deliberately not enforced
until cargo-allow issue #2119 can validate its dialect without creating a second
active goal or discarding execution metadata. The installed cargo-allow 0.1.10
requires `--config .allow/profiles/spec-system.toml` for this owned profile;
cargo-allow issue #2117 tracks the owned-versus-legacy default-path friction.

## Reconciled sequence

| Order | Work item | Dependency | Evidence |
| ---: | --- | --- | --- |
| 0A | `control-plane/cargo-allow-spec-system-adoption` | — | cargo-allow doctor, audit, and worklist artifacts |
| 0B | `control-plane/rust-one-shot-goal` | 0A | goals/doc/plan checks and structural indexing |
| 0C | `control-plane/cargo-allow-active-goal-dialect` | 0A | blocked on cargo-allow #2119 or separately approved migration |
| 1A | `output/bounded-start-here` | 0 | human/human-full fixtures and output contracts |
| 1B | `docs/first-screen-agent-loop` | 1A | README/doc checks |
| 2A | `review/card-oracle-projection` | 0 | review-card schema and traceability checks |
| 2B | `review/canonical-working-set-id` | 2A | canonical identity output contracts |
| 3A | `gate/exact-repair-route` | 1A, 2B | failure-time packet fixture |
| 3B | `gate/concrete-targeted-mutation` | 3A | candidate/limitation matrix |
| 4A | `analysis/field-constant-observation` | 2B | positive and adversarial corpus |
| 4B | `analysis/constructor-field-observation` | 2B | same-crate ambiguity corpus |
| 5 | `perf/targeted-rerun` | 4A, 4B | complete infrastructure: SPEC-0123, parity, invalidation, graph provenance, and benchmark receipt |
| 6A | `analysis/call-presence-gate-producer` | 3B | authorized real/current-repository caller-to-effect-sink receipt or named limitation |
| 6B | `dogfood/rust-route-quality-corpus` | 3B, 5 | authorized three-repository corpus, six-attempt pilot, then 20 receipt-backed attempts |
| 6C | `dogfood/route-quality-closeout` | 6A, 6B | support review and campaign closeout after corpus completion and CallPresence disposition |

Work items 1A and 2A may use isolated worktrees. Items 4A and 4B may be
parallel only after their file/contract overlap is checked. Dependencies are
landing dependencies, not approval gates.

## Trust and adoption phase (2026-07-12)

The remaining campaign asks whether developers can trust the shipped route on
real Rust work. Every actionable result must carry producer-owned
`canonical_gap_id`, `seam_id`, `file:line`, `gap_state`, changed behavior,
missing discriminator, related test or production caller, focused test intent,
verify command, targeted-rerun command, receipt command, inspection command,
and authority boundary. If any fact is unavailable, the result is a named
limitation with a concrete investigation route; no renderer or path heuristic
may manufacture it.

The CallPresence packet is complete only when an authorized real or
current-repository receipt proves `production caller -> exact call seam ->
observable call/effect sink -> matching observing test -> complete route`.
Dynamic or unresolved receivers, method-name strings, ambiguous aliases or
owners, helper-only reachability, unrelated assertions, and opaque
macro-generated calls remain limitations unless producer evidence becomes
unambiguous.
The current blocked packet is recorded in
`docs/handoffs/2026-07-12-call-presence-evidence-packet.md`; its synthetic
positive analyzer tests and stale bounded repository scan are explicitly
excluded from promotion.

The dogfood packet requires at least 20 real repair attempts across at least 3
authorized Rust repositories. Each attempt records its repository and revision,
canonical gap and seam, before receipt, test-only repair intent and changed
files, verification command/result, after receipt, movement outcome, and
limitations. Only `closed`, `improved`, `unchanged`, `regressed`, and `limited`
are counted; synthetic fixture rows are not real-repository evidence.

Corpus collection is independent of CallPresence closure. The corpus packet
may proceed once repository authorization is recorded and must measure
CallPresence limitations when they occur; final route-quality closeout waits
for both the corpus threshold and a CallPresence proof or durable limitation
disposition.

The governed input is `metrics/rust-repair-trust/corpus.json`. Run
`cargo xtask rust-repair-trust-report` to write the JSON and Markdown scorecard
under `target/ripr/reports/`. Missing authorization, malformed rows, or
under-threshold denominators remain `limited`; the report must not reinterpret
the legacy `fixtures/real-repair-attempts/corpus.json` as Rust adoption
evidence.

Targeted rerun remains governed by the accepted SPEC-0123 contract: exact
selector identity, explicit before state, cache/invalidation disclosure,
selector-scoped parity, input fingerprints, graph provenance, and closed
movement vocabulary. The registered benchmark remains a floor (warm p50 <= 30
seconds, at least 5x faster than registered cold full, matched selected-scope
parity); faster without parity is limited.

Certification is current-head evidence. Receipts name the exact head SHA, later
mutation invalidates prior certification, and a reviewer who changes the branch
acts as a fixer rather than an independent reviewer for that pass. Cargo-allow
remains advisory structural authority, RIPR xtask remains proof executor, and
`.ripr/goals/active.toml` remains the sole execution manifest until #2119 is
resolved.

## Contract

The default human surface has one `Start here:` state, at most one selected
item, an omitted count, and direct paths to exhaustive and JSON evidence.
Limited results never look clean. Actionable Rust packets use domain-supplied
canonical identity and complete repair, verify, receipt, and evidence fields;
partial packets fail closed. Gate output names the seam, missing discriminator,
test intent, verification, receipt, and explain route. Targeted mutation emits
a bounded candidate and command or a named limitation. Value-flow additions are
conservative and fixture-first. Targeted reruns disclose cache reuse and
invalidation, never silently serving stale evidence.

## Targeted rerun benchmark slice (2026-07-11)

The benchmark receipt command and controlled fixture are now available through
`cargo xtask targeted-rerun-benchmark`. The current-main receipt on
`5aecff41` uses five samples and records matched parity, warm targeted p50 of
228 ms, cold-full p50 of 1512 ms, and a 6.6316x cold-full to warm-targeted p50
speedup. It also exercises an explicit file-fact cache reset and records
`recomputed_file_facts`.

This closes the benchmark-receipt slice on current main. PR #1532 additionally
ships explicit `path::test_node` selection for changed-test reruns.
The first targeted-rerun delivery is complete on current main. PR #1550
completed the graph-provenance follow-up from issue #1548: receipts now
attribute local package/member and feature graph availability, explicitly name
unavailable external dependency metadata, and fail parity closed when required
local graph provenance is missing or differs. The implementation reads local
Cargo manifests directly and does not use network or ambient Cargo metadata.

## Targeted rerun invalidation and parity slices (2026-07-11)

PR #1534 merged file-content invalidation disclosure. When a prior same-path
file-fact envelope exists under a different content key, the rerun receipt now
reports `file_content_changed` and names the affected path; cold misses and
unsupported input families remain explicitly named.

PR #1535 merged targeted/full parity evidence expansion. `--check-parity` now
compares producer-owned related-test and missing-discriminator evidence in
addition to canonical identity, class, file, and owner, and emits per-seam
`parity.mismatches[]` fields before failing closed. At that point the
targeted-rerun item remained open for broader manifest/package/config
invalidation attribution, expanded parity over those inputs, and final
campaign closeout; subsequent workspace-input and parity slices are recorded
below.

PR #1537 merged owned workspace-input fingerprint disclosure. Aggregate cache
identity now includes recursively discovered workspace manifests, lockfile, and
toolchain selector (with `RIPR_CFG_FEATURES` included when supplied). Targeted
receipts carry the fingerprint, and an explicit before receipt names changed
components as `input_changed:<field>` with
`invalidation_status: "workspace_input_changed"`.

PR #1539 merged full-pipeline parity over the owned workspace-input fingerprint.
When targeted and full inventory inputs differ, parity fails closed with
`full_pipeline_parity_input_mismatch` and names the changed input components.
PR #1540 merged explicit selector-ledger content fingerprinting: `--gap`
receipts hash the supplied ledger bytes, while `--changed-test` reports that
field as `not_applicable`; a changed ledger is disclosed as
`input_changed:selector_ledger_hash`. The current-main benchmark revalidation
is recorded in #1547. The graph-provenance follow-up then merged as #1550 and
completed the targeted-rerun work item; only the explicitly blocked CallPresence
producer evidence and receipt-backed dogfood closeout remain.

## Gate route dogfood status (2026-07-10)

The structured route and shared human/generated-CI projection are shipped. A
CLI-backed calibrated blocking receipt now pins the full route directly in
`gate-decision.{json,md}` and rejects artifact-download or PR-guidance lookup as
the repair step.

The structural #1440 gate-route delivery is complete: policy-eligible
decisions now carry a self-contained repair route and producer-owned inspection
command without artifact archaeology. The remaining real CallPresence
acceptance is tracked separately in issue #1543. Current CallPresence probes
exercised through `ripr review-comments` can remain `static_limitation` when
propagation to a side-effect/call-effect sink is unknown; they therefore do not
carry a policy-eligible receipt. Keep those cases fail-closed rather than
synthesizing a blocking call-observation receipt. Resolving that producer
limitation belongs in the `analysis/call-presence-gate-producer` slice, not a
gate renderer.

The remaining campaign blockers are explicit: #1543 needs an authorized
real/current-repository CallPresence receipt before any policy-eligible route
can be claimed; the cargo-allow active-goal dialect remains blocked on #2119;
and the final dogfood item lacks the required receipt-backed attempts across at
least three authorized Rust repositories. Synthetic fixture rows do not satisfy
that corpus requirement.

## Targeted mutation route delivery (2026-07-11)

PR #1545 merged the concrete #1425 route. Diff-scoped PR evidence now emits a
`targeted_mutation_route` with a bounded producer-owned predicate/operator
candidate and `cargo mutants --file ...` command when the probe has an
unambiguous source file, line, and expression. Unsupported, ambiguous, or
missing producer facts remain `static_limitation` with a named reason. The
route is preserved through `impacted-evidence`; mutation execution and runtime
claims remain out of scope. Future work may broaden safe candidate families
only behind producer-owned facts and checked fixtures.

## Boundaries and closeout

No preview promotion, automatic test generation, default mutation execution,
default CI hardening, release/signing/publishing/credential work, workspace
restructure, or unsupported static-evidence claims. New dependencies and public
CLI/schema choices need separately accepted scope. Each item yields one PR,
planning update, or blocked report. Close only when items are landed,
superseded with evidence, or blocked; current-main proof and the protected
check are recorded; and source truth, receipts, benchmarks, limits, and support
review agree.
