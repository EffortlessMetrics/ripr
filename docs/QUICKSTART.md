# Quickstart

Use this path when you want the default RIPR loop without learning every report
format first. Most users should start from the surface they already use:

- editor users install the VS Code/Open VSX extension;
- CI users add the generated non-blocking GitHub workflow;
- CLI users run the same engine directly for local proof and automation.

The shared evidence loop is:

```text
ripr pilot
-> inspect the top seam, hover, or targeted test brief
-> add one focused test
-> capture an after snapshot
-> ripr outcome
-> ripr agent verify / ripr agent receipt when handing work to an agent
-> attach CI, cockpit, SARIF, badge, or calibration artifacts only when useful
```

## Use In VS Code

Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX, then
open a Rust/Cargo workspace.

The extension should provision a matching `ripr` server from GitHub Releases
without requiring `cargo install ripr`. It uses saved-workspace analysis by
default. Open a Rust file, save or refresh the workspace, then use:

- Problems diagnostics for actionable seam evidence;
- hover for why RIPR flagged the seam;
- `Write targeted test: copy brief` for the focused test to write next;
- `Agent handoff: copy packet command` and related verify/receipt commands for
  agent handoff;
- `Write targeted test: open best related test` to jump to the strongest
  imitation target.

Unsaved-buffer overlays are not enabled by default, and the extension does not
install Rust or Cargo. A separate CLI install remains the fallback for offline,
pinned, or controlled environments.

See [Editor extension](EDITOR_EXTENSION.md).

## Use In CI

Generate the advisory GitHub Actions workflow when a repository wants PR-visible
RIPR artifacts:

```bash
ripr init --ci github
```

Run that once with the CLI, or copy the workflow recipe from [CI](CI.md) if you
are adopting RIPR from a web-only CI setup.

The workflow runs `ripr pilot`, uploads pilot/report/agent artifacts, writes
badge JSON, and can optionally render/upload SARIF through `RIPR_UPLOAD_SARIF`.
It is non-blocking by default and does not run mutation tests.

Commit the generated workflow when the team wants that policy in the repo. See
[CI](CI.md).

## Use The CLI

```bash
cargo install ripr
```

The full editor-agent evidence loop requires `ripr 0.4.0` or later. The older
`0.3.1` crate contains the first defaults-first CLI loop but predates the final
0.4 editor-agent release proof, and `0.3.0` predates `ripr pilot` and
`ripr outcome`.

For local development from this repository:

```bash
cargo install --path crates/ripr
```

Cargo install is the normal direct-CLI path. The VS Code/Open VSX extension and
generated GitHub workflow both use the same engine, but editor users should not
need a separate CLI install for normal first use.

## Run a Pilot Packet

```bash
ripr pilot
```

`ripr.toml` is optional. If it is missing, RIPR uses built-in defaults — the
same defaults `ripr init` would materialize. Missing config is the normal
first-run state, not a degraded mode.

The command writes:

```text
target/ripr/pilot/repo-exposure.json
target/ripr/pilot/repo-exposure.md
target/ripr/pilot/agent-seam-packets.json
target/ripr/pilot/pilot-summary.json
target/ripr/pilot/pilot-summary.md
```

The terminal summary shows the top actionable seam, why RIPR ranked it, where
the structured packet lives, and the command to run after a focused test is
added.

When a human or external LLM tool is taking over one focused test, follow the
[LLM operator guide](LLM_OPERATOR_GUIDE.md) for the status, workflow packet,
verify, receipt, and reviewer-summary artifact loop.

If analysis exceeds the default budget, `ripr pilot` writes
`pilot-summary.{json,md}` with `status: partial` and a retry command instead of
waiting silently.

From the `ripr` source repository, `cargo xtask operator-cockpit` joins the
repo-local artifacts into `target/ripr/reports/operator-cockpit.md` and `.json`
when you want one cockpit for repo exposure, LSP, before/after snapshots,
agent verify, agent receipt, SARIF, badges, targeted-test receipts, and
optional calibration. `cargo xtask operator-cockpit-report` remains an alias
for existing automation.

To try the loop on known small examples from this repository, use
[`fixtures/EXAMPLE_CORPUS.md`](../fixtures/EXAMPLE_CORPUS.md). It maps the
boundary gap, weak oracle, exact error variant, opaque fixture/builder, LSP
action, receipt, and optional calibration artifacts.

Useful flags:

```bash
ripr pilot --root .
ripr pilot --out target/ripr/pilot
ripr pilot --mode ready
ripr pilot --max-seams 5
ripr pilot --timeout-ms 120000
```

## Optional Policy

Run `ripr init` only when the team wants to commit repo-local policy:

```bash
ripr init
```

`ripr init` writes `ripr.toml` so policy can be reviewed, versioned, and tuned;
it does not unlock basic usefulness, and it is not required for first value.
Most users only need it to commit repo policy, suppressions, tuned
severities/modes, or a generated CI workflow. The generated config is advisory,
includes unchanged tests by default, hides solved and governed seam classes
from default attention, and records the saved-workspace editor seam diagnostic
default.

## Add One Focused Test

Pick one seam from `pilot-summary.md` or use the VS Code action to copy the
targeted test brief. Add one focused test that exercises the missing
discriminator or oracle shape.

## Compare Afterward

After adding the test, rerun repo exposure:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json
```

Then compare the before and after snapshots:

```bash
ripr outcome \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json
```

Use `--format json` for tools, or `--out target/ripr/pilot/outcome.md` to write
the receipt instead of printing Markdown to stdout.

If a coding agent or review handoff needs a machine-readable verification
packet, write the agent artifacts beside the pilot packet:

```bash
mkdir -p target/ripr/agent
ripr agent verify \
  --root . \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json \
  --json > target/ripr/agent/agent-verify.json
ripr agent receipt \
  --root . \
  --verify-json target/ripr/agent/agent-verify.json \
  --seam-id <seam_id> \
  --json \
  --out target/ripr/agent/agent-receipt.json
```

The VS Code copy-command actions and generated GitHub workflow use the same
`target/ripr/pilot` and `target/ripr/agent` artifact paths.

## Optional Runtime Calibration

If cargo-mutants data already exists, import it without running mutation tests:

```bash
ripr calibrate cargo-mutants \
  --mutants-json target/mutants/outcomes.json \
  --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
```

Use `--format json` for tools, or `--out target/ripr/pilot/mutation-calibration.md`
to write the advisory calibration report to disk. Runtime vocabulary is kept in
this calibration report and does not change static RIPR classifications.

## Known Limits

RIPR reports static exposure evidence. It does not run mutation tests, prove
test adequacy, or replace execution-backed mutation testing. Runtime mutation
vocabulary belongs only in explicit calibration reports supplied with runtime
data.
