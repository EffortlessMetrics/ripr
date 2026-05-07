# RIPR-SPEC-0011: LLM Work Loop

Status: proposed

## Problem

Campaign 10 made the editor-agent evidence loop real: saved-workspace
diagnostics can hand users to agent packet, brief, verify, receipt, cockpit,
and CI artifacts. That loop is still easy for an LLM agent to misuse because
the agent has to infer state from files and remember the correct command
sequence.

The LLM work loop adds a control plane around the existing artifacts. It should
answer where the agent is in the workflow, what is missing, which seam links the
artifacts, and which command should run next.

## Behavior

`ripr agent status --root . --json` reads existing artifacts only:

```text
target/ripr/workflow/before.repo-exposure.json
target/ripr/workflow/after.repo-exposure.json
target/ripr/workflow/agent-brief.json
target/ripr/workflow/agent-packet.json
target/ripr/workflow/agent-verify.json
target/ripr/reports/agent-receipt.json
```

It must not run analysis, generate tests, edit source files, run mutation
testing, refresh LSP state, or change the schemas for brief, packet, verify, or
receipt.

The status report should:

- report each required artifact as present or missing;
- recover `seam_id` from receipt, verify, packet, or brief JSON when possible;
- emit a next command for every missing artifact;
- surface the first missing command as `next_command`;
- warn when timestamps suggest `agent verify` is older than a before/after
  snapshot or `agent receipt` is older than `agent verify`;
- keep all language advisory and static.

## JSON Shape

The status report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "incomplete",
  "root": ".",
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "source": "agent_receipt"
  },
  "artifacts": [
    {
      "name": "before_snapshot",
      "label": "before snapshot",
      "path": "target/ripr/workflow/before.repo-exposure.json",
      "required": true,
      "state": "present",
      "bytes": 12000,
      "modified_unix_ms": 1778179200000
    }
  ],
  "missing_commands": [
    {
      "step": "agent_packet",
      "artifact": "target/ripr/workflow/agent-packet.json",
      "reason": "agent packet artifact is missing",
      "command": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json"
    }
  ],
  "next_command": {
    "step": "agent_packet",
    "artifact": "target/ripr/workflow/agent-packet.json",
    "reason": "agent packet artifact is missing",
    "command": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json"
  },
  "warnings": []
}
```

## Required Evidence

The first LLM work-loop slice requires:

- a JSON status report with schema version `0.1`;
- artifact presence for before snapshot, after snapshot, agent brief, agent
  packet, agent verify, and agent receipt;
- recoverable seam identity when an existing artifact names one;
- one missing-input command for every absent artifact;
- `next_command` set to the first missing command, or `null` when no required
  artifact is missing;
- stale-looking warnings for timestamp drift between verify and snapshots, and
  between receipt and verify;
- output schema, traceability, capability, and campaign entries that point to
  the behavior.

## Non-Goals

The LLM work loop must not:

- edit source files;
- generate tests;
- run mutation testing;
- run repo analysis inside `ripr agent status`;
- refresh LSP state;
- add speculative editor surfaces;
- add public crates;
- change the existing brief, packet, verify, or receipt schemas.

## Acceptance Examples

- `ripr agent status --root . --json` succeeds without repo analysis when the
  root directory exists.
- Missing artifacts do not fail the command; they are reported with matching
  next commands.
- Malformed or unreadable present JSON artifacts are warnings, not hidden
  failures.
- The command can recover a seam from receipt, verify, packet, or brief JSON.
- Path arguments with spaces are quoted in generated next commands.
- No automatic edits, generated tests, runtime mutation execution, speculative
  LSP features, or new public crates are added.

## Test Mapping

- `crates/ripr/src/app/agent_status.rs::tests::agent_status_reports_missing_artifacts_and_next_commands`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_receipt`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_verify_packet_or_brief`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_warns_when_verify_or_receipt_look_stale`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_quotes_paths_with_spaces`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_parses_root_and_json`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_requires_json_and_rejects_unknown_arguments`
- `crates/ripr/src/cli/commands.rs::tests::agent_status_rejects_missing_root_before_reading_artifacts`
- `crates/ripr/src/loop_commands.rs::tests::workflow_commands_preserve_public_templates`
- `crates/ripr/src/loop_commands.rs::tests::editor_agent_commands_preserve_public_templates`
- `crates/ripr/src/lsp/tests.rs::agent_loop_command_payloads_stay_workspace_relative_for_platform_roots`
- `xtask/src/reports/operator.rs::tests::operator_cockpit_matches_editor_agent_loop_fixture`

## Implementation Mapping

- `crates/ripr/src/loop_commands.rs` centralizes the workflow and editor-agent
  artifact path profiles plus command templates used by status, brief, LSP,
  pilot, generated CI, and cockpit surfaces.
- `crates/ripr/src/app/agent_status.rs` builds and renders the report from
  existing artifact files.
- `crates/ripr/src/cli/agent.rs` parses the JSON-only status subcommand.
- `crates/ripr/src/cli/commands.rs` validates the root and dispatches the
  report.
- `crates/ripr/src/lsp/actions.rs` uses the shared editor-agent command
  templates for copied LSP commands.
- `crates/ripr/src/cli/help.rs` documents the command surface.
- `xtask/src/reports/operator.rs` uses the shared editor-agent command
  templates for cockpit missing-input guidance.
- `docs/OUTPUT_SCHEMA.md` defines the Agent Status output contract.
- `.ripr/traceability.toml` maps this spec to tests, code, outputs, and
  metrics.

## Metrics

- `agent_loop_status_available`
- missing artifact count by status report
- stale-looking warning count by status report
- recovered seam source distribution
