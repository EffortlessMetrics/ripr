pub(super) const PR_LEDGER_HELP: &str = r#"Record a read-only PR evidence ledger entry over existing reports.

Usage: ripr pr-ledger record --pr-number VALUE --base REV --head REV [--gate PATH] [--baseline-delta PATH] [--zero-status PATH] [--pr-guidance PATH] [--gap-ledger PATH] [--recommendation-calibration PATH] [--agent-receipt PATH] [--coverage PATH] [--history PATH] [--out PATH] [--out-md PATH]

Record options:
  --pr-number VALUE                    Pull request number or local identifier.
  --base REV                           Pull request base revision.
  --head REV                           Pull request head revision.
  --label LABEL                        Repeatable PR label to preserve in the record.
  --gate PATH                          Optional gate-decision JSON from `ripr gate evaluate`.
  --baseline-delta PATH                Optional baseline-debt-delta JSON from `ripr baseline diff`.
  --zero-status PATH                   Optional RIPR Zero status JSON from `ripr zero status`.
  --pr-guidance PATH                   Optional PR guidance JSON from `ripr review-comments`.
  --gap-ledger PATH                    Optional gap decision ledger JSON with policy-targeted repairable gap records.
  --recommendation-calibration PATH    Optional recommendation calibration JSON.
  --agent-receipt PATH                 Optional agent receipt JSON.
  --coverage PATH                      Optional coverage summary JSON.
  --history PATH                       Optional previous PR evidence ledger JSONL history.
  --out PATH                           JSON output path. Defaults to target/ripr/reports/pr-evidence-ledger.json.
  --out-md PATH                        Markdown output path. Defaults to target/ripr/reports/pr-evidence-ledger.md.

The PR evidence ledger is read-only advisory history over existing RIPR
artifacts. It records PR-local movement, waiver visibility, suppressions,
repair receipts, and optional coverage/grip frontier signals. It does not run
analysis, mutate baselines, post comments, edit source, generate tests, call an
LLM, run mutation testing, change gate policy, or make CI blocking by default.
"#;
pub(super) const PR_COMMENTS_HELP: &str = r#"Plan or publish bounded inline PR comments (off / plan / inline).

Usage: ripr pr-comments plan [--root PATH] [--pr-guidance PATH] [--existing-comments PATH] [--mode off|plan|inline] [--pull-request N] [--event-name NAME] [--head-repo OWNER/REPO] [--base-repo OWNER/REPO] [--token-available] [--no-write-permission] [--out PATH] [--out-md PATH]

Plan options:
  --root PATH                 Workspace root label. Defaults to current directory.
  --pr-guidance PATH          PR guidance JSON from `ripr review-comments`.
  --existing-comments PATH    Optional existing RIPR comment metadata.
  --mode MODE                 off, plan, or inline. Defaults to off.
  --pull-request N            Pull request number for inline safety checks.
  --event-name NAME           GitHub event name, usually pull_request.
  --head-repo OWNER/REPO      Pull request head repository.
  --base-repo OWNER/REPO      Pull request base repository.
  --token-available           Mark a pull-request write token as available.
  --no-token                  Mark the token as unavailable. This is the default.
  --write-permission          Mark pull-request write permission as available. This is the default.
  --no-write-permission       Mark pull-request write permission as unavailable.
  --max-inline-comments N     Maximum publishable inline comments. Defaults to 3.
  --out PATH                  JSON output path. Defaults to target/ripr/review/comment-publish-plan.json.
  --out-md PATH               Markdown output path. Defaults to target/ripr/review/comment-publish-plan.md.

The PR comments plan is a read-only advisory projection over existing
`ripr review-comments` output and optional existing-comment metadata. It emits
create/update/keep/delete/skip/blocked operations for a later explicit
publisher, but it never posts comments, calls GitHub, edits source, generates
tests, runs mutation testing, changes gate authority, or makes CI blocking by
default.
"#;
pub(super) const PR_REVIEW_HELP: &str = r#"Compose the first-screen PR review summary from existing review artifacts.

Usage: ripr pr-review front-panel [--root PATH] [--pr-guidance PATH] [--first-action PATH] [--assistant-proof PATH] [--assistant-health PATH] [--ledger PATH] [--baseline-delta PATH] [--zero-status PATH] [--gate-decision PATH] [--recommendation-calibration PATH] [--mutation-calibration PATH] [--coverage-frontier PATH] [--receipt PATH] [--out PATH] [--out-md PATH]

Front-panel options:
  --root PATH                         Workspace root label. Defaults to current directory.
  --pr-guidance PATH                  Optional PR guidance JSON from `ripr review-comments`.
  --first-action PATH                 Optional first-useful-action JSON.
  --assistant-proof PATH              Optional proof JSON from `ripr assistant-loop proof`.
  --assistant-health PATH             Optional health JSON from `ripr assistant-loop health`.
  --ledger PATH                       Optional PR evidence ledger JSON.
  --baseline-delta PATH               Optional baseline-debt-delta JSON.
  --zero-status PATH                  Optional RIPR Zero status JSON.
  --gate-decision PATH                Optional gate-decision JSON; gate remains pass/fail authority.
  --recommendation-calibration PATH   Optional recommendation calibration JSON.
  --mutation-calibration PATH         Optional imported mutation calibration JSON.
  --coverage-frontier PATH            Optional coverage/grip frontier JSON.
  --receipt PATH                      Optional agent or targeted-test receipt JSON.
  --out PATH                          JSON output path. Defaults to target/ripr/reports/pr-review-front-panel.json.
  --out-md PATH                       Markdown output path. Defaults to target/ripr/reports/pr-review-front-panel.md.

The PR review front panel is a read-only advisory first-screen report over
explicit existing RIPR artifacts. It shows the top issue or fallback, policy
state, movement, repair route, receipt state, calibration context, and artifact
groups. It does not rerun analysis, edit source, generate tests, call a
provider, run mutation testing, publish inline comments, or make CI blocking by
default.
"#;
