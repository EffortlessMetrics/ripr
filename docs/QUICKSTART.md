# Quickstart

Use this guide to get useful RIPR feedback in the first hour without learning
the full report topology. Start from the surface you already use.

RIPR finds changed Rust code where the nearby tests may not actually catch
the changed behavior. The static, draft-time question it answers is:

```text
For the behavior changed in this diff, do the current tests include an
assertion or check that would catch the changed behavior?
```

It does not edit source, generate tests, run mutation testing, or prove test
adequacy. The normal loop is:

```text
find the top test gap
-> inspect why ripr thinks the current tests are weak
-> write one focused test outside RIPR
-> capture an after snapshot
-> verify static movement
-> attach the receipt or review summary when useful
```

RIPR calls these locations "seams" in its JSON, specs, and reports. First-hour
docs use plain language; the precise vocabulary appears once you reach the
output and policy surfaces. See [Terminology](TERMINOLOGY.md) for the bridge
between plain wording and the internal model.

## Choose Your Path

| User type | Start here | You should see |
| --- | --- | --- |
| VS Code user | Install `EffortlessMetrics.ripr` and open a Rust/Cargo workspace. | Status bar state, Problems diagnostics, hover evidence, and intent-titled focused-test actions. |
| CI user | Run `ripr init --ci github` once or copy the workflow from [CI strategy](CI.md). | A non-blocking RIPR advisory summary plus uploaded pilot, workflow, agent, report, and review artifacts. |
| CLI user | Run `ripr pilot --root .`. | The top actionable test gap, why it matters, and before/after commands under `target/ripr/pilot`. |
| Agent or reviewer | Run `ripr agent status --root .`. | Existing artifact state, the selected change when recoverable, and the next command to run. |
| Preview language evaluator | Enable TypeScript, JavaScript, or Python in `[languages]`, then run the normal CLI, editor, or CI path. | Preview-labeled, syntax-first advisory evidence with static limits kept visible. |

`ripr.toml` is optional. `ripr init` materializes repo-local policy when a team
wants to review, version, and tune it. It is not activation, and it is not
required for first value.

For opt-in TypeScript, JavaScript, and Python evidence, see
[Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md). Rust remains
the default and preview languages stay advisory.

## VS Code First Hour

Use the editor path when you want saved-workspace feedback while writing or
reviewing Rust.

1. Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX.
2. Open a Rust/Cargo workspace.
3. Check the `ripr` status bar item or run `ripr: Show Status`.
4. Open the Problems panel and inspect ripr-flagged changes.
5. Hover a flagged change to see why ripr thinks the current tests are weak.
6. Use the focused-test actions around your intent:
   - `Write targeted test: copy brief`
   - `Write targeted test: open best related test`
   - `Agent handoff: copy packet command`
   - `Agent handoff: copy brief command`
   - `Verify after test: copy after-snapshot command`
   - `Verify after test: copy verify command`
   - `Review result: copy receipt command`
   - `Refresh Analysis - Saved Workspace Check`

Normal editor install should not require `cargo install ripr`. The extension
resolves the server from `ripr.server.path`, bundled or cached assets, verified
GitHub Release download, or PATH.

If no diagnostics appear, start with the status path:

```text
ripr: Show Status
ripr: Show Output
ripr: Restart Server
```

The editor analyzes the saved workspace. Unsaved-buffer overlays are not enabled
by default. Save the file or refresh analysis before trusting a stale diagnostic.

See [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md),
[Editor extension](EDITOR_EXTENSION.md), and
[Server provisioning](SERVER_PROVISIONING.md).

## CI First Hour

Use the CI path when you want PR-visible advisory evidence without asking every
reviewer to download raw artifacts.

Generate the GitHub workflow:

```bash
ripr init --ci github
```

Or copy the workflow from [CI strategy](CI.md) when adopting from the GitHub UI.

The generated workflow is advisory by default. It:

- runs `ripr pilot`;
- runs `ripr review-comments` on pull requests;
- writes a `RIPR advisory summary` in the GitHub job summary;
- uploads `target/ripr/pilot`, `target/ripr/workflow`, `target/ripr/agent`,
  `target/ripr/reports`, and `target/ripr/review`;
- writes repo badge JSON;
- renders and uploads SARIF when `RIPR_UPLOAD_SARIF` is `"true"`;
- emits non-blocking changed-line check annotations when
  `target/ripr/review/comments.json` exists.

The first thing to read in a PR is the job summary. It names:

- the top recommendation;
- the agent review packet when present;
- artifact paths;
- SARIF and badge status;
- PR guidance annotation counts when present;
- known limits.

Do not make generated CI blocking until the repository has reviewed its first
advisory baseline and explicitly opted into a policy gate.

See [CI strategy](CI.md).
See [PR review guidance](PR_REVIEW_GUIDANCE.md) for the annotation placement,
summary-only fallback, and inline-comment opt-in boundary.

## CLI First Hour

Use the CLI path when you want the reproducible local proof loop.

Install:

