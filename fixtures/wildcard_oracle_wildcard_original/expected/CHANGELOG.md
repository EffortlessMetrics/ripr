# Golden Output Changes

## Pending — wildcard_oracle_wildcard_original (19e43c73)

Reason:
Independent score(1)==2 original/wrong controls pin whole-wildcard
non-promotion and retained exact/guarded discrimination. Actual source-bound
fast-mode CLI output retains one finding and one related test; no runtime
execution or representative-project accuracy claim is made by the analyzer.

Command:
`ripr check --root fixtures/wildcard_oracle_wildcard_original/input --diff fixtures/wildcard_oracle_wildcard_original/diff.patch --mode fast --json`
Repeated directly for human and human-full output with distinct cold caches;
CLI binary SHA2563690bf6f1a6cd80e6bc7d9f1fb97b7c6a5686ae2da09ffb1daf2263c77bc927d.

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending - canonical fixture argv and landed Rust guidance

Reason:
The fixture runner passes forward-slash root/diff arguments on every platform.
The original frozen Windows calls used backslash arguments; quoting preceded
path normalization and left unnecessary quotes in the human goldens. Only
those known safe command arguments are reprojected here; hostile argument
quoting and the existing normalization/comparison policy remain unchanged.
The two wildcard short reports also retain the landed #5355/#5423
"Write a test for it" route at src/lib.rs:2; exposed and full reports do not
gain that route. Finding IDs, weak/strong oracles, evidence and recommendation
prose are unchanged. Required run 37215568518 observed the 12 human drifts
after policy, Clippy and 12016 tests passed; its first-difference report and
exact combined source justify this narrow expectation, pending successor CI.
No blanket bless, class promotion or representative accuracy claim is made.

Updated:
- expected/human.txt
- expected/human-full.txt

## Pending — wildcard_oracle_wildcard_original (1)

Reason:
RIPR-SPEC-0231: the examined wildcard test names why it misses the change (weak_assertion); verdict unchanged

Command:
`cargo xtask goldens bless wildcard_oracle_wildcard_original --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — wildcard_oracle_wildcard_original (2)

Reason:
RIPR-SPEC-0231: a matched related test keeps its oracle kind and strength in full output and adds why it still misses; verdicts unchanged

Command:
`cargo xtask goldens bless wildcard_oracle_wildcard_original --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — wildcard_oracle_wildcard_original (3)

Reason:
RIPR-SPEC-0108: One ReturnValue remains WeaklyExposed with weak relational wildcard and returned-value propagation. Refresh complete witness explanation/confidence; keep related observes_score. Independent control: Actual original runtime wildcard passes; matched wildcard/exact/guarded runtime control distinguishes2 versus3.

Command:
`cargo xtask goldens bless wildcard_oracle_wildcard_original --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
