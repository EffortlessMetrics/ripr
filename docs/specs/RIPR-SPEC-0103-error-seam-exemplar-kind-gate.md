# RIPR-SPEC-0103: Error-Seam Exemplar Kind-Gate

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1168

Linked PRs:

- None yet

Support-tier impact:

- Honesty fix for Rust primary-language output: `nearest_strong_test_to_imitate`
  in agent seam packets now emits `null` for an `ErrorVariant` seam when no related
  Strong test has an `ExactErrorVariant` oracle kind, rather than nominating a
  success-path `exact_value` test (actively misleading). No exposure-class or grip
  change; the seam stays `weakly_gripped`. This is a withdraw-only fix — it credits
  nothing and cannot raise any grip.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, or LSP servers.
- One new `pub(crate)` function `oracle_kind_matches_seam_kind` in
  `crates/ripr/src/analysis/test_grip_evidence.rs`; the existing private
  `oracle_kind_matches_seam` delegates to it (single source of truth).
- `nearest_strong_test_to_imitate` gains a `seam_kind: SeamKind` parameter;
  all four call sites updated (`agent_seam_packets.rs`, `agent_brief.rs`,
  `review_comments.rs`, `evidence_record.rs`).
- No `schema_version` bump. `"nearest_strong_test_to_imitate": null` is already
  a valid shape in the output contract (existing golden confirms this).
- Register this spec in `policy/doc-artifacts.toml` and `docs/specs/README.md`.

## Problem

For an `ErrorVariant` seam (e.g. `return Err(MyError::unavailable("no END_STREAM"))`),
`nearest_strong_test_to_imitate` in `output/agent_seam_packets.rs` performed a plain
`.iter().find(|t| t.oracle_strength == Strong)` over `evidence.related_tests`. This
returns the FIRST Strong test regardless of oracle kind.

In the common case where a function has multiple error returns and a success-path test,
the success-path `exact_value` Strong test is nominated as the "strong test to imitate"
for the error seam — actively misleading. An agent imitating it would write a
success-path test for an error seam, directly contradicting the packet's own
`patterns_to_avoid` and `missing_oracle_shape`.

### The single source of truth not being reused

`oracle_kind_matches_seam` in `test_grip_evidence.rs` already encodes the correct
kind-matching rule used by the grader:

- `ErrorVariant` seams accept ONLY `ExactErrorVariant` oracles.
- Value seams (`PredicateBoundary`, `ReturnValue`, `MatchArm`, `FieldConstruction`)
  accept `ExactValue`, `WholeObjectEquality`, `Snapshot`, `RelationalCheck`.
- `SideEffect` and `CallPresence` seams accept ONLY `MockExpectation`.

The exemplar selector was not calling this function — a violation of the "reuse don't
fork the shared validator" doctrine.

## Behavior

When `nearest_strong_test_to_imitate(seam_kind, evidence)` is called:

- It searches `evidence.related_tests` for the first test where BOTH
  `oracle_strength == Strong` AND `oracle_kind_matches_seam_kind(seam_kind, oracle_kind)` hold.
- For an `ErrorVariant` seam: only a `ExactErrorVariant` Strong test qualifies.
  A `ExactValue`, `WholeObjectEquality`, `Snapshot`, `RelationalCheck`, or `MockExpectation`
  Strong test does NOT qualify — it is skipped, and the function returns `None`.
- For value seams (`PredicateBoundary`, `ReturnValue`, `MatchArm`, `FieldConstruction`):
  a Strong `ExactValue`, `WholeObjectEquality`, `Snapshot`, or `RelationalCheck` test qualifies.
  A Strong `ExactErrorVariant` or `MockExpectation` does NOT qualify.
- For `SideEffect` / `CallPresence` seams: only a Strong `MockExpectation` qualifies.
- When the function returns `None`, the renderer emits `"nearest_strong_test_to_imitate": null`.
  The `null` shape is already a valid, supported output shape.
- When the function returns `Some(test)`, the renderer emits the test reference as before.
- No grip class changes. `current_grip` is unaffected.

The kind-matching rule is encoded in exactly one place: `oracle_kind_matches_seam_kind`
in `test_grip_evidence.rs`. Both the grader (`oracle_kind_matches_seam`) and the
exemplar selector delegate to it.

## Non-Goals

- Does NOT change any seam's grip class. `weakly_gripped` stays `weakly_gripped`.
- Does NOT change `missing_discriminators` (for `ErrorVariant` it is structurally
  empty; suppressing or downgrading the seam would be a false-clean on a
  safety-relevant seam — explicitly out of scope).
- Does NOT touch `candidate_values_for`, `candidate_value_from_required`, or
  `missing_oracle_shape`.
- Does NOT add `.unwrap_err()` / `expect_err` recognition. That is slice 2 /
  sub-problem A — a separate follow-up PR for `classify_assertion` /
  `extract/oracles/patterns.rs`.
- Does NOT change `classify_assertion`, `extract/oracles/patterns.rs`,
  `classify/reveal.rs`, or re-rank `related_tests`.
