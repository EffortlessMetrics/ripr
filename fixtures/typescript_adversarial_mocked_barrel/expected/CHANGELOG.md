# Golden Output Changes

## Pending — typescript_adversarial_mocked_barrel (1)

Reason:
RIPR-SPEC-0108: initial golden for the TypeScript mocked-barrel false-exposed guard (PR #4502 review)

Command:
`cargo xtask goldens bless typescript_adversarial_mocked_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_adversarial_mocked_barrel (2)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy (#4323, main merge)
RIPR-SPEC-0122: human lines lead with the plain word the check summary uses; schema value kept beside it

Command:
`cargo xtask goldens bless typescript_adversarial_mocked_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_adversarial_mocked_barrel (3)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless typescript_adversarial_mocked_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_adversarial_mocked_barrel (4)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless typescript_adversarial_mocked_barrel --reason "..."`

Updated:
- `expected/human.txt`
