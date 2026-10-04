# RIPR-SPEC-0219: Labeled Rust verdict corpus

Status: proposed

Owner: product-eval

Created: 2026-10-04

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. The corpus measures verdict accuracy on named cases. It does not
  promote a language, editor surface, gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- Add `fixtures/rust-verdict-corpus` to the manifest-only fixture set.
- Retained upstream Rust files are stored as `<name>.rs.txt`, byte-identical
  to upstream, and get their `.rs` name back only in the run-owned copy. The
  vendored code is fixture data: it adds no workspace Rust, no lint or
  process-policy allowlist rows, and no lines to the PR diff-scope budget.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawn sites, or support-tier changes. The harness reuses the fixture
  runner's ripr build and process owner.

## Problem

Trust in a ripr verdict has been argued from anecdotes: one serde loop that
ripr refused, one report that said "no static test path" beside 81 related
tests. Existing judged panels
([RIPR-SPEC-0092](RIPR-SPEC-0092-python-judged-pr-panel.md) for Python, the
Rust judged-behavior panel seed) fix the vocabulary of both error directions
but carry synthetic or unjudged Rust subjects. Nothing measures how often
ripr's Rust verdict is wrong on real tests whose behavior has been worked
out, so a fix to one verdict cannot show it did not break another.

## Behavior

`fixtures/rust-verdict-corpus/corpus.json` (`ripr_verdict_corpus.v1`) holds
subjects and cases.

A subject is one pinned upstream repository: URL, 40-hex commit, version
label, license, an optional reference into the shared Rust corpus manifest
(`corpus_version` `2026-10-04.1`), and the retained excerpt files under
`subjects/<id>/` with their sha256. Retained files are byte-identical to the
upstream commit and include its license files.

A case is one edit in one subject: a unified diff under `cases/`, the
anchored file and line the diff adds, the edit kind, behavior family, test
shape, an optional hard-case note, the truth label, the expected verdicts,
written reasoning, and what ripr reported when the case was labeled.

Truth is runtime evidence. The edit was applied to the full pinned checkout
and the crate's own test command run. For a `behavior_preserving_rewrite`
the edit keeps tests green and each listed mutant of the edited expression
was applied on top; for a `behavior_change` the edit is its own single
mutant. A mutant counts as detected (`tests_failed`) when the test command
fails after compiling.
Each mutant carries a written equivalence review and the test that failed.
Truth derives from how many mutants failed the tests: all is `discriminated`, none is
`not_discriminated`, otherwise `partially_discriminated`.

Expected verdicts follow one table the validator enforces:

| Truth | Ideal | Acceptable |
| --- | --- | --- |
| discriminated | credited | credited, limited, silent |
| partially_discriminated | gap | gap, limited |
| not_discriminated | gap | gap, limited |

The observed verdict reads only candidate-current findings on the anchor:
`exposed` is credited; a named `static_limit_kind` or a `no_static_path`,
`infection_unknown`, `propagation_unknown`, or `static_unknown` class is
limited; any other class is a gap; no anchored finding is silent. A gap on
the line outranks credit, and credit outranks a limit. The per-finding
reading mirrors the human "Start here" triage. The line-level precedence is
this corpus's own policy, not triage's ranking: triage orders findings to
pick where to start, while the corpus asks which verdict a developer reading
the line would act on, and a gap there routes repair work.

Each case scores as `ideal`, `abstained` (acceptable but not ideal),
`false_actionable` (a gap where tests discriminate), `false_exposed` (credit
where they do not fully), or `false_silent` (no verdict where they do not
fully).

Contradictions are internal to ripr's output and need no label:
`reach_yes_without_related_tests`, `no_static_path_with_related_tests`,
`exposed_without_discriminator`, `related_tests_listed_exceed_total`, and
summary counts that disagree with the findings list.

`cargo xtask verdict-corpus` has three subcommands:

- `validate` checks the corpus offline: schema, subject digests and
  unlisted files, diff anchors, truth derived from mutant outcomes, and the
  label table.
- `report [--out <dir>]` copies each subject to a run-owned workspace under
  `target/ripr/verdict-corpus/`, applies the case diff with a strict patch
  reader that refuses drifted context, runs `ripr check --json`, and writes
  `report.json` and `report.md`.
- `check [--out <dir>]` does the same and fails when `report.json` differs
  from `fixtures/rust-verdict-corpus/expected/report.json` or `report.md`
