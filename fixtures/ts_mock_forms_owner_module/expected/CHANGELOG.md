# Golden Output Changes

## Pending — ts_mock_forms_owner_module (1)

Reason:
RIPR-SPEC-0026: new fixture for #4294, renamed vi import plus typed import() mock of the owner module

Command:
`cargo xtask goldens bless ts_mock_forms_owner_module --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_mock_forms_owner_module (2)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_mock_forms_owner_module --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
