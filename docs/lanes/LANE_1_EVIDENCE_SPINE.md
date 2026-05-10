# Lane 1: Evidence Accuracy / Evidence Spine

Lane 1 owns RIPR's analyzer truth and the shared evidence spine. Its job is to
make behavioral test-grip evidence accurate enough to trust, structured enough
to reuse, and concrete enough to burn down toward RIPR 0.

Lane 1 follows this tracker, the evidence specs, capability matrix,
traceability manifest, and output contracts. It does not switch to another
campaign merely because `.ripr/goals/active.toml` points at a PR/CI, editor, or
policy surface.

## Scope

Lane 1 owns these evidence surfaces:

- seam identity and canonical behavioral gap identity;
- `seams[].evidence_record`;
- reach, activate, propagate, observe, and discriminate stages;
- observed values;
- missing discriminators;
- oracle strength and oracle semantics;
- related-test ranking;
- before/after evidence movement;
- static limitations;
- imported static/runtime calibration labels.

Downstream surfaces may consume this evidence, but they should not reinterpret
it independently.

## Current Stable Spine

The shared evidence record is stable within its documented v0.1 scope.

The stabilized spine is:

```text
ClassifiedSeam
-> seams[].evidence_record
-> repo exposure
-> thin consumers with legacy fallbacks
```

Completed slices:

- `output: add shared evidence record` (#651) added
  `seams[].evidence_record` to repo exposure while preserving existing seam
  fields.
- `output: route zero surfaces through evidence record` (#654) made agent seam
  packets and RIPR Zero repair routes prefer the shared record when supplied.
- `output: route evidence movement through evidence record` (#658) made
  targeted-test outcome and agent verify compare record-level stage, value,
  discriminator, oracle, and related-test movement when present.
- `output: route assistant proof through evidence record` (#661) made assistant
  proof prefer the record for selected seam identity, recommendation, static
  limits, and movement context.
- `output: route baseline ledgers through canonical gap identity` (#664) made
  baseline and PR ledger identity carry semantic gap IDs when supplied.
- `fixtures: pin evidence record contract matrix` (#667) pinned the v0.1
  record shape across representative gap, oracle, limitation, canonical ID, and
  calibration-placeholder cases.
- `analysis: stabilize related-test ranking` (#671) made related-test ordering
  deterministic while preserving `related_tests_total`.
- `analysis: stabilize oracle semantics` (#677) explained what supported oracle
  shapes observe, miss, and can upgrade.
- `analysis: deepen local delta flow sinks` (#679) expanded syntax-first flow
  sink families for return, error, field, match, and side-effect paths.
- `calibration: label static runtime confidence` (#680) added imported
  static/runtime confidence labels without changing static classifications.
- `analysis: generate canonical gap identity` (#685) generated deterministic
  `canonical_gap_id`, group size, and grouping reason for headline-eligible
  gaps.

## Stable Fixture-Backed Analysis Slices

These Lane 1 analysis slices are stable within documented syntax-first scope:

- Local delta flow identifies visible return-value, error-variant,
  struct-field, match-arm, event/outbound-call, state-write,
  persistence-write, log-message, configuration-change, and generic call-effect
  sinks. Unsupported or opaque propagation remains `propagation_unknown` with an
  explicit static limitation rather than a stronger claim.

## Current Open PRs

At tracker creation, there are no open upstream Lane 1 PRs. Current open PRs
belong to PR/CI or editor projection lanes unless explicitly re-scoped.

When opening future Lane 1 PRs, list them here until they merge or close:

| PR | Slice | State | Notes |
| --- | --- | --- | --- |
| none | - | - | - |

## Next Slices

These are Lane 1 candidates. Open them only when they are selected as evidence
accuracy work:

1. `analysis: promote activation value modeling where fixture-backed`
   - audit equality boundaries, enum/Option/Result branches, table rows,
     rstest cases, builder overrides, and same-file constants;
   - keep cross-file constants, macro-heavy cases, and opaque helpers explicit
     limitations.
2. `calibration: promote covered static/runtime labels`
   - promote only runtime-backed classes with fixture and calibration evidence;
   - do not run cargo-mutants or change static reports.
3. `fixtures: expand evidence record contract`
   - add cases only when a new evidence class or consumer requirement changes
     the v0.1 contract.

## Validation Gates

Docs and capability tracker changes should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Evidence behavior changes should add the relevant checks:

```bash
cargo test -p ripr evidence_record --lib
cargo test -p ripr canonical_gap --lib
cargo test -p ripr repo_exposure --lib
cargo test -p ripr agent_seam_packets --lib
cargo test -p ripr outcome --lib
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Cross-Lane Rules

- `.ripr/goals/active.toml` is the active Codex Goals manifest, not the whole
  product board.
- Lane 1 may add or change evidence consumed by PR/CI, editor, agent, baseline,
  or gate surfaces.
- Lane 1 should not implement PR/CI summary projection, editor UX polish,
  policy defaults, release flows, or provider integration.
- Cross-lane reports should consume `evidence_record` or documented legacy
  fallback fields instead of rebuilding seam truth ad hoc.
- If a downstream consumer exposes an evidence gap, fix the evidence source in
  Lane 1 and keep the projection change narrow.

## Non-Goals

Lane 1 does not own:

- default CI blocking;
- PR comment posting;
- generated CI front-panel composition;
- first-useful-action docs, dogfood, or closeout;
- LSP/VS Code UX polish;
- CodeLens, inlay hints, or unsaved-buffer overlays;
- generated tests;
- source edits;
- provider or model calls;
- runtime mutation execution;
- release, packaging, or security workflow mechanics;
- badge hosting policy.

## Operating Rule

Before taking a Lane 1 task, confirm it changes analyzer truth, evidence
identity, evidence structure, evidence movement, or calibration confidence. If
it mainly renders, governs, distributes, or secures existing evidence, route it
to the owning lane.
