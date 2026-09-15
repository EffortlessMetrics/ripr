# RIPR-SPEC-0108: Evidence-Promotion Honesty Meta-Gate

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- (none — standing capstone for the fake-clean bug class)

Linked PRs:

- None yet

Support-tier impact:

- Honesty enforcement meta-gate: no classifier behavior change; additive xtask
  report-envelope fields are allowed when documented here; no `ripr check` JSON
  schema bump and no version bump. This spec pins the semantic expectation that
  non-promoted charter fixtures must remain non-promoted, independently of
  whether a golden was re-blessed. Tier labels and claim boundaries remain
  governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, functions, LSP servers, or analyzer behavior changes.
- `check-evidence-promotion-honesty`: reads byte-pinned golden
  `expected/check.json` for each pure charter member and asserts the invariant.
- `check-evidence-promotion-honesty --pinned-external`: executes exact
  real-repo launch points against the current RIPR binary and asserts the same
  semantic product expectations.
- The command writes a typed corpus result envelope at
  `target/ripr/reports/corpus-summary.{json,md}` so infrastructure, budget,
  golden, and semantic failures do not collapse into one generic failure.
- New corpus manifest: `fixtures/evidence-promotion-honesty-corpus/corpus.json`.
- Every corpus case declares a `tier`: `pure` for tiny checked-in fixtures, or
  `pinned_external` for exact-repo cases with repo, command, commit, patch, and
  budget metadata.
- Registered in CI (routed-rust.yml and ci.yml) next to `check-fixture-contracts`.
- `evidence-promotion-honesty-corpus` added to `is_manifest_only_fixture_dir`
  denylist so `goldens check` skips it.
- Does NOT unify per-language matcher functions.
- Does NOT run mutants or re-classify any finding.

## Problem

### The recurring fake-clean bug class

Across multiple PRs (RIPR-SPEC-0094, RIPR-SPEC-0097, RIPR-SPEC-0098,
RIPR-SPEC-0103, RIPR-SPEC-0104, RIPR-SPEC-0106, RIPR-SPEC-0107), the same
failure mode re-appeared: a finding was promoted to `exposed` when its evidence
did not structurally match the seam. Each fix required a new fixture and a
golden to pin the corrected behavior.

### Why `goldens check` alone is not sufficient

`goldens check` asserts `binary == golden` (byte comparison). If a developer
changes the classifier and re-blesses the golden of a known fake-clean fixture
from `weakly_exposed` → `exposed`, `goldens check` passes — binary now matches
the new (dishonest) golden. The semantic expectation that *this fixture must
stay non-promoted* is not enforced by `goldens check`.

### The gap

There was no standing CI gate that read the byte-pinned golden and asserted "no
finding may be `exposed` for charter member X, regardless of what the golden
says it was re-blessed to."

## Behavior

### The invariant

> A finding may not be promoted to `exposed` unless its evidence STRUCTURALLY
> matches the seam. Each confirmed fake-clean is a pinned charter member that
> must stay non-promoted.

### The gate (`check-evidence-promotion-honesty`)

1. Loads `fixtures/evidence-promotion-honesty-corpus/corpus.json`. The load
   rejects duplicate object keys (issue #2277): `serde_json`'s last-wins rule
   would silently drop a spliced case's pin while the file still parses, so a
   duplicate key anywhere in the corpus fails the gate closed with a parse
   error naming the repeated key.
2. For each pure case, reads exactly one byte-pinned source artifact:
   - `source_fixture` -> the fixture's `expected/check.json` (covered by
     `goldens check`);
   - `source_report` -> a checked-in JSON report artifact for product states
     outside the standard `goldens check` runner.
2a. Validates the case tier before trusting the expectation:
   - `pure` cases are checked-in tiny examples and must not carry
     pinned-external metadata.
   - `pinned_external` cases must carry `external_repo`, `external_command`, a
     40-hex `external_commit`, an existing `external_patch`,
     `runtime_budget_seconds`, and `artifact_budget_bytes`.
   - Missing or unknown tiers fail the gate. A future real-repo case may not
     enter the corpus as an unbounded or branch-floating claim.
2b. Cases may declare typed semantic assertions under `assertions`. Unknown
    assertion types fail closed. Legacy fields such as
    `must_remain_non_promoted` and `expected_promoted` remain compatibility
    aliases, but the canonical corpus shape is the typed assertion list.
    Supported assertion types are:
    - `must_promote`
    - `must_not_promote`
    - `must_report_clean`
    - `must_not_report_clean`
    - `must_disclose_scope`
    - `must_disclose_no_scope`
    - `must_not_disclose_no_scope`
    - `must_disclose_unanalyzed_working_tree`
    - `must_not_disclose_unanalyzed_working_tree`
    - `must_emit_limitation` with `expected_limit_kind`
    - `must_not_emit_limitation`
    - `must_have_verify_command`
    - `must_not_have_verify_command`
    - `must_have_receipt_command`
    - `must_not_have_receipt_command`
    - `must_emit_repair_packet`
    - `must_not_emit_repair_packet`
    - `must_disclose_repair_packet_detail`
    - `expected_repair_packet_detail` with `canonical_gap_id`, `source_file`,
      `source_line`, `target_test`, `assertion_shape`, `authority_boundary`,
      `repair_kind`, `verify_command`, `receipt_command`,
      `allowed_edit_surface`, and `forbidden_files`
    - `must_not_have_contradictory_packet_messaging`
    - `expected_oracle` with `kind` and `strength`
    - `expected_class` with `class`
    - `maximum_class` with `class`
    - `expected_completeness` with `completeness`
    - `expected_changed_rust_files` with `count`
    - `must_disclose_witness`
    - `must_disclose_limitation_detail`
    - `expected_limitation_detail` with `last_established_edge`,
      `first_unresolved_edge`, and `non_claim`
    - `expected_limitation_route` with `route`
    - `must_not_claim_no_tests_found`
    - `must_see_changed_file` with `path`
   Every `must_not_promote` charter must also declare
   `must_not_report_clean`. Classification ceilings alone are vacuous when a
   re-blessed artifact has no findings; the independent non-clean assertion
   keeps disappearance of the governed subject fail-closed.
3. `must_remain_non_promoted` cases: asserts NO finding's `classification` is
   `exposed`. Also checks that no finding exceeds `expected_max_class` on the
   severity ordering `exposed > weakly_exposed > reachable_unrevealed/no_static_path > *_unknown`.
4. `expected_promoted` (control) cases: asserts at least one finding's
   `classification` is `exposed` (must-not-over-correct guard).