```bash
cargo install ripr
```

From this repository, use:

```bash
cargo install --path crates/ripr
```

Run the zero-config pilot:

```bash
ripr pilot --root .
```

The pilot writes:

```text
target/ripr/pilot/repo-exposure.json
target/ripr/pilot/repo-exposure.md
target/ripr/pilot/agent-seam-packets.json
target/ripr/pilot/pilot-summary.json
target/ripr/pilot/pilot-summary.md
```

Read `target/ripr/pilot/pilot-summary.md`. Pick the top actionable test gap
and write one focused test outside RIPR — an assertion or check that would
catch the changed behavior.

After the test is added, capture the after snapshot:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json
```

Compare before and after:

```bash
ripr outcome \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json
```

For machine-readable output, add `--format json` or `--out <path>` where the
command supports it.

If the pilot reports a partial result, use the retry command it prints rather
than guessing at cache or timeout settings.

## Agent Or Reviewer First Hour

Use this path when a human or external coding agent needs a deterministic work
packet for one focused test.

Ask RIPR what already exists:

```bash
ripr agent status --root .
```

For machine-readable state:

```bash
ripr agent status --root . --json > target/ripr/workflow/agent-status.json
```

When you have selected a seam, write a source-edit-free workflow packet:

```bash
ripr agent start --root . --seam-id <seam_id> --out target/ripr/workflow
```

Then follow the generated `target/ripr/workflow/commands.md`, or run the common
verification sequence directly:

```bash
ripr agent verify \
  --root . \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json \
  --json \
  > target/ripr/workflow/agent-verify.json

ripr agent receipt \
  --root . \
  --verify-json target/ripr/workflow/agent-verify.json \
  --seam-id <seam_id> \
  --json \
  --out target/ripr/reports/agent-receipt.json

ripr agent review-summary --root . > target/ripr/workflow/agent-review-summary.md
```

The status, workflow, receipt, and review-summary commands read or write
artifacts. They do not edit source files, generate tests, call an LLM API, run
mutation testing, refresh LSP state, or enable CI blocking.

See [LLM operator guide](LLM_OPERATOR_GUIDE.md).

## Troubleshooting

| Symptom | First check |
| --- | --- |
| VS Code shows no RIPR state, or shows "no focused test gap found." | Run `ripr: Show Status`, then `ripr: Show Output`. Confirm a Rust/Cargo workspace is open and saved. The internal status ID for the empty case is `no-actionable-seam`. |
| VS Code cannot start the server. | Check `ripr.server.path`, `ripr.server.autoDownload`, network access to GitHub Releases, and PATH fallback. |
| Diagnostics look stale. | Save the workspace file or run `Refresh Analysis - Saved Workspace Check`. |
| CI has no top recommendation. | Open the `RIPR advisory summary`, then inspect `target/ripr/pilot/pilot-summary.md` in the uploaded artifact. |
| CI did not upload SARIF. | Confirm `RIPR_UPLOAD_SARIF` is `"true"` and that GitHub code scanning permissions are available. |
| Agent status says artifacts are missing. | Run the `next_command` printed by `ripr agent status`. |
| Agent receipt looks stale. | Regenerate after snapshot, verify, and receipt in that order. |
| Local CLI behavior is surprising. | Run `ripr doctor` and inspect config precedence in [Configuration](CONFIGURATION.md). |

## Known Limits

RIPR reports static exposure evidence. It should not be read as runtime proof.

It does not:

- run mutants;
- report `killed` or `survived` outside supplied runtime calibration reports;
- prove test adequacy;
- generate tests;
- edit source files;
- replace coverage or execution-backed mutation testing;
- analyze unsaved editor buffers by default;
- make generated CI blocking by default.

Static classifications stay conservative: `exposed`, `weakly_exposed`,
`reachable_unrevealed`, `no_static_path`, `infection_unknown`,
`propagation_unknown`, and `static_unknown`.

When runtime mutation data already exists, import it as advisory calibration
data:

```bash
ripr calibrate cargo-mutants \
  --mutants-json target/mutants/outcomes.json \
  --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
```

Runtime vocabulary belongs in that calibration report, not in ordinary static
RIPR findings.

## Next Docs

- [Terminology](TERMINOLOGY.md) for the bridge between plain wording and the
  internal model (seam, discriminator, grip, canonical gap, etc.).
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) for the one-PR path from
  a repairable Rust gap to a focused proof and receipt.
- [Editor extension](EDITOR_EXTENSION.md) for VS Code install, commands, and
  saved-workspace refresh behavior.
- [CI strategy](CI.md) for the generated advisory workflow and artifact packet.
- [LLM operator guide](LLM_OPERATOR_GUIDE.md) for the source-edit-free agent
  loop.
- [Configuration](CONFIGURATION.md) for `ripr.toml`, modes, severities, and
  editor settings.
- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md) for opt-in
  TypeScript, JavaScript, and Python evidence.
- [Output schema](OUTPUT_SCHEMA.md) for JSON contracts.
