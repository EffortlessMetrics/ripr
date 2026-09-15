# Golden Output Changes

## Pending — wrapper_seam_callee_call_attribution (1)

Reason:
RIPR-SPEC-0106: #3714 round-1 review — retained fixture pinning call-based test-to-seam attribution (generic callee-calling test relates seam_callee_call/medium, weakly_exposed; unrelated-name test unrelated)

Command:
`cargo xtask goldens bless wrapper_seam_callee_call_attribution --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — wrapper_seam_callee_call_attribution (2)

Reason:
RIPR-SPEC-0106: #3714 round-2 review (devin hDRL2) — reach summary for callee-only relations names the converted callee instead of claiming owner reach; classifications unchanged (weakly_exposed)

Command:
`cargo xtask goldens bless wrapper_seam_callee_call_attribution --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — wrapper_seam_callee_call_attribution (3)

Reason:
RIPR-SPEC-0106: #3714 round-2 review — input gains Display/Error impls so the fixture compiles standalone; classifications unchanged (2 weakly_exposed, seam_callee_call relations intact)

Command:
`cargo xtask goldens bless wrapper_seam_callee_call_attribution --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — wrapper_seam_callee_call_attribution (4)

Reason:
RIPR-SPEC-0106: add the #3728 positional shadow-defeat witness test (captured call precedes a same-named binding; relation survives)

Command:
`cargo xtask goldens bless wrapper_seam_callee_call_attribution --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