- Does NOT bump `schema_version`.
- Does NOT bump crate version, publish, or touch release workflows.
- Static-language clean: all new code and output uses allowed vocabulary only.

## Required Evidence

Tests in `crates/ripr/src/output/agent_seam_packets.rs` (behavioral, fixtures 1-4, 6):

1. `kind_gate_error_variant_seam_with_only_exact_value_strong_test_yields_null_exemplar` —
   ErrorVariant seam + Strong `ExactValue` related test → `null` AND `weakly_gripped`.
   Includes static-language guard (fixture 6).

2. `kind_gate_predicate_boundary_seam_with_exact_value_strong_test_still_nominated` —
   `PredicateBoundary` seam + Strong `ExactValue` related test → test IS nominated
   (must-not-over-withdraw control).

3. `kind_gate_error_variant_seam_with_exact_error_variant_strong_test_is_nominated` —
   ErrorVariant seam + Strong `ExactErrorVariant` related test → test IS nominated
   (kind matches — positive control).

4. `kind_gate_error_variant_seam_with_relational_strong_test_yields_null_exemplar` —
   ErrorVariant seam + Strong `RelationalCheck` related test → `null` AND
   `weakly_gripped` (must-not-over-credit control).

Tests in `crates/ripr/src/analysis/test_grip_evidence.rs` (parity, fixture 5):

5. `oracle_kind_matches_seam_kind_error_variant_accepts_only_exact_error_variant` —
   table of all rejected kinds.
   `oracle_kind_matches_seam_kind_value_seams_accept_exact_value` —
   value seams accept exact_value and reject error/mock.
   `oracle_kind_matches_seam_kind_side_effect_accepts_only_mock_expectation` —
   effect seams.

## Test Mapping

| Test | Fixture |
|---|---|
| `kind_gate_error_variant_seam_with_only_exact_value_strong_test_yields_null_exemplar` | 1 + 6 — headline fix + static-language guard |
| `kind_gate_predicate_boundary_seam_with_exact_value_strong_test_still_nominated` | 2 — must-not-over-withdraw |
| `kind_gate_error_variant_seam_with_exact_error_variant_strong_test_is_nominated` | 3 — positive: kind matches |
| `kind_gate_error_variant_seam_with_relational_strong_test_yields_null_exemplar` | 4 — must-not-over-credit |
| `oracle_kind_matches_seam_kind_error_variant_accepts_only_exact_error_variant` | 5 — parity: ErrorVariant rejects value oracles |
| `oracle_kind_matches_seam_kind_value_seams_accept_exact_value` | 5 — parity: value seams |
| `oracle_kind_matches_seam_kind_side_effect_accepts_only_mock_expectation` | 5 — parity: effect seams |

## Acceptance Examples

### Before (incorrect — ErrorVariant seam nominates a success-path exact_value test)

```json
{
  "seam_kind": "error_variant",
  "current_grip": "weakly_gripped",
  "nearest_strong_test_to_imitate": {
    "name": "authenticate_succeeds",
    "oracle_kind": "exact_value",
    "oracle_strength": "strong"
  }
}
```

### After (correct — null: no kind-matching strong exemplar)

```json
{
  "seam_kind": "error_variant",
  "current_grip": "weakly_gripped",
  "nearest_strong_test_to_imitate": null
}
```

Value seam (unchanged — still nominates its exact_value Strong test):

```json
{
  "seam_kind": "predicate_boundary",
  "current_grip": "weakly_gripped",
  "nearest_strong_test_to_imitate": {
    "name": "below_threshold_has_no_discount",
    "oracle_kind": "exact_value",
    "oracle_strength": "strong"
  }
}
```

## Implementation Mapping

| Behavior | Code location |
|---|---|
| `oracle_kind_matches_seam_kind` (single source of truth) | `crates/ripr/src/analysis/test_grip_evidence.rs` |
| `oracle_kind_matches_seam` delegates to `oracle_kind_matches_seam_kind` | `crates/ripr/src/analysis/test_grip_evidence.rs` |
| `nearest_strong_test_to_imitate` with `seam_kind` filter | `crates/ripr/src/output/agent_seam_packets.rs` |
| Call site: agent-seam-packet JSON renderer | `crates/ripr/src/output/agent_seam_packets.rs` |
| Call site: `recommended_test_for` | `crates/ripr/src/output/agent_seam_packets.rs` |
| Call site: `agent_brief` packet renderer | `crates/ripr/src/output/agent_brief.rs` |
| Call site: review-comment packet renderer | `crates/ripr/src/output/review_comments.rs` |
| Call site: evidence-record recommendation | `crates/ripr/src/output/evidence_record.rs` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

- `error_seam_exemplar_honesty`: `nearest_strong_test_to_imitate` is `null` for an
  `ErrorVariant` seam that only has a `ExactValue` Strong related test (fixture 1).
  The withdrawal is fail-closed: worst case is "less guidance," never a wrong-kind
  nomination.
