# LLM Operator Guide

Use this guide when a human or external coding agent needs a bounded RIPR work
packet for one focused test. RIPR does not call a model, choose a provider,
generate tests, or edit source files. It compiles static test-gap context into
artifacts that a human, CI reviewer, or LLM tool can inspect.

The loop is:

```text
agent status
-> workflow packet
-> focused test target
-> after snapshot
-> agent verify
-> agent receipt
-> reviewer summary
-> CI/editor artifacts
```

## Boundaries

RIPR remains a static-analysis context compiler. In this workflow it must not:

- call LLM APIs;
- select or configure a model;
- generate tests;
- edit production or test source;
- run mutation testing;
- claim runtime confirmation;
- block CI by default;
- refresh LSP state or analyze unsaved buffers.

The expected agent behavior is narrow:

```text
read the packet
write one focused test outside RIPR
rerun the after snapshot
verify the static movement
attach the receipt
```

## Artifact Paths

The Campaign 11 workflow uses fixed paths so humans, CI, and agents can resume
without chat history:

| Artifact | Path |
| --- | --- |
| Before snapshot | `target/ripr/workflow/before.repo-exposure.json` |
| After snapshot | `target/ripr/workflow/after.repo-exposure.json` |
| Analysis outcome | `target/ripr/workflow/analysis-outcome.json` |
| Workflow manifest | `target/ripr/workflow/workflow.json` |
| Workflow commands | `target/ripr/workflow/commands.md` |
| Agent brief | `target/ripr/workflow/agent-brief.json` |
| Agent packet | `target/ripr/workflow/agent-packet.json` |
| Agent verify | `target/ripr/workflow/agent-verify.json` |
| Agent receipt | `target/ripr/reports/agent-receipt.json` |
| Agent status | `target/ripr/workflow/agent-status.{json,md}` |
| Agent review summary | `target/ripr/workflow/agent-review-summary.{json,md}` |

Generated GitHub CI also uploads these paths under the `ripr-reports` artifact.
It keeps compatibility copies of packet, brief, verify, and receipt JSON under
`target/ripr/agent/`.

## Ordinary path: `ripr agent repair`

For Rust, ignore Cargo's entire `target/` directory before starting (for example
`/target/` in `.gitignore`, or an equivalent Git exclude rule). The before phase
checks the effective rule even with no build output and refuses before preparing
workflow artifacts or an attempt if it is missing. This keeps the focused Cargo
test's build output separate from the enforced test-only edit surface.

For one named gap, the repair transaction writes the snapshot, packet, verify,
and receipt artifacts above for you:

