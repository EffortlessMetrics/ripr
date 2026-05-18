pub(super) const EVIDENCE_HEALTH_HELP: &str = r#"Summarize how strong the current static evidence looks across the workspace.

Usage: ripr evidence-health [--root PATH] [--out PATH] [--out-md PATH] [--mutation-calibration PATH]

Options:
  --root PATH                    Workspace root to summarize. Defaults to current directory.
  --out PATH                     JSON output path. Defaults to target/ripr/reports/evidence-health.json.
  --out-md PATH                  Markdown output path. Defaults to target/ripr/reports/evidence-health.md.
  --mutation-calibration PATH    Optional imported mutation-calibration JSON for calibration availability counts.

The evidence-health report is an advisory Lane 1 analyzer-health view. It
summarizes seam grip classes, missing discriminators, observed values, related
test confidence, oracle strength, unknown static stages, and optional imported
calibration availability. It does not change analyzer behavior, run mutation
testing, edit source files, configure CI policy, or make gate decisions.
"#;
pub(super) const REVIEW_COMMENTS_HELP: &str = r#"Write advisory PR test guidance on changed lines (does not post to GitHub).

Usage: ripr review-comments [--root PATH] --base SHA --head SHA [--gap-ledger PATH] [--out PATH]

Options:
  --root PATH    Workspace root. Defaults to current directory.
  --base SHA     Pull-request base revision.
  --head SHA     Pull-request head revision.
  --gap-ledger PATH
                 Optional gap decision ledger JSON; when supplied, changed-line
                 repair cards come only from `projection_eligibility.pr_comment`
                 GapRecord targets.
  --out PATH     JSON output path. Defaults to target/ripr/review/comments.json.

The review-comments command writes a bounded advisory PR guidance report as
JSON plus a sibling Markdown file. It joins existing static seam evidence with
the changed-line diff by default and only places line guidance on changed
lines. When `--gap-ledger` is supplied, it does not rerun analysis; it renders
only eligible PR-local repair cards from explicit GapRecord records. It does
not post to GitHub, edit source, generate tests, run mutation testing, or make
CI blocking by default.
"#;
pub(super) const GATE_HELP: &str = r#"Evaluate the optional pass/fail gate against existing PR guidance (advisory unless explicitly enabled).

Usage: ripr gate evaluate [--pr-guidance PATH | --gap-ledger PATH] [--mode MODE] [--out PATH] [--out-md PATH]

Options:
  --root PATH                         Workspace root. Defaults to current directory.
  --repo-exposure PATH                Optional repo-exposure JSON input.
  --pr-guidance PATH                  Optional PR guidance JSON from `ripr review-comments`.
  --gap-ledger PATH                   Optional gap decision ledger JSON; when supplied, gate candidates come from repairable GapRecord projection targets.
  --sarif-policy PATH                 Optional SARIF policy JSON input.
  --labels-json PATH                  Optional JSON array or object with labels.
  --label LABEL                       Repeatable current PR label input.
  --agent-verify PATH                 Optional agent verify JSON input.
  --agent-receipt PATH                Optional agent receipt JSON input.
  --recommendation-calibration PATH   Optional recommendation calibration JSON input.
  --mutation-calibration PATH         Optional imported mutation calibration JSON input.
  --baseline PATH                     Explicit baseline for baseline-check or calibrated-gate.
  --mode MODE                         visible-only, acknowledgeable, baseline-check, or calibrated-gate. Defaults to visible-only.
  --acknowledgement-label LABEL       Repeatable acknowledgement label. Defaults to ripr-waive.
  --out PATH                          JSON output path. Defaults to target/ripr/reports/gate-decision.json.
  --out-md PATH                       Markdown output path. Defaults to --out with .md extension.

The gate evaluator is read-only policy over existing RIPR evidence. It writes
JSON and Markdown before returning a non-zero exit for `blocked` or
`config_error` decisions. It does not post comments, edit source, generate
tests, run mutation testing, upload SARIF, mutate GitHub state, or change
generated workflow defaults.
"#;
pub(super) const BASELINE_HELP: &str = r#"Create, diff, and shrink a reviewed baseline of acknowledged test gaps.

Usage:
  ripr baseline create --from PATH [--out PATH] [--dry-run] [--force]
  ripr baseline diff --baseline PATH --current PATH [--out PATH] [--out-md PATH]
  ripr baseline update --baseline PATH --current PATH --remove-resolved [--out PATH]

