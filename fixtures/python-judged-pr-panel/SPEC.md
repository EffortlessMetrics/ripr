# Python Tier B Judged-Diff Panel Fixtures

Contract: [RIPR-SPEC-0092](../../docs/specs/RIPR-SPEC-0092-python-judged-pr-panel.md).

These inputs define the **schema** for a Tier B judged-diff panel — the bridge
from the Tier A robustness floor ([RIPR-SPEC-0086](../../docs/specs/RIPR-SPEC-0086-python-eval-sweep.md))
to a measured usable-tier number (false-actionable AND false-`exposed` rates).
This is a hand-vetted **seed** only: schema plus a few items, no judging engine
and no large corpus. Labels are `null` (unjudged) in this seed.

## Files

- `manifest.json` — the panel. Top-level envelope mirrors the Tier A manifest
  (`schema_version`, `kind`, `spec`, `tier`, `description`, `limits`), with an
  `items[]` array instead of `repos[]`.
- `diffs/*.diff` — the unified diff for each panel item.

## Panel item

Each item is one judged diff:

- `id` — unique, stable closure key.
- `repo` — a Tier A manifest id where applicable, or a synthetic-source label.
- `base` / `head` — pinned SHAs, or `null` for a standalone synthetic diff.
- `diff_path` — the unified diff under `diffs/`.
- `shape` — array reusing the Tier A shape vocabulary.
- `expected_direction` — `should_gap`, `should_stay_quiet`, or `should_limit`.
- `anchor` — `{ file, line, owner, boundary }`; `boundary` is free-form.
- `expected_classification` — conservative static ground truth
  (`weakly_exposed` / `exposed` / `static_unknown`).
- `expected_static_limit_kind` — non-`null` only for `should_limit`.
- `labels` — the judge verdict block, **all-nullable until judged**:
  `top_card_useful`, `false_actionable`, `false_exposed`, `verify_command_valid`,
  `suggested_location_valid`, `packet_boundaries_safe`, `limitation_quality`.
- `authority_boundary` = `review_advisory_only`; `repair_packet_ready` = `false`.

## Directional coverage (load-bearing)

The panel MUST include `should_stay_quiet` AND `should_limit`, not only
`should_gap` — otherwise it cannot measure false-actionable or false-`exposed`.
Per item, at most one of `false_actionable` / `false_exposed` is `true`.

## Boundaries

Schema + seed only. No judging engine, no analyzer change, no large corpus,
advisory only — never a default gate, badge, or RIPR Zero input. A later PR adds
the judging surface, populates the labels, and scales the corpus.
