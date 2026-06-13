# RIPR-SPEC-0092: Python Tier B Judged-Diff Panel Schema

Status: proposed

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
  `xtask/src/main.rs` (`is_manifest_only_fixture_dir`).

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

This PR is schema + seed; it adds no executable judging surface. The seed is
validated structurally by the manifest-only fixture contract and the doc gates.

Planned (a later judging PR):

- `python_judged_pr_panel::manifest_load_validates` — envelope + per-item field
  validation (unique ids, valid `expected_direction`, lattice consistency).
- `python_judged_pr_panel::lattice_rejects_double_error` — at most one error
  label `true` per item.

## Implementation Mapping

| Concern | Artifact |
| --- | --- |
| Panel schema + seed | `fixtures/python-judged-pr-panel/{manifest.json, SPEC.md}` |
| Seed diffs | `fixtures/python-judged-pr-panel/diffs/*.diff` |
| Manifest-only fixture exemption | `xtask/src/main.rs` (`is_manifest_only_fixture_dir`) |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

| Metric | Meaning |
| --- | --- |
| `false_actionable_rate` | fraction of judged items where `ripr` routed a repair for discriminated behavior (defined here; measured by a later judging PR) |
| `false_exposed_rate` | fraction of judged items where `ripr` stayed quiet / over-credited where no oracle discriminates (the silent error; measured later) |
| `directional_coverage` | presence of `should_gap` AND `should_stay_quiet` AND `should_limit` items in the panel |
