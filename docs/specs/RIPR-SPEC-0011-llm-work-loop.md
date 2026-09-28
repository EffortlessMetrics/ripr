# RIPR-SPEC-0011: LLM Work Loop

Status: proposed

Lifecycle review: 2026-08-25. Retained as proposed; acceptance remains blocked
on the complete work-loop artifact provenance, command-template, and boundary contract.

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
- select `next_command` by the repair-attempt-first order below (#3906), which
  falls back to the first missing command only when no repair attempt applies;
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

`ripr agent receipt --root . --verify-json <agent-verify-json> --seam-id <id>
--json` records provenance for the static artifacts behind the selected seam
receipt. The receipt renderer still reads existing artifacts only. It must not
run analysis, edit source files, generate tests, run mutation testing, call LLM
APIs, refresh LSP state, or claim runtime adequacy.

Receipt provenance records:

- `ripr_version`;
- `repo_root`;
- `config_fingerprint` when `ripr.toml` exists and can be read without running
  analysis;
- `command_template_version`;
- `generated_at`;
- before, after, and verify artifact paths plus SHA-256 hashes;
- selected `seam_id`, before class, after class, and movement;
- explicit static boundary flags.

`ripr agent review-summary --root .` reads existing artifacts and emits a
compact Markdown packet for PR review. `--json` emits the schema `0.1` JSON
contract. The command joins agent status, workflow, receipt, operator cockpit,
repo exposure, LSP cockpit when present, and local CI artifact file state. It
must not run analysis, edit source files, generate tests, run mutation testing,
call LLM APIs, refresh LSP state, or query GitHub Actions.

The review summary should answer:

- which seam was targeted;
- what static before/after movement the receipt records;
- which receipt and verify artifacts carry the evidence;
- which joined surfaces are present, missing, optional-missing, or malformed;
- what command should run next when the loop is incomplete;
- what the reviewer should inspect;
- which static limits remain.

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
  "test_run": null,
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

The agent receipt uses schema version `0.3` once provenance and structured
next-action guidance are present:

```json
{
  "schema_version": "0.3",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "agent_verify_json": "target/ripr/workflow/agent-verify.json",
    "before": "target/ripr/workflow/before.repo-exposure.json",
    "after": "target/ripr/workflow/after.repo-exposure.json"
  },
  "provenance": {
    "ripr_version": "0.7.0",
    "repo_root": ".",
    "config_fingerprint": "fnv1a64:4c94a2f6cfaa5c21",
    "command_template_version": "0.1",
    "generated_at": "unix_ms:1778179200000",
    "workflow_artifact": null,
    "before_artifact": {
      "path": "target/ripr/workflow/before.repo-exposure.json",
      "sha256": "sha256:..."
    },
    "after_artifact": {
      "path": "target/ripr/workflow/after.repo-exposure.json",
      "sha256": "sha256:..."
    },
    "verify_artifact": {
      "path": "target/ripr/workflow/agent-verify.json",
      "sha256": "sha256:..."
    },
    "seam_id": "67fc764ba37d77bd",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "movement": "improved",
    "limits": {
      "static_artifact_relationship": true,
      "runtime_mutation_execution": false,
      "runtime_adequacy_claim": false
    }
  },
  "summary": {
    "remaining_gap": "No remaining static gap is named by this receipt; inspect the current seam packet if review needs final assertion detail.",
    "next_recommendation": "Run the focused test with the project's test command and keep it only if it passes; ripr compared static evidence and did not run it. Then attach this receipt with the agent verify JSON.",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review.",
      "safe_to_merge": false
    }
  }
}
```

Receipt next-action guidance stays static and bounded. It derives only from the
selected seam movement in the saved `agent verify` JSON:

| Movement | `next_action.kind` | Guidance |
| --- | --- | --- |
| `improved` | `improved` | Run the focused test and keep it only if it passes; ripr did not run it. Then include the receipt in review. |
| `changed` | `changed` | Inspect the evidence delta and strengthen the discriminator named by the packet. |
| `regressed` | `regressed` | Revisit the test or code change before merge. |
| `unchanged` | `unchanged` | Add the missing discriminator or stronger assertion named by the packet. |
| `new` | `new_gap` | Generate a fresh packet or brief for the seam. |
| `resolved` | `resolved` | Confirm the seam disappeared intentionally before relying on the receipt. |

`safe_to_merge` remains `false` for every static receipt. The receipt is
review evidence, not a merge policy or runtime adequacy claim.

The agent review summary uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "ready",
  "root": ".",
  "target_seam": {
    "seam_id": "67fc764ba37d77bd",
    "source": "agent_receipt",
    "file": "src/lib.rs",
    "line": 42,
    "seam_kind": "predicate_boundary"
  },
  "static_movement": {
    "state": "improved",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "grip_class": "strongly_gripped",
    "evidence_artifact": "target/ripr/reports/agent-receipt.json",
    "verify_artifact": "target/ripr/workflow/agent-verify.json",
    "summary": "Static movement is improved (weakly_gripped -> strongly_gripped).",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review."
    }
  },
  "next_command": null,
  "surfaces": [],
  "ci_artifacts": [],
  "reviewer_summary": {
    "headline": "Review packet is ready for seam 67fc764ba37d77bd.",
    "what_changed": "Static movement is improved (weakly_gripped -> strongly_gripped).",
    "evidence": "Review target/ripr/reports/agent-receipt.json with target/ripr/workflow/agent-verify.json.",
    "remaining": "Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review.",
    "reviewer_should_inspect": [
      "target/ripr/reports/agent-receipt.json",
      "target/ripr/workflow/agent-verify.json"
    ]
  },
  "limits": {
    "static_artifact_relationship": true,
    "runtime_mutation_execution": false,
    "automatic_edits": false,
    "generated_tests": false
  }
}
```

## Amendment (#3906): next command selection

The legacy rule "first missing artifact" sent a user with a prepared repair
attempt back into the manual snapshot loop, sent a fresh workspace to a Bash
redirect into `target/ripr/workflow/` before that directory existed, and sent
a failed attempt to `ripr agent receipt`. `ripr agent status` now also reads
`target/ripr/repair-attempts/*/attempt.json` through the attempt authority
(`app::repair_attempt::inventory_repair_attempts`, the same validation the
after phase applies) and selects `next_command` in this order:

1. **Unreadable inventory.** If any attempt manifest fails validation, select
   nothing and warn `repair_attempt_unreadable`, naming the attempt.
2. **Resume.** Exactly one attempt in `awaiting_edit` whose after phase would
   evaluate the current `HEAD` as the attempt's: its recorded `next_command`
   (`--attempt <id> --phase after`), step `repair_attempt_after`. Status asks
   the attempt authority's head rule
   (`app::repair_attempt::after_phase_head_admission`), the same one the after
   phase applies: the prepared head, or for an ordinary attempt a commit that
   descends from it (a focused test committed between the phases). A
   trust-bound attempt needs the exact prepared head.
3. **Ambiguous.** More than one such attempt: select nothing and warn
   `ambiguous_repair_attempts`, listing each attempt's command. Status never
   picks the newest (docs/REPAIR_ATTEMPT.md).
4. **Head unknown.** `HEAD` cannot be read, so rules 2 and 3 match no
   attempt. If any attempt is `awaiting_edit`, select nothing and warn
   `repair_attempt_head_unknown` rather than restarting its seam.
5. **Restart.** Group attempts by seam. A seam with a `ready_to_finish`
   attempt is finished. A seam whose attempts all ended (`failed`, `stale`,
   `incomparable`), were prepared at another `HEAD`, or never left `prepared`
   is open. (Refined below: a `ready_to_finish` attempt finishes its seam only
   through the receipt issued for it.) An ordinary attempt "prepared at
   another `HEAD`" is one whose `HEAD` no longer descends from its prepared
   head; its reason repeats the after phase's recovery (`git reset --soft
   <prepared-head>` then the attempt's after command) before the new-attempt
   route. Exactly one open seam: `ripr agent repair --root <root> --seam-id
   <seam> --phase before`, step `repair_attempt_before`. Several: select
   nothing and warn `multiple_open_repair_seams`, listing each start command.
6. **Legacy loop.** Otherwise the first missing artifact's command, with two
   refusals. With no known seam the next command is `ripr pilot --root
   <root>` (step `select_seam`), never a `<seam-id>` placeholder, except
   that a complete pilot summary whose top seam recorded no repair start
   (`next.repair_command: null`) selects nothing and warns
   `pilot_found_no_repair_target`: rerunning pilot would rank the same seam
   again, so the warning names the hand step (a test in the seam's own
   crate) instead. Likewise a complete pilot summary that ranked no seam,
   recorded no repair card (`python_first_use` absent, `null`, or status
   `no_python_findings` or `no_repair_cards`; `analysis_unavailable` is not
   this fact), routed the code to `ripr check` (`language_routes.state:
   required` with a recorded route command) and recorded no repair start
   selects nothing and warns `pilot_routed_to_check_no_repair_target`, for
   any routed language. For an enabled route the warning names the recorded
   check command and the hand step (add or strengthen a test, then rerun
   that check); for a route with `enabled: false` it names the enable step
   (add the language to `[languages] enabled` in `ripr.toml`) instead. It
   also says to rerun pilot if the workspace changed since that run. A
   timed-out, missing or unreadable summary still routes to pilot. When the
   first missing artifact's directory does not exist and the seam is known,
   the next command starts a repair attempt, which writes the workflow
   artifacts itself, instead of redirecting into the missing directory.

`status` is `incomplete` whenever a next command is selected. The JSON report
adds `repair_attempts` (`attempt_id`, `seam_id`, `state`, `head_current`,
`disposition`, `manifest`, `command`); `missing_commands` keeps its legacy
meaning. `ripr agent review-summary` carries the same `next_command`.

### Refinement: the attempt's outcome, not only its state

`ready_to_finish` means the edit cage admitted the edit; it does not mean the
gap closed. Status reads the workflow receipt against each `ready_to_finish`
attempt, matching the receipt's `repair_attempt` binding (attempt ID, after
`HEAD`, delta and packet digests) to the attempt's after verdict, and
interprets it through the receipt owner (`output::agent_receipt`):

- `advisory` with movement `improved`: `finished`. Status words it as static
  grip improved (receipt advisory), not as a closed gap. The loop is
  `complete` only when the current `HEAD` is the head the attempt's after
  phase recorded (which may descend from its prepared head); otherwise a
  `repair_receipt_stale` warning names both heads.
- movement `unchanged`, `changed`, or `regressed`, whatever the receipt
  status: `gap_open`. The seam is open, and rule 5 starts a new attempt whose
  reason names the receipt status, movement, staleness, and the receipt's own
  next action.
- anything else (an `invalid` or `incomplete` improved receipt, a new or
  resolved seam, or no receipt issued for the attempt): `unconfirmed`. The seam
  is not restarted and a `repair_receipt_unconfirmed` warning says why.
- the workflow receipt is bound to another attempt: the workflow keeps one
  receipt, so a later attempt's after phase superseded this attempt's.
  `receipt.superseded_by` names that attempt; the reading is `unconfirmed`,
  because the earlier outcome can no longer be read, and the warning names the
  new-attempt command for the seam in case its gap is still open.

An after phase that refuses after selecting its attempt records the refusal on
the attempt (`last_after_refusal`, owned by `app::repair_attempt`): the final
error followed by the cause and recovery the after phase printed (for example
the changed analysis inputs), bounded by the attempt authority. Rule 2 still
resumes that attempt, but its reason names the refusal before repeating the
command. The next after phase that reaches the durable finish clears it.

Rerunning the after phase of an attempt that is no longer awaiting its edit is
refused with the attempt's documented state, whether it finished or ended, the
workflow receipt path, and the next step (`ripr agent status`, or a new
`--phase before` while the gap is open).

Recorded resume commands keep the `--root` spelling the before phase was given,
because the manifest is the authority for that command (including the
`--edit-authorized` suffix of trust-bound Python attempts).

## Required Evidence

The first LLM work-loop slice requires:

- a JSON status report with schema version `0.1`;
- artifact presence for before snapshot, after snapshot, agent brief, agent
  packet, agent verify, and agent receipt;
- recoverable seam identity when an existing artifact names one;
- one missing-input command for every absent artifact;
- `next_command` selected by the #3906 order below, or `null` when nothing is
  missing or when status cannot choose honestly;
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

The receipt provenance slice additionally requires:

- agent receipt schema version `0.3`;
- SHA-256 hashes for before, after, and verify artifacts;
- `ripr_version`, `repo_root`, optional config fingerprint, command template
  version, and render timestamp;
- selected seam identity, before class, after class, and movement copied from
  the verify artifact;
- static boundary flags that do not claim runtime adequacy;
- bounded `summary.next_action` guidance for improved, changed, regressed,
  unchanged, new-gap, and resolved receipt states;
- fixture, schema, traceability, capability, and campaign updates.

The reviewer-summary slice additionally requires:

- `ripr agent review-summary --root .` Markdown output;
- `ripr agent review-summary --root . --json` schema `0.1` output;
- read-only joins for agent status, workflow, receipt, operator cockpit, repo
  exposure, LSP cockpit when present, and local CI artifact file state;
- target seam recovery from receipt first, then workflow, then status;
- static movement and next-action text copied from the receipt;
- an incomplete state with the next status command when the receipt is missing;
- explicit static-limit flags;
- schema, traceability, capability, and campaign updates.

The fixture slice additionally requires checked, artifact-only work-loop cases
for:

- happy path with improved static movement;
- unchanged static movement;
- regressed static movement;
- missing artifact recovery;
- stale-looking verify or receipt artifacts;
- configured-off seam handoff;
- path arguments with spaces;
- Windows separator normalization.

## Non-Goals

The LLM work loop must not:

- edit source files;
- generate tests;
- run mutation testing;
- run repo analysis inside `ripr agent status`;
- refresh LSP state;
- add speculative editor surfaces;
- add public crates;
- change the existing brief, packet, or verify schemas. Receipt schema changes
  are limited to the provenance-backed `0.2` shape and the next-action `0.3`
  additive shape.

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
- `ripr agent receipt` records artifact hashes and static provenance without
  rerunning analysis.
- `ripr agent receipt` emits structured, static `summary.next_action` guidance
  for improved, changed, regressed, unchanged, new-gap, and resolved states.
- `ripr agent review-summary --root .` emits compact Markdown without rerunning
  analysis.
- `ripr agent review-summary --root . --json` joins status, receipt, cockpit,
  repo exposure, optional LSP cockpit, and local CI artifact status.
- Generated GitHub CI writes and uploads workflow packet artifacts under
  `target/ripr/workflow`, including status JSON/Markdown and review summary
  JSON/Markdown, while keeping the job advisory.
- `docs/LLM_OPERATOR_GUIDE.md` documents the source-edit-free operator loop
  from status through workflow packet, focused test target, after snapshot,
  verify, receipt, and reviewer summary.
- Missing optional cockpit artifacts are visible state, not command failures.
- A missing receipt yields `status: incomplete` and carries the next command
  from agent status.
- No automatic edits, generated tests, runtime mutation execution, speculative
  LSP features, or new public crates are added.

## Amendment (#4396): selected-root editor command copy

The LSP action payload keeps `root: "."` as a portable role, while its
copyable command binds the selected workspace as an absolute `--root`. The VS
Code copy handler accepts a fixed agent-loop action only when its command body
exactly matches the server template for the selected workspace (or its realpath)
and an output redirect, when present, names the declared artifact under that
**same** root spelling. Relative redirects, another workspace root, traversal,
extra shell tokens, and ambiguous quoting fail closed. The separate gap command
labels retain their own bounded legacy contract. Copying a command is advisory;
it does not execute the command or grant edit authority.

## Test Mapping

- `crates/ripr/src/app/agent_status.rs::tests::agent_status_reports_missing_artifacts_and_next_commands`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_receipt`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_recovers_seam_id_from_verify_packet_or_brief`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_warns_when_verify_or_receipt_look_stale`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_quotes_paths_with_spaces`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_refuses_to_choose_between_open_seams`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_does_not_restart_a_seam_that_finished`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_never_redirects_into_a_missing_workflow_directory`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_stops_when_pilot_routed_changed_code_to_check`
- `crates/ripr/tests/cli_smoke.rs::agent_status_routes_a_fresh_workspace_to_pilot`
- `crates/ripr/tests/cli_smoke.rs::agent_status_resumes_the_current_awaiting_repair_attempt`
- `crates/ripr/tests/cli_smoke.rs::agent_status_resumes_only_the_current_attempt_and_refuses_to_guess`
- `crates/ripr/tests/cli_smoke.rs::agent_status_restarts_a_failed_attempt_and_completes_a_finished_one`
- `crates/ripr/tests/cli_smoke.rs::agent_status_selects_nothing_past_an_unreadable_attempt`
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
- `crates/ripr/src/agent/loop_commands.rs::tests::anchored_redirect_target_roots_relative_outputs_at_root`
- `crates/ripr/src/agent/loop_commands.rs::tests::bound_roots_render_absolute_and_relative_roots_stay_portable`
- `crates/ripr/src/agent/loop_commands.rs::tests::bound_root_keeps_a_unix_backslash_directory_name`
- `crates/ripr/src/app/agent_status.rs::tests::pilot_select_command_binds_raw_and_bound_roots_once`
- `crates/ripr/tests/generated_review_workflow.rs::generated_status_command_runs_from_a_foreign_working_directory`
- `crates/ripr/src/lsp/tests.rs::agent_loop_command_payloads_stay_root_anchored_for_platform_roots`
- `editors/vscode/test/suite/extension.test.ts::agent loop commands must equal the body the server renders from the payload (#4225)`
- `editors/vscode/test/suite/extension.test.ts::real server surfaces seam diagnostic, hover provider, and agent actions`
- `crates/ripr/src/output/agent_workflow.rs::tests::workflow_json_is_structured_and_advisory`
- `crates/ripr/src/output/agent_workflow.rs::tests::workflow_markdown_lists_commands_and_boundaries`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_json_selects_changed_seam`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_improved_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_changed_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_regressed_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_unchanged_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_new_gap_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_guidance_covers_resolved_state`
- `crates/ripr/src/output/agent_receipt.rs::tests::agent_receipt_input_paths_extracts_verify_snapshot_paths`
- `crates/ripr/src/agent/provenance.rs::tests::sha256_file_hashes_artifact_bytes`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_review_summary_joins_status_receipt_cockpit_repo_and_lsp`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_llm_work_loop_review_summary_fixtures_pin_core_states`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_llm_work_loop_review_summary_fixture_pins_missing_artifact`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_llm_work_loop_review_summary_fixture_pins_stale_artifact`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_llm_work_loop_review_summary_fixtures_pin_path_arguments`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_review_summary_reports_missing_receipt_with_next_command`
- `crates/ripr/src/app/agent_review_summary.rs::tests::agent_review_summary_markdown_names_review_focus_and_limits`
- `crates/ripr/src/cli/agent.rs::tests::agent_review_summary_parses_root_json_and_human_default`
- `crates/ripr/src/cli/agent.rs::tests::agent_review_summary_requires_values_and_rejects_unknown_arguments`
- `crates/ripr/src/cli/commands.rs::tests::agent_review_summary_rejects_missing_root_before_reading_artifacts`
- `crates/ripr/tests/cli_smoke.rs::agent_receipt_writes_one_seam_handoff_json`
- `crates/ripr/tests/cli_smoke.rs::agent_start_writes_source_edit_free_workflow_packet`
- `crates/ripr/tests/cli_smoke.rs::agent_packet_rejects_configured_off_seam`
- `crates/ripr/src/lsp/tests.rs::agent_loop_command_payloads_stay_workspace_relative_for_platform_roots`
- `xtask/src/reports/operator.rs::tests::operator_cockpit_matches_editor_agent_loop_fixture`

## Implementation Mapping

- `crates/ripr/src/app/agent_status.rs` builds and renders the report from
  existing artifact files and the repair-attempt inventory.
- `crates/ripr/src/app/repair_attempt.rs::inventory_repair_attempts` lists and
  validates attempt manifests read-only for status.
- `crates/ripr/src/app/agent_review_summary.rs` joins existing agent status,
  workflow, receipt, cockpit, repo exposure, optional LSP cockpit, and local
  CI artifact file state into review-summary JSON and Markdown.
- `crates/ripr/src/app/agent_workflow.rs` builds a selected-seam workflow
  manifest from the generated agent brief and shared command templates.
- `crates/ripr/src/agent/loop_commands.rs` owns internal command and artifact
  templates for status, brief, LSP copy actions, pilot next commands, generated
  CI paths, and cockpit missing-input commands.
- `editors/vscode/src/client.ts` validates copied LSP command templates against
  the selected workspace root and artifact redirect before clipboard write.
- `crates/ripr/src/agent/provenance.rs` hashes receipt artifacts with SHA-256.
- `crates/ripr/src/cli/agent.rs` parses the status, start, and review-summary
  subcommands.
- `crates/ripr/src/cli/commands.rs` validates the root and dispatches the
  report, builds receipt provenance from existing artifacts, and reuses shared
  path templates for generated GitHub workflow agent artifacts.
- `crates/ripr/src/cli/help.rs` documents the command surface.
- `crates/ripr/src/cli/commands.rs` renders the generated GitHub workflow that
  uploads LLM work-loop packet artifacts without changing analyzer behavior.
- `docs/LLM_OPERATOR_GUIDE.md` documents the human and external-agent operating
  loop and anti-goals.
- `crates/ripr/src/output/agent_workflow.rs` renders the workflow JSON and
  commands Markdown.
- `crates/ripr/src/output/agent_receipt.rs` renders receipt schema `0.3` with
  provenance and structured next-action guidance.
- `crates/ripr/src/output/agent_brief.rs`, `crates/ripr/src/output/pilot/mod.rs`,
  and `crates/ripr/src/lsp/actions.rs` reuse the shared command builders for
  their current command payloads.
- `xtask/src/reports/operator.rs` reuses the shared command builder source for
  editor-agent cockpit missing-input commands.
- `docs/OUTPUT_SCHEMA.md` defines the Agent Status, Agent Workflow Manifest,
  Agent Receipt, and Agent Review Summary output contracts.
- `.ripr/traceability.toml` maps this spec to tests, code, outputs, and
  metrics.

## Metrics

- `agent_loop_status_available`
- `agent_workflow_manifest_available`
- `agent_receipt_provenance_available`
- `agent_receipt_next_action_guidance_available`
- `agent_review_summary_available`
- missing artifact count by status report
- stale-looking warning count by status report
- recovered seam source distribution
