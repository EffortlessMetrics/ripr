# RIPR-SPEC-0108: Evidence-Promotion Honesty Meta-Gate

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- (none — standing capstone for the fake-clean bug class)

Linked PRs:

- None yet

Support-tier impact:

- Honesty enforcement meta-gate: no classifier behavior change; no new output
  field; no schema bump; no version bump. This spec pins the semantic expectation
  that non-promoted charter fixtures must remain non-promoted, independently of
  whether a golden was re-blessed. Tier labels and claim boundaries remain
  governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, functions, LSP servers, or analyzer behavior changes.
- New xtask command `check-evidence-promotion-honesty`: reads byte-pinned golden
  `expected/check.json` for each charter member and asserts the invariant.
- New corpus manifest: `fixtures/evidence-promotion-honesty-corpus/corpus.json`.
- Registered in CI (routed-rust.yml and ci.yml) next to `check-fixture-contracts`.
- `evidence-promotion-honesty-corpus` added to `is_manifest_only_fixture_dir`
  denylist so `goldens check` skips it.
- Does NOT unify per-language matcher functions.
- Does NOT run mutants or re-classify any finding.

## Problem

### The recurring fake-clean bug class

Across multiple PRs (RIPR-SPEC-0094, RIPR-SPEC-0097, RIPR-SPEC-0098,
RIPR-SPEC-0103, RIPR-SPEC-0104, RIPR-SPEC-0106, RIPR-SPEC-0107), the same
failure mode re-appeared: a finding was promoted to `exposed` when its evidence
did not structurally match the seam. Each fix required a new fixture and a
golden to pin the corrected behavior.

### Why `goldens check` alone is not sufficient

`goldens check` asserts `binary == golden` (byte comparison). If a developer
changes the classifier and re-blesses the golden of a known fake-clean fixture
from `weakly_exposed` → `exposed`, `goldens check` passes — binary now matches
the new (dishonest) golden. The semantic expectation that *this fixture must
stay non-promoted* is not enforced by `goldens check`.

### The gap

There was no standing CI gate that read the byte-pinned golden and asserted "no
finding may be `exposed` for charter member X, regardless of what the golden
says it was re-blessed to."

## Behavior

### The invariant

> A finding may not be promoted to `exposed` unless its evidence STRUCTURALLY
> matches the seam. Each confirmed fake-clean is a pinned charter member that
> must stay non-promoted.

### The gate (`check-evidence-promotion-honesty`)

1. Loads `fixtures/evidence-promotion-honesty-corpus/corpus.json`.
2. For each case, reads the source fixture's `expected/check.json` (the
   byte-pinned golden — NOT a fresh `ripr` run).
3. `must_remain_non_promoted` cases: asserts NO finding's `classification` is
   `exposed`. Also checks that no finding exceeds `expected_max_class` on the
   severity ordering `exposed > weakly_exposed > reachable_unrevealed/no_static_path > *_unknown`.
4. `expected_promoted` (control) cases: asserts at least one finding's
   `classification` is `exposed` (must-not-over-correct guard).
4b. `must_emit_limitation` cases (additive, RIPR-SPEC-0114/0115): asserts at
   least one finding carries `static_limit_kind == expected_limit_kind`. This is
   an independent assertion (a case may combine it with `must_remain_non_promoted`)
   and guards against a re-bless that silently drops a named limitation back to a
   bare class — e.g. dropping `rust_transitive_reach_unresolved` so a transitive
   reach reads as genuinely untested. A missing or empty `expected_limit_kind`
   is itself a violation.
4c. `must_disclose_witness` cases (additive, RIPR-SPEC-0115): asserts at least one
   finding's `evidence` contains the concrete transitive-reach *witness* pointer
   (the "Where to look" line naming the witnessing test and entry symbol, prose
   beginning `For example, the test `). Independent of the other assertions; guards
   against a re-bless that drops the witness back to the bare 0114 limitation
   message, regressing the first-run-trust UX.
5. PARITY checks: every `source_fixture` must exist, have `expected/check.json`,
   and NOT be in the manifest-only denylist (so it stays covered by `goldens check`).
   Each of {python, typescript, rust} must have ≥1 non-promoted case; rust and
   typescript must each have ≥1 control case.
6. FAIL-CLOSED: missing fixture / missing check.json / a non-promoted case
   showing `exposed` / a control losing `exposed` / a language missing coverage
   → non-zero exit + report under `target/ripr/reports/`.

### Design: share invariant + corpus, NOT per-language matchers

Each language keeps its own taxonomy and matcher functions. The gate enforces
the OUTPUT property (classification in the golden) over the cross-language
corpus. This is intentional: a single invariant over pinned golden outputs
requires no knowledge of why a language classifies something — only what the
golden says.

### Why this catches a dishonest re-bless

If a developer re-blesses a charter fixture from `weakly_exposed` → `exposed`:

1. `goldens check` passes (binary matches the new golden).
2. `check-evidence-promotion-honesty` reads the same golden and finds
   `classification: exposed` for a `must_remain_non_promoted` case.
