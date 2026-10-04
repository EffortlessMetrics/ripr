# 4916: weak relational credit for scalar predicates

Owner: [#4916](https://github.com/EffortlessMetrics/ripr-swarm/issues/4916). This
is one private Rust oracle-classification correction. #4771 owns downstream
error/effect/probe work; the merged #4833 owner-result route and the governed
real-consumer trial remain separate.

## Behavior and boundary

The retained #1580 consumer has `suppressed_payload_bytes > 0` classified as
`RelationalCheck/Weak` but `published_payload_bytes > 0` as
`MockExpectation/Medium`. The latter was attributed by observer-like spelling,
not by a real mock call. A complete outer `assert!` whose first condition is
one simple ASCII path/field versus a decimal integer under `<`, `<=`, `>` or
`>=` now receives `RelationalCheck/Weak` before the generic observer/mock name
fallback. Complete enclosing parentheses are accepted. This is syntactic
evidence, not integer type inference. Exact/error/equality/snapshot/smoke
priority is unchanged; real mock calls and boolean observer assertions retain
their existing classification.

The parsed route still recognizes ordinary assertions. The line-scanned route
still recognizes its limited helper/observer/mock forms; `scan.rs` is unchanged
and this PR does not make every ordinary `assert!` visible there. The accepted
comparison grammar excludes calls, indexing, blocks, closures, compound
conditions, raw/commented/nested assertion spellings, and extra trailing
tokens. No new Strong credit, consumer admission, cache schema, public DTO, or
complete canonical route is claimed.

## Discriminating proof

At tests-first `4c5a8b2a8bbf6dbf5d762836a03382b3376d7eb1`, the library
compiled and all four new exact controls executed as intended RED (each
0 passed / 1 failed / 0 ignored with the classification marker). This is
separate from the earlier test-setup compile failure and the intermediate
production `1a3d0198` E0597 compile failure. The latter was repaired in
`eabee2648ac5a13c8a4c011089442cd55de72b16` without changing the
classifier decision.

At exact `eabee2648`/tree `5564e58814f643055da01a607afda686c449e81a`,
the admitted CargoJSON library test executable compiled, and the bounded
focused runner executed the oracle groups 12 + 1 + 3 and two cache controls
1 + 1: all 18 passed, none failed or ignored. The raw receipt and executable
custody are under `ripr-4382-proof/4916-eabee-green-native-v3/`. One earlier
retry stopped at a wrong 16-test list assumption (the broad oracle namespace
actually lists 97), and a subsequent attempt stopped before Cargo on a bounded
Git read timeout. Neither is product RED/GREEN evidence. These artifacts stay
retained; the successful runner uses the observed exact 12/1/3 namespaces.

The tests require nonempty parsed actual published/suppressed facts, preserve
source lines/tokens, and distinguish real mock calls and message-only names.
The cache control seeds historical Mock/Medium facts under a prior analyzer
identity, proves a current-key MISS, then proves an actual cold parser
recomputation to Weak and whole-facts-identical warm hit without reparsing. The
test-only cache-key helper changes only analyzer identity; path/content/schema
and production cache authority remain unchanged.

## Remaining delivery proof

On the final committed head, verify the impacted `ripr` all-targets compile,
`check-fast` selector/report, `precommit` full change set and commands, then
`goldens check` and `dogfood` for output blast radius. The frozen local
`origin/main` at `53b7059` makes the current fast/precommit comparison much
broader than this PR's seven owned paths; compare the actual selector to Git
before using a pass as evidence, and recompute if that ref changes. Hosted
required checks and substantive exact-published-head review still gate merge.
The real #1580 canonical consumer replay remains separate and must report its
first refusing route independently; this classifier proof alone does not show
actionability or complete LLM opportunity.

Rollback: revert the private classifier/argument change and its nearby tests
normally; retain the RED/GREEN and cache receipts for review. No migration is
required.

## Current-main salvage, 2026-10-02

The existing draft at `18390bb71b860132e8269bcb84112fdeb6a5150a` is
retained. Its actual conflict with main
`78a97edf334a701e75603e1033a08448b49ece6d` is resolved by keeping both
semantic authorities: scalar admission inspects the original complete outer
assertion, while existing diagnostic-operand projection continues to serve
all established oracle precedence. Testing scalar admission only after
projection would incorrectly admit nested/wrapped assertion text.

An isolated exact-source Rust 1.99.0 harness copies the current main or
integrated candidate argument, classifier, pattern, scanner, mask, shadow,
token and constructor modules byte-for-byte. Only the domain/fact type
scaffolding omits serialization derives; no classification decision is
substituted. It does not stand in for the Cargo library, cache integration,
native pinned Rust 1.95.0 checks, or the complete consumer replay.

- Current-main RED: four relational controls execute and fail with the actual
  `MockExpectation/Medium` versus required `RelationalCheck/Weak` mismatch;
  zero passes, four failures, zero ignored (exit 101).
- Integrated GREEN: 110 source-module tests execute and pass, zero failures or
  ignored, including all four relational controls, retained mock/boolean
  observer behavior, exact/error/snapshot/smoke precedence, and main's
  diagnostic-expression projection (exit 0).
- Wrong-integration challenge: moving scalar admission after diagnostic
  projection causes the existing wrapped-spelling control to reject
  `wrapper(assert!(plan.published > 0));` (exit 101). Thus merely unioning
  the conflicting imports is not sufficient conflict repair.

The historical receipts above remain evidence only for their original heads.
Current candidate compile, repository guards, cache integration, golden blast
radius, pinned native CI and published-head review must be recorded separately
on the PR before merge. The governed #1580 complete route remains a non-claim.

Independent salvage review also caught the existing parenthesis helper using
a delimiter reader that accepts blocks and arrays. The scalar wrapper helper
now explicitly requires an opening parenthesis. Its new block/array exclusion
control first failed on the inherited helper (`RelationalCheck/Weak` for a
block expression), then passed with retained nested-parenthesis positives.
The final focused source-module suite contains 111 passing tests.
