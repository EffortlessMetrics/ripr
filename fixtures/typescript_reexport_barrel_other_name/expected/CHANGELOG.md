# Golden Output Changes

## Pending — typescript_reexport_barrel_other_name (1)

Reason:
RIPR-SPEC-0095 bounded re-export chains: new barrel negative control; the test imports only formatCents from the barrel, so the changed applyDiscount forwarded by the same barrel stays no_static_path with 0 related tests; initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
