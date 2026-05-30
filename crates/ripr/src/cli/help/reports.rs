pub(super) const REPORTS_HELP: &str = r#"Write reviewer-first report projections from explicit artifacts.

Usage:
  ripr reports index [--root PATH] [--reports-dir PATH] [--review-dir PATH] [--receipts-dir PATH] [--workflow-dir PATH] [--agent-dir PATH] [--pilot-dir PATH] [--ci-dir PATH] [--out PATH] [--out-md PATH]
  ripr reports gap-ledger --records PATH [--root PATH] [--out PATH] [--out-md PATH]
  ripr reports gap-ledger --check-output PATH [--root PATH] [--out PATH] [--out-md PATH]

Index options:
  --root PATH           Workspace root label. Defaults to current directory.
  --reports-dir PATH    Directory containing report artifacts. Defaults to target/ripr/reports.
  --review-dir PATH     Directory containing PR guidance artifacts. Defaults to target/ripr/review.
  --receipts-dir PATH   Directory containing receipt artifacts. Defaults to target/ripr/receipts.
  --workflow-dir PATH   Directory containing workflow repair artifacts. Defaults to target/ripr/workflow.
  --agent-dir PATH      Directory containing agent handoff artifacts. Defaults to target/ripr/agent.
  --pilot-dir PATH      Directory containing pilot artifacts. Defaults to target/ripr/pilot.
  --ci-dir PATH         Directory containing CI context artifacts. Defaults to target/ci.
  --out PATH            JSON output path. Defaults to target/ripr/reports/index.json.
  --out-md PATH         Markdown output path. Defaults to target/ripr/reports/index.md.

Gap ledger options:
  --records PATH        Explicit GapRecord JSON, gap_records JSON, or fixture corpus JSON.
  --repo-exposure PATH  Derive repo-scoped Rust GapRecords from seams[].evidence_record.
  --check-output PATH   Derive PR-local presentation/output and Python repair gaps from check JSON.
  --root PATH           Workspace root label. Defaults to current directory.
  --out PATH            JSON output path. Defaults to target/ripr/reports/gap-decision-ledger.json.
  --out-md PATH         Markdown output path. Defaults to target/ripr/reports/gap-decision-ledger.md.

The report-packet index is a read-only advisory map over explicit existing
RIPR artifacts. It groups reports by reviewer use, names the start-here
artifact, preserves gate-decision as the configured pass/fail authority,
lists missing expected surfaces with regeneration commands when known, and
does not rerun analysis, edit source, generate tests, call providers, run
mutation testing, publish inline comments, or make CI blocking by default.

The gap decision ledger command is a read-only advisory renderer for explicit
GapRecord input, existing repo-exposure evidence records, or check-output
repair findings. It normalizes supplied or derived gap records into JSON and Markdown,
summarizes projection eligibility, preserves gate-decision as the configured
pass/fail authority, and does not rerun hidden analysis, publish comments,
edit source, generate tests, call providers, run mutation testing, or change
default CI blocking.
"#;
pub(super) const COVERAGE_GRIP_HELP: &str = r#"Report whether line coverage and behavior evidence moved together.

Usage: ripr coverage-grip frontier (--ledger PATH|--baseline-delta PATH|--zero-status PATH) [--coverage PATH] [--out PATH] [--out-md PATH]

Frontier options:
  --coverage PATH          Optional coverage summary JSON.
  --ledger PATH            Optional PR evidence ledger JSON from `ripr pr-ledger record`.
  --baseline-delta PATH    Optional baseline-debt-delta JSON from `ripr baseline diff`.
  --zero-status PATH       Optional RIPR Zero status JSON from `ripr zero status`.
  --out PATH               JSON output path. Defaults to target/ripr/reports/coverage-grip-frontier.json.
  --out-md PATH            Markdown output path. Defaults to target/ripr/reports/coverage-grip-frontier.md.

The coverage/grip frontier report is read-only advisory evidence. It keeps
line execution coverage and RIPR behavioral grip movement as separate axes. It
does not treat coverage as adequacy, run mutation testing, change gate policy,
or make CI blocking by default.
"#;
pub(super) const ASSISTANT_LOOP_HELP: &str = r#"Produce or summarize advisory agent proof and proof-loop health.

Usage:
  ripr assistant-loop proof [--pr-guidance PATH] [--agent-packet PATH] [--before PATH] [--after PATH] [--receipt PATH] [--ledger PATH] [--coverage-frontier PATH] [--gate-decision PATH] [--out PATH] [--out-md PATH]
  ripr assistant-loop health --proof PATH [--proof PATH ...] [--out PATH] [--out-md PATH]