differs from `expected/report.md`. It refuses an
  `--out` that is the expected directory, so it cannot replace its golden.

The report states false-verdict, false-actionable (over discriminated
cases), false-exposed and false-silent (over the rest), ideal, abstention,
and contradiction rates as exact fractions, plus one row per case with a
`changed_since_labeling` flag.

The contradiction rate counts candidate-current findings across each case's
whole `ripr check` run, not only the anchor line, because a
self-contradicting finding anywhere is an internal inconsistency.
`contradictions_by_code` uses the same unit: findings carrying each code,
plus one per run for a summary-count code. A row's `contradictions` lists
the distinct codes seen in that case's run.

## Required Evidence

- The committed corpus validates and contains both a `discriminated` and a
  `not_discriminated` case.
- Each validation rule rejects a tampered corpus.
- The scoring table, verdict projection, contradiction codes, and rate
  arithmetic are pinned by unit tests.
- The committed expected report agrees with the corpus labels row by row.

## Non-Goals

- Running mutation testing, `cargo test`, or network access from the
  harness. Truth was established once, at labeling, and is recorded.
- A population estimate. Rates describe these cases only.
- Replacing the judged panels or the shared Rust corpus; this corpus draws
  on the shared corpus pins where they exist.
- Wiring the check into CI; a regression gate consumes the report later.

## Acceptance Examples

- serde `format_u8` (`n >= 100` rewritten as `n > 99`): both boundary
  mutants fail `test_format_u8`, which checks all 256 values inside `loop`.
  ripr reports `reachable_unrevealed`, so the case scores
  `false_actionable`. A `static_limit` on the loop would score `abstained`.
- semver 1.0.23 `digit > b'9'` to `digit >= b'9'` (the first-run edit):
  four semver tests fail, so the tests discriminate the change. ripr 0.10's
  `reachable_unrevealed` would score `false_actionable`; the current
  `infection_unknown` scores `abstained`.
- itoa `remain > 9` rewritten as `remain >= 10`: the macro-generated tests
  still pass under both mutants, so a gap is ideal and ripr's
  `no_static_path` scores `abstained`.
- semver `op()` at 1.0.23 `src/parse.rs:272`: ripr says a related test
  reaches `op` while `related_tests_total` is 0, recorded as
  `reach_yes_without_related_tests`.

## Test Mapping

Tests live in `xtask/src/reports/verdict_corpus_tests.rs`:

- `score_follows_the_truth_table_in_both_error_directions`
- `finding_verdict_mirrors_triage_classes_and_named_limits`
- `anchored_findings_keep_only_candidate_current_findings_on_the_anchor`
- `case_verdict_ranks_gap_over_credit_over_limit`
- `contradictions_flag_each_internal_inconsistency_and_pass_a_clean_finding`
- `summary_contradictions_compare_counts_with_the_findings_list`
- `summary_contradictions_account_for_suppressed_findings`
- `ratio_text_is_fixed_precision_and_names_an_empty_denominator`
- `apply_patch_rewrites_the_anchored_line_and_reports_it_as_added`
- `apply_patch_refuses_drifted_context`
- `parse_patch_refuses_renames_and_empty_input`
- `parse_patch_holds_hunks_to_their_declared_counts_and_starts`
- `relativize_probe_files_strips_only_the_run_root`
- `committed_corpus_is_valid_and_measures_both_error_directions`
- `validator_rejects_a_label_that_contradicts_its_mutant_outcomes`
- `validator_rejects_an_expected_verdict_outside_the_truth_table`
- `validator_rejects_a_retained_file_whose_digest_moved`
- `validator_rejects_an_anchor_the_diff_does_not_add`
- `validator_rejects_ids_that_are_not_one_safe_path_segment`
- `validator_rejects_a_diff_that_patches_an_unretained_path`
- `validator_requires_both_truth_directions`
- `expected_report_rows_agree_with_corpus_labels`
- `build_report_counts_rates_over_the_right_denominators`
- `contradiction_counts_use_one_per_finding_unit`
- `stored_paths_keep_vendored_rust_out_of_the_workspace`

## Implementation Mapping

- `xtask/src/reports/verdict_corpus.rs` owns validation, materialization,
  scoring, and rendering.
- `fixtures/rust-verdict-corpus/` holds the corpus, retained subjects, case
  diffs, and the expected report.

## Metrics

- `verdict_corpus_false_verdict_rate`
- `verdict_corpus_false_actionable_rate`
- `verdict_corpus_contradiction_rate`
