# Actionable Evidence Projection Implementation Plan

Status: proposed

Owner: cross-lane; product-facing projection alignment across badge, PR/CI, first-pr packet, and editor packet consumers

Linked proposal: `RIPR-PROP-0013`

Linked spec: `RIPR-SPEC-0055`

Linked ADR: `ADR-0017`

## Goal

Make public/user-facing RIPR surfaces count and render the same unresolved
actionable canonical repair queue.

## PR Sequence

1. `docs(spec): add actionable evidence projection stack`
2. `badge: add public badge basis audit`
3. `evidence: define public projection eligibility`
4. `badge: generate public endpoints from actionable projection`
5. `reports: preserve seam-native inventory as internal pressure gauge`
6. `badge: guard public badge projection basis`
7. `badge: refresh public endpoints from actionable basis`
8. `reports: assert public surfaces lead with actionable gaps`
9. `docs: align badge and actionable evidence wording`
10. `campaign: close actionable evidence projection`

## Boundaries

- Projection contract only; no analyzer semantic expansion.
- Preserve seam-native and raw counts as internal/supporting diagnostics.
- No manual edits to generated badge endpoint files.
- No policy-promotion, release/publish, runtime, or CI-default-blocking changes.

## Campaign closeout proof commands

```bash
cargo xtask badge-basis
cargo xtask badges --check
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-product-copy
cargo xtask check-static-language
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```
