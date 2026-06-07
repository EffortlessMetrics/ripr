# RIPR-SPEC-0068: PR Review-Card Use Case

Status: proposed

Owner: product / swarm

Created: 2026-06-06

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- plans/use-case-specs/implementation-plan.md (planned)

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec writes the user-facing contract for the existing PR
  review-card surface (`target/ripr/review/comments.json` and the
  inline-comment publish plan). It promotes no language, surface, or
  evidence class to a stronger support tier. Preview-language cards
  (TypeScript/Bun, Perl) stay advisory under their own specs and the
  canonical boundary in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Nothing new beyond the spec itself: no new crate, binary,
  dependency, parser, runtime executor, artifact type, or workflow.

## Problem

The PR review surface already exists as mechanism: review guidance
JSON/Markdown (`crates/ripr/src/output/review_comments.rs`) and an
advisory inline-comment publish plan
(`crates/ripr/src/output/pr_inline_comment_publish_plan.rs`). What is
missing is the product contract that says what a single review card
must answer for the reviewer, what it must never imply, and which
states must refuse to render as success.

The user question this surface owns:

```text
Where is the issue, what does it mean,
and what should I do next?
```

Today a card carries placement, seam identity, missing-discriminator
guidance, and verify guidance — but no written rule forces every card
to be navigational (a place the reviewer can click to) rather than
merely descriptive, and no written rule keeps the sparse 3-inline cap
from being treated as a defect instead of the feature it is.

## Behavior

### Surface and artifacts

The review-card surface is produced by `ripr` and consumed by humans
in PR review (directly, or via the advisory publish plan):

- `target/ripr/review/comments.json` — review guidance, schema
  version `0.1` (`REVIEW_COMMENTS_SCHEMA_VERSION`).
- `target/ripr/review/comments.md` — human projection of the same
  guidance.
- `target/ripr/review/comment-publish-plan.json` — advisory inline
  publish plan with operations `create` / `update` / `keep` /
  `delete`, plus `skipped`, `blocked`, and `safe_to_publish`.
- `target/ripr/review/comment-publish-plan.md` — human projection of
  the plan.

Comment mode is a closed vocabulary: `off`, `plan`, `inline`
(`CommentMode` in `pr_inline_comment_publish_plan.rs`). The default
workflow does not post comments; `plan` is dry-run; `inline` posting
remains advisory and permission-gated (`safe_to_publish` plus the
permission context), never a gate decision.

```text
The user should be able to answer:
- Where is the issue?       -> placement path + line on a changed
                               production line, or an explicit
                               summary entry carrying the repair
                               route attached to a named limitation.
- What does it mean?        -> changed owner, changed behavior, and
                               the missing discriminator in plain
                               conservative static language.
- What should I do next?    -> one bounded repair (suggested
                               assertion shape near a named related
                               test) or one named limitation with
                               its repair route, plus the verify and
                               receipt commands when the card is
                               actionable.
```

### Required card fields

Every rendered review card must include:

- `file:line` — the placement (`placement.path`, `placement.line`,
  `placement.mode`) or, when no safe changed-line placement exists,
  an explicit `source_location` with a summary reason. A card with
  neither is invalid.
- `canonical_gap_id` when the card projects a gap-ledger record;
  `seam_id` whenever a seam identity exists.
- `gap_state` using the canonical actionability vocabulary
  (RIPR-SPEC-0061); cards never invent an alternate state. Today
  `gap_state` ships on gap-ledger cards and cross-language
  limitation cards while working-set actionable cards carry
  `grip_class` only; carrying `gap_state` on every card is
  contract-to-implement in the linked plan slice.
- a card repair summary — a projection of the canonical repair
  packet (RIPR-SPEC-0061), never a packet contract of its own:
  changed owner, changed behavior or missing discriminator,
  `suggested_test.assertion_shape`, and the recommended test
  (`suggested_test.recommended_file`,
  `suggested_test.recommended_name`, `suggested_test.near_test`) —
  or a named limitation with a `repair_route` when the repair
  contract cannot be satisfied. Two extensions are
  contract-to-implement in the linked plan slice: a structured
  related-test object `{name, file, line}` (today
  `GapRepairRoute.related_test` is a single string) and card-level
  `oracle_kind` / `oracle_strength` (today carried by agent briefs
  and seam packets, not review cards).
