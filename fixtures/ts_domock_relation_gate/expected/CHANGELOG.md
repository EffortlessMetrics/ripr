# Golden Output Changes

## Pending — ts_domock_relation_gate (1)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_domock_relation_gate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_domock_relation_gate (2)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_domock_relation_gate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
