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
