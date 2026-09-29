# Golden Output Changes

## Pending — ts_repair_packet_wrong_family_oracle (1)

Reason:
RIPR-SPEC-0087: new negative fixture; a wrong-family exact-value oracle is not borrowed as the error-path repair target, so the packet stays not ready

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (3)

Reason:
RIPR-SPEC-0027 merge interaction (#3953): a guard without a comparison is weakly_exposed with no invented discriminator (line 3 predicate exposed -> weakly_exposed) and brace-only changed lines carry no behavior (line 5 probe removed); the fixture's error-path finding is unchanged - weakly_exposed, no wrong-family oracle borrow, repair packet stays not ready

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (4)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): preview notes use language display names with singular file counts (1 TypeScript file); mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (5)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (6)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (7)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_wrong_family_oracle (8)

Reason:
RIPR-SPEC-0122: Hidden remainder names omitted preview-language identity (#4395); formatting-only, no class or JSON change.

Command:
`cargo xtask goldens bless ts_repair_packet_wrong_family_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
