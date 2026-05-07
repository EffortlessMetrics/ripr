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

The loop command templates are centralized in one internal module before the
workflow manifest is introduced. That module owns the current workflow artifact
paths, the editor/CI pilot-agent artifact paths, and the command builders for:

```text
ripr agent start
ripr check --format repo-exposure-json
ripr check --format agent-seam-packets-json
ripr agent packet
ripr agent brief
ripr agent verify
ripr agent receipt
ripr agent status
ripr agent review-summary
ripr outcome
```

Current consumers must preserve their existing emitted command text while
sharing these builders where they construct command payloads or missing-input
commands.

`ripr agent start --root . --seam-id <id> --out target/ripr/workflow` writes a
source-edit-free workflow packet for one visible seam:

```text
target/ripr/workflow/workflow.json
target/ripr/workflow/commands.md
target/ripr/workflow/agent-brief.json
```

The command may run the same static seam selection used by
`ripr agent brief --seam-id`, because the manifest needs the selected seam's
missing discriminator, suggested assertion shape, related-test target, and
effective mode. It must not edit source files, generate tests, run mutation
testing, call LLM APIs, refresh LSP state, configure CI blocking, or add
vendor-specific prompt/model behavior. The packet is deterministic context for
humans and external agents.

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

The workflow manifest uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "ready",
  "root": ".",
  "mode": "draft",
  "out_dir": "target/ripr/workflow",
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "file": "src/pricing.rs",
    "line": 88,
    "seam_kind": "predicate_boundary",
    "grip_class": "weakly_gripped",
    "why": "caller requested seam_id 67fc764ba37d77bd",
    "missing_discriminator": "amount == discount_threshold",
    "assertion_shape": "assert_eq!(...)",
    "recommended_test_file": "tests/pricing.rs",
    "recommended_test_name": "discount_threshold_equality_boundary_is_asserted",
    "related_test_to_imitate": "applies_discount_above_threshold"
  },
  "outputs": {
    "workflow_manifest": "target/ripr/workflow/workflow.json",
    "commands_markdown": "target/ripr/workflow/commands.md",
    "agent_brief": "target/ripr/workflow/agent-brief.json"
  },
  "artifacts": [],
  "commands": [],
  "missing_inputs": [],
  "next_command": null,
  "boundaries": {
    "source_edits": false,
    "generated_tests": false,
    "runtime_mutation_execution": false,
    "llm_api_calls": false,
    "ci_blocking": false
  }
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
- shared command templates for the existing status next commands, agent brief
  next commands, LSP copy actions, pilot next commands, generated CI artifact
  paths, and operator cockpit missing-input commands.

The workflow manifest slice additionally requires:

- `ripr agent start --root . --seam-id <id> --out target/ripr/workflow`;
- `workflow.json`, `commands.md`, and `agent-brief.json` outputs;
- selected seam details from the agent brief;
- artifact paths and commands for before snapshot, packet, brief, after
  snapshot, verify, and receipt;
- explicit boundary flags proving the packet does not edit source, generate
  tests, call LLM APIs, run mutation testing, or configure CI blocking.

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
- LSP agent-loop copy action command payloads remain byte-for-byte compatible
  with the existing fixture expectations.
- Operator cockpit missing-input commands remain fixture-compatible while
  sharing the same template source as the CLI/LSP command builders.
- No automatic edits, generated tests, runtime mutation execution, speculative
  LSP features, or new public crates are added.

## Test Mapping

- `crates/ripr/src/app/agent_status.rs::tests::agent_status_reports_missing_artifacts_and_next_commands`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_receipt`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_verify_packet_or_brief`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_warns_when_verify_or_receipt_look_stale`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_quotes_paths_with_spaces`
- `crates/ripr/src/app/agent_workflow.rs::tests::workflow_manifest_extracts_seam_and_commands`
- `crates/ripr/src/app/agent_workflow.rs::tests::workflow_manifest_errors_when_brief_does_not_return_seam`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_parses_root_and_json`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_requires_json_and_rejects_unknown_arguments`
- `crates/ripr/src/cli/agent.rs::tests::agent_args_parse_start_request`
- `crates/ripr/src/cli/agent.rs::tests::agent_start_defaults_out_dir_and_requires_seam_id`
- `crates/ripr/src/cli/commands.rs::tests::agent_status_rejects_missing_root_before_reading_artifacts`
- `crates/ripr/src/cli/commands.rs::tests::agent_start_rejects_missing_root_before_analysis`
- `crates/ripr/src/agent/loop_commands.rs::tests::workflow_commands_match_existing_status_templates`
- `crates/ripr/src/agent/loop_commands.rs::tests::editor_commands_match_existing_lsp_templates`
- `crates/ripr/src/output/agent_workflow.rs::tests::workflow_json_is_structured_and_advisory`
- `crates/ripr/src/output/agent_workflow.rs::tests::workflow_markdown_lists_commands_and_boundaries`
- `crates/ripr/tests/cli_smoke.rs::agent_start_writes_source_edit_free_workflow_packet`
- `crates/ripr/src/lsp/tests.rs::agent_loop_command_payloads_stay_workspace_relative_for_platform_roots`
- `xtask/src/reports/operator.rs::tests::operator_cockpit_matches_editor_agent_loop_fixture`

## Implementation Mapping

- `crates/ripr/src/app/agent_status.rs` builds and renders the report from
  existing artifact files.
- `crates/ripr/src/app/agent_workflow.rs` builds a selected-seam workflow
  manifest from the generated agent brief and shared command templates.
- `crates/ripr/src/agent/loop_commands.rs` owns internal command and artifact
  templates for status, brief, LSP copy actions, pilot next commands, generated
  CI paths, and cockpit missing-input commands.
- `crates/ripr/src/cli/agent.rs` parses the status and start subcommands.
- `crates/ripr/src/cli/commands.rs` validates the root and dispatches the
  report, and reuses shared path templates for generated GitHub workflow agent
  artifacts.
- `crates/ripr/src/cli/help.rs` documents the command surface.
- `crates/ripr/src/output/agent_workflow.rs` renders the workflow JSON and
  commands Markdown.
- `crates/ripr/src/output/agent_brief.rs`, `crates/ripr/src/output/pilot.rs`,
  and `crates/ripr/src/lsp/actions.rs` reuse the shared command builders for
  their current command payloads.
- `xtask/src/reports/operator.rs` reuses the shared command builder source for
  editor-agent cockpit missing-input commands.
- `docs/OUTPUT_SCHEMA.md` defines the Agent Status output contract.
- `.ripr/traceability.toml` maps this spec to tests, code, outputs, and
  metrics.

## Metrics

- `agent_loop_status_available`
- `agent_workflow_manifest_available`
- missing artifact count by status report
- stale-looking warning count by status report
- recovered seam source distribution
