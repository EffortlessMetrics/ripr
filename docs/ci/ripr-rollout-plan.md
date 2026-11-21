# MSRV 1.95 + Panic-Free + CI Economics Rollout Plan

This document is the anchor for the `ripr` reference-repo rollout. It records
the target state, the PR stack that gets there, the order, and the rules each PR
must follow. Later PRs use this document to stay on track without reconstructing
intent from chat history.

## What "done" looks like

```text
workspace.package.rust-version = "1.95"
unsafe_code = "forbid"
panic-family lints are deny
indexing_slicing = deny
string_slice = deny
no clippy.toml test carveouts exist
policy/clippy-lints.toml msrv = "1.95"
planned 1.94/1.95 lints are either active or explicitly retained with reason
policy/no-panic-allowlist.toml is canonical schema 0.3
.ripr/no-panic-allowlist.toml is retired or compatibility-only
cargo xtask check-no-panic-family reads the canonical policy file
test unwrap/expect/panic debt is removed or short-expiry receipted
policy/non-rust-allowlist.toml remains canonical and checked
bare #[allow(...)] is rejected
#[expect(...)] requires a reason
ci-plan.json is emitted
ci-actuals.json is emitted
VS Code lane is path/label/main gated
ripr self-dogfood advisory artifacts are uploaded
soft budget guard warns and enforces only the hard ceiling
ripr soft gate is deferred until advisory data exists
```

## Why this is not a full rebuild

The current repo is already strong:

- `unsafe_code = "forbid"` workspace-wide — correct, stays.
- Strict panic-family Clippy profile at MSRV 1.93 — advance it, don't rebuild.
- CI already has synchronize-only cancellation and main-only cache saves.
- CI release-surface checks are already gated behind push/main or labels.
- `policy/non-rust-allowlist.toml` is already TOML with rich metadata.
- `policy/clippy-lints.toml` already tracks planned 1.94/1.95 lints.

The rollout is a **ratchet**, not a rebuild.

## Why `ripr` is the reference repo

`ripr` is the tool that makes the rest of the estate's rollout economically
viable. It answers one draft-time question:

> For the behavior changed in this diff, do the current tests appear to contain
> a discriminator that would notice if that behavior were wrong?

That is mutation-testing-lite value at static-analysis prices. Running `ripr` on
the entire estate is only practical if `ripr` itself demonstrates disciplined CI
economics. A repo that pays $20/commit in CI to prove its own correctness is not
a reference implementation for cost-aware verification.

See `docs/ci/cost-and-verification-policy.md` for the verification-economics
framing.

## Hard rules for every PR in this stack

- Do not weaken the `ripr` product contract.
- Do not make `ripr` findings blocking until advisory data exists.
- Do not use runtime mutation terms (`killed`, `survived`) outside explicit
  runtime mutation calibration reports.
- Do not add Clippy test carveouts.
- Do not add bare `#[allow(...)]`.
- Do not weaken `unsafe_code = "forbid"`.
- Do not hide panic debt by lowering lint levels globally.
- Do not hard-enforce learned LEM budgets before actuals exist.
- Do not combine docs path, MSRV bump, panic debt cleanup, CI routing, and
  soft-gate implementation into one PR.

## Merge policy

- Merge PR 00 once docs checks pass.
- Merge MSRV 1.95 only after 1.95 check/clippy/test are clean or documented
  with targeted fixes.
- Merge lint policy changes only after `cargo xtask check-lint-policy` passes.
- Merge no-panic canonicalization only after `cargo xtask check-no-panic-family`
  passes.
- Merge test panic cleanup only if the allowlist count drops or every retained
  entry has owner/reason/expiry.
- Merge CI workflow changes only after `cargo xtask check-workflows` passes.
- Do not make `ripr` findings blocking until advisory data exists.
- Do not enforce learned budgets before `ci-actuals.json` has accumulated history.
- Treat bot quota/rate-limit notices as non-actionable unless they include a
  concrete current-head finding.

## PR stack