- `verify_command` when the card is actionable.
- a receipt command when the card is actionable —
  contract-to-implement in the linked plan slice; no review card
  carries a receipt command today.
- explicit non-claims (`language_status`, `authority_boundary`) when
  the card is advisory or preview.

Hard rule, verbatim:

```text
No seam ID without file:line or explicit source_location_unresolved.
```

Cards are navigational, not just descriptive. A reviewer must be able
to jump from every card to a concrete location or be told, in a named
state, why no location resolves.

### Sparse by default

The inline cap (`DEFAULT_REVIEW_MAX_INLINE_COMMENTS = 3`) and summary
cap (`DEFAULT_REVIEW_MAX_SUMMARY_ITEMS = 10`) are product features,
not implementation limits. Three high-confidence navigational cards
beat thirty descriptive ones. Selection beyond the caps lands in
summary-only or suppressed collections with a closed-vocabulary
reason. This vocabulary is scoped to the working-set selection path
in `review_comments.rs`; each token is marked existing or planned.
The implementation slice delivers the planned tokens by replacing
today's free-text `summary_reason` strings with machine tokens, so
the rename is traceable string-for-string:

| Reason token | Status | Meaning |
| --- | --- | --- |
| `inline_comment_cap_reached` | planned — tokenizes today's free-text `summary_reason` "inline comment cap reached"; the existing publish-plan skip reason `cap_reached` (`pr_inline_comment_publish_plan.rs`) and the RIPR-SPEC-0025 metric name `pr_inline_comment_cap_reached` name the same condition, and the slice must collapse the three names into this one token | placement existed; the inline cap was already filled. |
| `no_safe_changed_line_placement` | planned — tokenizes today's free-text `summary_reason` "no safe changed-line placement was available for this seam" | no changed production line was a safe anchor; card moves to summary with `source_location`. |
| `navigation_only_cross_language_target` | planned — tokenizes today's free-text `summary_reason` "navigation-only cross-language target limitation; no PR repair comment emitted" | cross-language test target unresolved; navigation context only, no repair comment. |
| `nearby_test_changed` | existing | the recommended test file changed in this PR; the author is already working there. |
| `summary_cap` | existing | the summary item cap was reached. |
| `missing_verification_command` | existing (emitted on the gap-ledger path today; the working-set path adopts it in the same slice) | actionable guidance requires a verify command; without one the card is suppressed, not weakened. |

No selection or suppression reason outside this vocabulary may ship
on the working-set selection path without amending this spec. The
gap-ledger projection path (`gap_record_comment_json` in
`review_comments.rs`) carries its own closed suppression-reason set
— `not_pr_comment_eligible`, `not_pr_local_repairable`,
`policy_state_not_commentable`, `missing_anchor`,
`missing_dedupe_fingerprint`, `duplicate_dedupe_fingerprint`,
`missing_repair_route`, and `missing_verification_command` — and
the same closure rule applies to that set.

### Scope honesty

The working-set (diff-scoped) guidance artifact carries an
`analysis_scope` with `run_status` (`limited_diff_scope` in
production today; `scoped` exists only in a test helper and a
production `scoped` run_status is contract-to-implement), `basis`
(for diff-scoped runs:
`changed_production_files_plus_immediate_callers`),
`downstream_consumable`, a named `limitation`, and a `repair_route`.
The gap-ledger guidance artifact
(`render_gap_record_review_comments_json`) carries no
`analysis_scope` today; extending `analysis_scope` to every
guidance artifact is contract-to-implement in the linked plan
slice. Diff-scoped guidance must never present itself as a
full-repo verdict.

### Required and forbidden wording

Required wording examples:

- "This changed boundary has no related test that would notice an
  off-by-one; add a boundary assertion near
  `tests/pricing.rs::discount_above_threshold`."
- "No safe changed-line placement was available; see the summary
  entry for `seam:...` at `src/pricing.rs:88`."
- "Diff-scoped run: changed production files plus immediate callers
  only. Not a full-repo verdict."

Forbidden wording examples:

- "This PR is fully covered." (an empty card set is a scope
  statement, never an all-clear)
- "This mutant would be caught." (runtime mutation vocabulary does
  not belong on a static card)
- Any state outside the conservative static vocabulary enforced by
  `cargo xtask check-static-language`.

## Non-Goals

- No autonomous comment posting by default; `off` stays the default
  and `inline` stays advisory and permission-gated.
- No gate decisions from this surface; gating is a separate contract
  (RIPR-SPEC-0067 lane).
