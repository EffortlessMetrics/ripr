# Golden Output Changes

## Pending

Reason:
Campaign 31 E1: establish CPAN-style three-outcome alpha fixture

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

Reason:
RIPR-SPEC-0082 Perl preview-language disclosure: emit the detected Perl file as an explicit advisory without claiming analysis.

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — perl_cpan_alpha (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — perl_cpan_alpha (4)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

Reason:
RIPR-SPEC-0082: an uncompiled Perl adapter names its real prerequisites (lang-perl build plus the unpublished perl-ripr-facts exporter) instead of a ripr.toml edit this build rejects

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — perl_cpan_alpha (3)

Reason:
RIPR-SPEC-0082 follow-through: the why line must use the not-compiled wording from the unavailable-adapter recovery story (ripr.toml [languages] alone cannot enable Perl), matching the branch's intended check.json flip

Command:
`cargo xtask goldens bless perl_cpan_alpha --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
