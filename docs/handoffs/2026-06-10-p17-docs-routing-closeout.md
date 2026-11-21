# P17 closeout — docs/spec proof-routing (first real CI skip)

**Date:** 2026-06-10
**Landed:** PR #1081 (squash `05fd3ef0`). Closes #1080.
**Evidence:** #1078 / `docs/handoffs/2026-06-10-proof-routing-dry-run-evidence.md`.

## What landed

The first proof-routing decision that actually changes CI behavior, scoped to the lowest-risk surface the P16 dry-run evidence cleared (docs/markdown-only). A PR whose every changed file is docs/spec/markdown — and which touches no release surface — now skips the heavy routed Rust lanes and instead runs the `docs-spec` pack's checks in a lightweight `docs-gate`.

Mechanism (single file, `.github/workflows/routed-rust.yml`):
- `detect-docs-only` job computes a conservative `docs_only` signal in pure bash; any non-docs file **or** any release surface (`CHANGELOG.md`/`Cargo.toml`/`Cargo.lock`/`crates/ripr/Cargo.toml`) forces `docs_only=false`.
- The four routed impl jobs gain `&& docs_only != 'true'`.
- `docs-gate` runs `check-static-language` / `check-spec-format` / `check-spec-numbering` / `check-doc-index` / `check-local-context`.
- The only branch-protection required check, **Ripr Rust Small Result**, stays green on both paths.

## Verification chain

1. Spec adversarially verified (8 PR scenarios) before implementation.
2. Detection unit-tested on 9 cases, including the `CHANGELOG.md` release-surface footgun.
3. Local gates green; run-block 27/63 lines; `check-workflows` passes.
4. Independent adversarial diff review: SAFE-TO-MERGE, zero blocking findings.
5. **Live (non-docs path):** PR #1081 itself touched `routed-rust.yml`, so it ran full proof on itself — `Detect Docs-Only Surface` correctly returned `docs_only=false`, the impl job ran, `Ripr Docs Gate` skipped.
6. **Live (docs-only path):** this very PR is docs-only; it is the first live exercise of the skip — `Detect Docs-Only Surface` should return `docs_only=true`, the heavy impl jobs should skip, `Ripr Docs Gate` should run, and `Ripr Rust Small Result` should stay green.

## Not done / follow-ups

- **P18 report/schema routing (#1079) is blocked** on the uncovered-lanes prerequisite: `rust`, `msrv`, `coverage`, and `policy` are owned by no proof pack, so routing marks them skipped on every PR. They must be mapped onto the code packs (`analysis-fixture`/`xtask-report`) or pinned unconditional before any code-surface routing.
- `check-traceability` / `check-capabilities` run in no workflow today (pre-existing gap, not worsened by P17) — wire them in unconditionally as a follow-up.
- The `docs/specs/**` and `docs/handoffs/**` globs match any extension; only spec/handoff text lives there by convention. A future hardening could restrict to text/markdown if needed.
