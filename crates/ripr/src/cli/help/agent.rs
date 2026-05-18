pub(super) const AGENT_HELP: &str = r#"Create a bounded packet for a coding agent and verify what it did.

Usage: ripr agent <subcommand>

Subcommands:
  start      Write a source-edit-free workflow manifest for one seam.
  brief      Rank a working-set brief for the agent-active router.
  packet     Expand one visible seam into the existing agent seam packet JSON.
  verify     Compare before/after repo-exposure JSON for agent verification.
  receipt    Summarize one seam from agent verify JSON for review handoff.
  status     Report existing agent-loop artifacts and the next missing command.
  review-summary
             Join agent-loop artifacts into a compact review packet.

Run `ripr agent start --help` for the workflow manifest, `ripr agent brief
--help`, `ripr agent packet --help`, or `ripr agent verify --help` for
JSON-only agent surfaces. Run `ripr agent receipt --help` for the verification
receipt surface, `ripr agent status --help` for the artifact status lens, and
`ripr agent review-summary --help` for the PR-review packet.
"#;
pub(super) const AGENT_START_HELP: &str = r#"Start a source-edit-free workflow packet for one selected change.

Usage: ripr agent start [--root PATH] --seam-id ID [--out PATH]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --seam-id ID     Select one visible seam by ID.
  --out PATH       Workflow output directory. Defaults to target/ripr/workflow.

The start command writes a source-edit-free workflow packet for one seam:
workflow.json, commands.md, and agent-brief.json. The packet contains artifact
paths and shared command templates for the before snapshot, packet, brief,
after snapshot, verify, and receipt steps. It remains advisory and static; it
does not call an LLM API, run mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_BRIEF_HELP: &str = r#"Write a bounded brief for a coding agent over the current diff or change.

Usage: ripr agent brief [--root PATH] (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json [--max-seams N]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --diff PATH      Select a diff file and line-level working set.
  --base REV       Derive the working set from a base revision.
  --files PATHS    Comma-separated repo-relative file paths.
  --seam-id ID     Select one visible seam by ID.
  --json           Required until a human brief surface exists.
  --max-seams N    Requested seam cap. Defaults to 3 and cannot exceed 10.

This parser is the first implementation seam for RIPR-SPEC-0010. The brief
router remains advisory and static; it does not run mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_PACKET_HELP: &str = r#"Write a per-change handoff packet for a coding agent.

Usage: ripr agent packet [--root PATH] --seam-id ID --json
       ripr agent packet [--root PATH] --gap-ledger PATH --gap-id ID --json

Options:
  --root PATH        Workspace root. Defaults to current directory.
  --seam-id ID       Select one visible seam by ID.
  --gap-ledger PATH  Explicit gap decision ledger JSON.
  --gap-id ID        Select a GapRecord by gap_id or canonical_gap_id.
  --json             Required until a human packet surface exists.

The packet command expands a seam selected by `ripr agent brief` into the
existing agent-seam-packets-json envelope with one packet. With `--gap-ledger`,
it renders one agent packet from an explicit agent-packet-eligible GapRecord
without rerunning analysis. It remains advisory and static; it does not run
mutation testing, generate tests, edit files, change cache behavior, or touch
LSP/MCP surfaces.
"#;
pub(super) const AGENT_VERIFY_HELP: &str = r#"Verify static-evidence movement between a before and after snapshot.

Usage: ripr agent verify [--root PATH] --before PATH --after PATH --json

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --before PATH    Before `repo-exposure-json` snapshot.
  --after PATH     After `repo-exposure-json` snapshot.
  --json           Required until a human verify surface exists.

The verify command compares two saved static repo-exposure artifacts and emits
an agent-focused before/after summary. Snapshot paths must resolve under
`--root`. The command remains advisory and static; it does not run analysis,
mutation testing, generate tests, edit files, change cache behavior, or touch
LSP/MCP surfaces.
"#;
pub(super) const AGENT_RECEIPT_HELP: &str = r#"Write a provenance receipt with bounded next-action guidance for one change.

Usage: ripr agent receipt [--root PATH] --verify-json PATH --seam-id ID --json [--test NAME] [--command CMD] [--out PATH]

Options:
  --root PATH         Workspace root. Defaults to current directory.
  --verify-json PATH  JSON emitted by `ripr agent verify`.
  --seam-id ID        Select one seam from the verify JSON.
  --json              Required until a human receipt surface exists.
  --test NAME         Optional focused test added or changed by the agent.
  --command CMD       Optional verification command that was run. Repeatable.
  --out PATH          Write the JSON receipt to a file instead of stdout.

The receipt command narrows a saved agent verify artifact to one seam and adds
handoff metadata for review. The verify JSON path and the before/after snapshot
paths named inside it must resolve under `--root`; receipt provenance hashes
those three artifacts without rerunning analysis. It remains advisory and
static; it does not run analysis, mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_STATUS_HELP: &str = r#"Report local agent-loop artifact state and the next command to run.

Usage: ripr agent status [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable status report. Human Markdown is the default.

The status command reads existing agent-loop artifacts under target/ripr only
and reports which before snapshot, after snapshot, brief, packet, verify, and
receipt files are present or missing. It may recover a seam_id from those
artifacts and emits the next command to run for missing inputs. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_REVIEW_SUMMARY_HELP: &str = r#"Summarize agent-loop artifacts into a compact review packet.

Usage: ripr agent review-summary [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable review summary. Human Markdown is the default.

The review-summary command reads existing agent-loop artifacts and joins agent
status, receipt, workflow, operator cockpit, repo exposure, LSP cockpit when
present, and local CI artifact state into a compact review packet. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
