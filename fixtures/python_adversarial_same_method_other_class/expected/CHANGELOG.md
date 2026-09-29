# Golden Output Changes

## Pending

Reason:
false-exposed guard: same-named method on a different class stays weakly_exposed/orthogonal — bare method-name requires owner-class identity (analysis/python-false-exposed-attribute-call-owner-name)

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
graduate 4 python adversarial fixtures into honesty corpus (RIPR-SPEC-0108, #1945): add expected/human-full.txt required by the evidence-promotion-honesty gate

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (2)

Reason:
RIPR-SPEC-0108: bind discriminator narration to final verdict (#3670)

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (3)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (4)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (5)

Reason:
RIPR-SPEC-0122: #4216 row 1, the Python preview_limited safe next action names why no ripr command routes the finding instead of asking for repair-packet fields the operator cannot complete

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (6)

Reason:
RIPR-SPEC-0028 (#4216 row 6): Python missing discriminator no longer restates the changed return expression; non-literal return names <expected value>

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (7)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (8)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_adversarial_same_method_other_class (9)

Reason:
RIPR-SPEC-0122: Python pytest verify commands now emit python -m pytest so a flat-layout package imports from the repository root; wording only, no exposure class change

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
