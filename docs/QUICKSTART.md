# Quickstart

Use this path when you want the default RIPR loop without learning every report
format first.

## Install

```bash
cargo install ripr
```

For local development from this repository:

```bash
cargo install --path crates/ripr
```

## Run a Pilot Packet

```bash
ripr pilot
```

`ripr.toml` is optional. If it is missing, RIPR uses the same conservative
built-in defaults that `ripr init` would materialize.

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

Useful flags:

```bash
ripr pilot --root .
ripr pilot --out target/ripr/pilot
ripr pilot --mode ready
ripr pilot --max-seams 5
```

## Optional Policy

Run `ripr init` when the team wants to review and tune repo policy:

```bash
ripr init
```

`ripr init` writes `ripr.toml`; it is not required for first value. The
generated config is advisory, includes unchanged tests by default, hides solved
and governed seam classes from default attention, and records the saved-workspace
editor seam diagnostic default.

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

## Optional Editor and CI Paths

In VS Code, open a Rust file and use RIPR diagnostics, hovers, Copy Targeted
Test Brief, and Open Best Related Test from the editor. The editor uses
saved-workspace analysis by default; unsaved-buffer overlays are not enabled by
default.

For CI, start with the non-blocking SARIF recipe in [CI](CI.md). CI blocking
policy remains opt-in.
