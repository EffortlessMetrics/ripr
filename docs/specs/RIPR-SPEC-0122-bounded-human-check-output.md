# RIPR-SPEC-0122: Bounded Human Check Output

Status: accepted

Owner: product / swarm

Created: 2026-07-07

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- [#2273](https://github.com/EffortlessMetrics/ripr-swarm/issues/2273) -
  digest discriminator label and `preview_limited` safe-next-action wording
  must reflect discriminator state and repair-packet completeness.

Linked PRs:

- [#1489](https://github.com/EffortlessMetrics/ripr-swarm/pull/1489) -
  bounds default human output, preserves exhaustive output as `human-full`,
  warns on repo-scoped formats with diff-bounding flags, and clarifies
  `first-pr --check` missing-packet recovery.

Support-tier impact:

- No tier change. This spec changes the default terminal presentation for
  `ripr check --format human`; it does not change analyzer classification,
  pass/fail authority, JSON schema, SARIF, GitHub annotations, badge output, or
  repo-exposure evidence.
- Support-tier definitions remain governed by
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md).
- Human output remains static advisory evidence. It must not claim runtime
  mutation confirmation, test adequacy, or exhaustive correctness.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

`ripr check --format human` previously rendered every finding as a full evidence
section. On large diffs, this turned the default terminal surface into an
evidence dump instead of a repair triage view. The default human surface should
answer "what do I inspect first?" while raw evidence remains available through
an explicit full format or JSON.

Repo-scoped formats also surprise users when combined with `--base` or `--diff`:
those flags do not bound formats such as `repo-exposure-json`, `repo-sarif`, or
`agent-seam-packets-json`.

## Behavior

### Default human output

`ripr check --format human` and the `text` alias render a bounded start-here
view:

```text
Header
Summary counts
Start here:
  State: <plain words> (top_gap | no_actionable_gap | preview_limited | static_limited | missing_scope)
  One selected finding or safe next action
Hidden:                                    (only when N > 0)
  N lower-priority finding(s) omitted from default human output [(language identity)].
  Full evidence: rerun with --format human-full
  Machine data: rerun with --format json
```

The Summary denominator counts unsuppressed findings against the total
(`N of M finding(s) unsuppressed`), disclosing the suppressed remainder; the
word is never "shown", because the bounded digest renders exactly one finding
and the `Hidden:` block below names the rest.

Human lines lead with plain words and keep the stable id in parentheses, so a
reader does not need the internal vocabulary and a script can still match the
id: `State: a test gap to inspect or repair (top_gap)`, `Analysis outcome:
findings below (analysis complete; complete_with_findings).` and `Static exposure: weak
(weakly_exposed, warning, confidence 0.92)`. The ids and their meanings are
unchanged.

The trailing block is state-dependent, because a `Hidden:` heading over a
literal `0 lower-priority finding(s) omitted` line claims a suppressed
remainder that does not exist:

- `N > 0` — the heading is `Hidden:` and the count line is rendered. The
  omission is the reason the section exists. When any omitted finding carries
  preview `language_status` or a non-Rust `language`, the count line appends
  a parenthetical identity breakdown from those fields (`Python preview: 1`).
  Unlabeled preview remainder uses `preview-language: N` rather than inventing
  a language name. Rust-only remainder stays the count line with no breakdown.
  This reads finding identity already on the omitted records; it is not the
  language-availability projection owned by #2615.
- The omitted set's currentness mix is named wherever it is not purely
  lower-priority candidates, whether or not a top gap was selected (#5021).
  Base-side evidence (`base_deleted`, `moved_or_renamed`) and
  `unresolved_subject` findings are not candidate edit targets, so when they
  share the omitted set with lower-ranked candidates the count line names the
  mix — `L lower-priority finding(s) omitted; B base-side evidence, not
  candidate edit targets` (an `U unresolved currentness, not candidate edit
  targets` clause joins when present) — and an omitted set that is entirely
  base-side or entirely unresolved currentness says so (`All N omitted
  finding(s) are base-side evidence, not candidate edit targets.`). Pure
  lower-priority omitted sets keep the single count clause.
- `N == 0` — the heading is `More:` and the count line is not rendered. The
  two format pointers still render, unchanged, because they remain useful
  when nothing was omitted.

The two format-pointer lines are identical in both states, so a consumer
scraping them does not need to know which heading was used.

When a selected finding exists, the digest includes file and line, static
exposure class, changed behavior, first missing discriminator when known,
related test when known, suggested repair or verify command when known, and a
short evidence summary.

The evidence summary leads with one compact line naming all five stage states,
because evidence ordering is pipeline-ordered (reach, infection, propagation,
observation, discriminator) and a purely positional detail window hides the
decisive stages behind a remainder count (#4324):

```text
  Evidence: reach yes · infection weak · propagation yes · observation yes · discriminator missing
```

Every stage always carries an evidence line, so the compact line names all
five stages for every finding and never silently drops one. The discriminator
token keeps the full evidence line's semantic: the `discriminate` stage grades
the strongest related oracle, so on a non-`exposed` finding a `yes` grade
renders as `missing` (a named missing discriminating input exists) or
`not established` rather than claiming a discriminator the digest
simultaneously reports missing. The per-stage prose detail stays in the
bounded window beneath it: the first two detail lines render verbatim, and
when detail remains the line
`- N more detail line(s) in --format human-full` discloses the count and names
the recovery format. `--format human-full` still renders every evidence line,
and no machine format reads the compact line.

Start here ranks a finding with a repair route ahead of one without. For a
stable finding the route is a recommended next step or suggested verify
command; for a Python preview finding it is a repair card from the Python
repair-card authority. A card-less Python finding therefore never hides a
carded one, so `check` does not report "no repair card" while `ripr pilot` and
`ripr first-pr` route a card for the same diff. Classification is unchanged.

The digest's discriminator line label reflects the discriminator state:

- `Missing discriminator` — the finding is not `exposed`; the named
  discriminator is absent from the related tests.
- `Discriminator (observed, advisory)` — the finding is `exposed` and the
  field carries an observation rationale instead of a missing discriminator.
  Preview-language classifiers record that rationale in the same field, so an
  unconditional `Missing discriminator` header would contradict the exposure
  class. Rust `exposed` findings carry no such entry and are unaffected.

The line carries the discriminator value alone; the label is not restated
inside it. `Finding.missing` mixes value-shaped entries, which the classifier
builds as `Missing discriminator value: <value>`, with prose entries such as
`No strong discriminator was detected`. Rendering a value-shaped entry verbatim
under the label produced
`Missing discriminator: Missing discriminator value: AuthError::RevokedToken`,
so the renderer strips that prefix and emits
`Missing discriminator: AuthError::RevokedToken`. Prose entries carry no such
prefix and are rendered unchanged. This governs the human digest only; no
machine format reads the label.

The digest's `Why <class>:` line restates the stage evidence that placed the
finding in its class, so it must agree with the `reach` / `observe` evidence
lines rendered beneath it:

- `no_static_path` with reach `no` — `no related test was found that reaches
  this change` (the classifier's own reason; it never claims an output trace
  was attempted);
- `reachable_unrevealed` with observe `no` — `a related test reaches this
  change, but no assertion observes the changed behavior`;
- any other stage combination for those classes falls back to wording that
  does not deny a reaching test.

The bounded renderer selects at most one visible unsuppressed finding. The
selector is deterministic:

1. Non-preview findings outrank preview-language findings.
2. Non-exposed findings with repair routes outrank findings without repair
   routes.
3. `exposed` findings do not outrank non-exposed findings merely because they
   carry generic next-step text.
4. Class, gap metadata, related tests, missing evidence, confidence, path, and
   line provide stable tie-breakers.

The selected finding's `Next step` is never truncated, because the guidance
ends with its remedy (#4323). Text within the digest line budget stays on one
line; longer guidance wraps onto four-space continuation lines.

### Triage states

| State | Meaning |
| --- | --- |
| `top_gap` | A non-preview, non-exposed finding was selected as the first safe repair or inspection candidate. |
| `no_actionable_gap` | Only `exposed` visible findings were selected; the output is not runtime proof or test adequacy. |
| `preview_limited` | The selected finding is from a preview-language adapter; evidence is advisory until the preview contract explicitly promotes it. |
| `static_limited` | The selected finding is no-path, unknown, or carries a producer-owned typed static limitation. A typed limitation remains authoritative even when the retained classification is `reachable_unrevealed` or `weakly_exposed`; inspect the named limitation before treating it as repair-ready. When a selected `no_static_path` finding has no typed limitation, review the unresolved static path and existing tests instead. The finding classification does not change. |
| `missing_scope` | The run produced no findings because no analysis scope was provided. This empty output is not an all-clear. |

The `preview_limited` safe next action distinguishes repair-packet
completeness, with the shared repair-packet validator as the only authority:

- When the selected preview finding projects a complete repair packet, the
  action states that the packet is complete but remains advisory and must be
  verified independently before acting.
- When the packet is blocked but no actionability fields are missing and the
  finding carries a structured static-limit kind, the action names the real
  blocker: the named static limitation holds the packet, and the operator
  must resolve the limitation and rerun preview evidence before acting.
  Without a structured static-limit kind the finding falls through to the
  exposed and closed-packet rules below.
- An `exposed` preview finding says there is no repair to make and must be
  verified independently, in every preview language (#4216).
- Otherwise, when `preview_actionability_for` projects a packet the shared
  validator kept closed (TypeScript and JavaScript today), the action is
  terminal (#4216): it quotes `preview_actionability_for`'s
  `why_not_actionable` (in most closed-packet cases the validator never ran;
  the part after `validator: ` when present, without a leading
  `is not agent-packet eligible: `, so the specific cause and its remedy
  survive the budget;
  collapsed to one line and bounded to the digest line budget), states that
  `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route the
  finding, and names the manual step before rerunning `ripr check`: add a test
  that calls the code when no test reaches it (`no_static_path`); check by hand
  whether a test observes the change for an unknown class (`static_unknown`,
  `infection_unknown`, `propagation_unknown`, for example a Bun-bridge
  visibility limit); otherwise add or strengthen a test by hand. The routing
  and manual-step parts are never truncated. The renderer only reads
  readiness; it never decides it.
- A preview finding with no projected actionability (for example Perl, whose
  production findings do not yet carry the projected actionability evidence)
  keeps the generic line directing the operator to complete the missing
  repair-packet fields.
- Python has no structured repair-packet projection; the Python repair card
  (`output/python_repair_card.rs`) is the authority on whether a Python
  finding carries a repair route (#4216). An `exposed` finding says there is
  no repair to make. A finding with a card points at its suggested test and
  verify command. A finding without a card is terminal: the action names why
  no card exists (a named static limitation, no Python test reaching the
  code, or no concrete missing discriminator), states that `ripr pilot`,
  `ripr agent repair` and `ripr first-pr` will not route it, and names the
  manual step before rerunning `ripr check`. It never asks the operator to
  complete fields they cannot supply.

### Exhaustive human output

`ripr check --format human-full` and the `text-full` alias render the previous
full per-finding evidence report. This format is diff-scoped like `human` and
is not a repo-scoped format.

When `ripr check` renders `human-full` itself, each rendered finding ends with
a `Drill in:` block holding the same `ripr explain` / `ripr context --at`
commands the bounded digest prints for its top finding (#4379). The digest
sends readers to `human-full` for full evidence, so that rerun must not lose
the only runnable next commands. Library renders without CLI navigation omit
the block.

### Repo-scope warnings

When a repo-scoped check format is combined with `--base` or `--diff`, the CLI
emits this warning on stderr before rendering:

```text
ripr: format <format> is repo-scoped; --base/--diff does not bound it.
Use --format json for diff-scoped findings, or --format repo-exposure-summary-json for a bounded repo summary.
```

The warning does not change format behavior or exit status. It prevents a user
from reading `--base` or `--diff` as a size bound for repo-scoped formats.

### `first-pr --check` missing-packet recovery

`ripr first-pr --check` validates an existing start-here packet. It does not
create one. If the expected packet is missing, the error names validate-only
mode, prints the missing path, and shows a create-and-validate command using
the same root, head, check-output, out-dir, and explicit base and gap-ledger
inputs where present. The recovery is printed before an omitted base is
resolved, so a checkout with no resolvable default branch still gets it
(#4285). An omitted `--base` stays omitted when the default branch resolves,
because the write run resolves it the same way; when nothing resolves, the
command carries `--base <ref>` followed by the resolution error, so the
suggested write cannot fail on the same missing base.

## Non-Claims

- Bounded human output is not runtime mutation evidence.
- `human-full` is not a schema change and does not add gate authority.
- Repo-scope warnings do not bound repo-scoped formats; they only disclose the
  scope mismatch.
- `first-pr --check` recovery text does not run analysis or write artifacts.

## Non-Goals

- Analyzer classification changes.
- Runtime mutation testing.
- Generated tests or source edits.
- JSON schema changes.
- SARIF, GitHub annotation, badge, or repo-exposure shape changes.
- CI blocking policy changes.

## Required Evidence

- Unit tests for bounded human selection, omitted count, no-scope
  `missing_scope`, preview-limited state, stable-gap-over-preview ranking,
  all-suppressed policy output, and `human-full` preservation.
- Unit tests for the digest discriminator label (`Discriminator (observed,
  advisory)` for `exposed` findings with an observation rationale; `Missing
  discriminator` otherwise) and for both `preview_limited` safe-next-action
  arms (complete-but-advisory packet versus missing packet fields).
- Format parsing tests for `human-full` and `text-full`.
- CLI unit tests for repo-scope warnings with `--base` and `--diff`, and no
  warning for diff-scoped JSON.
- First-pr recovery tests for missing start-here packets.
- Evidence-promotion fixture checks use `expected/human-full.txt` for
  exhaustive human projection assertions while `expected/human.txt` stays the
  bounded default output.
- Output-contract, static-language, traceability, and check-pr gates.

## Test Mapping

- `crates/ripr/src/output/human.rs::tests::bounded_human_output_caps_many_findings_and_reports_omitted_count`
- `crates/ripr/src/output/human.rs::tests::bounded_human_output_does_not_select_exposed_over_non_exposed_repair`
- `crates/ripr/src/output/human.rs::tests::bounded_human_output_reports_missing_scope_as_start_here_state`
- `crates/ripr/src/output/human.rs::tests::start_here_prefers_a_python_finding_with_a_repair_card`
- `crates/ripr/src/output/human.rs::tests::bounded_human_output_keeps_preview_language_in_preview_limited_state`
- `crates/ripr/src/output/human.rs::tests::bounded_human_output_prefers_stable_gap_over_preview_with_route`
- `crates/ripr/src/output/human.rs::tests::bounded_human_output_reports_no_actionable_gap_when_all_findings_suppressed`
- `crates/ripr/src/output/human.rs::tests::digest_labels_observation_rationale_as_observed_advisory_for_exposed`
- `crates/ripr/src/output/human.rs::tests::digest_keeps_missing_discriminator_label_for_non_exposed_classes`
- `crates/ripr/src/output/human.rs::tests::preview_limited_safe_action_names_terminal_manual_step_for_closed_packet`
- `crates/ripr/src/output/human.rs::tests::preview_limited_safe_action_says_no_repair_for_exposed_typescript`
- `crates/ripr/src/output/human.rs::tests::preview_limited_closed_packet_unknown_class_asks_for_manual_check`
- `crates/ripr/src/output/human.rs::tests::preview_limited_closed_packet_shows_validator_cause_over_preamble`
- `crates/ripr/src/output/human.rs::tests::preview_limited_safe_action_names_complete_but_advisory_packet`
- `crates/ripr/src/output/human.rs::tests::preview_limited_safe_action_names_limitation_block_when_no_fields_missing`
- `crates/ripr/src/output/human.rs::tests::preview_limited_safe_action_uses_closed_packet_line_without_static_limit_kind`
- `crates/ripr/src/output/human.rs::tests::preview_limited_python_no_static_path_names_untested_code`
- `crates/ripr/tests/cli_smoke.rs::check_python_finding_without_repair_card_names_the_terminal_manual_step`
- `crates/ripr/tests/cli_smoke.rs::check_python_finding_with_repair_card_points_at_the_card`
- `crates/ripr/src/output/human.rs::tests::human_full_preserves_legacy_all_findings_output`
- `crates/ripr/src/output/format.rs::tests::parses_human_full_aliases`
- `crates/ripr/src/output/format.rs::tests::human_full_is_not_repo_scope`
- `crates/ripr/src/cli/commands.rs::tests::repo_scope_format_with_base_emits_scope_warning`
- `crates/ripr/src/cli/commands.rs::tests::repo_scope_format_with_diff_emits_scope_warning`
- `crates/ripr/src/cli/commands.rs::tests::diff_json_with_base_does_not_emit_repo_scope_warning`
- `crates/ripr/src/output/first_pr.rs::tests::first_pr_check_missing_packet_error_explains_validate_only_mode`
- `crates/ripr/src/output/first_pr.rs::tests::first_pr_write_command_preserves_explicit_gap_ledger_only`
- `crates/ripr/src/output/first_pr.rs::tests::first_pr_write_command_renders_only_an_explicit_base`
- `crates/ripr/tests/cli_smoke.rs::first_pr_check_missing_packet_suggests_rooted_out_dir`
- `crates/ripr/tests/cli_smoke.rs::first_pr_check_missing_packet_recovers_without_a_resolvable_base`
- `crates/ripr/tests/cli_smoke.rs::first_pr_check_recovery_write_resolves_the_default_base`
- `crates/ripr/src/output/human.rs::tests::evidence_window_discloses_related_tests_cap`
- `crates/ripr/src/output/human.rs::tests::evidence_window_discloses_observed_values_cap`
- `crates/ripr/src/output/human.rs::tests::evidence_window_observed_values_pointer_names_json_cap_beyond_it`
- `crates/ripr/src/output/human.rs::tests::digest_related_test_line_carries_the_total`
- `crates/ripr/src/output/human.rs::tests::digest_missing_discriminator_discloses_one_of_n_window`
- `crates/ripr/src/output/human.rs::tests::hidden_block_lists_omitted_findings_by_file_line_and_class`
- `crates/ripr/src/output/human.rs::tests::hidden_block_all_base_side_run_names_base_side_evidence`
- `crates/ripr/src/output/human.rs::tests::hidden_block_unresolved_subject_run_names_the_unknown_not_base_side`
- `crates/ripr/src/output/human.rs::tests::hidden_block_mixed_currentness_run_names_base_side_and_unresolved_counts`
- `crates/ripr/src/output/human.rs::tests::hidden_block_list_discloses_remainder_beyond_its_window`
- `cargo xtask goldens check`

## Implementation Mapping

| Component | Location |
|---|---|
| Format enum and aliases | `crates/ripr/src/output/format.rs` |
| Bounded/default human renderer | `crates/ripr/src/output/human.rs` |
| Triage selection and state text | `crates/ripr/src/output/human/triage.rs` |
| Finding digest renderer | `crates/ripr/src/output/human/sections.rs` |
| Format dispatch | `crates/ripr/src/output/render.rs` |
| Repo-scope warning and suppression-policy wording | `crates/ripr/src/cli/commands.rs` |
| CLI help | `crates/ripr/src/cli/help/core.rs` |
| First-pr missing-packet recovery | `crates/ripr/src/output/first_pr.rs` |
| First-pr command options | `crates/ripr/src/output/first_pr/options.rs` |
| Full-human fixture projection guard | `xtask/src/main.rs` |
| Output contract docs | `docs/OUTPUT_SCHEMA.md` |
| Human golden fixtures | `fixtures/*/expected/human.txt` |
| Full-human projection fixtures | selected `fixtures/*/expected/human-full.txt` |

## CI Proof

- `cargo test -p ripr output::human --lib`
- `cargo test -p ripr output::format --lib`
- `cargo test -p ripr repo_scope_format --lib`
- `cargo test -p ripr diff_json_with_base_does_not_emit_repo_scope_warning --lib`
- `cargo test -p ripr first_pr_check_missing_packet_error_explains_validate_only_mode --lib`
- `cargo test -p ripr first_pr_write_command_preserves_explicit_gap_ledger_only --lib`
- `cargo xtask goldens check`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-traceability`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`
- `cargo xtask check-doc-index`
- `cargo xtask check-pr`

## Metrics

- Default human output line count is bounded by rendering one selected finding
  digest plus hidden-count pointers instead of every finding body.
- The hidden-count line is rendered only when it reports a non-zero omission,
  so a bounded run that omitted nothing costs two trailing lines, not four.
- `human-full` remains available for full evidence inspection.
- Repo-scoped formats disclose when diff-bounding flags do not bound the run.

## Acceptance Examples

1. A run with hundreds of findings emits one `Start here:` block, omits the
   lower-priority finding bodies, and points to `--format human-full` and
   `--format json`.
2. A non-exposed repair candidate beats an `exposed` finding that only carries
   generic next-step text.
3. A preview-language finding with a repair route renders
   `State: preview_limited`, not `top_gap`.
4. A stable Rust repair candidate outranks a preview-language finding that
   only carries advisory repair text.
5. When all findings are suppressed by policy, the output renders
   `State: no_actionable_gap` with suppression-specific recovery text rather
   than naming a missing static limitation.
6. Bare no-scope empty output renders `State: missing_scope` and keeps the
   no-scope disclosure.
7. `--format human-full` renders every visible finding body.
8. `--format repo-exposure-json --base origin/main` emits the repo-scope
   warning.
9. A run whose findings all fit in the bounded view renders `More:` with the
   two format pointers and no `Hidden:` heading and no
   `0 lower-priority finding(s) omitted` line.
10. A run that omitted at least one finding renders `Hidden:` with the non-zero
    count line above the same two format pointers. When the omitted set includes
    preview-language or non-Rust identity, that line names the per-language
    counts; a Rust-only remainder stays the count line alone.