Create options:
  --from PATH    Gate-decision JSON from `ripr gate evaluate`.
  --out PATH     Baseline ledger path. Defaults to .ripr/gate-baseline.json.
  --dry-run      Print the baseline ledger JSON without writing.
  --force        Overwrite an existing baseline ledger.

Diff options:
  --baseline PATH    Reviewed baseline ledger. Defaults are supplied by callers.
  --current PATH     Current gate-decision JSON from `ripr gate evaluate`.
  --out PATH         JSON output path. Defaults to target/ripr/reports/baseline-debt-delta.json.
  --out-md PATH      Markdown output path. Defaults to target/ripr/reports/baseline-debt-delta.md.

Update options:
  --baseline PATH       Reviewed baseline ledger to refresh.
  --current PATH        Current gate-decision JSON from `ripr gate evaluate`.
  --remove-resolved     Required shrink-only mode; remove identities absent from current evidence.
  --out PATH            Updated baseline path. Defaults to --baseline.

The baseline create command writes a stable reviewed historical-debt ledger
from existing gate-decision evidence. It includes advisory, acknowledged, and
blocking identities; skips suppressed, configured-off, not-applicable, and
malformed decisions; and refuses to overwrite by default. It does not edit
source, run analysis, run mutation testing, generate tests, change gate policy,
or make CI blocking by default.

The baseline diff command compares a reviewed baseline ledger with current
gate-decision evidence and writes advisory JSON/Markdown debt movement. It
reports still-present, resolved, new policy-eligible, acknowledged, suppressed,
stale, invalid, and missing-input identities. It does not update baselines,
edit source, run analysis, run mutation testing, generate tests, change gate
policy, or make CI blocking by default.

The baseline update command refreshes a reviewed baseline ledger in shrink-only
mode. `--remove-resolved` removes reviewed identities that are absent from the
current gate-decision evidence, preserves malformed or ambiguous entries for
manual review, and never adopts new current debt. Generated CI should not use
this command to rewrite checked-in baselines automatically.
"#;
pub(super) const ZERO_HELP: &str = r#"Summarize current RIPR Zero progress over existing baselines and gate decisions.

Usage: ripr zero status --delta PATH [--baseline PATH] [--gap-ledger PATH] [--gate PATH] [--pr-guidance PATH] [--recommendation-calibration PATH] [--out PATH] [--out-md PATH]

Status options:
  --baseline PATH                       Optional reviewed gate baseline ledger.
  --delta PATH                          Required baseline-debt-delta JSON from `ripr baseline diff`.
  --gap-ledger PATH                     Optional gap decision ledger JSON whose ripr_zero_count targets define the visible target count.
  --gate PATH                           Optional gate-decision JSON from `ripr gate evaluate`.
  --pr-guidance PATH                    Optional PR guidance JSON from `ripr review-comments`.
  --recommendation-calibration PATH     Optional recommendation calibration JSON.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/ripr-zero-status.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/ripr-zero-status.md.

The RIPR Zero status report is read-only advisory progress evidence over
existing baselines, baseline debt deltas, gap decision ledgers, gate decisions, PR guidance, and
optional calibration artifacts. It reports visible unresolved debt, baseline
movement, metadata health, top debt areas, and bounded repair routes. It does
not run analysis, mutate baselines, edit source, generate tests, call an LLM,
run mutation testing, change gate policy, or make CI blocking by default.
"#;
pub(super) const POLICY_HELP: &str = r#"Summarize which RIPR policy posture is safe for the current repo.

Usage: ripr policy readiness [--root PATH] [--gate-decision PATH] [--baseline-delta PATH] [--recommendation-calibration PATH] [--mutation-calibration PATH] [--waiver-aging PATH] [--suppression-health PATH] [--repo-config PATH] [--previous-readiness PATH] [--out PATH] [--out-md PATH]
       ripr policy operations [--root PATH] --policy-readiness PATH [--waiver-aging PATH] [--suppression-health PATH] [--baseline-delta PATH] [--gate-decision PATH] [--recommendation-calibration PATH] [--mutation-calibration PATH] [--preview-boundary PATH] [--out PATH] [--out-md PATH]
       ripr policy history [--root PATH] --current PATH [--history PATH] [--commit REV] [--pr-number NUMBER] [--out PATH] [--out-md PATH]
       ripr policy promote [--root PATH] --to MODE --operations PATH [--history PATH] [--out PATH] [--out-md PATH]
       ripr policy preview-promote [--root PATH] --language LANGUAGE --class CLASS [--evidence PATH] [--out PATH] [--out-md PATH]
       ripr policy waiver-aging [--root PATH] [--ledger PATH] [--history PATH] [--out PATH] [--out-md PATH]
       ripr policy suppression-health [--root PATH] [--manifest PATH] [--out PATH] [--out-md PATH]

