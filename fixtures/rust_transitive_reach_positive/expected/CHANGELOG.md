# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0114: pin initial transitive-reach positive fixture golden output (limitation fires: static_limit_kind=rust_transitive_reach_unresolved, class stays no_static_path)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "RIPR-SPEC-0114: initial transitive-reach positive fixture golden"`

Updated:
- `expected/check.json`
- `expected/human.txt`
## Pending

Reason:
initial bless: fixture runner uses mode=fast with relative paths; behavioral correctness verified (no_static_path + rust_transitive_reach_unresolved fires)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0115: transitive-reach limitation now names the witnessing test (file:line) and entry symbol in evidence; class and static_limit_kind unchanged (message-only drift)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0115: human output now surfaces the transitive-reach witness under a 'Where to look' section (file:line + entry symbol); JSON evidence single-sourced via shared prefix

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#1162 explain enhancement: human output now shows the named static limitation + plain-English meaning (additive 'Static limitation' section, message-only)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0115: witnessed transitive limitation no longer claims no tests were found

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
rust limitation evidence now surfaces last edge, unresolved edge, analyzer route, and non-claim

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
structured-static-limitation-detail-json

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`

## Pending

Reason:
align transitive-reach limitation depth text with RIPR-SPEC-0114

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
name integration public API reach limitation

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0001: wrap long human advisory prose at a fixed 100-column boundary for #2784

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0152: hidden-count denominator keeps non-current findings; mutation candidates and review-comment authority require candidate_current

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0159: the outer->inner chain is genuinely resolvable under the typed transfer (unique callee, positional argument binding, workspace-complete index), so the probe relates its tests and promotes past no_static_path; the 0114 lexical walk stays fail-closed

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0159: a second function named inner makes the callee non-unique, so the typed transfer refuses this chain and the RIPR-SPEC-0114 lexical-walk limitation stays the pinned outcome (corpus case preserved)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (3)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (4)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (5)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (6)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_transitive_reach_positive (7)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/human.txt`
