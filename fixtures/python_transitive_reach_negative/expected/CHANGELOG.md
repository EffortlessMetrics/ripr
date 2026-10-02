# Golden Output Changes

## Pending — python_transitive_reach_negative (1)

Reason:
RIPR-SPEC-0201: initial Python same-class transitive-reach negative fixture

Command:
`cargo xtask goldens bless python_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_transitive_reach_negative (2)

Reason:
RIPR-SPEC-0201: conflict repair against main. The fixture diff declared @@ -10,7 +10,7 @@ but carried 6 old / 6 new body lines; main's stricter hunk-span validation (e0cfe2186, #4439) discloses that as malformed_diff and flips the outcome to unsupported_input. Header now declares the honest -10,6 +10,6 span (changed lines and coordinates unchanged); goldens re-blessed for main's current renderer (summary vocabulary, source_subject, analysis_complete) while the negative control stays silent.

Command:
`cargo xtask goldens bless python_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_transitive_reach_negative (3)

Reason:
RIPR-SPEC-0201: review repair (CodeRabbit thread PRRT_kwDOSiSx0c6oY-y2). Same off-by-one hunk start as the positive fixture: headers now declare @@ -9,6 +9,6 @@ so coordinates match the true source lines; negative control stays silent.

Command:
`cargo xtask goldens bless python_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
