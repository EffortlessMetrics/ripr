pub(super) const HELP: &str = r#"ripr — find changed Rust code where nearby tests may not actually catch the changed behavior.

Usage:
  ripr init [--root PATH] [--ci github] [--dry-run] [--force]
  ripr pilot [--root PATH] [--out PATH] [--mode draft] [--max-seams 5] [--timeout-ms 30000]
  ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]
  ripr evidence-health [--root PATH] [--out PATH] [--out-md PATH] [--mutation-calibration PATH]
  ripr review-comments --root . --base SHA --head SHA [--out target/ripr/review/comments.json]
  ripr gate evaluate --pr-guidance PATH [--mode visible-only] [--out target/ripr/reports/gate-decision.json]
  ripr baseline create --from target/ripr/reports/gate-decision.json [--out .ripr/gate-baseline.json] [--dry-run] [--force]
  ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json [--out target/ripr/reports/baseline-debt-delta.json] [--out-md target/ripr/reports/baseline-debt-delta.md]
  ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved [--out .ripr/gate-baseline.json]
  ripr zero status --delta target/ripr/reports/baseline-debt-delta.json [--baseline .ripr/gate-baseline.json] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--gate target/ripr/reports/gate-decision.json] [--out target/ripr/reports/ripr-zero-status.json] [--out-md target/ripr/reports/ripr-zero-status.md]
  ripr policy readiness [--gate-decision target/ripr/reports/gate-decision.json] [--baseline-delta target/ripr/reports/baseline-debt-delta.json] [--out target/ripr/reports/policy-readiness.json] [--out-md target/ripr/reports/policy-readiness.md]
  ripr policy operations --policy-readiness target/ripr/reports/policy-readiness.json [--waiver-aging target/ripr/reports/waiver-aging.json] [--suppression-health target/ripr/reports/suppression-health.json] [--out target/ripr/reports/policy-operations.json] [--out-md target/ripr/reports/policy-operations.md]
  ripr policy history --current target/ripr/reports/policy-operations.json [--history .ripr/policy-history.jsonl] [--commit HEAD] [--pr-number 123] [--out target/ripr/reports/policy-history.json] [--out-md target/ripr/reports/policy-history.md]
  ripr policy promote --to baseline-check --operations target/ripr/reports/policy-operations.json [--history target/ripr/reports/policy-history.json] [--out target/ripr/reports/policy-promotion-baseline-check.json] [--out-md target/ripr/reports/policy-promotion-baseline-check.md]
  ripr policy preview-promote --language typescript --class boundary_gap [--evidence target/ripr/reports/preview-promotion-evidence.json] [--out target/ripr/reports/preview-promotion-typescript-boundary-gap.json] [--out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md]
  ripr policy waiver-aging [--ledger target/ripr/reports/pr-evidence-ledger.json] [--history .ripr/pr-evidence-ledger.jsonl] [--out target/ripr/reports/waiver-aging.json] [--out-md target/ripr/reports/waiver-aging.md]
  ripr policy suppression-health [--root .] [--manifest .ripr/suppressions.toml] [--out target/ripr/reports/suppression-health.json] [--out-md target/ripr/reports/suppression-health.md]
  ripr pr-ledger record --pr-number 123 --base SHA --head SHA [--gate target/ripr/reports/gate-decision.json] [--baseline-delta target/ripr/reports/baseline-debt-delta.json] [--zero-status target/ripr/reports/ripr-zero-status.json] [--out target/ripr/reports/pr-evidence-ledger.json]
  ripr pr-comments plan --pr-guidance target/ripr/review/comments.json [--existing-comments target/ripr/review/existing-comments.json] [--mode off|plan|inline] [--out target/ripr/review/comment-publish-plan.json]
  ripr pr-review front-panel [--pr-guidance target/ripr/review/comments.json] [--first-action target/ripr/reports/first-useful-action.json] [--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json] [--assistant-health target/ripr/reports/assistant-loop-health.json] [--ledger target/ripr/reports/pr-evidence-ledger.json] [--out target/ripr/reports/pr-review-front-panel.json]
  ripr coverage-grip frontier (--ledger target/ripr/reports/pr-evidence-ledger.json|--baseline-delta target/ripr/reports/baseline-debt-delta.json|--zero-status target/ripr/reports/ripr-zero-status.json) [--coverage target/ripr/reports/coverage-summary.json] [--out target/ripr/reports/coverage-grip-frontier.json]
  ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json [--out target/ripr/reports/test-oracle-assistant-proof.json]
  ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json [--out target/ripr/reports/assistant-loop-health.json]
  ripr first-pr [--root .] [--base origin/main] [--head HEAD] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--out-dir target/ripr/reports] [--check]
  ripr first-action [--root .] [--pr-guidance target/ripr/review/comments.json] [--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--ledger target/ripr/reports/pr-evidence-ledger.json] [--out target/ripr/reports/first-useful-action.json]
  ripr reports index [--reports-dir target/ripr/reports] [--review-dir target/ripr/review] [--out target/ripr/reports/index.json]
  ripr reports gap-ledger --records fixtures/gap-decision-ledger/corpus.json [--out target/ripr/reports/gap-decision-ledger.json]
  ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]
  ripr agent start --root . --seam-id ID [--out target/ripr/workflow]
  ripr agent brief --root . (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json
  ripr agent packet --root . --seam-id ID --json
  ripr agent verify --root . --before before.json --after after.json --json
  ripr agent receipt --root . --verify-json agent-verify.json --seam-id ID --json
  ripr agent status --root . [--json]
  ripr agent review-summary --root . [--json]
  ripr check [--base origin/main] [--diff PATH] [--mode draft] [--format FORMAT]
  ripr explain [--base REV|--diff PATH] <finding-id|file:line>
  ripr context [--base REV|--diff PATH] --at <finding-id|file:line>
  ripr lsp [--stdio]
  ripr doctor

What it does:
  Reads changed Rust code, creates mutation-like probes, and estimates whether
  tests appear to reach, infect, propagate, and reveal the changed behavior
  through meaningful oracles. It does not run mutants.

Quick start:
  ripr doctor
  ripr pilot
  ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json
  ripr evidence-health --root .
  ripr review-comments --root . --base origin/main --head HEAD --out target/ripr/review/comments.json
  ripr gate evaluate --pr-guidance target/ripr/review/comments.json --mode visible-only
  ripr baseline create --from target/ripr/reports/gate-decision.json --out .ripr/gate-baseline.json
  ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json
  ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved
  ripr zero status --baseline .ripr/gate-baseline.json --delta target/ripr/reports/baseline-debt-delta.json --gate target/ripr/reports/gate-decision.json
  ripr policy readiness --gate-decision target/ripr/reports/gate-decision.json --baseline-delta target/ripr/reports/baseline-debt-delta.json
  ripr policy operations --policy-readiness target/ripr/reports/policy-readiness.json --waiver-aging target/ripr/reports/waiver-aging.json --suppression-health target/ripr/reports/suppression-health.json
  ripr policy history --current target/ripr/reports/policy-operations.json --history .ripr/policy-history.jsonl
  ripr policy promote --to baseline-check --operations target/ripr/reports/policy-operations.json --history target/ripr/reports/policy-history.json
  ripr policy preview-promote --language typescript --class boundary_gap
  ripr policy waiver-aging --ledger target/ripr/reports/pr-evidence-ledger.json --history .ripr/pr-evidence-ledger.jsonl
  ripr policy suppression-health
  ripr pr-ledger record --pr-number 123 --base origin/main --head HEAD --baseline-delta target/ripr/reports/baseline-debt-delta.json --zero-status target/ripr/reports/ripr-zero-status.json
  ripr pr-comments plan --pr-guidance target/ripr/review/comments.json --mode plan
  ripr pr-review front-panel --pr-guidance target/ripr/review/comments.json --first-action target/ripr/reports/first-useful-action.json --ledger target/ripr/reports/pr-evidence-ledger.json
  ripr coverage-grip frontier --ledger target/ripr/reports/pr-evidence-ledger.json --coverage target/ripr/reports/coverage-summary.json
  ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json
  ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json
  ripr first-pr --root . --base origin/main --head HEAD
  ripr first-action --pr-guidance target/ripr/review/comments.json --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json --ledger target/ripr/reports/pr-evidence-ledger.json
  ripr reports index
  ripr reports gap-ledger --records fixtures/gap-decision-ledger/corpus.json
  ripr calibrate cargo-mutants --mutants-json target/mutants/outcomes.json --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
  ripr agent start --root . --seam-id f3c9e4d21a0b7c88
  ripr agent brief --root . --diff change.diff --json
  ripr agent packet --root . --seam-id f3c9e4d21a0b7c88 --json
  ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json
  ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id f3c9e4d21a0b7c88 --json
  ripr agent status --root .
  ripr agent review-summary --root .
  ripr check --diff crates/ripr/examples/sample/example.diff
  ripr check --diff crates/ripr/examples/sample/example.diff --json
  ripr explain --diff crates/ripr/examples/sample/example.diff <finding-id>

Start-here path:
  - `ripr doctor` checks whether the local workspace and config can produce evidence.
  - `ripr first-pr` writes `target/ripr/reports/start-here.{json,md}` from existing artifacts.
  - Safe next action means repair one named gap, regenerate a missing or malformed artifact, or stop on no-action.
  - Verify command, receipt command, and receipt path are the static proof rail; receipts are advisory, not runtime adequacy or gate approval.
  - Preview-limited evidence stays syntax-first and advisory, with static limits before repair language.
"#;
