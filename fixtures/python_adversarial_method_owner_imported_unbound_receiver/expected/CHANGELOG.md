# Golden Output Changes

## Pending

Reason:
pin residual method-owner false-exposed: imported+constructed owner class but asserted method runs on an unrelated receiver; receiver identity required

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
