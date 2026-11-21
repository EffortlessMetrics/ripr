# Fixture: typescript-preview-false-actionable-audit

This manifest-only fixture audits TypeScript/JavaScript preview evidence classes
that must not become actionable repair packets without stricter proof.

The corpus points each audit row at an existing checked TypeScript-family
fixture finding and records its current safe disposition:

- safe advisory guidance;
- named static limitation;
- candidate future support;
- must remain non-actionable.

The audit preserves the preview boundary: no default gates, badges, RIPR Zero,
runtime Jest/Vitest execution, generated tests, source edits, provider calls,
mutation execution, or support-tier promotion.

## Validate

```bash
cargo xtask check-fixture-contracts
cargo test -p xtask typescript_preview_false_actionable_audit_cases_are_checked -- --test-threads=1
```