```bash
ripr agent repair --root . --seam-id <seam-id> --phase before
# edit one focused test outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

The before phase writes the before snapshot, brief, packet, and workflow files
and prints the exact `--attempt` command for the after phase. The after phase
writes the after snapshot, analysis outcome, verify JSON, and receipt. Its
stdout is exactly one JSON document — the versioned `repair_after_result`
envelope (`schema_version` `0.1`) carrying the verify 0.3 document unchanged
under `verify` with the status report embedded beside it under
`agent_status` — so an orchestrator can `JSON.parse` stdout once and every
stdout document's shape is identifiable from its `schema_version`; narration
stays on stderr. When the after phase refuses after the verify render, stdout
is the bare verify 0.3 document alone; a typed refusal before it (diverged
HEAD, drifted analysis inputs, no movement; exit `3`) prints the
`repair_after_refusal` document with the cause and recovery. For the
separately authorized `verify` phase of a trust-bound Python attempt, see
[Repair attempt identity](REPAIR_ATTEMPT.md). The numbered steps below are the
lower-level manual equivalent, kept for explicit control and debugging.

## 1. Check Status

Start by asking RIPR what already exists:

```bash
ripr agent status --root .
```

For machine-readable state:

```bash
ripr agent status --root . --json > target/ripr/workflow/agent-status.json
```

`agent status` reads existing artifacts only. It reports the repair attempts
under `target/ripr/repair-attempts/` and the workflow artifacts above as present
or missing, recovers a `seam_id` when possible, warns about stale-looking verify
or receipt files, and prints one next command:

| State | Next command |
| --- | --- |
| No seam known yet | `ripr pilot --root <root>`, which selects the seam |
| One repair attempt waiting for its test edit at the current `HEAD`, or with that test committed on top of its prepared head | that attempt's `ripr agent repair --attempt <id> --phase after`; when its last after phase was refused, the reason names the refusal first |
| A seam whose attempt failed, went stale, or was prepared at a `HEAD` the current one does not descend from | `ripr agent repair --seam-id <seam-id> --phase before`, which starts a new attempt; for rewritten history the reason first names the `git reset --soft <prepared-head>` recovery that resumes the attempt |
| A seam whose finished attempt's receipt shows grip `unchanged`, `changed`, or `regressed` | `ripr agent repair --seam-id <seam-id> --phase before`: the gap is still open, so start a new attempt and strengthen the test |
| A finished attempt whose receipt is `invalid` or `incomplete`, or whose evidence was recorded at another `HEAD` | none; status is `warning` and the warning says which |
| Every artifact present, and any finished attempt's receipt is `advisory` with grip `improved` at the current `HEAD` | none; status is `complete`, and `test_run.status` is `not_recorded`: run the focused test yourself and keep it only if it passes |
| Otherwise | the first missing artifact's command below |

When picking would mean guessing (two waiting attempts, several open seams, an
unreadable attempt manifest, or an unreadable `HEAD`), status prints no next
command and a warning that lists the choices. It never picks the newest attempt.
The full selection order is in RIPR-SPEC-0011 (amendment #3906).

The numbered steps below are the manual path. If no before snapshot exists yet,
create one:

```bash
mkdir -p target/ripr/workflow
ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json
```

If you already ran `ripr pilot`, you can reuse its snapshot:

```bash
mkdir -p target/ripr/workflow
cp target/ripr/pilot/repo-exposure.json target/ripr/workflow/before.repo-exposure.json
cp target/ripr/pilot/agent-seam-packets.json target/ripr/workflow/agent-seam-packets.json
```

## 2. Start A Workflow Packet

Choose one seam from `ripr pilot`, repo exposure, a CI annotation, or an editor
diagnostic, then write a source-edit-free workflow packet:

```bash
ripr agent start --root . --seam-id <seam-id> --out target/ripr/workflow
```

This writes:

```text
target/ripr/workflow/workflow.json
target/ripr/workflow/commands.md
target/ripr/workflow/agent-brief.json
```

The manifest names the selected seam, missing discriminator or observation,
recommended test target when available, artifact paths, and exact commands for
the rest of the loop.

The legacy `command` strings in `workflow.json` use Bash quoting and
redirection; `command_shell: "bash"` describes those display strings.
`commands.md` includes a PowerShell form when a command needs a different one,
including guarded BOM-free UTF-8 output writes. A command without a separate
PowerShell form runs unchanged there unless the document explicitly says the
form is unavailable. Use the emitted form for your shell; cmd.exe is not
supported. WSL Bash is not a drop-in substitute for native Windows paths:
drive-letter paths require translation to `/mnt/...` and a RIPR installation
inside WSL.

Some workflow command objects also carry a producer-owned `command_spec`.
Currently these are the packet and brief regeneration steps when their output
paths satisfy the root-relative validation contract; other steps remain
legacy-string-only. Machine consumers should validate the spec and honor its
`program`, `args`, `working_directory`, policies, and `execution_mode` rather
than reconstruct argv from the display. A `shell_required` spec still needs
shell handling for its redirection; its argv alone does not write the named
artifact. A spec describes a route, not permission to execute it. See the
[workflow output contract](OUTPUT_SCHEMA.md#agent-workflow-manifest) for the complete fields. For the
ordinary focused-test loop, prefer the two-phase `ripr agent repair` path above
to wiring these artifacts together manually.

If the operator needs the full seam packet as well:

```bash
ripr agent packet --root . --seam-id <seam-id> --json > target/ripr/workflow/agent-packet.json
```

If the operator is starting from a gap decision ledger instead of live seam
analysis, the same packet envelope can be rendered straight from a ledger
record:

```bash
ripr agent packet --root . --gap-ledger target/ripr/reports/gap-decision-ledger.json --gap-id <gap-id> --json > target/ripr/workflow/agent-packet.json
```

This route does not rerun analysis: the record is read from the ledger named
in the envelope's `inputs.gap_ledger`, and the packet reports
`source: "gap_decision_ledger"`. It requires the record to carry a
`repair_route` and at least one `verification_commands` entry; `--seam-id`
and `--gap-ledger`/`--gap-id` are mutually exclusive. Against the seam
packet, the per-gap packet fields add gap identity and governance state —
`gap_id`, `canonical_gap_id`, `gap_kind`, `language`, `policy_state`,
`gap_state`, `evidence_class`, `repairability`, `current_evidence_strength` —
plus `allowed_edit_surface`/`allowed_files`/`forbidden_files`,
`conflict_group`, `stop_conditions`, `must_not_change`,
`discriminator_guidance`, `recommended_test`, `verify_command`,
`receipt_command`, `receipt_status`, a `repair_card`, and `llm_guidance`
with the prompt, stop conditions, and a copyable packet. The edit-surface
authority, stop conditions, and verify/receipt commands carry the same
advisory boundary as the seam packet.

If the operator is starting from a working set rather than one seam, use a
brief command instead:

```bash
ripr agent brief --root . --diff target/ripr/reports/pr.diff --json
ripr agent brief --root . --base origin/main --json
ripr agent brief --root . --files src/lib.rs,tests/lib.rs --json
```

Use `agent start` once a seam is selected; use `agent brief` to rank candidate
seams for a diff, base revision, or file list.

## 3. Write One Focused Test

The human or external agent writes the test. RIPR should not edit files.

Use the packet fields as constraints:

- target the named seam;
- exercise the missing discriminator or observation;
- imitate the related test style when one is provided;
- use the suggested assertion shape when present;
- avoid broad snapshot or smoke assertions unless the packet recommends that
  shape;
- avoid production edits unless a reviewer explicitly changes the task.

The desired outcome is one focused test that makes the static evidence clearer.
It is acceptable if static evidence does not move; the receipt should make that
visible.

## 4. Capture The After Snapshot

After the test is added, regenerate repo exposure into the fixed after path:

```bash
ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json
```

Use the same mode as the before snapshot. If CI used `ripr pilot --mode ready`
or a ready-mode before snapshot, keep the after snapshot in ready mode too:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json
```