- No new analyzer behavior, mutation execution, generated tests, or
  provider integration.
- No raised caps as a quality fix; card quality work must improve
  selection, not volume.
- No claim of full test adequacy in the runtime sense; cards speak
  only conservative static exposure language.
- No public repair packet from preview-language evidence unless the
  complete actionability/edit/verify/receipt contract is satisfied
  under that language's own spec.

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- Existing unit tests in `review_comments.rs` and
  `pr_inline_comment_publish_plan.rs` mapped to the card-field
  contract as implementation slices land.
- Fixture-backed examples for: an actionable inline card, a
  summary-only card with each closed-vocabulary reason, a suppressed
  card, and a `limited_diff_scope` run.

Fail-closed verifier reject list — the surface must refuse to render
these states as success:

- a card with a `seam_id` but neither `file:line` placement nor an
  explicit `source_location_unresolved` marker;
- an actionable card missing a verify command or receipt command;
- a card presenting a preview-language gap without
  `language_status` and `authority_boundary` non-claims;
- a publish plan with `safe_to_publish = false` (missing token,
  missing write permission, fork head, or non-PR event) rendered as
  publishable;
- a `limited_diff_scope` run rendered without its `run_status`,
  `limitation`, and `repair_route`;
- summary-only guidance published as an inline comment;
- an empty card set rendered as "all clear" instead of a scope
  statement;
- a selection or suppression reason outside the closed vocabularies
  (the working-set selection set or the gap-ledger suppression set).

## Acceptance Examples

- A reviewer opens a PR with three inline cards. Each card names the
  changed owner, the missing discriminator, a suggested assertion
  shape, a related test with file and line, and a copyable verify
  command. Clicking the placement lands on the changed line.
- A fourth qualifying seam appears in the summary with reason
  `inline_comment_cap_reached` (the planned token for today's
  free-text "inline comment cap reached"), keeping the inline
  surface sparse.
- A cross-language card whose test target does not resolve renders
  as navigation-only with `gap_state = "static_limitation"`,
  `repairability = "no_action"`, and the `repair_route` attached to
  the named limitation — never as a repair instruction.
- A run on a fork PR produces a publish plan with
  `safe_to_publish = false` and a blocked reason; nothing posts.
- A diff-scoped run on a large repo states its basis
  (`changed_production_files_plus_immediate_callers`) and never
  claims full-repo authority.

## Test Mapping

- None yet. This spec is docs-only; traceability entries are added
  when the implementation slices land tests against the card-field
  contract, the closed reason vocabulary, and the reject list above.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0068-pr-review-card-use-case.md — this
  document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "review file:line" slice: enforce the hard navigational rule
  (placement or explicit `source_location_unresolved` on every
  card), close the selection-reason vocabulary by replacing today's
  free-text `summary_reason` strings with the planned tokens,
  deliver the contract-to-implement card fields (receipt command,
  `gap_state` on every card, the structured related-test object,
  card-level `oracle_kind` / `oracle_strength`, `analysis_scope` on
  the gap-ledger guidance artifact), and add the reject-list checks
  to output-contract tests.
- Existing mechanism: `crates/ripr/src/output/review_comments.rs`
  and `crates/ripr/src/output/pr_inline_comment_publish_plan.rs`.

## Metrics

- Card navigability rate: share of rendered cards with a resolvable
  `file:line` or an explicit unresolved marker (target: 100%, by
  construction once the hard rule is enforced).
- Actionable-card completeness: share of actionable cards carrying
  verify and receipt commands.
- Selection-reason coverage: every summary-only and suppressed item
  carries a closed-vocabulary reason.
- False-actionable rate on review cards, tracked alongside receipt
  closure.
- Promotion rule: move this spec to `accepted` when the hard
  navigational rule, the closed selection vocabulary, and the
  reject-list checks are enforced by tests or output-contract
  gates, and the linked plan slice is complete.

## Failure Modes

- A seam without a safe placement leaks into the inline set — reject
  list plus placement tests make this a named defect.
- The caps get raised to "show more findings" — this spec records
  sparse-by-default as the product position; changing it requires a
  spec change, not a constant edit.
- Preview-language cards drift into repair instructions — non-claim
  fields plus the static-language gate keep them advisory.
- Publish plan posts without permission — `safe_to_publish` and the
  blocked collection fail closed; posting paths must check both.