4b. `must_not_report_clean` cases (additive, first-run trust): asserts the
   byte-pinned golden still carries a non-clean signal: findings, a no-scope
   disclosure, an unanalyzed-worktree disclosure, preview-language advisory,
   `limitations[]`, or a named `static_limit_kind`. This guards against a
   re-bless that makes a known unresolved edge or incomplete scope disappear
   into a clean-looking empty result.
4c. `must_emit_limitation` cases (additive, RIPR-SPEC-0114/0115; surfaces
   extended by #3636): asserts the expected limitation kind was emitted on a
   supported surface — at least one finding carries
   `static_limit_kind == expected_limit_kind`, or the check output's
   `test_harnesses[].limitations[].code` emits it (harness-limitation kinds
   such as `registration_unreachable` live only in the harness projection).
   `must_not_emit_limitation` symmetrically forbids non-empty harness
   projections. This is
   an independent assertion (a case may combine it with `must_remain_non_promoted`)
   and guards against a re-bless that silently drops a named limitation back to a
   bare class — e.g. dropping `rust_transitive_reach_unresolved` so a transitive
   reach reads as genuinely untested. A missing or empty `expected_limit_kind`
   is itself a violation.
   Current integration/public-API path cases use
   `rust_integration_public_api_path_unresolved`; generic non-integration helper
   paths continue to use `rust_transitive_reach_unresolved`.
4d. `must_disclose_witness` cases (additive, RIPR-SPEC-0115): asserts at least one
   finding's `evidence` contains the concrete transitive-reach *witness* pointer
   (prose beginning `For example, the test `), and for fixture-backed cases asserts
   `expected/human.txt` exists and surfaces the same exact witness line under
   `Where to look`.
   Independent of the other assertions; guards against a re-bless that drops the
   witness back to the bare 0114 limitation message or lets JSON and human output
   drift apart, regressing the first-run-trust UX.
4d1. `must_disclose_limitation_detail` cases (additive, RIPR-SPEC-0114/0117):
    asserts every finding with `static_limit_kind` carries evidence lines naming
    the last established edge, first unresolved edge, analyzer route, and
    non-claim. For fixture-backed cases, the assertion also requires
    `expected/human.txt` to project the same exact details under `Limitation
    detail`. This guards against a named limitation that tells the user a path is
    blocked but hides where the analyzer stopped or what route would unlock it.
4d1a. `expected_limitation_detail` cases (additive, named-limitation honesty):
    asserts every static limitation in the case carries the exact last
    established edge, first unresolved edge, and non-claim values named by the
    assertion. This makes the user-facing limitation boundary executable corpus
    data instead of accepting any non-empty edge prose.
4d1b. `expected_limitation_route` cases (additive, route-quality seed): asserts
    every static limitation in the case carries the exact `limitation_analyzer_route`
    value named by the assertion. This makes the analyzer backlog route executable
    corpus data instead of unconstrained prose.
4d2. `must_not_claim_no_tests_found` cases (additive, RIPR-SPEC-0115): asserts the
    JSON report does not contain the string `No tests were found` anywhere. For
    fixture-backed cases, the assertion also checks `expected/human.txt`. This is
    for witnessed limitation cases: once RIPR names a candidate test to inspect,
    the same artifact must not also claim no tests were found. It may still say no
    *statically reachable* test path was confirmed.
4e. `must_disclose_scope` cases (additive, first-run trust): asserts the
    byte-pinned `expected/check.json` still carries the report-level scope header
    (`schema_version`, `tool`, `mode`, `root`, and `base`). This guards against a
    re-bless that keeps findings or named limitations but removes the
    machine-readable statement of the analyzed scope.
4f. `must_not_emit_repair_packet` cases (additive, delegation honesty): asserts
    the byte-pinned `expected/check.json` does not contain
    `repair_packet_ready=true` anywhere in the report. This guards against a
    named limitation or non-promoted case becoming delegatable without the full
    repair-packet contract.
4g. `must_not_have_verify_command` and `must_not_have_receipt_command` cases
    (additive, command honesty): assert the report does not contain non-empty
    `verify_command` or `receipt_command` fields. Named limitations use these
    with `must_not_emit_repair_packet` so a re-bless cannot silently turn an
    unresolved edge into a command-shaped handoff while still claiming
    limitation status. `must_have_verify_command` and
    `must_have_receipt_command` are the positive packet-contract controls.
    Fixture-backed cases also assert human output agrees with the command
    contract: positive command assertions must project the same command, while
    negative command assertions must not show a concrete `verify:` or `receipt:`
    command line. Unavailable placeholders such as
    `receipt: unavailable_until_python_gap_ledger` remain status disclosures,
    not receipt commands.
4h. `must_disclose_repair_packet_detail` cases (additive, packet-contract
    honesty): assert a packet-ready report carries the JSON handoff fields users
    and agents need: canonical gap, source/target, edit cage, repair shape,
    verify/receipt commands, must-not-change constraints, and raw evidence refs.
    Fixture-backed cases also assert the human output surfaces the same repair
    packet section with canonical gap, source, target, edit surface, verify,
    receipt, must-not-change, and authority lines.
4h1. `expected_repair_packet_detail` cases (additive, exact packet handoff):
    assert the packet-ready handoff points at the exact canonical gap, changed
    source line, target test, assertion shape, verify/receipt commands, and edit
    cage declared by the corpus case. Fixture-backed cases also assert human
    output carries those exact values. This guards against a structurally
    complete packet silently drifting to the wrong test, wrong command, or wrong
    edit surface.
4i. `must_not_have_contradictory_packet_messaging` cases (additive, projection
    honesty): assert packet-ready findings do not retain blocked preview evidence
    strings such as `gap_state: advisory`,
    `actionability_category: incomplete_repair_packet`, `why_not_actionable:`,
    `repair_route:`, `missing_actionability_fields:`, or non-empty
    `evidence_needed_to_promote:` lines in the rendered JSON evidence. A
    complete packet must project actionable-reading evidence instead of the
    earlier incomplete-packet explanation. Fixture-backed cases also assert
    human output does not retain blocked packet language such as
    `status: not actionable`, `why not actionable:`, `limitation:`,
    `missing fields:`, or `evidence needed:` in the matching packet-ready
    finding section.
4j. `expected_oracle` cases (additive, dialect semantics): assert every finding
    carries the exact `oracle_kind` and `oracle_strength` named by the case.
    Fixture-backed cases also assert `expected/human.txt` projects the same
    oracle kind and strength. This lets the corpus pin framework- and
    dialect-specific oracle semantics directly, for example TypeScript
    execution-context `t.equal(...)` as `exact_value/strong`, `t.truthy(...)` as
    `smoke_only/smoke`, and unsupported or wrong-receiver `t.*` shapes as
    `unknown/unknown`, without allowing human output to drift from the canonical
    JSON result.
4k. `expected_class` cases (additive, projection honesty): assert every finding
    carries the exact `classification` named by the case. Fixture-backed cases
    also assert `expected/human.txt` projects the same classification under the
    human static-exposure output, with exact class-token matching so `exposed`
    cannot be satisfied by `weakly_exposed`. `maximum_class` remains a JSON
    severity ceiling rather than a human projection assertion.
5. PARITY checks: every pure case must declare exactly one of `source_fixture`
   or `source_report`. A `source_fixture` must exist, have
   `expected/check.json`, and NOT be in the manifest-only denylist (so it stays
   covered by `goldens check`). A `source_report` must exist and parse as JSON.
   Each of {python, typescript, rust} must have ≥1 non-promoted case; rust and
   typescript must each have ≥1 control case.
6. FAIL-CLOSED: missing artifact / missing check.json / a non-promoted case
   showing `exposed` / a control losing `exposed` / a language missing coverage
   → non-zero exit + report under `target/ripr/reports/`.

### Pinned external execution

`cargo xtask check-evidence-promotion-honesty --pinned-external` executes
`tier: pinned_external` cases from the same corpus. Clone and network access are
never part of the default gate: `--clone` is required to create or refresh the
bounded checkout cache under
`target/ripr/evidence-promotion-honesty/checkouts`. Without `--clone`, the
runner may only reuse an existing checkout that already contains the exact
commit.

The runner:

1. Reuses or clones the external repository into the bounded cache.
2. Checks out the exact 40-character commit and cleans the checkout.
3. Verifies and applies the checked-in patch.
4. Builds the current `ripr` binary once.
5. Runs `ripr check --root {checkout} --diff {external_patch} --mode fast --json`.
6. Enforces runtime and artifact-size budgets.
7. Enforces semantic assertions such as non-clean output, scope disclosure,
   named limitation, non-promotion, witness disclosure, no-tests contradiction
   rejection, and no repair packet.
8. Resets and cleans the checkout after the run.

The first pinned external case is
`rust_semver_matches_greater_external_limitation` against `dtolnay/semver` at
commit `2c18cc482244f4bb9cc65003b07426c18a79a190`, with the checked-in patch
`fixtures/evidence-promotion-honesty-corpus/patches/semver-matches-greater.diff`.
It asserts that RIPR sees `src/eval.rs`, does not report a clean empty result,
emits `rust_integration_public_api_path_unresolved`, stays at or below `no_static_path`,
does not emit a repair packet, discloses scope plus a witness, and does not
claim no tests were found after naming that witness.

### Corpus summary envelope

Every `check-evidence-promotion-honesty` run writes:

```text
target/ripr/reports/corpus-summary.json
target/ripr/reports/corpus-summary.md
```

The JSON report uses `schema_version: "0.1"` and `kind: "corpus_summary"`.
Each case carries:

```yaml
id:
language:
tier:
status: pass | fail | not_run
result_kind:
message:
runtime_ms:
artifact_bytes:
external_case:
  repo:
  commit:
  patch:
  command:
  runtime_budget_seconds:
  artifact_budget_bytes:
```

Pinned external cases are `not_run` unless `--pinned-external` selects them.
That state is visible but does not fail the pure PR gate. Pinned external rows
carry their launch metadata even when not run, so the envelope still records the
exact repository, commit, patch, command template, and resource budgets that
would be exercised by the external tier. The detailed pinned-external report
uses the same launch metadata for executed cases. Failures must be classified
into one of:

```text
semantic_failure
golden_drift
setup_failure
network_unavailable
runtime_budget_exceeded
artifact_budget_exceeded
unexpected_limitation
unexpected_promotion
```

This separation is part of the product contract: a clone/fetch/cache/setup
problem must not look like a passing product case or an analyzer semantic
regression, and a golden re-bless drift must not look like external network
unavailability.

### Design: share invariant + corpus, NOT per-language matchers

Each language keeps its own taxonomy and matcher functions. The gate enforces
the OUTPUT property (classification in the golden) over the cross-language
corpus. This is intentional: a single invariant over pinned golden outputs
requires no knowledge of why a language classifies something — only what the
golden says.

### Why this catches a dishonest re-bless

If a developer re-blesses a charter fixture from `weakly_exposed` → `exposed`:

1. `goldens check` passes (binary matches the new golden).
2. `check-evidence-promotion-honesty` reads the same golden and finds
   `classification: exposed` for a `must_remain_non_promoted` case.
3. Gate FAILS with a message naming the charter member and explaining that a
   dishonest re-bless was detected.

Reverting the golden restores gate passage. Adding a new charter member to the
corpus prevents the same regression in future.

## Required Evidence

### Corpus manifest

`fixtures/evidence-promotion-honesty-corpus/corpus.json` -- cross-language
pinned adversarial corpus with pure non-promoted charter members, pure control
cases, scope/packet projection cases, TypeScript execution-context oracle cases,
and pinned external real-repo cases. Rust reach-limitation charter members
additionally assert non-clean output, named limitations, witness disclosure,
report scope disclosure, exact limitation detail, exact analyzer route, and no
repair-packet delegation.

Pure cases are tiny checked-in fixtures with byte-pinned `expected/check.json`
reports or byte-pinned `source_report` JSON artifacts for states not expressible
by the standard fixture runner. Pinned external cases are exact real-repo launch
points and must name the upstream repository, command template, exact 40-hex
commit, checked-in patch, runtime budget, and artifact-size budget before the
gate will accept them.

The corpus summary envelope is the canonical result/failure projection for this
corpus runner. The older `evidence-promotion-honesty.md` and
`evidence-promotion-pinned-external.{json,md}` reports remain detailed
gate-specific artifacts.

### Charter members (must_not_promote)

| id | language | source artifact | vector |
|---|---|---|---|
| py_token_substring | python | python_adversarial_buffer_token | token_substring_coincidence (also `expected_oracle=exact_value/strong`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_mock_call_not_value | python | python_adversarial_mock_call_not_value | mock_call_not_value (also `expected_oracle=mock_expectation/medium`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_dict_sibling_key | python | python_adversarial_dict_field_sibling_key | changed_dict_element_sibling_key_oracle (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_list_sibling_index | python | python_adversarial_list_element_sibling_index | changed_list_element_sibling_index_oracle (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_operator_delta_input_operand | python | python_adversarial_operator_delta_input_operand | operator_only_value_change_input_operand_oracle (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_changed_sink_non_delta_operand | python | python_adversarial_changed_sink_non_delta_operand | changed_sink_token_non_delta_operand (also `expected_class=static_unknown`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_default_value_overridden | python | python_adversarial_default_value_overridden | changed_default_value_explicitly_overridden (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_error_path_untaken_branch | python | python_adversarial_error_path_untaken_branch | changed_exception_type_untaken_branch (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_fstring_length_aggregate | python | python_adversarial_fstring_length_invariant_aggregate | length_invariant_fstring_aggregate_oracle (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| py_local_assignment_operator_input | python | python_adversarial_local_assignment_operator_input | local_assignment_operator_input_oracle (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_broad_tothrow | typescript | typescript_broad_tothrow | cross_family_oracle_seam (also `expected_oracle=broad_error/weak`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_t_truthy_smoke_only | typescript | typescript_t_truthy_oracle | execution_context_truthy_smoke_only (also `expected_oracle=smoke_only/smoke`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_t_wrong_receiver_unknown_oracle | typescript | typescript_t_wrong_receiver_no_oracle | execution_context_wrong_receiver_not_credited (also `expected_oracle=unknown/unknown`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_t_unknown_method_unknown_oracle | typescript | typescript_t_unknown_method_no_oracle | execution_context_unknown_method_not_credited (also `expected_oracle=unknown/unknown`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_negated_t_oracle | typescript | typescript_negated_t_oracle | negated_equality_not_exact_value (also `expected_oracle=relational_check/weak`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, and no repair packet or receipt command) |
| ts_complete_repair_packet_contract | typescript | ts_repair_packet_complete | complete TypeScript repair packet stays weakly_exposed, packet-ready, command-bearing, detail-complete, exact-targeted, and free of blocked packet messaging rather than promoted to exposed |
| rust_weak_error_oracle | rust | weak_error_oracle | non_variant_observing_error_oracle (also `expected_class=weakly_exposed` and `must_not_report_clean`) |
| rust_error_path_sibling_oracle | rust | error_path_sibling_oracle_fake_clean | sibling_oracle_does_not_confirm_error_path (also `expected_class=weakly_exposed` and `must_not_report_clean`) |
| rust_integration_public_api_path_named_limitation | rust | rust_transitive_reach_positive | integration_public_api_path_named_not_silently_clean (also `must_not_report_clean` + `must_disclose_scope` + `must_emit_limitation: rust_integration_public_api_path_unresolved` + `must_not_emit_repair_packet` + no verify/receipt commands + `must_disclose_witness` + `must_disclose_limitation_detail` + `expected_limitation_detail` + `expected_limitation_route: analysis/rust-public-api-transitive-reach` + `must_not_claim_no_tests_found`) |
| rust_integration_public_api_test_helper_chain_named_limitation | rust | rust_transitive_reach_test_helper_chain | test_helper_public_api_path_named_not_silently_clean (also `must_not_report_clean` + `must_disclose_scope` + `must_emit_limitation: rust_integration_public_api_path_unresolved` + `must_not_emit_repair_packet` + no verify/receipt commands + `must_disclose_witness` + `must_disclose_limitation_detail` + `expected_limitation_detail` + `expected_limitation_route: analysis/rust-public-api-transitive-reach` + `must_not_claim_no_tests_found`) |
| rust_macro_reach_named_limitation | rust | rust_macro_reach_limitation | macro_reach_named_not_silently_clean (also `must_not_report_clean` + `must_disclose_scope` + `must_emit_limitation: rust_macro_reach_unresolved` + `must_not_emit_repair_packet` + no verify/receipt commands + `must_disclose_witness` + `must_disclose_limitation_detail` + `expected_limitation_detail` + `expected_limitation_route: analysis/rust-macro-aware-reach` + `must_not_claim_no_tests_found`) |
| rust_macro_wrapped_test_call_named_limitation | rust | rust_macro_wrapped_test_call_limitation | direct_test_macro_call_named_not_silently_clean (also `must_not_report_clean` + `must_disclose_scope` + `must_emit_limitation: rust_macro_wrapped_test_call_unresolved` + `must_not_emit_repair_packet` + no verify/receipt commands + `must_disclose_witness` + `must_disclose_limitation_detail` + `expected_limitation_detail` + `expected_limitation_route: analysis/rust-macro-aware-reach` + `must_not_claim_no_tests_found`) |
| rust_macro_wrapped_assertion_named_limitation | rust | rust_macro_wrapped_assertion_limitation | custom_assertion_macro_named_not_silently_clean (also `must_not_report_clean` + `must_disclose_scope` + `must_emit_limitation: rust_macro_wrapped_assertion_unresolved` + `must_not_emit_repair_packet` + no verify/receipt commands + `must_disclose_witness` + `must_disclose_limitation_detail` + `expected_limitation_detail` + `expected_limitation_route: analysis/rust-macro-assertion-oracle` + `must_not_claim_no_tests_found`) |
| perl_preview_card_advisory_no_repair_packet | perl | reports/perl-preview-advisory-no-packet.json | perl_preview_card_advisory_only (also `expected_class=weakly_exposed`, production-shaped `perl_preview_card.v1`, `must_not_report_clean`, `must_disclose_scope`, no `verify_command`/`receipt_command` delegation fields, no promotion, and no repair packet) |
| scope_committed_diff_changed_rust_file | rust | boundary_gap | committed_diff_changed_file_scope_count (also `must_not_report_clean` + `must_disclose_scope` + `expected_changed_rust_files: 1` + no verify/receipt commands + no repair packet + `expected_class: weakly_exposed`) |
| scope_no_scope_empty_not_clean | rust | reports/scope-no-scope-empty-not-clean.json | empty_result_no_scope_disclosure_not_clean (also `must_disclose_no_scope`) |
| scope_unanalyzed_worktree_empty_not_clean | rust | reports/scope-unanalyzed-worktree-empty-not-clean.json | empty_base_head_dirty_worktree_disclosure_not_clean (also `must_disclose_unanalyzed_working_tree`) |
| scope_worktree_dirty_analyzed_not_excluded | rust | reports/scope-worktree-dirty-analyzed-not-excluded.json | dirty_worktree_explicitly_analyzed_not_reported_excluded (also `must_see_changed_file: src/lib.rs`, `expected_changed_rust_files: 1`, `must_disclose_scope`, `must_not_disclose_no_scope`, `must_not_disclose_unanalyzed_working_tree`, no verify/receipt commands, and no repair packet) |
| scope_limited_empty_not_clean | rust | reports/scope-limited-empty-not-clean.json | empty_limited_scope_not_clean (also `expected_completeness: limited`) |
| ts_same_method_other_class | typescript | typescript_adversarial_same_method_other_class | method_owner_same_name_different_class_identity (also `expected_class=no_static_path`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_imported_function_name_collision | typescript | typescript_reexport_no_false_credit | imported_function_name_collision_barrel_reexport_identity (also `maximum_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_decorator_indirection_limit | typescript | typescript_static_limit_taxonomy | decorator_indirection_named_limitation_not_promoted (also `must_emit_limitation: decorator_indirection`, `maximum_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_dynamic_dispatch_limit | typescript | typescript_static_limit_taxonomy | dynamic_dispatch_named_limitation_not_promoted (also `must_emit_limitation: dynamic_dispatch`, `maximum_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_missing_import_graph_limit | typescript | typescript_static_limit_taxonomy | unresolved_import_missing_import_graph_named_limitation (also `must_emit_limitation: missing_import_graph`, `maximum_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_hoc_wrapped_owner | typescript | typescript_adversarial_hoc_wrapped_owner | higher_order_wrapper_obscures_owner_identity (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_token_substring | typescript | typescript_adversarial_token_substring | token_substring_coincidence (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_render_reaches_unobserved_sink | typescript | typescript_adversarial_render_unobserved_sink | render_test_reaches_not_observes_changed_sink (also `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| ts_owner_identity_after_insertion | typescript | typescript_adversarial_owner_identity_after_insertion | stale_line_owner_identity_after_insertion (also `maximum_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_same_sub_other_package | perl | reports/perl_same_sub_other_package.json | same_sub_name_other_package_package_reference_downgrade (also `expected_oracle=exact_value/strong`, `expected_class=reachable_unrevealed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_import_alias_defining_package | perl | reports/perl_import_alias_defining_package.json | imported_alias_defining_package_mismatch_package_reference_downgrade (also `expected_oracle=exact_value/strong`, `expected_class=reachable_unrevealed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_moose_accessor_indirection | perl | reports/perl_moose_accessor_indirection.json | moose_accessor_generated_symbol_boundary_named_limitation (also `must_emit_limitation: metaprogramming`, `expected_class=static_unknown`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_monkeypatch_symbol_table | perl | reports/perl_monkeypatch_symbol_table.json | monkeypatch_or_symbol_patch_boundary_named_limitation (also `must_emit_limitation: metaprogramming`, `expected_class=static_unknown`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_mocked_module_unrelated_assertion | perl | reports/perl_mocked_module_unrelated_assertion.json | mocked_module_strong_oracle_observes_unrelated_sink (also `expected_oracle=exact_value/strong`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_dynamic_require_eval_dispatch | perl | reports/perl_dynamic_require_eval_dispatch.json | dynamic_require_eval_dispatch_boundary_named_limitation (also `must_emit_limitation: metaprogramming`, `expected_class=static_unknown`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_token_substring | perl | reports/perl_token_substring.json | token_substring_observed_sink_not_aligned (also `expected_oracle=exact_value/strong`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_fixture_setup_no_discrimination | perl | reports/perl_fixture_setup_no_discrimination.json | fixture_harness_reaches_not_discriminates_fixture_setup_downgrade (also `expected_oracle=exact_value/strong`, `expected_class=reachable_unrevealed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |
| perl_direct_owner_advisory_positive | perl | reports/perl_direct_owner_advisory_positive.json | direct_owner_call_relation_fires_advisory_only (also `expected_oracle=exact_value/strong`, `expected_class=weakly_exposed`, `must_not_report_clean`, `must_disclose_scope`, no verify/receipt commands, and no repair packet) |

### Pinned external cases

| id | language | external repo | commit | vector |
|---|---|---|---|---|
| rust_semver_matches_greater_external_limitation | rust | `https://github.com/dtolnay/semver` | `2c18cc482244f4bb9cc65003b07426c18a79a190` | semver public API to internal transitive reach must disclose `rust_integration_public_api_path_unresolved` with exact limitation detail and route `analysis/rust-public-api-transitive-reach`, no verify/receipt commands, not clean or actionable |

### Control cases (must_promote)

| id | language | source artifact |
|---|---|---|
| rust_strong_error_oracle_control | rust | strong_error_oracle |
| rust_unwrap_err_variant_positive_control | rust | unwrap_err_variant_positive |
| ts_strong_oracle_control | typescript | typescript_strong_oracle |
| ts_ava_t_is_exact_value | typescript | ts_runner_detect_ava_devdep (`expected_oracle=exact_value/strong`, `expected_class=exposed`, no repair packet or receipt command) |
| ts_tape_equal_exact_value | typescript | typescript_tape_equal_oracle (`expected_oracle=exact_value/strong`, `expected_class=exposed`, no repair packet or receipt command) |
| ts_dynamic_expected_incomplete_packet | typescript | typescript_dynamic_assertion_unresolved (`expected_oracle=exact_value/strong`, `expected_class=exposed`, no repair packet or receipt command) |
| ts_same_method_owner_identity_positive_control | typescript | typescript_same_method_owner_identity_positive (`expected_oracle=exact_value/strong`, `expected_class=exposed`, no repair packet or receipt command) |
| perl_sink_aligned_positive_control | perl | reports/perl_sink_aligned_positive_control.json (`expected_oracle=exact_value/strong`, `expected_class=exposed`, no repair packet or receipt command) |
| scope_clean_complete_empty_may_be_clean | rust | reports/scope-clean-complete-empty-may-be-clean.json (`expected_changed_rust_files: 0`) |

### TypeScript/JavaScript and Perl family coverage (issue #1983)

The corpus covers the cross-language false-promotion families shared by the
TypeScript/JavaScript and Perl preview tiers. The invariant and the corpus are
shared; the per-language matchers stay separate (different taxonomies,
different edge policies).

TypeScript/JavaScript families and their charter coverage:

| family | corpus case | expected state |
|---|---|---|
| same method name on a different class/receiver | `ts_same_method_other_class` | `no_static_path` (receiver identity required) |
| imported function name collision across modules | `ts_imported_function_name_collision` | `maximum_class=weakly_exposed` (barrel re-export resolves to the other module) |
| decorator wrapper obscuring the owner | `ts_decorator_indirection_limit` | `must_emit_limitation: decorator_indirection`, not promoted |
| higher-order wrapper obscuring the owner | `ts_hoc_wrapped_owner` | `weakly_exposed` (wrapper boundary stays opaque) |
| token substring collision | `ts_token_substring` | `weakly_exposed` (call-boundary match, heuristic-only link) |
| component/render test reaches but does not observe the changed sink | `ts_render_reaches_unobserved_sink` | `weakly_exposed` (heuristic link cannot borrow the strong assertion) |
| stale line/owner identity after an insertion | `ts_owner_identity_after_insertion` | `maximum_class=weakly_exposed` (post-insertion owner identity; sibling oracle not borrowed) |
| dynamic dispatch / unresolved import | `ts_dynamic_dispatch_limit`, `ts_missing_import_graph_limit` | named limitations, not promoted |

Perl families and their charter coverage. Perl cases are `source_report`
cases: the standard fixture runner builds `ripr` with default features
(rust/typescript/python), and the Perl adapter is a feature-gated
(`lang-perl`) fact-packet consumer, so Perl findings are not expressible as
`goldens check` fixtures. Each byte-pinned report under
`fixtures/evidence-promotion-honesty-corpus/reports/` is real consumer output,
regenerated from the checked-in hand-authored `ripr-perl-facts-v1` packet of
the same name under
`fixtures/evidence-promotion-honesty-corpus/perl-packets/` with:

```bash
cargo build -p ripr --features lang-perl
ripr check --perl-facts fixtures/evidence-promotion-honesty-corpus/perl-packets/<case>.json \
  --base origin/main --json \
  > fixtures/evidence-promotion-honesty-corpus/reports/<case>.json
```

The packet is the input (the tempting wrong relation or boundary is encoded
there); the report is the byte-pinned consumer output the gate enforces.
Packet fingerprints follow the `recompute_packet_fingerprint` recipe in
`crates/ripr/src/analysis/language/perl/mod.rs`; the consumer rejects a stale
or tampered fingerprint at ingestion.

| family | corpus case | expected state |
|---|---|---|
| same sub/method name in another package | `perl_same_sub_other_package` | `reachable_unrevealed` (package-reference downgrade) |
| imported alias vs defining package | `perl_import_alias_defining_package` | `reachable_unrevealed` (package-reference downgrade) |
| Moose/accessor indirection | `perl_moose_accessor_indirection` | `static_unknown` + `must_emit_limitation: metaprogramming` (generated-symbol boundary) |
| monkey patch / symbol-table mutation | `perl_monkeypatch_symbol_table` | `static_unknown` + `must_emit_limitation: metaprogramming` |
| mocked module / unrelated strong assertion | `perl_mocked_module_unrelated_assertion` | `weakly_exposed` (observed sink does not align to the changed observable) |
| dynamic require/eval dispatch | `perl_dynamic_require_eval_dispatch` | `static_unknown` + `must_emit_limitation: metaprogramming` |
| token substring collision | `perl_token_substring` | `weakly_exposed` (sink alignment requires exact equality) |
| fixture/harness test reaches but does not discriminate | `perl_fixture_setup_no_discrimination` | `reachable_unrevealed` (fixture-setup relations are advisory-only) |

Positive controls (same-entity relations must still fire; preview/advisory
per support policy, no gate, badge, or RIPR Zero role):

- `ts_same_method_owner_identity_positive_control` — true receiver identity
  (`new TokenValidator(...)` + exact-value assertion) keeps `exposed`.
- `perl_sink_aligned_positive_control` — direct owner call with a strong
  oracle whose observed sink exactly aligns to the changed observable keeps
  `exposed` (already-observed).
- `perl_direct_owner_advisory_positive` — direct owner call with a strong
  owner-targeted oracle but no observed-sink fact stays `weakly_exposed`,
  advisory only.

#### Resolved live finding: TypeScript owner-module mock over-credit

**Resolved by #2269 (PR #2272).** The Function/ArrowFunction owner path in
`owner_call_relation` now applies the same `test_mocks_owner_module` guard
the Method/ClassMethod/ModuleFunction paths already applied, so no owner-call
relation is credited when the test mocks the changed owner's own module; the
finding lands at `weakly_exposed` with the `mocked_module` limitation
disclosed and `repair_packet_ready: false`. The graduated corpus case
`ts_mocked_owner_module_unrelated_assertion` (fixture
`fixtures/typescript_adversarial_owner_module_mock`) is now a charter member
with `must_not_promote` and `maximum_class: weakly_exposed`.

The issue #1983 family "mocked module or dependency with unrelated assertion"
had one uncovered TypeScript arm that could not be a charter member while the
analyzer over-credited it. Historical reproduction (base `97acf1a4`):

```bash
# workspace: src/discount.ts exports applyDiscount; tests/discount.test.ts
# adds `jest.mock('../src/discount')`, stubs the mock, and asserts
# `expect(result).toBe(90)`.
ripr check --root <workspace> --diff <predicate-change.diff> --mode fast --json
```

Actual: `classification: exposed` with `static_limit_kind: mocked_module`
(`relation_reason: direct_owner_call`, `oracle_kind: exact_value/strong`).
Expected: at most `weakly_exposed` — when the mocked module IS the changed
owner's module, the owner call executes the mock, not the changed code, so a
strong oracle on the stubbed return value cannot observe the changed sink.
The named `typescript_mock_only_observer` limitation is emitted and
`repair_packet_ready` stays `false`, but the exposure class over-credits:
`receiver_owner_call_relation` / `class_method_owner_call_relation` apply the
`test_mocks_owner_module` guard for Method/ClassMethod/ModuleFunction owners,
while the Function/ArrowFunction owner path in `owner_call_relation`
(`crates/ripr/src/analysis/language/typescript/related_tests.rs`) credits
`DirectOwnerCall` without that guard. This is a candidate false-`exposed`
finding for a follow-up analyzer PR (production matcher change is out of
scope here); no golden was blessed for this state and no corpus case was
added, because a `must_not_promote` entry would correctly fail the gate until
the matcher is fixed. The dependency-mock arm (the test mocks a *different*
module than the owner) is already pinned by
`fixtures/typescript_mocked_module_limit`, and the equivalent Perl family
(`perl_mocked_module_unrelated_assertion`) stays `weakly_exposed`.

### Validation by `check-fixture-contracts`

`validate_evidence_promotion_honesty_corpus` is called from
`check_fixture_contracts()` to verify the corpus is structurally valid (no
duplicate ids, pure source artifacts exist, fixture-backed cases have
`expected/check.json`, fixture-backed cases are not manifest-only, parity
language coverage). This runs in CI as part of the existing
`check-fixture-contracts` gate. The same validator rejects missing or unknown
case tiers, rejects pinned-external metadata on `pure` cases, and rejects
`pinned_external` cases that lack exact repo/command/commit/patch and budget
metadata. Pinned external cases do not need checked-in golden output; their
semantic assertions are enforced by opt-in execution against the current binary.

## Non-Goals

- Does NOT unify per-language matcher functions; each language keeps its own
  taxonomy.
- Does NOT run mutants. The default pure gate reads byte-pinned goldens; the
  opt-in pinned external lane runs the current RIPR binary against exact
  real-repo launch points.
- Does NOT re-classify any finding.
- Does NOT bump schema_version, crate version, or touch release workflows.
- Does NOT replace `goldens check`; composes with it.
- Static-language clean: gate output uses the conservative static vocabulary
  (`exposed`, `weakly_exposed`, `reachable_unrevealed`, `no_static_path`,
  `infection_unknown`, `propagation_unknown`, `static_unknown`) — all allowed
  vocabulary. The `*_unknown` classes appear only as named-limitation states
  (for example alongside `must_emit_limitation`), never as promoted classes.

## Acceptance Examples

### Gate passes (all charter members at expected class)

```
pass: all charter members at expected class; no clean-guard case lost its
findings; scope-guard cases kept report scope headers; no promoted case carries
exposed; all controls retain exposed
```

### Gate fails (dishonest re-bless detected)

```
FAIL: evidence promotion honesty case `py_token_substring`
(fixture `fixtures/python_adversarial_buffer_token`):
finding `probe:src_pack.py:python_preview:cfc61771` has classification `exposed`
but `must_remain_non_promoted` is true — dishonest re-bless detected;
revert the golden or remove this charter member
```

### Gate fails (control lost exposed)

```
FAIL: evidence promotion honesty control case `rust_strong_error_oracle_control`
(fixture `fixtures/strong_error_oracle`):
`expected_promoted` is true but no finding has classification `exposed` —
the gate has over-corrected or the fixture needs re-blessing
```

## Test Mapping

| Test | Spec control |
|---|---|
| `cargo run -p xtask -- check-evidence-promotion-honesty` | End-to-end gate pass |
| Flip charter golden to `exposed` → gate fails naming it | Dishonest re-bless proof |
| Flip control golden to `weakly_exposed` → gate fails naming it | Over-correct guard proof |
| Flip named-limitation golden to zero findings -> gate fails naming `must_not_report_clean` | False-clean re-bless proof |
| Remove `schema_version`/`tool`/`mode`/`root`/`base` from a scope-guard golden -> gate fails naming `must_disclose_scope` | Scope-disclosure re-bless proof |
| Set `repair_packet_ready=true` in a named-limitation golden -> gate fails naming `must_not_emit_repair_packet` | False-delegation re-bless proof |
| Omit `tier` or use an unknown value -> gate fails naming the case | Corpus tier contract |
| Mark `tier: pinned_external` without repo/command/commit/patch/budgets -> gate fails naming missing metadata | Real-repo pinning contract |
| Complete `tier: pinned_external` metadata with an exact repo, command template, 40-hex commit, existing patch, and positive budgets -> validator accepts the case | External corpus contract |
| `cargo xtask check-evidence-promotion-honesty --pinned-external --clone --case rust_semver_matches_greater_external_limitation` | First real-repo launch point executes and enforces semver limitation expectations |
| `target/ripr/reports/corpus-summary.{json,md}` exists after the gate | Corpus result envelope projection |
| `evidence_promotion_corpus_summary_report_writes_pure_and_not_run_external_cases` | Pure run reports passing pure cases and visible not-run pinned external cases with exact launch metadata |
| `evidence_promotion_pinned_external_report_projects_launch_metadata` | Pinned-external detail report projects repo, commit, patch, command, and budget metadata into JSON and Markdown |
| `evidence_promotion_corpus_summary_reports_pinned_external_setup_failure` | Malformed pinned-external setup metadata still writes the corpus summary with `setup_failure` instead of exiting before the envelope exists |
| `evidence_promotion_corpus_summary_classifies_failure_kinds` | Summary envelope distinguishes golden drift, setup, budget, unexpected-limitation, and unexpected-promotion failures |
| `evidence_promotion_honesty_pass_report_names_clean_guard` | Pass report names the false-clean guard invariant |
| `evidence_promotion_honesty_rejects_duplicate_keys_in_case_object` | Corpus load fails closed on duplicate object keys (spliced case pin loss) |
| `evidence_promotion_honesty_rejects_missing_unknown_and_impure_tiers` | Validator rejects missing/unknown tiers and external metadata on pure cases |
| `evidence_promotion_honesty_rejects_incomplete_pinned_external_tier` | Validator rejects branch-floating or budgetless external cases |
| `evidence_promotion_honesty_accepts_complete_pinned_external_tier` | Validator accepts complete pinned external metadata |
| `evidence_promotion_honesty_accepts_typed_assertion_vocabulary` | Validator accepts typed semantic assertions as the canonical corpus contract |
| `evidence_promotion_honesty_rejects_unknown_assertion_type` | Validator fails closed on an unknown typed assertion |
| `evidence_promotion_semantic_assertions_reject_projection_drift` | Shared assertion evaluator rejects verify-command, receipt-command, packet, limitation, and completeness drift |
| `evidence_promotion_semantic_assertions_accept_expected_changed_rust_files` | Shared assertion evaluator accepts an exact changed Rust file count and rejects missing count data |
| `evidence_promotion_semantic_assertions_reject_human_missing_verify_command_projection` | Shared assertion evaluator rejects fixture human output that hides a JSON-backed verify command |
| `evidence_promotion_semantic_assertions_reject_human_invented_verify_command` | Shared assertion evaluator rejects fixture human output that invents a verify command when JSON has none |
| `evidence_promotion_semantic_assertions_reject_human_invented_receipt_command` | Shared assertion evaluator rejects fixture human output that invents a receipt command when JSON has none |
| `evidence_promotion_semantic_assertions_accept_unavailable_human_receipt_status` | Shared assertion evaluator treats an unavailable receipt status as a non-command disclosure |
| `evidence_promotion_semantic_assertions_reject_missing_receipt_command` | Shared assertion evaluator rejects a packet contract that lacks a receipt command |
| `evidence_promotion_semantic_assertions_reject_missing_repair_packet_detail` | Shared assertion evaluator rejects a packet-ready report missing target/evidence detail |
| `evidence_promotion_semantic_assertions_reject_wrong_repair_packet_detail` | Shared assertion evaluator rejects a packet-ready report whose target test or verify command drifts from the corpus contract |
| `evidence_promotion_semantic_assertions_reject_human_missing_repair_packet_detail` | Shared assertion evaluator rejects a fixture human golden that drops repair-packet handoff detail |
| `evidence_promotion_semantic_assertions_reject_human_contradictory_packet_messaging` | Shared assertion evaluator rejects fixture human output that keeps blocked/not-actionable packet messaging for a packet-ready finding |
| `evidence_promotion_semantic_assertions_accept_human_complete_packet_messaging` | Shared assertion evaluator accepts fixture human output that projects complete packet messaging without blocked/not-actionable language |
| `evidence_promotion_semantic_assertions_accept_human_mixed_packet_and_blocked_messaging` | Shared assertion evaluator accepts a mixed fixture whose packet-ready section is clean while a separate non-actionable finding section remains blocked |
| `evidence_promotion_semantic_assertions_reject_oracle_drift` | Shared assertion evaluator rejects a report whose oracle kind or strength drifts from `expected_oracle` |
| `evidence_promotion_semantic_assertions_accept_human_oracle_projection` | Shared assertion evaluator accepts fixture human output that projects the expected oracle kind and strength |
| `evidence_promotion_semantic_assertions_reject_human_missing_oracle_projection` | Shared assertion evaluator rejects a fixture human golden that drops oracle kind or strength projection |
| `evidence_promotion_semantic_assertions_reject_missing_human_oracle_golden` | Shared assertion evaluator rejects a fixture-backed oracle assertion with no `expected/human.txt` |
| `evidence_promotion_human_oracle_line_matches_normalized_projection` | Shared assertion evaluator accepts normalized human oracle projection formats without treating kind-only prose as strength evidence |
| `evidence_promotion_semantic_assertions_reject_human_missing_class_projection` | Shared assertion evaluator rejects a fixture human golden that drops the expected exposure classification |
| `evidence_promotion_semantic_assertions_reject_missing_human_class_golden` | Shared assertion evaluator rejects a fixture-backed class assertion with no `expected/human.txt` |
| `evidence_promotion_human_class_line_matches_exact_class_token` | Shared assertion evaluator uses exact class-token matching so `exposed` does not match `weakly_exposed` |
| `evidence_promotion_semantic_assertions_reject_no_tests_claim_with_witness` | Shared assertion evaluator rejects a witnessed limitation that still claims `No tests were found` |
| `evidence_promotion_semantic_assertions_reject_human_missing_witness_projection` | Shared assertion evaluator rejects a fixture human golden that drops the witnessed `Where to look` projection |
| `evidence_promotion_semantic_assertions_reject_human_mismatched_witness_projection` | Shared assertion evaluator rejects a fixture human golden that keeps a stale witness line |
| `evidence_promotion_semantic_assertions_reject_missing_human_witness_golden` | Shared assertion evaluator rejects a fixture-backed witnessed case with no `expected/human.txt` |
| `evidence_promotion_semantic_assertions_reject_human_no_tests_claim_with_witness` | Shared assertion evaluator rejects a fixture human golden that still claims `No tests were found` |
| `evidence_promotion_semantic_assertions_reject_missing_limitation_detail` | Shared assertion evaluator rejects a static limitation that omits last-edge, unresolved-edge, route, or non-claim evidence |
| `evidence_promotion_semantic_assertions_reject_human_missing_limitation_detail` | Shared assertion evaluator rejects a fixture human golden that drops the `Limitation detail` projection |
| `evidence_promotion_semantic_assertions_accept_limitation_detail_projection` | Shared assertion evaluator accepts matching JSON evidence and human limitation detail |
| `evidence_promotion_semantic_assertions_reject_wrong_limitation_detail` | Shared assertion evaluator rejects a static limitation whose last edge, unresolved edge, or non-claim does not match the expected corpus detail |
| `evidence_promotion_semantic_assertions_reject_wrong_limitation_route` | Shared assertion evaluator rejects a static limitation whose analyzer route does not match the expected corpus route |
| `evidence_promotion_semantic_assertions_accept_scope_limited_empty_results` | Shared assertion evaluator treats no-scope and unanalyzed-worktree disclosures as non-clean empty results |
| `evidence_promotion_semantic_assertions_reject_false_unanalyzed_worktree_disclosure` | Shared assertion evaluator rejects reports that claim an explicitly analyzed working-tree draft was excluded |
| `evidence_promotion_semantic_assertions_reject_false_no_scope_disclosure` | Shared assertion evaluator rejects reports that claim an explicit analysis scope and a no-scope disclosure at once |
| `evidence_promotion_semantic_assertions_reject_bare_empty_false_clean` | Shared assertion evaluator rejects a bare empty result for `must_not_report_clean` |
| `evidence_promotion_pinned_external_semantics_accept_semver_limitation_shape` | Semantic assertion accepts the current semver limitation shape |
| `evidence_promotion_pinned_external_semantics_reject_false_clean_and_packet` | Semantic assertion rejects false clean, false promotion, missing witness, and false packet readiness |
| `evidence_promotion_honesty_rejects_missing_scope_for_scope_guard_case` | Validator rejects scope-guard cases without a report scope header |
| `evidence_promotion_honesty_rejects_packet_ready_limitation_case` | Validator rejects packet-ready delegation for opted-in limitation cases |
| `cargo xtask check-fixture-contracts` | Corpus structural validity |
| `cargo xtask check-command-catalog` | Command registration |
| `cargo xtask check-workflows` | CI registration |

## Implementation Mapping

| Component | Location |
|---|---|
| Corpus manifest | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |
| First external patch | `fixtures/evidence-promotion-honesty-corpus/patches/semver-matches-greater.diff` |
| Gate implementation | `xtask/src/main.rs::check_evidence_promotion_honesty` |
| Corpus summary writer | `xtask/src/main.rs::write_evidence_promotion_corpus_summary_report` |
| Pinned external runner | `xtask/src/main.rs::run_evidence_promotion_pinned_external_cases` |
| Corpus validator | `xtask/src/main.rs::validate_evidence_promotion_honesty_corpus_at` |
| Manifest-only denylist | `xtask/src/main.rs::is_manifest_only_fixture_dir` |
| Command enum | `xtask/src/command.rs::XtaskCommand::CheckEvidencePromotionHonesty` |
| Command catalog | `xtask/src/command.rs::command_catalog` |
| Dispatch | `xtask/src/dispatch.rs` |
| CI routed | `.github/workflows/routed-rust.yml` |
| CI fast | `.github/workflows/ci.yml` |

## Metrics

- `evidence_promotion_honesty_charter_members`: count of `must_not_promote` cases in corpus
- `evidence_promotion_honesty_control_cases`: count of `must_promote` cases in corpus