`ripr agent status` also expects the analysis outcome for the same run before
it reports the loop complete:

```bash
ripr check --root . --mode draft --format json > target/ripr/workflow/analysis-outcome.json
```

## 5. Verify Static Movement

Compare before and after snapshots:

```bash
ripr agent verify \
  --root . \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json \
  --json \
  > target/ripr/workflow/agent-verify.json
```

`agent verify` reports static before/after movement. It does not run tests or
mutation testing.

## 6. Emit The Receipt

Create a focused receipt for the seam:

```bash
ripr agent receipt \
  --root . \
  --verify-json target/ripr/workflow/agent-verify.json \
  --seam-id <seam-id> \
  --json \
  --out target/ripr/reports/agent-receipt.json
```

The receipt records static provenance: RIPR version, repo root, config
fingerprint when available, command-template version, artifact hashes, before
class, after class, movement, and static boundary flags. It stays advisory and
does not claim runtime proof.

Receipt states should be read conservatively:

| State | Reviewer interpretation |
| --- | --- |
| `improved` | Static evidence improved; inspect the focused test and keep the receipt. |
| `unchanged` | Static evidence did not move; inspect whether the test observes the missing discriminator. |
| `regressed` | Static evidence weakened; inspect the test change and classification context. |
| `resolved` | The selected gap no longer appears in the after snapshot. |
| `new_gap` | A new static gap appeared; inspect changed owner/test relation. |
| `missing_artifact` | Run the missing command from `agent status`. |
| `stale_artifact` | Regenerate before/after, verify, or receipt artifacts. |

## 7. Build The Reviewer Summary

Render a compact review packet:

```bash
ripr agent review-summary --root . > target/ripr/workflow/agent-review-summary.md
ripr agent review-summary --root . --json > target/ripr/workflow/agent-review-summary.json
```

The review summary joins:

