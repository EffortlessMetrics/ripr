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

A subject has an `origin`. An `upstream` subject (the default) is one
pinned upstream repository: URL, 40-hex commit, version label, license, an
optional reference into the shared Rust corpus manifest (`corpus_version`
`2026-10-04.1`), and the retained excerpt files under `subjects/<id>/` with
their sha256. Retained files are byte-identical to the upstream commit and
include its license files.

An `authored` subject is a small crate written for this corpus to fill a
verdict or probe-family cell the upstream cases leave empty. Its id starts
with `authored-` (an upstream id may not), it carries no upstream URL,
commit, or shared-corpus reference, its license is this repository's
(`MIT OR Apache-2.0`), it retains no license file of its own, and the whole
crate is retained. Its cases carry runtime truth exactly as upstream cases
do, with the whole stored crate standing in for the pinned checkout.
Authored cases are chosen to fill cells, so their rates are not real-world
rates: the report gives the false-verdict, false-actionable, false-exposed,
false-silent, ideal, and abstention rates again per origin under
`by_origin`, and a row names its origin.

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
Each mutant carries a written equivalence review and the one test that
failed, named exactly (other failing tests go in the review). A mutant of a
`behavior_preserving_rewrite` also carries `mutated_line`: the trimmed anchor
line with the mutant applied, empty when the mutant removes the statement.
`replacement` stays the human-readable description; `mutated_line` is what a
replay writes, so no tool has to guess which span a replacement stands for.
A `behavior_change` mutant has no `mutated_line`, because it is the edit.
A mutant that is equivalent in the pinned build cannot carry truth: its
passing tests show nothing about a missing discriminator. A reported miss
that turns out equivalent is replaced by a non-equivalent mutant of the same
expression, or the line is left out. Truth runs in a build that compiles
the anchored line and enables the features its behavior depends on; a mutant
in code a feature gate leaves out is not evidence. Each case records the
toolchain and test command its truth ran under.
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
limited; any other class is a gap; no anchored finding is silent. When the
anchor is a changed single-line `let`, RIPR-SPEC-0157 moves ripr's probe to
the predicate that uses the binding, so the projection also reads
candidate-current findings in the anchor file whose evidence carries
``binding_predicate_relation: changed binding `<name>` `` for the binding the
anchor declares, with the anchor's initializer as the new initializer, and
the row records `followed_retarget`. The case keeps the line its diff adds as
its anchor. Only a plain `let name` (optionally `mut`) is followed; patterns
fail closed. This couples the projection to the wording of that evidence
line: a wording change stops the following, and a retargeting case then
reads silent until the projection is updated. A gap on
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

