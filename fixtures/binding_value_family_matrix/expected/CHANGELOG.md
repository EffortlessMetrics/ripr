# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0158: new fixture pinning the bounded value-transfer behavior (family matrix observes exact boundaries; unsupported chains fail closed by name)

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: review round 2 — operand resolution hoisted per probe (provenance text unchanged in content but regenerated), the starts_with hunk made a real behavior change, and quote-aware splitting/char escapes/dedup refinements

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: the body_of fallback is now observed by its own oracle (the else branch returns the changed default), and the starts_with hunk is a real behavior change; exact operands compare canonical renderings directly

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: the additive per-value provenance field now surfaces the evaluation chains and call sources the line-keyed assertion_texts map dropped (deferred #3295 follow-up)

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0162: the propagation_unknown why-hint stops asserting the propagation the class marks unknown, and unknown-class limitation prose renders under the Analyzer limit label (human-only; the shared decision-layer text is unchanged)

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158 (#3215 acceptance): add chars().next()/chars().next_back() parser-helper families to the binding_value_family_matrix matrix — two new boundary predicates observed from exact test inputs through the bounded value-transfer chain (provenance retained); existing four families unchanged

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158 (#3215 acceptance, review): owner-led test names fix the token-coincidence wrong-owner reach on bump's checked_add probe, and head gains a Some-arm boundary row (head("xray")) so the chars().next() Some arm is observed exactly — a constant-'f' Some-arm mutation now fails the fixture

Command:
`cargo xtask goldens bless binding_value_family_matrix --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