3. Gate FAILS with a message naming the charter member and explaining that a
   dishonest re-bless was detected.

Reverting the golden restores gate passage. Adding a new charter member to the
corpus prevents the same regression in future.

## Required Evidence

### Corpus manifest

`fixtures/evidence-promotion-honesty-corpus/corpus.json` — cross-language
pinned adversarial corpus with 6 non-promoted charter members (python ×2,
typescript ×1, rust ×3) and 3 control cases (rust ×2, typescript ×1). One rust
charter member additionally asserts a named limitation via `must_emit_limitation`.

### Charter members (must_remain_non_promoted)

| id | language | source_fixture | vector |
|---|---|---|---|
| py_token_substring | python | python_adversarial_buffer_token | token_substring_coincidence |
| py_mock_call_not_value | python | python_adversarial_mock_call_not_value | mock_call_not_value |
| ts_broad_tothrow | typescript | typescript_broad_tothrow | cross_family_oracle_seam |
| rust_weak_error_oracle | rust | weak_error_oracle | non_variant_observing_error_oracle |
| rust_error_path_sibling_oracle | rust | error_path_sibling_oracle_fake_clean | sibling_oracle_does_not_confirm_error_path |
| rust_transitive_reach_named_limitation | rust | rust_transitive_reach_positive | transitive_reach_named_not_silently_clean (also `must_emit_limitation: rust_transitive_reach_unresolved` + `must_disclose_witness`) |

### Control cases (expected_promoted)

| id | language | source_fixture |
|---|---|---|
| rust_strong_error_oracle_control | rust | strong_error_oracle |
| rust_unwrap_err_variant_positive_control | rust | unwrap_err_variant_positive |
| ts_strong_oracle_control | typescript | typescript_strong_oracle |

### Validation by `check-fixture-contracts`

`validate_evidence_promotion_honesty_corpus` is called from
`check_fixture_contracts()` to verify the corpus is structurally valid (no
duplicate ids, all fixtures exist, all have `expected/check.json`, no fixture is
manifest-only, parity language coverage). This runs in CI as part of the
existing `check-fixture-contracts` gate.

## Non-Goals

- Does NOT unify per-language matcher functions; each language keeps its own
  taxonomy.
- Does NOT run mutants; reads byte-pinned goldens only.
- Does NOT re-classify any finding.
- Does NOT bump schema_version, crate version, or touch release workflows.
- Does NOT replace `goldens check`; composes with it.
- Static-language clean: gate output uses `exposed`, `weakly_exposed`,
  `reachable_unrevealed` only — all allowed vocabulary.

## Acceptance Examples

### Gate passes (all charter members at expected class)

```
pass: all charter members at expected class; no promoted case carries exposed;
all controls retain exposed
```

### Gate fails (dishonest re-bless detected)

```
FAIL: evidence promotion honesty case `py_token_substring`
(fixture `fixtures/python_adversarial_buffer_token`):
finding `probe:src_pack.py:python_preview:cfc61771` has classification `exposed`
but `must_remain_non_promoted` is true — dishonest re-bless detected;
revert the golden or remove this charter member
```

### Gate fails (control lost exposed)

```
FAIL: evidence promotion honesty control case `rust_strong_error_oracle_control`
(fixture `fixtures/strong_error_oracle`):
`expected_promoted` is true but no finding has classification `exposed` —
the gate has over-corrected or the fixture needs re-blessing
```

## Test Mapping

| Test | Spec control |
|---|---|
| `cargo run -p xtask -- check-evidence-promotion-honesty` | End-to-end gate pass |
| Flip charter golden to `exposed` → gate fails naming it | Dishonest re-bless proof |
| Flip control golden to `weakly_exposed` → gate fails naming it | Over-correct guard proof |
| `cargo xtask check-fixture-contracts` | Corpus structural validity |
| `cargo xtask check-command-catalog` | Command registration |
| `cargo xtask check-workflows` | CI registration |

## Implementation Mapping

| Component | Location |
|---|---|
| Corpus manifest | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |
| Gate implementation | `xtask/src/main.rs::check_evidence_promotion_honesty` |
| Corpus validator | `xtask/src/main.rs::validate_evidence_promotion_honesty_corpus_at` |
| Manifest-only denylist | `xtask/src/main.rs::is_manifest_only_fixture_dir` |
| Command enum | `xtask/src/command.rs::XtaskCommand::CheckEvidencePromotionHonesty` |
| Command catalog | `xtask/src/command.rs::command_catalog` |
| Dispatch | `xtask/src/dispatch.rs` |
| CI routed | `.github/workflows/routed-rust.yml` |
| CI fast | `.github/workflows/ci.yml` |

## Metrics

- `evidence_promotion_honesty_charter_members`: count of `must_remain_non_promoted` cases in corpus
- `evidence_promotion_honesty_control_cases`: count of `expected_promoted` cases in corpus
