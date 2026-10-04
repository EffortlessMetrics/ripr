# Golden Output Changes

## Pending

Reason:
Initial golden for no-package TypeScript discovery: typescript_package_root_unresolved limitation emitted (fail-closed), no package_root guessed, as required by RIPR-SPEC-0085 PR 2

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR 3: fail-closed typescript_test_runner_unresolved when no package.json found (package_root=None → no verify command)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR5: additive oracle metadata lines (observed/expected/confidence/evidence_ref)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0076: raise exposed default severity from info to warning so the strongest finding class is not quieter than weaker classes

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (1)

Reason:
RIPR-SPEC-0027: predicate boundary witness; no strong assertion observes an owner call at the changed comparison boundary, so the predicate downgrades from exposed to weakly_exposed
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (2)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (3)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (4)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (5)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (6)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value
RIPR-SPEC-0122: Hidden remainder names omitted preview-language identity (#4395); formatting-only, no class or JSON change.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (7)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (8)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (9)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/human.txt`

## Pending — ts_pkg_discovery_no_package (10)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`