| PR | Title | Depends on |
| --- | --- | --- |
| 00 | docs(policy): document ripr MSRV 1.95 policy rollout | — |
| 01 | policy(msrv): audit Rust 1.95 compatibility | 00 |
| 02 | policy(msrv): move ripr to Rust 1.95 | 01 |
| 03 | policy(clippy): promote planned Rust 1.95 lints | 02 |
| 04 | policy(panic): make schema 0.3 no-panic allowlist canonical | 00 |
| 05 | policy(panic): remove or receipt test panic debt | 04 |
| 06 | policy(clippy): require expect-with-reason suppressions | 05 |
| 07 | policy(clippy): activate AST slicing and indexing rails | 06 |
| 08 | testing: add fallible test helpers | 05 |
| 09 | docs(ci): document current PR Plan and budget path | 00 |
| 10 | ci(plan): add structural advisory PR Plan workflow | 09 |
| 11 | ci(plan): implement numeric LEM PR Plan | 10 |
| 12 | ci(telemetry): emit CI actuals | 11 |
| 13 | ci(vscode): route extension lane by extension risk | 00 |
| 14 | ci(ripr): add self-dogfood advisory lane | 00 |
| 15 | ci(budget): add soft LEM guard | 12 |
| 16 | ci(metrics): scaffold learned LEM estimates | 15 |
| 17 | ci(ripr): implement acknowledgeable soft gate | 14, 16 |
| 18 | policy(test): add fallible assertion campaign | 08 (optional) |

Completed receipts:

- PR 00: #605 merged the docs/policy rollout anchor.
- PR 01: `docs/ci/msrv-1.95-audit.md` records a passing Rust 1.95
  compatibility audit for check, test, and Clippy. PR 02 may move the declared
  MSRV; PR 03 still owns planned-lint promotion.
- PR 02: the declared workspace MSRV, pinned toolchain, `clippy.toml`, and
  `policy/clippy-lints.toml` now target Rust 1.95. PR 03 still owns planned
  Clippy lint promotion.
- PR 03: clean Rust 1.94/1.95 planned lints are now active in
  `[workspace.lints.clippy]` and `policy/clippy-lints.toml`. `disallowed_fields`
  remains planned until a protected-seam `clippy.toml` config lands;
  `manual_pop_if` remains planned because Rust 1.95.0 Clippy does not
  recognize it.
- PR 04: `policy/no-panic-allowlist.toml` is now the canonical schema 0.3
  allowlist read by `cargo xtask check-no-panic-family`; `.ripr/` remains a
  legacy compatibility mirror while older branches drain.
- PR 04A / #325: strengthen no-panic drift reporting and validation before
  removing or receipting more test panic debt. This step keeps the checker
  reviewable by reporting allowed findings, advisory drift, stale entries,
  unallowed findings, and warnings, and by rejecting ambiguous or duplicate
  semantic selectors.
- PR 04B / #324: add the review-only `cargo xtask check-no-panic-family
  --propose` migration proposal command after drift reporting is hardened. Do
  not auto-adopt proposed entries.

Natural stacks:

```text
00 → 01 → 02 → 03        (MSRV ratchet)
04 → 05 → 06 → 07 → 08   (panic-free ratchet)
09 → 10 → 11 → 12 → 15 → 16   (CI economics)
14 → 17                  (ripr self-dogfood → soft gate)
13                       (VS Code lane routing, independent)
18                       (optional fallible-assertion campaign)
```

## Review loop for every PR

1. Open as draft initially.
2. Include: purpose, default PR LEM impact, workflows touched, branch
   protection impact, failure mode caught, cheaper signal considered, rollback
   path, commands run.
3. Read all bot/reviewer comments.
4. Fix actionable comments.
5. Treat quota/rate-limit comments as non-actionable noise.
6. Treat stale comments against old commits as stale after verifying HEAD.
7. Re-run relevant checks.
8. Mark ready only after self-review.
9. Merge when required checks are green and actionable feedback is resolved.
10. Rebase dependent PRs after each merge.
