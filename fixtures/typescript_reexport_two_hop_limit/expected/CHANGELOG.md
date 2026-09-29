# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0095 two-hop limitation: test imports via index.ts->errors.ts->util.ts (two hops); ripr only resolves one hop (fail-closed), so the test honestly stays uncredited (no_static_path, 0 related tests); disclosed limitation

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests; new fixtures: typescript_reexport_single_hop (credit), typescript_reexport_no_false_credit (no-false-credit control), typescript_reexport_two_hop_limit (two-hop control); initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: wrap long human advisory prose at a fixed 100-column boundary for #2784

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (3)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (4)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (5)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (6)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (7)

Reason:
RIPR-SPEC-0095 bounded re-export chains: the two-hop named chain (index.ts to errors.ts to util.ts) is within MAX_REEXPORT_HOPS (4) and ends at the owner's own export, so the test is credited (re_export_chain_followed, medium) and the finding flips from no_static_path to exposed; test comment and title drop the single-hop limitation wording

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (8)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity (#4429, merged from main)

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_two_hop_limit (9)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