`cargo xtask verdict-corpus` has four subcommands:

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
- `relabel [--sample <n> [--seed <s>] | --case <id>...] [--checkouts <dir>]
  [--repeat <k>] [--timeout-secs <t>] [--out <dir>] [--work-dir <dir>]`
  re-derives truth instead of trusting it. For each selected case it copies
  the subject to a run-owned tree, runs the case's own `cargo test` command
  on the unedited tree (which must pass), on the edit (which must pass for a
  rewrite and is the mutant for a behavior change), and on each mutant's
  `mutated_line` written over the anchor, each `--repeat` times (default 2).
  It fails when an outcome or the named failing test drifts from the label,
  when the replayed outcomes derive a different truth, when a mutant does not
  compile, times out, or equals the edited line, when repeated runs disagree,
  when `rustc --version` differs from the labeled toolchain, and when a
  run that exits zero executed no test. A failed run counts as a test
  failure only when a test failed, a test binary stopped without its
  result, or cargo reports a test binary exiting nonzero; any other failure
  after every binary passed (rustdoc failing before any doctest ran) is a
  build failure. Upstream excerpts replay only from a
  full checkout at the pinned commit, with no local changes or untracked
  files, under `--checkouts <dir>/<subject_id>`; without one they are listed
  as not replayed, never counted as passing. Only the checkout's tracked
  files (`git ls-files`) are copied, so ignored local files cannot change a
  replay; a tracked symlink that resolves outside the checkout's tracked
  paths, through any chain of links, or that does not resolve, is refused,
  and so is a submodule. Cargo resolves dependencies offline (`CARGO_NET_OFFLINE=true`),
  so a checkout's dependencies must already be in the cargo cache (`cargo
  fetch`); the command adds no network access of its own, but a subject's
  build scripts and tests run unsandboxed. `RUSTC` and `RUSTDOC` point at
  the rustup proxies, so doctests also run on the labeled toolchain; the
  caller's wrappers, `RUSTC_BOOTSTRAP`, and `RUSTFLAGS`-family variables are
  cleared, and the empty `RUSTFLAGS` family also overrides config-file
  rustflags. The test
  command may not pass `--manifest-path`, `--target-dir`, `--config`,
  `--no-run`, or a short flag bundling `-Z` or `-C` before `--`, nor
  `--list`, `--format`, or `--quiet` to the test binary; `validate` refuses
  such a command too. `--sample` picks a deterministic subset, for the same
  set of checkouts, ordered by sha256 of the seed and case id, so a
  scheduled run can rotate seeds through the corpus; `--case` inspects only
  the selected cases' checkouts. It writes `relabel.json` and never clones,
  fetches, or edits the corpus. Subject trees live under a per-process
  directory, so concurrent runs sharing a `--work-dir` do not clear each
  other's trees.

  Known limits: failing-test names match by `::` suffix across all test
  binaries; a binary that aborts (a stack overflow, `process::exit`) names
  no failing test, so only its failed outcome is checked; cargo stops at the
  first failing binary, so a labeled test in a later binary reads as not
  failing (fail-closed); a doctest name contains spaces and cannot be a
  `failing_test`; the short-flag check also refuses an attached value
  containing `Z` or `C` (`-pZstd`), so use the long form; the default
  `--work-dir` sits under this repository's `target`, so this repository's
  `.cargo/config.toml` applies to subject builds; other caller `CARGO_*`
  settings such as `CARGO_PROFILE_*` overflow checks still reach the subject
  build; and a killed run leaves its per-process tree directory behind.

The report states false-verdict, false-actionable (over discriminated
cases), false-exposed and false-silent (over the rest), ideal, abstention,
and contradiction rates as exact fractions, the verdict rates again per
subject origin, and one row per case with its origin and a
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
- The validator holds upstream subjects to a pinned URL, commit, and license
  file, and authored subjects to the `authored-` id prefix, no upstream
  provenance, no license file, and this repository's license; the report
  keeps authored rates apart from upstream rates. Relabeling a vendored
  excerpt as authored therefore means renaming the subject and every case
  that names it, which review sees; the validator cannot detect a rename
  that also strips the license file.

## Non-Goals

- Running mutation testing, `cargo test`, or network access from `report`
  or `check`. Truth was established at labeling and is recorded; only
  `relabel` runs the test commands, on demand, with cargo offline; it does
  not sandbox the subject's own build scripts or tests.
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
- strsim `dacc84c` `src/lib.rs:195` `sim > 0.7`: the spot-check's `>=`
  miss is equivalent: no string pair up to 70 characters long gives a Jaro
  value of exactly 0.7. The case uses threshold shifts instead: 0.75 fails a
  test and 0.65 does not, so the truth is `partially_discriminated` and ripr's `weakly_exposed` scores
  `ideal`.
- authored `pricing-quote-total-field` (`subtotal + shipping` rewritten as
  `shipping + subtotal` in a struct literal): the only test asserting
  `total_cents` uses a free-shipping order, so dropping the shipping term
  passes while dropping the subtotal fails. The truth is
  `partially_discriminated`, and ripr's `exposed` scores `false_exposed`, in
  the authored rates only.
- authored `checkout-tax-self-computed-expected` (`subtotal * 8 / 100`
  rewritten as `subtotal * 2 * 4 / 100`): the only test's expected value is
  `sub + tax(sub)`, computed through the changed function, so every mutant
  passes and the truth is `not_discriminated`. ripr's `exposed` scores
  `false_exposed`, the self-computed expected value RIPR-SPEC-0004 and
  RIPR-SPEC-0035 say must not count as a strong oracle.
- bytesize `as_kb` division (`src/lib.rs:258`): ripr reports
  `no_static_path` while naming related tests, recorded as
  `no_static_path_with_related_tests`. semver `op()` at 1.0.23
  (`src/parse.rs:272`) carried `reach_yes_without_related_tests` until #5424
  named every examined test; its row now records no contradiction.

## Test Mapping

Tests live in `xtask/src/reports/verdict_corpus_tests.rs`:

- `score_follows_the_truth_table_in_both_error_directions`
- `finding_verdict_mirrors_triage_classes_and_named_limits`
- `anchored_findings_keep_only_candidate_current_findings_on_the_anchor`
- `declared_binding_reads_only_a_let_declaration`
- `anchored_findings_follow_a_retarget_only_for_the_anchor_binding`
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
- `validator_holds_each_subject_origin_to_its_own_provenance`
- `report_keeps_authored_rates_apart_from_upstream_rates`
- `validator_requires_a_replayable_mutated_line_that_changes_the_anchor`
- `validator_refuses_a_mutated_line_on_a_behavior_change`
- `validator_refuses_a_test_command_the_replay_cannot_run`

Relabel tests live in `xtask/src/reports/verdict_corpus_relabel_tests.rs`:

- `parse_args_defaults_and_rejects_conflicts`
- `sample_order_is_deterministic_per_seed_and_rotates_across_seeds`
- `test_command_args_accepts_only_plain_cargo_test`
- `classify_run_separates_test_failures_from_build_failures`
- `names_failing_test_accepts_module_qualified_forms_only`
- `apply_mutated_line_keeps_indent_and_line_numbers`
- `mutant_drift_names_each_way_a_label_can_be_wrong`
- `observed_truth_follows_the_corpus_rule`
- `toolchain_release_ignores_the_host_triple`
- `declares_workspace_reads_only_a_workspace_table`
- `labeled_toolchain_names_the_rustup_release`
- `link_stays_inside_refuses_links_that_leave_the_copy`
- `copy_checkout_refuses_a_chain_of_links_that_resolves_outside`

## Implementation Mapping

- `xtask/src/reports/verdict_corpus.rs` owns validation, materialization,
  scoring, and rendering.
- `xtask/src/reports/verdict_corpus_relabel.rs` owns replaying runtime
  truth.
- `fixtures/rust-verdict-corpus/` holds the corpus, retained upstream and
  authored subjects, case diffs, and the expected report.

## Metrics

- `verdict_corpus_false_verdict_rate`
- `verdict_corpus_false_actionable_rate`
- `verdict_corpus_contradiction_rate`