Readiness options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --gate-decision PATH                  Optional gate-decision JSON from `ripr gate evaluate`.
  --baseline-delta PATH                 Optional baseline-debt-delta JSON from `ripr baseline diff`.
  --recommendation-calibration PATH     Optional recommendation calibration JSON.
  --mutation-calibration PATH           Optional imported mutation calibration JSON.
  --waiver-aging PATH                   Optional waiver-aging JSON.
  --suppression-health PATH             Optional suppression-health JSON.
  --repo-config PATH                    Optional repo config summary JSON.
  --previous-readiness PATH             Optional prior policy-readiness JSON.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/policy-readiness.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/policy-readiness.md.

Operations options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --policy-readiness PATH               Policy-readiness JSON from `ripr policy readiness`.
  --waiver-aging PATH                   Optional waiver-aging JSON.
  --suppression-health PATH             Optional suppression-health JSON.
  --baseline-delta PATH                 Optional baseline-debt-delta JSON.
  --gate-decision PATH                  Optional gate-decision JSON.
  --recommendation-calibration PATH     Optional recommendation calibration JSON.
  --mutation-calibration PATH           Optional imported mutation calibration JSON.
  --preview-boundary PATH               Optional preview-boundary JSON.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/policy-operations.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/policy-operations.md.

History options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --current PATH                        Policy-operations JSON from `ripr policy operations`.
  --history PATH                        Optional policy history JSONL.
  --commit REV                          Optional current snapshot commit identity.
  --pr-number NUMBER                    Optional current snapshot PR number.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/policy-history.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/policy-history.md.

Promotion options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --to MODE                             Target mode: visible-only, acknowledgeable, baseline-check, or calibrated-gate.
  --operations PATH                     Policy-operations JSON from `ripr policy operations`.
  --history PATH                        Optional policy-history JSON from `ripr policy history`.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/policy-promotion-<mode>.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/policy-promotion-<mode>.md.

Preview promotion options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --language LANGUAGE                   Preview language under review: typescript or python.
  --class CLASS                         Preview evidence class under review, for example boundary_gap.
  --evidence PATH                       Optional explicit preview promotion evidence receipts JSON.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/preview-promotion-<language>-<class>.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/preview-promotion-<language>-<class>.md.

Waiver aging options:
  --root PATH                           Display root for the report. Defaults to current directory.
  --ledger PATH                         Optional current PR evidence ledger JSON.
  --history PATH                        Optional PR evidence ledger JSONL history.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/waiver-aging.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/waiver-aging.md.

Suppression health options:
  --root PATH                           Workspace root for reading the manifest. Defaults to current directory.
  --manifest PATH                       Suppression manifest path. Defaults to .ripr/suppressions.toml.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/suppression-health.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/suppression-health.md.

The policy readiness report is read-only advisory governance over explicit
existing artifacts. It recommends advisory-only, visible-only, acknowledgeable,
baseline-check, or calibrated-gate posture without executing a gate. Preview
language evidence stays visible and advisory by default. The policy operations
report composes existing policy artifacts into current ceiling, next safe
action, safe/not-safe promotion modes, blockers, and input health without
promoting anything. The policy history report shows whether readiness, waivers,
suppressions, baseline debt, calibration, and preview boundaries are improving
or decaying without appending history. The policy promotion packet reads policy
operations plus optional policy history and writes manual-review promotion
evidence without changing config. The preview promotion packet writes default
blocked evidence accounting for TypeScript and Python preview classes while
keeping preview evidence visible, advisory, non-gating, outside RIPR Zero, and
outside calibrated confidence until a later explicit policy is reviewed. The
waiver-aging report keeps repeated waivers visible as repair or policy-review
signals. The suppression-health report flags durable exception metadata gaps
while keeping suppressed findings visible. These commands do not run analysis,
mutate baselines or suppressions, post comments, edit source, generate tests,
run mutation testing, change gate policy, promote preview evidence, or make CI
blocking.
"#;
