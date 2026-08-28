# RIPR-SPEC-0118: Rust Integration Public-API Path Limitation

Status: accepted

Owner: product / swarm

Created: 2026-06-20

Linked issues:

- First-run-trust / P3 Rust reachability lane: distinguish integration-test
  public-API reach limits from generic transitive helper limits.

Linked PRs:

- This PR

Support-tier impact:

- No tier change. This spec adds one additive `static_limit_kind` value,
  `rust_integration_public_api_path_unresolved`.
- Classification stays `no_static_path`. The value names a limitation, not a
  coverage claim, related-test relation, repair packet, or public-API
  reachability proof.
- No `schema_version` bump is required because `static_limit_kind` is already
  additive optional finding metadata.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Update `docs/OUTPUT_SCHEMA.md`, `docs/STATIC_LIMITS.md`, and
  `.ripr/traceability.toml`.
- No new crates, dependencies, workflow permissions, runtime executors, macro
  expansion, rustc driver integration, or LSP-specific behavior.

## Problem

RIPR-SPEC-0114 and RIPR-SPEC-0115 made Rust transitive reach limitations visible:
when a test appears to call an entry point that may lead to the changed owner,
RIPR emits a named `no_static_path` limitation and a concrete witness. But the
same generic `rust_transitive_reach_unresolved` label covers two different
operator questions:

- a local helper-chain path inside ordinary Rust source;
- an integration test under `tests/` that reaches the crate through public API
  or a test helper that calls public API.

For first-run trust, the second case should name the adoption blocker directly:
RIPR saw an integration test and candidate public-API path, but cannot yet cross
that path with enough identity/reach/oracle proof to promote the finding.

## Behavior

### Trigger condition

After direct related-test classification returns `no_static_path` with no
related tests, and after the RIPR-SPEC-0114 bounded transitive witness check
returns a witness, the Rust adapter chooses the limitation kind from the
witnessing test path:

1. If `TransitiveWitness.test_file` is an integration test path
   (`tests/...` or contains `/tests/` after path normalization), set
   `Finding.static_limit_kind =
   Some(StaticLimitKind::RustIntegrationPublicApiPathUnresolved)`.
2. Otherwise, keep the existing generic
   `StaticLimitKind::RustTransitiveReachUnresolved`.

### When found

The existing RIPR-SPEC-0114/0115 evidence remains unchanged:

- push `StopReason::TransitiveReachUnresolved`;
- keep `classification = no_static_path`;
- keep the witness pointer in `evidence`;
- keep structured `static_limitation` detail when the edge fields are present.

Only the static limitation kind becomes more specific for integration-test
origin witnesses.

### Fail-closed boundaries

- Never promote the finding.
- Never add the witness to `related_tests`.
- Never claim the integration test reaches, covers, tests, exercises, or
  discriminates the changed owner.
- Do not infer visibility, crate exports, trait dispatch, generics, macro
  expansion, or type identity from the path kind.
- If the witness path is not recognized as an integration test path, keep the
  existing generic `rust_transitive_reach_unresolved` kind.

## Wire Format

| Field | Integration-test witness | Non-integration transitive witness |
|---|---|---|
| `classification` | `no_static_path` | `no_static_path` |
| `static_limit_kind` | `rust_integration_public_api_path_unresolved` | `rust_transitive_reach_unresolved` |
| `stop_reasons` | includes `transitive_reach_unresolved` | includes `transitive_reach_unresolved` |
| `related_tests` | unchanged; witness is not added | unchanged; witness is not added |
| `static_limitation.analyzer_route` | `analysis/rust-public-api-transitive-reach` | unchanged |

## Non-Goals

- Public-API to internal reach promotion.
- Full crate visibility or type analysis.
- Macro-aware reach.
- Repair packet emission.
- Changing verify or receipt command availability.
- Changing the pinned external checkout model.

## Acceptance Examples

1. **Integration test calls public API**: `tests/it.rs` calls `outer()`, and
   `outer()` may lead to changed owner `inner`. Result: `no_static_path` plus
   `static_limit_kind: rust_integration_public_api_path_unresolved`.
   Fixture: `fixtures/rust_transitive_reach_positive/`.
