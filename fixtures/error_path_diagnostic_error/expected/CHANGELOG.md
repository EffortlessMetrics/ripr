# Golden Output Changes

## Pending — error_path_diagnostic_error (1)

Reason:
RIPR-SPEC-0108: diagnostics cannot confirm ErrorPath observation (#4748); eight isolated runtime-tested controls

Command:
`cargo xtask goldens bless error_path_diagnostic_error --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

2026-10-02: refresh only the analysis input identity after trimming the trailing
unchanged blank context line from diff.patch (hunk7→6). The old/new SHA-256
values exactly match the producer's raw-diff hashing; all other JSON fields and
human outputs are unchanged. Full golden and independent honesty checks rerun.

## Pending — error_path_diagnostic_error (2)

Reason:
RIPR-SPEC-0122: check names the one-step ripr agent stub route for Rust value gaps (#5355)

Command:
`cargo xtask goldens bless error_path_diagnostic_error --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — error_path_diagnostic_error (3)

Reason:
RIPR-SPEC-0231: related tests ripr examined stay listed and each names why it misses the change (#5344); verdicts unchanged

Command:
`cargo xtask goldens bless error_path_diagnostic_error --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_path_diagnostic_error (4)

Reason:
RIPR-SPEC-0231: a matched related test keeps its oracle kind and strength in full output and adds why it still misses; verdicts unchanged

Command:
`cargo xtask goldens bless error_path_diagnostic_error --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
