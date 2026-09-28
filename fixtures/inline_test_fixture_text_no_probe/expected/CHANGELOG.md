# Golden Output Changes

## Pending — inline_test_fixture_text_no_probe (1)

Reason:
RIPR-SPEC-0002: new #3718 fixture, inline cfg(test) fixture-text change yields zero probes (scaffold placeholder manually replaced after inspection)

Command:
`cargo xtask goldens bless inline_test_fixture_text_no_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — inline_test_fixture_text_no_probe (2)

Reason:
RIPR-SPEC-0002: drop trailing blank-context line from diff.patch for git diff --check; input_identity re-hash only

Command:
`cargo xtask goldens bless inline_test_fixture_text_no_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — inline_test_fixture_text_no_probe (3)

Reason:
RIPR-SPEC-0002: correct hunk line counts after trailing-context removal; input_identity re-hash only

Command:
`cargo xtask goldens bless inline_test_fixture_text_no_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — inline_test_fixture_text_no_probe (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless inline_test_fixture_text_no_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
