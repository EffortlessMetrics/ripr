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

## 1. Check Status

Start by asking RIPR what already exists:

```bash
ripr agent status --root .
```

For machine-readable state:

```bash
ripr agent status --root . --json > target/ripr/workflow/agent-status.json
```

`agent status` reads existing artifacts only. It reports required artifacts as
present or missing, recovers a `seam_id` when possible, warns about stale-looking
verify or receipt files, and prints the next missing command.

If no before snapshot exists yet, create one:

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

If the operator needs the full seam packet as well:

```bash
ripr agent packet --root . --seam-id <seam-id> --json > target/ripr/workflow/agent-packet.json
```

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
