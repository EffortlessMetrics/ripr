# Quickstart

Use this guide to get useful RIPR feedback in the first hour without learning
the full report topology. Pick one path, get one repairable Rust gap or a clear
no-action state, and keep the receipt.

RIPR finds changed Rust code where the nearby tests may not actually catch
the changed behavior. The static, draft-time question it answers is:

```text
For the behavior changed in this diff, do the current tests include an
assertion or check that would catch the changed behavior?
```

It does not edit source, generate tests, run mutation testing, or prove test
adequacy. The normal first-hour loop is:

```text
find the top test gap
-> inspect why ripr thinks the current tests are weak
-> write one focused test outside RIPR
-> capture an after snapshot
-> verify static movement
-> attach the receipt or review summary when useful
```

RIPR calls these locations "seams" in JSON, specs, and reports. First-hour docs
use plain language first; [Terminology](TERMINOLOGY.md) bridges to the internal
model when you need it.

## Choose Your Path

Most adopters should choose one of these first-hour paths:

| Path | Use when | Start with | First success |
| --- | --- | --- | --- |
| CLI first | You want one local before/after proof. | `ripr pilot --root .` | Top Rust gap, one focused proof, `ripr outcome` receipt. |
| PR first | You want reviewers to see advisory evidence in GitHub. | `ripr init --ci github` | Non-blocking summary, repair card, artifact packet. |
| Editor or agent first | You are repairing while coding, or handing work to an LLM. | VS Code `ripr: Show Status` or `ripr agent status --root .` | Current-work packet, related test, verify command. |

For TypeScript, JavaScript, or Python, first read
[Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md). Preview
language evidence is opt-in, syntax-first, visibly preview/advisory, and not a
default gate input.

`ripr.toml` is optional. `ripr init` materializes repo-local policy when a team
wants to review, version, and tune it. It is not activation, and it is not
required for first value.

## VS Code First Hour

Use this path when you want saved-workspace feedback while writing or reviewing
Rust.

1. Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX.
2. Open a Rust/Cargo workspace.
3. Check the `ripr` status bar item or run `ripr: Show Status`.
4. Open the Problems panel, hover a RIPR diagnostic, and inspect the gap.
5. Use `ripr: Start Current Repair` or the focused-test actions to copy the
   repair packet, open the related test, and copy the verify command.

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

Deep links: [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md),
[Editor extension](EDITOR_EXTENSION.md), [Server provisioning](SERVER_PROVISIONING.md).

## CI First Hour

Use this path when you want PR-visible advisory evidence without asking every
reviewer to download raw artifacts.

Generate the GitHub workflow:

```bash
ripr init --ci github
```

Or copy the workflow from [CI strategy](CI.md) when adopting from the GitHub UI.

The generated workflow is advisory by default. On a PR, read the job summary
first. It should show the first-run status, top repairable gap or no-action
state, repair route, verify command, artifact links, and gate-authority
boundary. The uploaded packet keeps the detailed pilot, workflow, agent,
report, and review artifacts.

Do not make generated CI blocking until the repository has reviewed its first
advisory baseline and explicitly opted into a policy gate.

Deep links: [CI strategy](CI.md), [PR review guidance](PR_REVIEW_GUIDANCE.md),
[Blocking readiness](BLOCKING_READINESS.md).

## CLI First Hour

Use this path when you want the reproducible local proof loop.

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

Read `target/ripr/pilot/pilot-summary.md`. Pick the top actionable Rust gap and
write one focused test or output proof outside RIPR.

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

If the pilot reports a partial result, use the retry command it prints rather
than guessing at cache or timeout settings.

For the first-run front-door packet, use:

```bash
ripr first-pr --root . --base origin/main --head HEAD
```

It composes existing artifacts into `target/ripr/reports/start-here.{json,md}`
and does not add analyzer truth. Inside this repo, `cargo xtask first-pr` is a
compatibility wrapper over the same public command.

Read the front door with the same vocabulary across CLI, editor, and PR
surfaces:

- `start here`: open `target/ripr/reports/start-here.md` first when it exists.
- `safe next action`: repair one named gap, regenerate missing evidence, or
  stop on no-action.
- `missing artifact`, `stale evidence`, `wrong root`, and `malformed artifact`:
  fail closed before repair work.
- `preview-limited evidence`: syntax-first and advisory, with static limits
  before repair language.
- `verify command`, `receipt command`, and `receipt path`: the static movement
  proof rail, not runtime adequacy or gate approval.

When a surface boundary is unclear, use the ownership table in
[First successful PR workflow](FIRST_PR_WORKFLOW.md#surface-ownership). It
names which surface owns start-here, generated CI, editor handoff, agent
packets, badges, PR evidence, and gate authority.

## Agent Or Reviewer First Hour

Use this path when a human or external coding agent needs a deterministic work
packet for one focused test.

Ask RIPR what already exists:

```bash
ripr agent status --root .
```

When you have selected a seam, write a source-edit-free workflow packet:

```bash
ripr agent start --root . --seam-id <seam_id> --out target/ripr/workflow
```

Then follow the generated `target/ripr/workflow/commands.md`. It should give
the task, context, repair route, verification command, stop conditions, and
receipt path.

The status and workflow commands read or write artifacts. They do not edit
source files, generate tests, call an LLM API, run mutation testing, refresh LSP
state, or enable CI blocking.

See [LLM operator guide](LLM_OPERATOR_GUIDE.md).

## Troubleshooting

| Symptom | First check |
| --- | --- |
| VS Code shows no RIPR state, or shows no focused test gap. | Run `ripr: Show Status`, then `ripr: Show Output`. Confirm a Rust/Cargo workspace is open and saved. |
| VS Code cannot start the server. | Check [Server provisioning](SERVER_PROVISIONING.md) for configured path, bundled or cached assets, GitHub Release download, and PATH fallback. |
| Diagnostics look stale. | Save the workspace file or run `Refresh Analysis - Saved Workspace Check`. |
| CI has no top recommendation. | Open the advisory job summary, then inspect the uploaded report packet. |
| Agent status says artifacts are missing. | Run the `next_command` printed by `ripr agent status`. |
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
data through [runtime calibration](TARGETED_TEST_WORKFLOW.md#runtime-calibration).
Runtime vocabulary belongs in that calibration report, not in ordinary static
RIPR findings.

## Next Docs

- [Terminology](TERMINOLOGY.md) for the bridge between plain wording and the
  internal model (seam, discriminator, grip, canonical gap, etc.).
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) for the one-PR path from
  a repairable Rust gap to a focused proof and receipt.
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) for the deeper
  before/after evidence and optional calibration loop.
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
