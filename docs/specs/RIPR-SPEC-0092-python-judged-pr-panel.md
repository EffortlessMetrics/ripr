# RIPR-SPEC-0092: Python Tier B Judged-Diff Panel Schema

Status: proposed

Status note (2026-08-14): populated historical judged artifacts exist —
`fixtures/python-judged-pr-panel/starter-judged.json` and
`scaled-judged.json` carry past hand-judged labels for three and four items,
respectively (bounded historical combined `n=7`). The spec remains
**proposed**: no executable semantic validator, current promotion-grade rerun,
accepted threshold, or scaled promotion corpus is established, and the seed
(`manifest.json`) still ships unjudged (`null`) labels by design. Judged
artifacts are historical evidence, not a measured current
false-actionable/false-`exposed` rate.

Status note (2026-09-04, #3555 PR A): a typed loader and semantic validator
now exist — `xtask/src/python_judged_panel.rs` behind
`cargo xtask python-judged-panel check [--check]`, wired into
`cargo xtask precommit` as `cargo xtask check-python-judged-panel`. It
validates envelope identity, direction and judgment contracts, row-derived
totals, and diff/anchor proofs over the retained inventory, and keeps null
labels unjudged.

Status note (2026-09-04, #3555 PR B): an offline replay lane now exists —
`cargo xtask python-judged-panel replay [--check] [--limit <n>] [--network]`
(`xtask/src/python_judged_panel_replay.rs`) consumes the same validated
loader, materializes diff-proved temp workspaces for rows whose base/head
content is fully determined by the retained diff, invokes the real `ripr
check --mode fast --json` binary with isolated `RIPR_CACHE_DIR`, and retains
typed candidate records with mismatch reasons outside the accepted panel
under `target/ripr/python-judged-panel/replay/`. Accepted judgments,
directions, labels, and historical artifacts are never rewritten; judged-row
comparisons are candidate-vs-prior-actual with a `PriorActualStale` note when
the retained `judged_against` identity does not name the replayed binary
version. `--network` is declared and refused: live materialization lands with
a later slice, so replay exists offline over retained content only.
Review-round hardening: parse-failure typing consumes producer-owned facts
(analysis kind and limitation records) rather than candidate-count
heuristics; comparisons are emitted only for completed analyses (timeouts,
failed runs, parse failures, and partial analyses carry an explicit
comparison-unavailable marker); the anchor candidate binds by exact owner
identity — the canonical-gap owner, or for exposed findings without a
canonical gap a language-qualified probe-owner suffix — with no proximity
fallback; each run clears its own records directory; a case-id slug
collision fails the run before any execution; and every record binds the
base-tree digest plus the retained-rendition-head disclosure alongside the
binary, diff, and config identities.

Status note (2026-09-08, #3677): every replay record echoes the anchor
identity the subject was reconstructed from — `anchor: {file, line, owner}` —
as an additive field under schema 0.1. The report treats currency as bound to
the anchor as well as the diff, row kind, and prior-actual state: a record
whose echo no longer matches the row's anchor (anchor moved) or that predates
the echo (`missing_echo`) lands in the `anchor_stale` count with a per-case
named reason and can never bind a rate's as-of identity. Malformed echoes
(blank file or owner) fail the record read.

Status note (2026-09-08, #3680): regression feedback exists —
`cargo xtask python-judged-panel feedback [--records <dir>] [--adjudications
<dir>] [--out <dir>] [--check]` derives the report through the shared
report pipeline and stages one deterministic proposal per confirmed
over-credit (two independent roles agreeing verdict `exposed` with
`false_exposed` decided `true`, on a should_gap/should_limit row whose
replay is still current) under
`target/ripr/python-judged-panel/feedback/`, each with the full provenance
chain and a promotion recipe: extract a minimal regular fixture (the
SPEC-0108 meta-gate rejects manifest-only fixture dirs), add a corpus case
with `must_not_promote` assertions, run
`cargo xtask check-evidence-promotion-honesty`, and re-bless goldens in the
same PR. Proposals never write the corpus and never change analyzer
behaviour; promotion is a human-reviewed PR, and a promotion case that
fails the meta-gate means the analyzer genuinely over-promotes, so the
analyzer fix lands in that same PR.
Staged shapes (output contract): `index.json` carries `schema_version`
("0.1"), `kind` `python_judged_panel_feedback_index`, `spec`,
`generated_at` (the single non-deterministic field),
`confirmed_over_credits`, `proposal_files`, `authority_boundary`, and the
never-writes note; each `<case-slug>.proposal.json` carries
`schema_version`, `kind` `python_judged_panel_feedback_proposal`, `spec`,
`case_id`, `authority_boundary`, `confirmed_over_credit` (verdict,
false_exposed, roles, expected_direction, row_kind, behavior_family),
`provenance` (panel_digest, source_envelope, diff_path,
adjudication_record, replay_record, replay_candidate_classification), and
`promotion_recipe` (four steps: fixture extraction, corpus case with
must_not_promote, meta-gate run, golden re-bless). `--check` re-derives
and compares every staged file after stripping `generated_at`, and also
fails on an unexpected file set (a proposal whose confirmation
disappeared must not survive as a stale leftover).

Status note (2026-09-04, #3555 PR C): adjudication and reporting now exist —
`cargo xtask python-judged-panel adjudicate --case <id> --verdict
<classification> --role <role> (--reviewer <identity> | env
RIPR_PANEL_ADJUDICATOR) --evidence <ref>` records current independent
judgments under `target/ripr/python-judged-panel/adjudications/` without
touching the accepted panel: reviewer identity is required, at least one own
evidence citation is required, `must_not_claim` is echoed from the validated
row, RIPR's replay candidate is stored only as a named advisory reference,
and a case counts as adjudicated only with two distinct recorded roles and
identities (independence is self-claimed, not verified); one role stays `pending_second_role`, disagreement is
`disputed`, and an agreeing judgment with no direction-admitted error axis
decided is `inconclusive` — never a pass.
`cargo xtask python-judged-panel report` derives deterministic, byte-stable
JSON and Markdown from the validated inventory, the replay records, and the
adjudication records: separate false-actionable and false-`exposed`
numerators/denominators/rates scoped by the direction lattice, coverage by
direction, repository, behavior family, oracle alignment, and limitation
kind (relation basis is disclosed unavailable — the retained schema carries
no such field and none is invented), wrong-target/invalid-command/
limitation-correctness counts, unjudged and no-replay-record counts, and
per-case references; every rate carries its exact numerator, denominator,
coverage boundary, denominator case ids, and the records' binary as-of
identity bound by the denominator cases' own replay records, and no
denominator means no rate. With an explicit
`--threshold-policy <file>` the report evaluates the supplied candidate per
threshold as `pass`/`fail`/`not_evaluable` and echoes the policy's own
rationale and authority; it never selects a threshold from observed results,
never promotes support, and never writes a tier claim. The historical `n=7`
is not inherited; the report states its actual achieved denominator
(currently: 11 selected, 3 replayed, 0 adjudicated — both error rates
`not_evaluable`). The spec's remaining acceptance criteria (a current
replayed panel covering all three directions with recorded adjudication, an
accepted threshold, and a scaled promotion corpus) remain unmet; the status
stays **proposed**.

Status note (2026-09-07, #3555 PR C review round): the adjudication and
report lane hardened — adjudication records publish atomically (staged temp
sibling + flush + rename, with a copy fallback) and an unreadable record
fails instead of being overwritten; every stored judgment is re-checked
against the same semantic rules the CLI enforces (violations fail the report
named per case); carryover rows can never be adjudicated and are excluded
defensively; verdict-to-error coherence mirrors the retained validator's
outcome table (`exposed` on a should_gap/should_limit row requires
false_exposed true); a rate's as-of identity binds only the denominator
cases' own replay records (`no_common_binary_identity` otherwise); records
bind the full row revision (every field plus the diff content) and drift
marks them `stale_row` with stored-vs-current digests; report.json and
report.md publish as one generation with rollback on partial publication;
independence of the two recorded roles/identities is disclosed as
self-claimed, not verified. The status stays **proposed**.

Owner: language-adapter / swarm

Linked proposal:

- None. This is a standalone evidence-schema contract building on the
  RIPR-SPEC-0086 Tier A sweep. It adds no product library behavior and no public
  API; it defines a fixture schema and ships a hand-vetted seed.

Linked ADRs:

- [ADR 0009](../adr/0009-python-parser-substrate.md) (Python parser substrate;
  the panel judges the current `rustpython-parser`-backed lane).

Linked spec:

- [RIPR-SPEC-0086](RIPR-SPEC-0086-python-eval-sweep.md) — the Tier A robustness
  floor this panel extends with judgment.

Linked issues:

- [release(py): Python usable-tier readiness checklist](https://github.com/EffortlessMetrics/ripr-swarm/issues/1160)

Linked PRs:

- (this PR)

## Problem

Tier A (RIPR-SPEC-0086) measures robustness by counting emitted findings, so it
is structurally blind to **false-`exposed`** (silent over-credit): when `ripr`
stays quiet it emits nothing, so the error cannot be found by inspecting output —
only against ground truth on the cases where `ripr` stayed quiet. The
usable-tier question — *what are `ripr`'s measured false-actionable AND
false-`exposed` rates on Python diffs?* — has no schema to even record an answer.

This spec defines the **schema** for a Tier B judged-diff panel and ships a
small hand-vetted **seed** manifest. It does **not** produce the rates; it
defines the panel that a later judging PR will populate and measure.

## Behavior

### One production delta

Add the `fixtures/python-judged-pr-panel/` schema and seed. It introduces no
change to `crates/ripr` and no `xtask` judging command. The panel reads the
existing `ripr check` surface only.

### The panel unit is a diff

Each panel item is one judged diff. An item carries:

- `id` — unique, stable closure key (kebab/snake).
- `repo` — a Tier A manifest id where applicable, or a synthetic-source label.
- `base` — pinned pre-change commit SHA (or `null` for a standalone synthetic
  diff).
- `head` — pinned post-change SHA, or `null` for a synthetic diff.
- `diff_path` — path to the unified diff under `diffs/`.
- `shape` — array reusing the Tier A shape vocabulary
  (`pytest_library`, `unittest_library`, `click_typer`, `api_json`, …).
- `expected_direction` — one of `should_gap`, `should_stay_quiet`, `should_limit`.
- `anchor` — the changed seam: `{ file, line, owner, boundary }`. `boundary` is a
  free-form string describing the changed sink (e.g. "stream comparison
  return").
- `expected_classification` — the conservative static ground-truth verdict:
  `should_gap` → `weakly_exposed` or `reachable_unrevealed`; `should_stay_quiet`
  → `exposed`; `should_limit` → `static_unknown`.
- `expected_static_limit_kind` — non-`null` only for `should_limit`. One of the
  registered Python static-limit kinds: `decorator_indirection`,
  `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`, `mocked_module`,
  `opaque_custom_assertion_helper`, `property_based_test`,
  `unresolved_pytest_fixture`, `unsupported_syntax`.
- `labels` — the judge verdict block, **all-nullable until judged** (see below).
- `authority_boundary` — the constant `review_advisory_only`.
- `repair_packet_ready` — the constant `false` (non-productization guard).

### Labels — the two-error model (load-bearing)

The point of Tier B is the two error directions from
`docs/STATIC_EXPOSURE_MODEL.md` (Two error rates):

- **`false_actionable`** (visible): `ripr` routed an actionable repair for
  behavior that *is* discriminated. Measurable on `should_stay_quiet` items.
- **`false_exposed`** (silent over-credit): `ripr` called behavior covered /
  stayed quiet when **no** oracle discriminates the change. Recordable only
  against ground truth on items where `ripr` emitted nothing. This is exactly
  the error eval-sweep cannot see.

The remaining labels record quality: `top_card_useful`, `verify_command_valid`,
`suggested_location_valid`, `packet_boundaries_safe`, and `limitation_quality`
(`precise | imprecise | wrong_kind | over_limited | null`).

### Outcome consistency

Per item, **at most one** of `false_actionable` / `false_exposed` is `true`.
Both `false`, with the direction-appropriate quality label set, denotes a
correct judgment.

| `expected_direction` | correct `ripr` behavior | `false_actionable` when | `false_exposed` when |
| --- | --- | --- | --- |
| `should_gap` | emit `weakly_exposed` gap + repair card | n/a | `ripr` stayed quiet or credited `exposed` (missed gap) |
| `should_stay_quiet` | emit `exposed`, no repair card | `ripr` routed a repair packet | n/a |
| `should_limit` | emit `static_unknown`, no card | `ripr` routed a repair packet | `ripr` credited `exposed` past the limit |

### Required directional coverage

The panel MUST include `should_stay_quiet` AND `should_limit` items, not only
`should_gap`. A panel of only boundary-flip should-gap diffs can never measure
false-actionable or false-`exposed`; it would only confirm the direction the
analyzer is already tuned for. The panel MUST be able to record both "`ripr` was
quiet and that was correct" (a `should_stay_quiet` true negative) and "`ripr`
was quiet and that was wrong" (a `should_gap` item with `false_exposed: true`).

### Judged panels

A *populated* panel (labels filled, `judgment_source` set) additionally records,
per item, the observed verdict alongside the expectation — `actual_classification`
and `actual_oracle_alignment` — and an envelope-level `measurement_summary`
(`items_judged`, `false_exposed_count`, `false_actionable_count`, and a note).
Judging may be `manual_review` until an automated judging surface exists. A
populated panel is descriptive evidence and is **advisory only**: a small judged
set is directional, not a statistically robust rate, and never gates anything.

`fixtures/python-judged-pr-panel/starter-judged.json` is the first populated
panel — the three real Tier A starter-sweep diffs (click/six/tenacity), judged
by manual review against each repo's tests: 0 false-`exposed` and 1
false-actionable (tenacity, mapping to the documented `__call__`-via-local-instance
limitation). It confirms `ripr` errs conservative (over-suggest, never
over-credit) on real external code.

### Schema is additive; judging populates it

The seed ships labels as `null` (unjudged); a judged panel fills them. No
`cargo xtask` judging command and no analyzer change are required to record a
manual judgment. A later PR may add an automated judging surface and scale the
corpus.

## Required Evidence

- This spec, registered in `policy/doc-artifacts.toml` and `docs/specs/README.md`.
- A `[[behavior]]` entry in `.ripr/traceability.toml` mapping this spec to the
  seed manifest fixture.
- `fixtures/python-judged-pr-panel/{SPEC.md, manifest.json, diffs/*.diff}`, with
  at least one item per `expected_direction`.
- The manifest-only fixture exemption arm for `python-judged-pr-panel` in
  `xtask/src/reports/fixtures.rs` (`is_manifest_only_fixture_dir`). This helper
  is structural routing only; it is not a semantic judged-panel validator.

## Non-Goals

- No release or support-tier claim.
- No large corpus — a hand-vetted seed only.
- No automated judging engine in this PR.
- No change to `crates/ripr` analyzer behavior or public API.
- No mutation execution, provider calls, generated tests, or source / PR / CI edits.
- Never a default gate, badge, or RIPR Zero input — advisory only.

## Acceptance Examples

### A should-gap missed gap recorded as false-`exposed`

```text
expected_direction = "should_gap", expected_classification = "weakly_exposed";
a judging PR finds ripr emitted nothing -> labels.false_exposed = true
(a silent over-credit eval-sweep could not have seen).
```

### A should-stay-quiet true negative

```text
expected_direction = "should_stay_quiet", expected_classification = "exposed";
ripr emits exposed with no repair card -> false_actionable = false,
false_exposed = n/a, top_card_useful = n/a -> a correct judgment.
```

### A should-limit decorator-indirection item

```text
expected_direction = "should_limit",
expected_static_limit_kind = "decorator_indirection";
ripr fails closed to static_unknown with no card -> limitation_quality = "precise".
```

## Test Mapping

The panel now has a typed loader and semantic validator
(`xtask/src/python_judged_panel.rs`, `cargo xtask python-judged-panel check`),
an offline replay lane (`xtask/src/python_judged_panel_replay.rs`,
`cargo xtask python-judged-panel replay`), and an adjudication + reporting
lane (`xtask/src/python_judged_panel_report.rs`, `cargo xtask
python-judged-panel adjudicate` / `report`). The manifest-only fixture helper
only exempts this schema fixture from an unrelated generic requirement; doc
gates validate registration and traceability, not the meaning of the manual
judgments.

Landed with #3555 PR A (validator):

- `python_judged_panel::tests::retained_and_alternate_inventories_validate` —
  the retained inventory validates with row-derived aggregates, and a fully
  synthetic alternate inventory proves the validator is not hard coded.
- `python_judged_panel::tests::panel_contract_rejects_inventory_judgment_and_totals_drift` —
  direction/lattice consistency, at most one error label true per item, and
  hand-entered totals that disagree with rows.

Landed with #3555 PR B (replay):

- `python_judged_panel_replay::tests::replay_materializes_row_and_runs_real_check_end_to_end` —
  a synthetic row is materialized from its retained diff alone, the real
  `ripr check` binary runs over the temp workspace, the candidate
  classification is extracted at the anchor, the record lands outside the
  panel, and the panel digest is unchanged.
- `python_judged_panel_replay::tests::replay_records_typed_mismatch_without_touching_accepted_judgment` —
  divergent (or quiet) candidates produce typed mismatches against the
  accepted expectation and the accepted envelope bytes stay identical.
- `python_judged_panel_replay::tests::replay_types_insufficient_identity_as_not_run` —
  a tenacity-style row whose hunks start at line 88 replays as typed
  `not_run` with a reason, no workspace, and no fabricated comparison.
- `python_judged_panel_replay::tests::replay_notes_prior_actual_stale_for_judged_rows`
  — the `PriorActualStale` note keys on the retained judged-against
  identity (naming the current version suppresses it), not the row kind.
- `python_judged_panel_replay::tests::replay_honors_limit_and_discloses_bounded_out`,
  `replay_network_flag_fails_closed`, and
  `reconstruction_reverses_added_lines_and_skips_unproved_sides` — limit
  accounting, the `--network` refusal, and proved-side reconstruction.
- `python_judged_panel_replay::tests::replay_clears_stale_records_between_runs`
  and `replay_rejects_slug_collision_as_setup_violation` — each run
  retains exactly its own record set, and a case-id slug collision fails
  before any execution.
- `python_judged_panel_replay::tests::comparison_unavailable_for_non_completed_outcomes`
  — failed runs carry `ComparisonUnavailable` (never a quiet mismatch);
  a completed quiet run still types `ExpectedButQuiet`.
- `python_judged_panel_replay::tests::candidate_binds_exposed_findings_by_owner_identity`
  — exposed findings without a canonical gap bind by exact
  language-qualified probe-owner suffix; conflicts and near-suffixes do
  not bind.
- `python_judged_panel_replay::tests::outcome_parse_failure_uses_producer_facts`
  — a valid complete run with no behavioral candidates stays a complete
  quiet comparison; `analysis_failed`/`unsupported_input` and
  parse/input limitations map to the typed parse-failed outcome.
- `python_judged_panel_replay::tests::workspace_guard_removes_tree_on_failure`
  — the workspace build guard removes the temp tree on failure and keeps
  it when the value completes.

Landed with #3555 PR C (adjudication + reports):

- `python_judged_panel_report::tests::report_bytes_are_stable_across_independent_runs`
  — the byte-stability pin: two independent replay runs (records embed
  different temp workspace paths and command lines) plus two adjudication
  sets stamped at different times render byte-identical JSON and Markdown,
  no volatile path leaks into the report, the two-role case counts as
  adjudicated, and both surfaces state the same counts with the honesty
  notes (no combined score, unavailable relation basis) visible.
- `python_judged_panel_report::tests::report_derives_separate_error_denominators_and_rates`
  — direction-scoped denominators (a decided `should_gap` label feeds only
  the false-`exposed` rate), wrong-target/invalid-command counts from
  adjudications only, inconclusive rows excluded from limitation
  correctness, and the empty state discloses its actual achieved
  denominator with no rate where no denominator exists.
- `python_judged_panel_report::tests::threshold_evaluation_is_explicit_per_threshold_and_non_authoritative`
  — per-threshold `pass`/`fail`/`not_evaluable`, the policy rationale
  echoed, `not_evaluable` without denominators, a failing measured rate
  reported next to passing ones, and the no-tier-claim note present.
- `python_judged_panel_report::tests::adjudicate_rejects_unattributed_lattice_and_vocabulary_drift`
  — reviewer identity and own evidence citations required, conservative
  verdict vocabulary, the terminal lattice and direction admission gates,
  `should_limit`-scoped limitation grading, and the
  pending/disputed/inconclusive state machine.
- `python_judged_panel_report::tests::report_fails_closed_on_record_set_rot`
  — unknown case ids, mixed as-of binary identity, and foreign record
  kinds are rejected instead of folded into the counts.
- `python_judged_panel_report::tests::adjudication_writes_are_atomic_and_read_failures_are_refused`
  — an injected write failure preserves the prior record bytes with no temp
  residue, and invalid UTF-8 at the record path is a named error, never an
  overwrite.
- `python_judged_panel_report::tests::report_fails_on_semantically_invalid_stored_judgments`
  — empty evidence, unknown verdicts, and both-error-flag records fail the
  report named per case (the stored judgments re-run the CLI's semantic
  rules).
- `python_judged_panel_report::tests::carryover_rows_are_never_adjudicated_or_counted`
  — the retained carryover shape (null expected_classification) refuses
  adjudication and an injected record stays out of every adjudicated count.
- `python_judged_panel_report::tests::verdict_error_coherence_follows_the_direction_lattice`
  — `exposed` on a should_gap row requires false_exposed true on the CLI and
  in stored judgments; the coherent over-credit judgment feeds the
  false_exposed numerator.
- `python_judged_panel_report::tests::rate_as_of_identity_binds_the_denominator_cases_records`
  — a denominator case without its own replay record forces the
  `no_common_binary_identity` disclosure; bound cases cite their identity.
- `python_judged_panel_report::tests::adjudications_stale_against_a_changed_row_revision`
  — a row or diff change after adjudication makes the record `stale_row`,
  excluded from the denominator and disclosed with stored-vs-current
  digests.
- `python_judged_panel_report::tests::report_publication_is_one_generation`
  — a failure between the two report publications is rolled back to the
  prior pair with no staged temp residue.
## Implementation Mapping

| Concern | Artifact |
| --- | --- |
| Panel schema + seed | `fixtures/python-judged-pr-panel/{manifest.json, SPEC.md}` |
| Historical manual panels | `fixtures/python-judged-pr-panel/{starter-judged.json, scaled-judged.json}` |
| Seed diffs | `fixtures/python-judged-pr-panel/diffs/*.diff` |
| Manifest-only fixture exemption | `xtask/src/reports/fixtures.rs` (`is_manifest_only_fixture_dir`) |
| Typed loader + semantic validator | `xtask/src/python_judged_panel.rs` |
| Panel check command | `cargo xtask python-judged-panel check [--check]`; precommit alias `cargo xtask check-python-judged-panel` |
| Offline replay lane (implementation) | `xtask/src/python_judged_panel_replay.rs` |
| Panel replay command | `cargo xtask python-judged-panel replay [--check] [--limit <n>] [--network]` |
| Replay record output | one record per case at `target/ripr/python-judged-panel/replay/<stable_case_slug(case_id)>.json` — slug collisions fail the run before any execution (`python_judged_panel_replay_record` 0.1: binary/diff/config identity, head+base workspace digests, reconstruction disclosure, typed outcome, comparison) |
| Replay tests | `xtask/src/python_judged_panel_replay.rs` `mod tests` (end-to-end real-binary run, panel immutability, typed mismatches, `not_run` identity, stale/current notes, limit accounting, network refusal, reconstruction, record clearing, slug collisions, comparison availability, owner binding, parse survival, workspace guard) |
| Adjudication + report lane (implementation) | `xtask/src/python_judged_panel_report.rs` |
| Panel adjudicate command | `cargo xtask python-judged-panel adjudicate --case <id> --verdict <classification> --role <role> (--reviewer <identity> \| env RIPR_PANEL_ADJUDICATOR) --evidence <ref> [--adjudications <dir>] [--records <dir>]` |
| Adjudication record output | `target/ripr/python-judged-panel/adjudications/<stable_case_slug(case_id)>.json` (`python_judged_panel_adjudication_record` 0.1: echoed must_not_claim/envelope/direction, cited replay record as advisory reference only, per-role judgments with own evidence citations) |
| Panel report command | `cargo xtask python-judged-panel report [--records <dir>] [--adjudications <dir>] [--threshold-policy <path>] [--out <dir>] [--check]` |
| Report output | `target/ripr/python-judged-panel/report.{json,md}` (`python_judged_panel_report` 0.1: byte-stable across runs on identical inputs; separate two-error lattice, guarded denominators, per-threshold non-authoritative evaluation when a policy is supplied) |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

| Metric | Meaning |
| --- | --- |
| `false_actionable_rate` | fraction of judged items where `ripr` routed a repair for discriminated behavior (defined here; measured by a later judging PR) |
| `false_exposed_rate` | fraction of judged items where `ripr` stayed quiet / over-credited where no oracle discriminates (the silent error; measured later) |
| `directional_coverage` | presence of `should_gap` AND `should_stay_quiet` AND `should_limit` items in the panel |