- computed agent status;
- workflow manifest when present;
- agent receipt;
- operator cockpit when present;
- repo exposure when present;
- LSP cockpit when present;
- local CI artifact file state.

It should tell the reviewer which seam was targeted, what static movement was
recorded, which receipt and verify artifacts carry the evidence, what is still
missing, and what static limits remain.

## Bounded execution: `ripr agent verify-execute`

`ripr agent verify-execute` is the only standalone command in the agent loop
that executes a process; `ripr agent repair --phase verify` also runs the
retained packet's typed verify route through the bounded execution runner. It
accepts one canonical producer-shaped packet and runs only the direct
`ripr agent verify` route that packet declares — no shell, no display-text
execution, no receipt issuance:

```bash
ripr agent verify-execute \
  --root . \
  --packet target/ripr/workflow/agent-packet.json \
  --result-json target/ripr/workflow/verification-execution.json \
  --authorize \
  --json
```

- `--packet` and `--result-json` are required, along with `--json`; `--root`
  selects the repository the packet and result paths must stay inside.
- `--authorize` is required for execution. Omitting it is not a usage error;
  it is a policy refusal reported as the typed
  `verification_rejected_policy` disposition.
- `--cancel-after-ms <n>` requests cancellation after `n` milliseconds; it
  must be a positive value below the command timeout.
- Typed JSON on every terminal state: each run — including pre-execution
  refusals and a failing verify command — emits one JSON response on stdout
  with a `disposition` field (for example `verification_executed_pass`,
  `verification_executed_fail`, `verification_rejected_policy`,
  `verification_wrong_root`, `verification_timed_out`,
  `verification_cancelled`). Usage errors stay plain stderr messages with no
  disposition. The exit status distinguishes only whether a bounded
  observation was committed.

The packet chooses which provenance-validated producer artifacts to compare;
it never authors the command. Route authority is layered: the typed
`command_specs.verify` must be reproducible from the packet's own
`verification_commands`, and the route's `--before`/`--after` inputs must
pass the repo-exposure provenance contract, after which RIPR recomputes the
canonical route and requires equality. See
[`OUTPUT_SCHEMA.md`](OUTPUT_SCHEMA.md#agent-verify-execute) for the full
envelope, environment posture, and declared limitations.

## MCP adapter

`ripr mcp --stdio` exposes a bounded, read-only workspace-status projection
for MCP clients: one tool (`ripr_workspace_status`) and one resource
(`ripr://workspace/status`), both returning the same
`ripr-mcp-workspace-status-v1` document. It declares no source-edit,
verification-execution, mutation-execution, or model-provider authority
(ADR 0022), and the status is resolved once at process startup — a static
snapshot for the life of the server, not a live view. See
[`docs/interop/mcp.md`](interop/mcp.md).

## CI Path

The generated GitHub workflow from:

```bash
ripr init --ci github
```

runs the same advisory loop around the top pilot seam when one is available. It
uploads:

```text
target/ripr/pilot
target/ripr/workflow
target/ripr/agent
target/ripr/reports
```

It also appends the pilot summary and agent review summary to the GitHub job
summary. Optional SARIF upload is controlled by `RIPR_UPLOAD_SARIF`; PR
blocking remains off by default.

## Editor Path

The VS Code/Open VSX extension starts from saved-workspace diagnostics. A user
can hover for evidence, copy the targeted test brief, copy packet/verify/receipt
commands, and open the strongest related test. The external agent still consumes
the same command and artifact model shown above.

## Minimal Handoff

When handing work to a human or external LLM tool, include:

```text
target/ripr/workflow/workflow.json
target/ripr/workflow/commands.md
target/ripr/workflow/agent-brief.json
target/ripr/workflow/agent-packet.json when present
target/ripr/workflow/agent-verify.json after edit
target/ripr/reports/agent-receipt.json after verify
target/ripr/workflow/agent-review-summary.md
```

The handoff instruction should stay bounded:

```text
Write one focused Rust test for the selected seam.
Imitate the related test style.
Target the missing discriminator or observation.
Do not edit production code unless explicitly asked.
After editing, rerun the after snapshot, verify command, receipt command, and
review summary.
```