Proof options:
  --root PATH                 Workspace root label. Defaults to current directory.
  --pr-guidance PATH          Optional PR guidance JSON from `ripr review-comments`.
  --agent-packet PATH         Optional editor/agent handoff JSON from `ripr agent brief`.
  --before PATH               Optional before repo-exposure JSON snapshot.
  --after PATH                Optional after repo-exposure JSON snapshot.
  --receipt PATH              Optional agent receipt JSON from `ripr agent receipt`.
  --ledger PATH               Optional PR evidence ledger JSON from `ripr pr-ledger record`.
  --coverage-frontier PATH    Optional coverage/grip frontier JSON.
  --gate-decision PATH        Optional gate-decision JSON from `ripr gate evaluate`.
  --out PATH                  JSON output path. Defaults to target/ripr/reports/test-oracle-assistant-proof.json.
  --out-md PATH               Markdown output path. Defaults to target/ripr/reports/test-oracle-assistant-proof.md.

The assistant-loop proof report is read-only advisory evidence over explicit
Campaign 20 artifacts. It requires at least one supplied artifact input, joins
PR guidance, agent handoff packets, before and after static evidence, receipts,
and optional CI projection artifacts, and marks missing proof pieces as
incomplete or unknown. It does not rerun analysis, post comments, edit source,
generate tests, call a provider, run mutation testing, change gate policy, or
make CI blocking by default.

Health options:
  --root PATH                 Workspace root label. Defaults to current directory.
  --proof PATH                Explicit test-oracle assistant proof JSON. Repeatable.
  --out PATH                  JSON output path. Defaults to target/ripr/reports/assistant-loop-health.json.
  --out-md PATH               Markdown output path. Defaults to target/ripr/reports/assistant-loop-health.md.

The assistant-loop health report is read-only advisory evidence over explicit
test-oracle assistant proof JSON artifacts. It summarizes proof completeness,
missing inputs, static movement, recurring warnings, and bounded repair queues.
It does not rerun analysis, inspect source to infer missing data, edit source,
generate tests, call a provider, run mutation testing, change gate policy, or
make CI blocking by default.
"#;
pub(super) const FIRST_ACTION_HELP: &str = r#"Recommend the next focused test to add from existing review artifacts.

Usage: ripr first-action [--root PATH] [--pr-guidance PATH] [--assistant-proof PATH] [--gap-ledger PATH] [--ledger PATH] [--baseline-delta PATH] [--receipt PATH] [--gate-decision PATH] [--coverage-frontier PATH] [--editor-context PATH] [--out PATH] [--out-md PATH]

Options:
  --root PATH                Workspace root label. Defaults to current directory.
  --pr-guidance PATH         Optional PR guidance JSON from `ripr review-comments`.
  --assistant-proof PATH     Optional proof JSON from `ripr assistant-loop proof`.
  --gap-ledger PATH          Optional gap decision ledger JSON from `ripr reports gap-ledger`.
  --ledger PATH              Optional PR evidence ledger JSON from `ripr pr-ledger record`.
  --baseline-delta PATH      Optional baseline-debt-delta JSON from `ripr baseline diff`.
  --receipt PATH             Optional agent receipt JSON from `ripr agent receipt`.
  --gate-decision PATH       Optional gate-decision JSON from `ripr gate evaluate`.
  --coverage-frontier PATH   Optional coverage/grip frontier JSON.
  --editor-context PATH      Optional editor evidence context packet JSON.
  --out PATH                 JSON output path. Defaults to target/ripr/reports/first-useful-action.json.
  --out-md PATH              Markdown output path. Defaults to target/ripr/reports/first-useful-action.md.

The first-action report is a read-only advisory router over explicit existing
RIPR artifacts. It writes one next useful test action or one fallback reason
for developers, reviewers, or coding agents. It does not rerun analysis, post
comments, edit source, generate tests, call a provider, run mutation testing,
invent policy, or make CI blocking by default.

Start-here vocabulary:
  - safe next action means add one focused proof, refresh missing/stale/wrong-root/malformed evidence, or stop on no-action.
  - verify command, receipt command, and receipt path are static movement evidence, not runtime adequacy.
  - preview-limited evidence stays advisory and must show static limits before repair language.
"#;
pub(super) const CALIBRATE_HELP: &str = r#"Import cargo-mutants outcomes as advisory calibration over static evidence.

Usage: ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]

Options:
  --mutants-json PATH          cargo-mutants JSON file, or directory containing outcomes.json and/or mutants.json.
  --repo-exposure-json PATH    RIPR repo-exposure-json snapshot to join against.
  --format FORMAT             md, markdown, text, or json. Defaults to md.
  --out PATH                  Write the rendered calibration report to a file instead of stdout.

The calibration report is advisory. It imports already-produced runtime
mutation data and joins it to static seam evidence by seam_id first, then by
unambiguous file/line. It does not run mutation testing, alter static
classifications, or configure CI policy.
"#;
