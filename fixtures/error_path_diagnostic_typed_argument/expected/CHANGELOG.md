# Golden Output Changes

## Pending — error_path_diagnostic_typed_argument (1)

Reason:
RIPR-SPEC-0108: diagnostics cannot confirm ErrorPath observation (#4748); eight isolated runtime-tested controls

Command:
`cargo xtask goldens bless error_path_diagnostic_typed_argument --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

2026-10-02: refresh only the analysis input identity after trimming the trailing
unchanged blank context line from diff.patch (hunk7→6). The old/new SHA-256
values exactly match the producer's raw-diff hashing; all other JSON fields and
human outputs are unchanged. Full golden and independent honesty checks rerun.
