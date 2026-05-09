# RIPR-SPEC-0005: Repo Seam Inventory and Test Grip

Status: proposed

## Problem

`ripr` currently produces `Finding` and `StageEvidence` from diff-scoped
analysis. This works for Voice A (changed-line probe shapes), but it does not
inventory the full set of behavior seams in a repository or classify how
strongly current tests grip each seam.

Campaign 4B needs a first-class seam model so that:

1. The analyzer can walk production files and enumerate seams independent of a
diff.
2. Each seam carries stable, deterministic identifiers that do not depend on
file walk order.
3. Test-grip evidence can attach to seams, not just to changed-line findings.
4. Editor diagnostics and agent packets can surface ungripped seams without
requiring a diff input.
5. Badge counts can derive from seam classification without breaking the
existing schema.

Without a spec-locked seam model, `RepoSeam` boundaries would be implicit and
the mapping from seam classification to existing `ExposureClass` and badge
counts would be accidental.

## Behavior

`ripr` should be able to inventory a repository and emit a seam catalog where
each seam carries:

- **SeamId**: a stable, deterministic identifier.
- **SeamKind**: the category of behavior seam (e.g. `predicate_boundary`,
`error_variant`, `return_value`, `field_construction`, `side_effect`,
`match_arm`, `validation_branch`, `call_presence`).
- **RequiredDiscriminator**: what a test would need to observe to grip this
seam (e.g. exact boundary value, exact error variant, exact return value).
- **TestGripEvidence**: reach / activate / propagate / observe / discriminate
facts built from existing test/oracle analysis.
- **SeamGripClass**: the classification derived from the evidence (e.g.
`strongly_gripped`, `weakly_gripped`, `ungripped`, `reachable_unrevealed`,
`activation_unknown`, `propagation_unknown`, `observation_unknown`,
`discrimination_unknown`, `opaque`, `intentional`, `suppressed`).

The seam inventory should be deterministic: two runs over the same source tree
with the same analyzer version and configuration must produce the same seam IDs
in the same order.

When the analyzed root is the repository root, repo automation and fixture data
are outside the product seam surface. The walker excludes `xtask/` and the
top-level `fixtures/` tree so repo-scoped public signals describe the published
`ripr` package, not its harness. Passing a fixture workspace itself as `--root`
still analyzes that fixture normally.

### Stable Seam ID Rules

Seam IDs must be stable across runs and across input file walk reorderings.
Derivation rules:

1. **File path**: repo-root-relative normalized path (Unix separators, no
leading `./`).
2. **Owner symbol**: the fully qualified module/impl-qualified owner string.
3. **Seam kind**: the `SeamKind` discriminant string.
4. **Local coordinates**: byte offset or line/column pair of the seam origin
within the file. Byte offset is preferred for stability across formatting
changes; line/column may be used only if the syntax adapter already normalizes
whitespace.
5. **Hash**: a deterministic hash (e.g. FxHash) of the concatenation of the
above fields, encoded as a fixed-length lowercase hex string.

The hash must not depend on walk order, timestamp, or process ID.

### Relationship to ProbeShapeFact and Finding

- `ProbeShapeFact` (Voice A) is a diff-scoped, changed-line probe. It is a
*trigger* for analysis, not a repo-wide seam.
- `RepoSeam` (Voice B) is a repo-wide, file-scoped behavior seam. It may be
inventoried even when no diff is present.
- A `Finding` is a diff-scoped output. When a diff touches a seam, the
`Finding` should reference the seam by `SeamId` so that diagnostics and
packets can map back to the repo-wide inventory.
- `ExposureClass` values (exposed, weakly_exposed, no_static_path, etc.) are
diff-scoped classifications. They map to `SeamGripClass` through an explicit,
versioned table, not through type extension.

### Headline Count vs Visible-Only Mapping

The spec must define which `SeamGripClass` values count toward the headline
badge and which are visible but non-headline:

- **Headline eligible**: `ungripped`, `weakly_gripped`, `reachable_unrevealed`,
`activation_unknown`, `propagation_unknown`, `observation_unknown`,
`discrimination_unknown`.
- **Visible but non-headline**: `strongly_gripped`, `intentional`, `suppressed`.
- **Opaque**: `opaque` is visible in detailed reports; its headline treatment is
determined by the badge policy (currently non-headline for `ripr`,
`unknowns_test_efficiency` for `ripr+`).

The mapping table is versioned and lives in the spec, not in the badge
renderer.

### Static-Language Boundaries

All static output must use the conservative vocabulary defined in
`docs/STATIC_EXPOSURE_MODEL.md`. Specifically:

- Forbidden in static output: `killed`, `survived`, `untested`, `proven`,
`adequate`.
- Allowed: `exposed`, `weakly_exposed`, `reachable_unrevealed`,
`no_static_path`, `infection_unknown`, `propagation_unknown`,
`static_unknown`, `strongly_gripped`, `weakly_gripped`, `ungripped`,
`activation_unknown`, `propagation_unknown`, `observation_unknown`,
`discrimination_unknown`, `opaque`, `intentional`, `suppressed`.

### Voice A vs Voice B Contract

- **Voice A**: currently-probeable production syntax shapes, diff-scoped.
Produces `Finding`/`ExposureClass` output. This is the existing analyzer mode.
- **Voice B**: first-class behavior seams + test-grip evidence, repo-scoped.
Produces `RepoSeam`/`SeamGripClass` output. This is the Campaign 4B mode.

Neither voice claims mutation adequacy. Real mutation testing (e.g.
cargo-mutants) is a separate calibration step that may compare static
`SeamGripClass` against runtime outcomes, but static output must never claim
mutation results.

