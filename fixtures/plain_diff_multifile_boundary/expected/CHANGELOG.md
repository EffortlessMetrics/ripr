# Golden Output Changes

## Pending

Reason:
new fixture for RANK-2 fix: plain unified diff with two file sections now correctly produces changed_rust_files=2 with probes on src/a.rs and src/b.rs, no phantom path-marker probe

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
