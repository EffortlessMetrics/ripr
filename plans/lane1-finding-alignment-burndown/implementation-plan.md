# Lane 1 Finding Alignment Burn-Down Implementation Plan

Status: open

Owner: Lane 1 - Evidence Accuracy

Linked tracker:
`docs/lanes/LANE_1_FINDING_ALIGNMENT_BURNDOWN.md`

Linked specs: `RIPR-SPEC-0045`, `RIPR-SPEC-0043`, `RIPR-SPEC-0048`

## Current State

Lane 1 has closed Evidence Quality Leadership and User-Visible Output Evidence
in documented scope. Presentation text now proves the model:

```text
raw findings -> canonical evidence items -> actionable gaps / no-action / limitation
```

#1106, #1109, and the shippable finding-alignment closeout make the repo-local
scorecard and audit report the current burn-down inputs. The current
merged-main audit reports 47,181 raw alignment signals, 38,027 canonical
alignment items, 149 actionable canonical items, and zero actionable canonical
items missing repair routes, verify commands, or named static limitations.

The next work is maintenance-mode alignment usefulness: keep those invariants
true while burning down measured gaps such as incomplete placement fields,
large named limitation buckets, unsupported config/policy flows, and static-only
runtime confidence coverage.

## Current Run-Reliability Slice

The 2026-05-18 live audit exposed an operational reliability gap before the
next analyzer class repair: direct `ripr check --root . --mode instant --format
repo-exposure-json` completed when stdout was redirected to a file, but
`cargo xtask lane1-evidence-audit` tried to buffer the large repo-exposure JSON
payload in memory before writing the audit input. `cargo xtask
evidence-health` also failed through its nested `cargo run` wrapper while the
already-built `ripr` binary completed the same live-repo report. The supported
fix is to stream the audit subprocess stdout directly to the temporary
repo-exposure input file while capturing stderr latency breadcrumbs and bounded
status diagnostics, and to have evidence-health invoke the built debug binary
directly after `cargo build -p ripr`.

Current live proof after the streaming fix:

- repo-exposure generation status: `pass`;
- generated stdout bytes: 521,053,340;
- latency trace events captured: 95;
- raw alignment signals: 47,626;
- canonical evidence items: 38,564;
- actionable canonical gaps: 162;
- named static limitations: 26,250;
- top remaining named limitation bucket: `activation_value_unresolved` at
  25,881.
- `cargo xtask evidence-health` completes through the same built-binary path and
  writes `target/ripr/reports/evidence-health.{json,md}`.

This slice is run-reliability hardening only. It does not change evidence
classification, public badge semantics, PR/CI rendering, gates, provider/model
calls, generated tests, source edits, or mutation execution.

## Hard Boundaries

- fixture first;
- audit-driven repairs before heuristics;
- raw findings remain diagnostic evidence;
- canonical items are the countable unit;
- actionable canonical gaps require repair routes;
- named limitations are analyzer work, not user test debt;
- runtime-only signal does not create static gaps;
- policy/adoption overlays stay separate from Lane 1 evidence state;
- no PR/CI rendering changes;
- no inline PR comment publishing;
- no LSP or editor polish;
- no gate policy or default blocking changes;
- no public badge or score redefinition;
- no generated tests;
- no automatic source edits;
- no provider or model calls;
- no mutation execution.

## Work Item 1: report: audit finding alignment coverage by evidence class

