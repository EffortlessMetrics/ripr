# RIPR-SPEC-0155: Cross-surface source-role projection

Status: proposed

Issue: #3285 (parent #3213; builds on #3273, #3283, #3284)

## Problem

After #3283 gave ripr one producer-owned source-role model, exactly one
production surface still re-inferred role from paths: the LSP
out-of-scope partition used the retired path predicate, so an opted-in
`production_like_targets` target seeded CLI findings that the editor
partition dropped as out-of-scope — the last surface disagreement in
the #3213 matrix.

## Behavior

- The LSP scope partition consumes the producer-owned source-role model
  (`classify_with` with declared Cargo targets and the repository's
  `production_like_targets` opt-in), the same authority diff seeding,
  the repo production sets, and the seam inventory use. An opted-in
  target keeps its editor projection; evidence-role anchors stay
  excluded; hover, actions, and status inherit the converged snapshot
  unchanged.
- Because the partition and the seeding surface now share one authority
  and one config, the partition suppresses nothing the seeding model
  already excluded: the `out_of_scope_test_file_findings` disclosure
  count becomes a structural zero for ordinary configs and stays as a
  safety net for anchor-model divergence.
- The retired `is_production_rust_path` predicate is deleted from
  production code; its contract (src requirement, exclusion components,
  `tests.rs` stem) lives on in the role model's layout base, pinned by
  the carry-over test. No renderer, gate, ledger, badge, or editor
  surface re-infers role from path, filename, function name,
  attributes, or prose.
- Harness plumbing vocabulary — `Result<()>` plumbing with `?`,
  `map_err` chains, `Ok(())` terminals, Err-return guards, harness-only
  `.contains()` output checks, assertion-driver helpers — generates no
  production obligation under any supported form while the exact
  assertion inside the same test still credits the production owner.

## Required Evidence

- The opt-in editor projection test (verified failing on main: the
  partition dropped the target's findings) and the converged
  mixed-scope and test-only tests.
- The `source_role_harness_suppression` fixture: the full plumbing
  vocabulary in one changed integration test produces zero production
  obligations while the exact boundary assertion keeps crediting the
  production owner.
- Zero production references to the retired predicate (grep) with the
  carry-over contract pin in the role model.
- Existing role fixtures (`benches_harness_evidence`,
  `assertion_form_parity_*`, `assertion_shaped_oracle_*`) remain green.

## Required guards

- No surface may reintroduce path-based role inference; new role
  judgments extend the role model, not a consumer predicate.
- The disclosure count must never be silently dropped; it stays the
  typed record of partition drops.
- Evidence-role harness code stays visible as related-test/oracle
  evidence for production owners.

## Acceptance Examples

- Accept: `production_like_targets = ["tests/api_contract.rs"]` — the
  target's findings appear in the editor with the same scope the CLI
  seeds.
- Accept: a changed `tests/contract.rs` full of `?`/`map_err`/`Ok(())`
  plumbing — zero probes, zero findings, exact assertion still credits
  the owner.
- Reject: any renderer or gate classifying role from a path fragment.

## Test Mapping

`lsp/tests.rs` (`workspace_diagnostics_production_like_opt_in_target_keeps_editor_projection`,
rewritten mixed-scope and test-only tests); fixtures
`source_role_harness_suppression`; `analysis/workspace/source_role.rs`
carry-over pin; `analysis/workspace/select.rs` role pin.

## Non-Goals

- No schema-version bump or new output field: findings are production
  subjects by construction, so a per-finding role field would be
  constant; disclosure flows through the existing surfaces.
- No downstream-suppression removal (#3213 row 7 is blocked on a
  published release carrying this proof).
- No TS/Python adapter test-file models (adapter-owned).

## Implementation Mapping

- `lsp/diagnostics.rs` — converged partition + context construction.
- `analysis/workspace/classify.rs` — predicate retired; package-root
  helpers remain.
- `analysis/seam_inventory.rs` — test-path consumers converged.

## Metrics

No new metric; the editor projection count now reconciles with the CLI
seeding scope by construction.
