# Handoff: Editor Agent Readiness Proof

Date: 2026-05-07
Branch / PR: `release-editor-agent-readiness-proof` / pending
Latest merged PR: #490 `docs: document full evidence loop`

## Current work item

`release/editor-agent-readiness-proof`

This work item proves the Campaign 10 editor-agent loop from repo artifacts:
installed CLI command surface, boundary-gap `ripr pilot`, `ripr outcome`,
`ripr agent verify`, focused `ripr agent receipt`, repo-exposure latency, LSP
cockpit, advisory workflow defaults, VSIX packaging path, and known-limit docs.

## Next work item

`campaign/editor-agent-integration-closeout`

Close Campaign 10 without adding analyzer families, runtime mutation execution,
CI blocking, public crate splits, automatic edits, or speculative editor
features.

## Verification run

```bash
cargo xtask release-readiness --version 0.4.0
cargo package -p ripr --list
cargo publish -p ripr --dry-run
npm --prefix editors/vscode run package
cargo test -p xtask release_readiness
```

Release-readiness status is `warn` only because the requested release version
is `0.4.0` while `crates/ripr` is still `0.3.1`; `package-list` and
`publish-dry-run` are therefore recorded as release-prep follow-up gates in the
report. The required readiness checks pass.

Before the readiness run, a longer bounded latency run seeded the
classified-seam cache:

```bash
RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS=120000 cargo xtask repo-exposure-latency-report
```

The subsequent default release-readiness run observed a passing
repo-exposure-latency report on the warm path.

## Artifacts

- `target/ripr/reports/release-readiness.md`
- `target/ripr/reports/release-readiness.json`
- `target/ripr/reports/repo-exposure-latency.md`
- `target/ripr/release-readiness/agent-verify.json`
- `target/ripr/release-readiness/agent-receipt.json`
- `editors/vscode/dist/ripr-0.3.1.vsix`

## Recommended next action

Start `campaign/editor-agent-integration-closeout` from current `main` after
this proof lands and `cargo xtask goals next` reports closeout as ready.

## What not to do

- Do not bump to 0.4.0 in the readiness-proof PR.
- Do not publish crates, GitHub releases, Marketplace, or Open VSX from this
  proof PR.
- Do not add analyzer behavior, LSP features, unsaved-buffer overlays, runtime
  mutation execution, CI blocking, or public crate splits.