## Required Evidence

Each seam entry must include enough evidence for a reviewer or agent to decide
what test is missing and why:

- seam ID, kind, file path, owner symbol
- related tests (name, file, line, oracle kind/strength)
- observed activation values when known
- missing discriminator hypothesis (e.g. "boundary value 100 never tested")
- reach evidence: does any test call the owner?
- activate evidence: does any test supply the triggering input?
- propagate evidence: does the test observe the changed state downstream?
- observe evidence: does the test assert on the visible sink?
- discriminate evidence: does the oracle distinguish the changed behavior from
the original?

Related tests must keep `related_tests_total` as the full matched count while
ranking the capped `related_tests` array deterministically by relation
confidence, relation reason, oracle strength, activation-value overlap, file,
name, and line. Relation confidence and reason decide whether a test is likely
related; oracle strength and activation overlap only choose the best imitation
target inside otherwise equivalent relationships.

Related-test oracle semantics must explain what the oracle observes, what
discriminator remains missing, and which assertion upgrade would improve the
seam when the current shape is broad, smoke-only, or unknown. These semantics
do not change `oracle_kind`, `oracle_strength`, relation confidence, or static
classification; they make the existing oracle facts actionable.

## Non-Goals

This spec does not require:

- MIR-based or full trait-resolution seam detection (may be added later, but
initial seam kinds are syntax-backed).
- Real-time incremental seam inventory during editing.
- Automatic test generation from seam packets.
- Global suite scoring or test-deletion recommendations.
- Runtime mutation execution as part of the static inventory.
- Repository-specific seam suppressions in the first version (suppressions may
be added after the classification surface is stable).
- Changing the existing diff-scoped `Finding` output schema in a breaking way
(the schema may gain optional `seam_id` fields, but existing fields remain).

## Acceptance Examples

### Predicate boundary seam with missing discriminator

```text
Given a function check_discount that compares amount >= discount_threshold,
when ripr inventories the repo, then it emits a RepoSeam:

  seam_id: "a1b2c3d4..."
  kind: predicate_boundary
  owner: "pricing::check_discount"
  file: "src/pricing.rs"
  required_discriminator:
    - boundary value: discount_threshold
  test_grip_evidence:
    reach: true (test "test_premium" calls owner)
    activate: true (test supplies amount = 200)
    propagate: true (result reaches assertion)
    observe: true (test asserts on result)
    discriminate: false (no test uses amount = discount_threshold)
  seam_grip_class: weakly_gripped
  missing_discriminator: ["discount_threshold boundary value not tested"]
```

### Deterministic seam ID stability

```text
Given the same source tree analyzed twice with file walk order reversed,
when ripr emits the seam inventory both times,
then the seam IDs, order, and counts must be identical.
```

### Headline vs visible mapping

```text
Given a seam with seam_grip_class = strongly_gripped,
when ripr renders the repo badge,
then the seam appears in detailed reports but does not count toward the
headline seam-native unresolved-gap count.
```

## Test Mapping

Tests for this spec will be added as the implementation work items land:

- `analysis/repo-seam-model-v1`: unit tests for `SeamId` stability,
`SeamKind` round-trips, `RequiredDiscriminator` construction.
- `analysis/repo-seam-inventory-v1`: golden tests for seam inventory output
against fixture repos.
- `analysis/test-grip-evidence-v1`: tests that evidence attaches to the
correct seam and cites the correct related tests.
- `analysis/related-test-ranking-v2-stabilization`: tests that direct owner
calls outrank weaker relationship signals, strong oracles outrank smoke-only
oracles inside the same relation, activation-value overlap breaks remaining
ties, comment/string-only mentions do not match, and stable file/name/line
ordering remains deterministic.
- `analysis/oracle-semantics-v3-stabilization`: tests that broad error,
smoke-only, exact-value, and record-projected related-test oracles explain what
they observe, what they miss, and when an assertion upgrade is available.
- `analysis/repo-ripr-classification-v1`: tests for `SeamGripClass`
classification rules and headline mapping.

## Implementation Mapping

Planned implementation work items (from `.ripr/goals/active.toml`):

1. `analysis/repo-seam-model-v1`: introduces `RepoSeam`, `SeamId`, `SeamKind`,
`RequiredDiscriminator`, `TestGripEvidence`, `SeamGripClass` as crate-private
types.
2. `analysis/repo-seam-inventory-v1`: walks production Rust files and emits
`Vec<RepoSeam>`.
3. `analysis/test-grip-evidence-v1`: attaches per-seam evidence records.
4. `analysis/repo-ripr-classification-v1`: classifies each seam into
`SeamGripClass`.
5. `output/repo-exposure-report-v1`: renders Markdown + JSON repo report.
6. `lsp/repo-seam-diagnostics-v1`: surfaces ungripped seams in the editor.
7. `lsp/seam-evidence-hover-v1`: hover renders evidence path with cited tests.
8. `context/agent-seam-packets-v1`: agent packets carry seam + grip + missing
discriminator.

## Metrics

Proposed metrics to track as the implementation lands:

- `seam_inventory_count`: total seams inventoried per repo.
- `seam_kind_distribution`: count per `SeamKind`.
- `seam_grip_class_distribution`: count per `SeamGripClass`.
- `ungripped_seam_count`: seams with no meaningful test grip.
- `stable_id_collision_rate`: collisions per seam inventory run (must be zero).
- `seam_to_finding_link_rate`: percentage of diff findings that reference a
known `SeamId`.
