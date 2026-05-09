# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0019: pin canonical test-oracle assistant loop replay corpus across recommendation, handoff, receipt, and PR ledger projection

Command:
`cargo test -p ripr test_oracle_assistant`

Updated:
- `expected/test-oracle-assistant-loop/canonical/README.md`
- `expected/test-oracle-assistant-loop/canonical/pr-guidance.json`
- `expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json`
- `expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json`
- `expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.md`

## Pending

Reason:
RIPR-SPEC-0001: baseline current predicate boundary fixture output

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0005: pin editor-facing seam diagnostic and code-action expectations for the boundary-gap fixture

Command:
`cargo test -p ripr boundary_gap_lsp`

Updated:
- `expected/lsp-diagnostics.json`
- `expected/lsp-code-actions.json`

## Pending

Reason:
RIPR-SPEC-0001: unknown findings must include stop reasons

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Human output formatting: align Discriminate spacing with other RIPR evidence lines.

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: oracle-strength-v2 distinguishes exact, broad, and smoke oracles

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: local delta flow names the returned value sink for changed predicates

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: activation modeling names observed values and missing equality discriminator

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: evidence-first output renders flow, activation, weakness, and next action

Command:
`cargo xtask goldens bless boundary_gap --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
