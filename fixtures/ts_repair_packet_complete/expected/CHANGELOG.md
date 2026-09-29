# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0087 §PR7: complete TS repair packet now actionable

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: actionable TS packet now surfaced via shared renderer (verify+receipt+edit-surface+must_not_change+canonical_gap_id)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 PR8 fix: actionable complete_repair_packet finding no longer emits contradictory blocked-case messaging (why not actionable / evidence needed / only-after-available); preview actionability + card now read why-actionable + repair-action, Next step confirms completeness

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
fix: JSON/SARIF recommended_next_step serialized raw blocked-case string despite repair_packet_ready:true; now uses shared reconcile_next_step fn (RIPR-SPEC-0088 §PR8)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive typescript_test_runner evidence field (TS must-use roadmap item 3)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
text-only update: weak_oracle_missing_summary _-arm now mentions toThrow exact-payload forms (RIPR-SPEC-0097)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
delta-5: drop verify_command from missing list when runner resolved

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088: packet-ready JSON drops blocked actionability messaging

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

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
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2347: advisory command strings route every argument through the shared bash
argv encoder, so the TypeScript packet's receipt command now single-quotes the
verify command instead of double-quoting it.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human-full.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (3)

Reason:
RIPR-SPEC-0087: repair action leads with the missing discriminator and the target shape uses an expected placeholder instead of re-using the borrowed toBeGreaterThan(50) literal under toBe; readiness and class unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (4)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (5)

Reason:
RIPR-SPEC-0079: TypeScript preview receipt_command is the canonical ripr receipt write command from the shared receipt-write owner, not ripr outcome (#3906)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (6)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first
RIPR-SPEC-0087: derive the TypeScript boundary input from a read-only parameter and a literal or single immutable integer module const; a complete packet's Start-here line names its action, test file, and verify command

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (7)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: Next step counts its label against the line budget and wraps instead of overflowing; fixture_opaque gloss states the heuristic (#4323 review)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_complete (8)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