Issue: [swarm #229](https://github.com/EffortlessMetrics/ripr-swarm/issues/229)
/ [source #1140](https://github.com/EffortlessMetrics/ripr/issues/1140)

Status: done in swarm #229 proof on 2026-05-22.

### Goal

Add the class-by-class audit map that shows where raw signals still leak or
canonical evidence items lack actionability support.

### Production Delta

Extend the Lane 1 audit or scorecard with alignment coverage by evidence
class, unaligned raw finding counts, duplicate groups, unnamed static unknowns,
missing repair routes, missing verify commands, and top examples.

### Acceptance

- `alignment_coverage_by_class` is reported.
- `unaligned_raw_findings_by_class` is reported.
- `top_unaligned_examples` include enough evidence class and raw-finding
  context to drive fixtures.
- `same_line_duplicate_groups` is reported.
- `static_unknown_without_named_limitation` is reported.
- `canonical_items_without_repair_route` is reported.
- `canonical_items_without_verify_command` is reported.
- `raw_to_canonical_ratio` remains visible.

### Current Proof

The 2026-05-22 live sampled audit reports:

- `alignment_coverage_by_class`: 7 evidence classes;
- `unaligned_raw_findings_by_class`: empty;
- `top_unaligned_examples`: empty because no unaligned raw findings remain in
  the sampled audit;
- `same_line_duplicate_groups`: 10 groups;
- `static_unknown_without_named_limitation`: 0;
- `canonical_items_without_repair_route`: 0;
- `canonical_items_without_verify_command`: 0;
- `raw_to_canonical_ratio`: 1.1392.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 2: analysis: keep static_unknown canonical items named

Issue: [swarm #233](https://github.com/EffortlessMetrics/ripr-swarm/issues/233)
/ [source #1141](https://github.com/EffortlessMetrics/ripr/issues/1141)

Status: done in swarm #241 / #244 proof on 2026-05-22.

### Goal

Preserve the invariant that user-facing static unknown canonical items have a
named static limitation and repair route.

### Acceptance

- `static_unknown_without_named_limitation = 0` remains true.
- Unknowns stay visible.
- Unknowns do not become actionable without fixture-backed evidence.
- Limitation categories point to repair routes.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item 3: analysis: make primary_anchor and raw_spans complete

Issue: [#1158](https://github.com/EffortlessMetrics/ripr/issues/1158)

### Goal

Complete the placement/supporting-span projection enough that downstream
surfaces can place one annotation without treating every raw finding as a user
item.

### Acceptance

- Canonical items expose `primary_anchor` where a safe placement exists.
- Raw spans or raw findings preserve contributing line-local evidence.
- Unsupported placement is explicit rather than inferred.
- Downstream consumers do not need to route raw findings as independent
  annotations.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 4: analysis: burn down top named static limitation bucket

Issue: [swarm #238](https://github.com/EffortlessMetrics/ripr-swarm/issues/238)
/ [source #1159](https://github.com/EffortlessMetrics/ripr/issues/1159)

Status: done in swarm #238 / #240 proof on 2026-05-22.

### Goal

Use #1140 to pick the largest repairable named static limitation bucket and
turn it into a fixture-backed analyzer repair.

### Acceptance

- The selected bucket is identified from audit data.
- Positive and must-not-claim fixtures land before or with the repair.
- Supported cases move out of limitation.
- Unsupported cases remain named limitations.
- The PR reports before/after audit or scorecard delta.

### Current Selected Slice

The 2026-05-22 sampled audit selected `call_presence` /
`activation_owner_call_unresolved` as the top named static limitation bucket.
This slice supports the direct owner-call plus same-file direct-wrapper
sub-shapes:

- a related test directly calls the owner and carries an explicit mock
  expectation; or
- a non-test wrapper in the same file directly calls exactly one specific local
  owner;
- a test calls that wrapper;
- the owner contains a value-insensitive `call_presence` seam.

The supported case moves activation to `yes` without inventing observed values.
Wrappers that skip the owner, two-hop wrapper chains, assertion-target affinity
alone, generated tests, provider calls, PR/CI rendering, gate policy, public
score semantics, and mutation execution remain out of scope.

Sampled audit delta for this slice:

- before: `call_presence` had 770 static-limitation items, including 719
  `activation_owner_call_unresolved` limitations;
- after: `call_presence` has 715 static-limitation items, including 663
  `activation_owner_call_unresolved` limitations;
- delta: -55 `call_presence` static limitations and -56
  `activation_owner_call_unresolved` limitations;
- current `call_presence` scorecard work score: 4,554.

### Historical Proof

The 2026-05-18 live audit selected `activation_value_unresolved` as the top
named static limitation bucket. The first merged slice burned down the
value-insensitive no-argument owner-call sub-shape:

- before: 27,677 static limitations, including 27,288
  `activation_value_unresolved`;
- after: 26,339 static limitations, including 25,967
  `activation_value_unresolved`;
- delta: -1,338 static limitations and -1,321
  `activation_value_unresolved` limitations.

The supported case is a direct no-argument owner call for a value-insensitive
seam. It moves activation out of a limitation without inventing observed values.
Predicate-boundary value checks, non-direct owner affinity, helper-only flows,
generated tests, provider calls, PR/CI rendering, gate policy, public score
semantics, and mutation execution remain out of scope.

The follow-up kept the same historical bucket and widened the supported
value-insensitive owner-call path to direct calls whose argument values remain
opaque. It still did not invent observed activation values and still required
concrete activation values for predicate-boundary checks. The 2026-05-19 live
audit before that slice reported 26,277 static limitations, including 25,908
`activation_value_unresolved` limitations. The after-audit reported 19,106
static limitations, including 18,859 `activation_value_unresolved`
limitations, while actionable canonical gaps stayed at 162. Delta: -7,171 total
static limitations and -7,049 `activation_value_unresolved` limitations.

The 2026-05-22 sampled audit follow-up selected `call_presence` /
`activation_owner_call_unresolved` as the current top named static limitation
bucket. Swarm #240 added fixture-backed positive and must-not-claim coverage for
the supported direct owner-call plus mock-expectation sub-shape and kept
specific call-target affinity as a named limitation when no owner call is
observed. The refreshed bounded audit reported:

- `call_presence` limitations: 770 -> 769;
- `call_presence` `activation_owner_call_unresolved`: 719 -> 718;
- `call_presence` work score: 5000 -> 4994;
- total `activation_owner_call_unresolved`: 1217 -> 1216.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 5: docs(spec): refine config/policy unsupported-flow expansion

Issue: [swarm #241](https://github.com/EffortlessMetrics/ripr-swarm/issues/241)
/ [source #1142](https://github.com/EffortlessMetrics/ripr/issues/1142)

Status: done in swarm #246 / #249 proof on 2026-05-22.

### Goal

Refine the already-existing config/policy evidence contract for the next
unsupported-flow expansion.

### Acceptance

- The existing RIPR-SPEC-0048 remains the source of truth.
- The update identifies `opaque_config_lookup` as the next unsupported-flow
  expansion target.
- Generated config/schema output, macro output, dynamic dispatch, and
  unsupported cross-file flow remain named limitations until selected by a
  later fixture-backed slice.
- The update states required benchmark and audit deltas before analyzer
  changes.
- Internal policy/config metadata alone remains no-action.

### Proof Commands

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item 6: fixtures: add config-policy unsupported-flow burn-down cases

Issue: [swarm #246](https://github.com/EffortlessMetrics/ripr-swarm/issues/246)
/ [source #1143](https://github.com/EffortlessMetrics/ripr/issues/1143)

Status: ready.

### Goal

Pin unsupported-flow expansion cases before implementation.

### Acceptance

- Fixtures cover at least two selected unsupported-flow categories.
- Each fixture records expected canonical item, gap state, actionability, and
  static limitation category.
- Unsupported flow remains a named limitation until analyzer support lands.
- Internal metadata remains no-action.

### Proof Commands

```bash
cargo test -p xtask evidence_quality_benchmark
cargo xtask check-fixture-contracts
cargo xtask check-doc-index
cargo xtask check-pr
git diff --check
```

## Work Item 7: analysis: expand config/policy unsupported-flow support

Issue: [swarm #250](https://github.com/EffortlessMetrics/ripr-swarm/issues/250)
/ [source #1144](https://github.com/EffortlessMetrics/ripr/issues/1144)

Status: ready.

### Goal

Move one selected config/policy unsupported-flow category out of limitation
only when fixture-backed.

### Acceptance

- One selected unsupported-flow category becomes observed, actionable, or
  no-action in supported fixture-backed scope.
- Other unsupported flows remain named limitations.
- Scorecard or audit reports before/after delta.
- Raw findings remain attached as supporting evidence.

### Proof Commands

```bash
cargo test -p ripr config_policy --lib
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 8: analysis: require repair routes for actionable canonical items

Issue: [#1145](https://github.com/EffortlessMetrics/ripr/issues/1145)

### Goal

Make `actionable` mean the item says what repair is needed.

### Acceptance

- Actionable items include `recommended_repair`.
- Actionable items include `repair_kind`.
- Actionable items include target test type, assertion shape, or output
  observer shape when applicable.
- Non-action states do not fake repair routes.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 9: analysis: attach verify commands to actionable canonical items

Issue: [#1146](https://github.com/EffortlessMetrics/ripr/issues/1146)

### Goal

Give humans and agents a proof step for each actionable canonical item when
one is safely known.

### Acceptance

- Actionable items carry `verify_command` where feasible.
- Missing commands are explicit and counted.
- Commands preserve advisory/static boundaries and do not imply mutation
  execution.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 10: report: preserve scorecard lead with actionable canonical gaps

Issue: [#1147](https://github.com/EffortlessMetrics/ripr/issues/1147)

### Goal

Keep internal scorecards led by user work while new evidence classes land.

### Acceptance

- `actionable_canonical_gaps` leads the scorecard summary.
- Already observed, internal/no-action, static limitation, raw-finding,
  canonical-item, raw-to-canonical, repair-route, and verify-command coverage
  remain visible.
- Public badge semantics do not change in this PR.

### Proof Commands

```bash
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 11: calibration: audit runtime confidence coverage

Issue: [#1160](https://github.com/EffortlessMetrics/ripr/issues/1160)

### Goal

Show calibrated-supported versus static-only canonical items by evidence class.

### Acceptance

- Scorecard or audit reports calibrated support coverage by class.
- Top static-only classes are visible.
- Runtime-only signal does not create static gaps.
- Cargo-mutants import planning stays linked to #323 and does not become live
  mutation execution.

### Proof Commands

```bash
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item 12: dogfood: refresh finding alignment examples from burn-down deltas

Issue: [#1149](https://github.com/EffortlessMetrics/ripr/issues/1149)

### Goal

Refresh the existing gallery only when a new burn-down delta changes useful
examples.

### Acceptance

- Examples include before/after audit or scorecard context for the changed
  class.
- Existing presentation-text and config/policy examples are not duplicated
  without a new delta.

### Proof Commands

```bash
cargo xtask dogfood
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```

## Work Item 13: docs: refresh canonical finding alignment handoff

Issue: [#1153](https://github.com/EffortlessMetrics/ripr/issues/1153)

### Goal

Refresh the existing v2 downstream handoff only after material burn-down
changes.

### Acceptance

- The existing v2 contract remains the baseline.
- New fields, class behavior, or guidance from burn-down work are reflected.
- No refresh happens for unchanged existing examples alone.
- No rendering behavior changes land in this docs handoff PR.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Closeout

Close the burn-down rail only after the tracker closeout conditions are met and
the final handoff records:

- what evidence classes improved;
- what remains unknown;
- which counts moved;
- which issues remain open;
- which downstream lanes can safely consume the v2 contract;
- which evidence class should be repaired next.