2. **Integration test helper chain**: `tests/version_req.rs` calls
   `assert_public_api_accepts`, which calls crate public API that may lead to
   changed owner `matches_greater`. Result: `no_static_path` plus
   `static_limit_kind: rust_integration_public_api_path_unresolved`.
   Fixture: `fixtures/rust_transitive_reach_test_helper_chain/`.
3. **Pinned external semver launch point**: the semver boundary patch emits
   `rust_integration_public_api_path_unresolved`, discloses the exact witness
   and limitation detail, and stays non-actionable.
4. **Non-integration helper chain**: a transitive witness from ordinary Rust
   source keeps `rust_transitive_reach_unresolved`.

## Required Evidence

- `StaticLimitKind::RustIntegrationPublicApiPathUnresolved` in
  `crates/ripr/src/domain/language.rs`.
- Rust adapter selection in `crates/ripr/src/analysis/language/rust.rs`.
- Unit guard proving integration paths and non-integration paths select
  different limitation kinds.
- Pure fixture goldens for `fixtures/rust_transitive_reach_positive/` and
  `fixtures/rust_transitive_reach_test_helper_chain/`.
- Honesty corpus assertions expecting
  `rust_integration_public_api_path_unresolved` for the two pure fixtures and
  the semver pinned external case.
- Output schema, static-limit documentation, spec index, doc-artifact ledger,
  LSP gap-artifact validation, and traceability entries updated.

## Test Mapping

- `crates/ripr/src/analysis/language/rust.rs::tests::transitive_reach_limit_kind_names_integration_test_path`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_wire_strings_are_stable`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_describe_is_present_and_distinct`
- `crates/ripr/src/lsp/gap_artifacts.rs::tests::validation_accepts_rust_integration_public_api_static_limit_kind`
- `fixtures/rust_transitive_reach_positive/expected/check.json`
- `fixtures/rust_transitive_reach_test_helper_chain/expected/check.json`
- `cargo xtask check-evidence-promotion-honesty`
- `cargo xtask check-evidence-promotion-honesty --pinned-external --clone --case rust_semver_matches_greater_external_limitation`

## Implementation Mapping

| Component | Location |
|---|---|
| Static limit enum and text | `crates/ripr/src/domain/language.rs` |
| Integration-vs-generic selection | `crates/ripr/src/analysis/language/rust.rs` |
| Path predicate reused by selection | `crates/ripr/src/analysis/rust_index.rs` |
| LSP gap-artifact known-kind validation | `crates/ripr/src/lsp/gap_artifacts.rs` |
| Output contract docs | `docs/OUTPUT_SCHEMA.md` |
| Static limit docs | `docs/STATIC_LIMITS.md` |
| Pure fixture goldens | `fixtures/rust_transitive_reach_positive/`, `fixtures/rust_transitive_reach_test_helper_chain/` |
| Honesty corpus | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |

## CI Proof

- `cargo test -p ripr analysis::language::rust::tests::transitive_reach_limit_kind_names_integration_test_path`
- `cargo test -p ripr domain::language::tests::static_limit_kind_wire_strings_are_stable`
- `cargo xtask fixtures rust_transitive_reach_positive`
- `cargo xtask fixtures rust_transitive_reach_test_helper_chain`
- `cargo xtask check-evidence-promotion-honesty`
- `cargo xtask check-evidence-promotion-honesty --pinned-external --clone --case rust_semver_matches_greater_external_limitation`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`
- `cargo xtask check-traceability`
- `cargo fmt --check`

## Metrics

- Corpus invariant: integration-origin transitive witnesses emit
  `rust_integration_public_api_path_unresolved`, disclose witness and limitation
  detail, and remain non-promoted at `no_static_path`.
- Corpus invariant: semver pinned external remains not clean, not actionable,
  and within configured runtime/artifact budgets.
- Generic non-integration transitive witness behavior remains available as
  `rust_transitive_reach_unresolved`.
