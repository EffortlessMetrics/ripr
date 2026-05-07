# Quickstart

Use this path when you want the default RIPR loop without learning every report
format first.

## Install

```bash
cargo install ripr
```

The defaults-first public install path requires `ripr 0.3.1` or later. The
older `0.3.0` crate installs, but predates `ripr pilot` and `ripr outcome`.

For local development from this repository:

```bash
cargo install --path crates/ripr
```

Cargo install is the normal CLI path. The VS Code/Open VSX extension should
self-provision a matching server from GitHub Releases; installing the CLI
separately is only a fallback for offline, pinned, or controlled environments.

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

If analysis exceeds the default budget, `ripr pilot` writes
`pilot-summary.{json,md}` with `status: partial` and a retry command instead of
waiting silently.

From the `ripr` source repository, `cargo xtask operator-cockpit` joins the
repo-local report artifacts into `target/ripr/reports/operator-cockpit.md` and
`.json` when you want one cockpit for repo exposure, LSP, SARIF, badges,
receipts, and optional calibration. `cargo xtask operator-cockpit-report`
remains an alias for existing automation.

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

## Optional Editor and CI Paths

In VS Code, install `EffortlessMetrics.ripr`, open a Rust/Cargo workspace, and
use RIPR diagnostics, hovers, Copy Targeted Test Brief, and Open Best Related
Test from the editor. The extension should resolve the server automatically;
`cargo install ripr` is only a fallback for offline, pinned, or controlled
environments. The editor uses saved-workspace analysis by default; unsaved-buffer
overlays are not enabled by default.

For CI, generate the non-blocking GitHub Actions workflow when the team wants
PR report artifacts and optional code-scanning guidance:

```bash
ripr init --ci github
```

The generated workflow is advisory by default. It uploads the pilot packet,
report artifacts, and repo badge JSON; SARIF rendering/upload is controlled by
the workflow's `RIPR_UPLOAD_SARIF` setting and remains non-blocking. CI
blocking policy remains opt-in. The copyable recipe and policy details live in
[CI](CI.md).

## Known Limits

RIPR reports static exposure evidence. It does not run mutation tests, prove
test adequacy, or replace execution-backed mutation testing. Runtime mutation
vocabulary belongs only in explicit calibration reports supplied with runtime
data.
