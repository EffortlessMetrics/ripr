# Exposure Summary Routing Guardrail

Date: 2026-05-31

Issues: #589, #594

## Summary

The bounded `repo-exposure-summary-json` surface is now the local default for
repo exposure summary data. This keeps ordinary badge, receipt, planning, and
agent queue workflows away from the full `repo-exposure-json` payload that can
grow into multi-GB artifacts on this repo.

## Local Rules

- Use `cargo xtask repo-exposure-summary-report` for bounded repo exposure
  metrics. It writes `target/ripr/reports/repo-exposure-summary.json`.
- Keep `cargo xtask repo-exposure-report` as an explicit deep-inspection
  command. It writes full `repo-exposure.{json,md}` evidence and should not be
  used for ordinary badge, receipt, top-file, or packet-queue paths.
- Prefer `repo-badge-json`, generated receipts, gap ledgers, or the bounded
  summary report before starting any full evidence scan.
- Keep ad-hoc root-level files such as `exposure.json`,
  `repo-exposure.json`, and `before.repo-exposure.json` ignored. Prefer
  `target/ripr/` for temporary artifacts and remove full dumps after
  inspection.

## Verification Boundary

This note does not remove the existing full exposure output. Full
`repo-exposure-json` remains available for targeted evidence debugging,
before/after repair receipts, and fixture/golden contracts that intentionally
need per-seam evidence.
