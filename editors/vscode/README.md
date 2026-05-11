# ripr: Static Mutation Exposure

[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-4%20installs-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)

<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-10 after the 0.5.0 publish: 4 installs. Refresh the count and date from publisher metrics whenever you check; do not use live VS Marketplace Shields routes. -->

Preview VS Code/Open VSX extension for `ripr`, a static Rust analysis tool that
finds changed code where the nearby tests may run but not actually check the
changed behavior.

It is a fast static companion to mutation testing: it does not run mutants,
but it points reviewers and coding agents at the focused test most likely to
matter. The extension starts `ripr lsp --stdio`, surfaces saved-workspace
diagnostics, and helps a human or coding agent move from a flagged change to
one focused test.

## Requirements

The extension can download and cache the matching `ripr` server binary from
GitHub Releases on first activation. Manual installation is still supported for
offline, pinned, or enterprise-controlled environments.

## Install and First Run

Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX. The
extension should resolve its server automatically, so `cargo install ripr` is a
fallback rather than a required first step.

After opening a Rust/Cargo workspace:

1. Check the `ripr` status bar item for the current state: server status,
   workspace, analysis progress, the recommended next action, "no focused
   test gap found," or "analysis stale / failed." The status bar projects an
   existing workspace-matched
   `target/ripr/reports/first-useful-action.json` report when one is present,
   without rerunning analysis. (Internal status IDs such as
   `no-actionable-seam` and `first-useful-action` remain stable in JSON.)
2. Use the Problems panel to find changed code that ripr flagged as
   mutation-exposed in the saved workspace.
3. Hover a flagged location to see why ripr thinks the current tests are
   weak: the assertion or check that appears to be missing, the related test
   to imitate, a suggested test shape, and verify and receipt commands.
4. Use the intent-titled code actions to copy the targeted test brief, the
   suggested assertion, or the agent handoff command chain.
5. Open the best related test when ripr finds an imitation target.
6. Add one focused test outside the editor.
7. Verify with the copied command chain or the CI artifact packet.

Unsaved-buffer overlays are not enabled by default.

For the full editor loop from diagnostic to receipt, see
[`docs/EDITOR_EVIDENCE_WORKFLOW.md`](../../docs/EDITOR_EVIDENCE_WORKFLOW.md).

## What ripr Does

`ripr` scans Rust code for mutation-exposed locations — places where the
changed behavior could plausibly differ — and reports whether nearby tests
appear to contain an assertion or check that would catch the change. It uses
conservative static language and is meant to guide the next useful test, not
to prove test adequacy.

Under the hood, ripr uses the RIPR model: reachability, infection,
propagation, and revealability. Reports, JSON, and specs use the precise
internal vocabulary (seams, discriminators, oracle strength); the editor
surface keeps that vocabulary out of the first-hour path. See the
[Terminology bridge](https://github.com/EffortlessMetrics/ripr/blob/main/docs/TERMINOLOGY.md)
to map between the two.

The 0.5.x extension surfaces saved-workspace diagnostics, evidence-aware
hovers, intent-titled code actions for inspecting the flagged change /
writing the targeted test / copying the agent handoff / verifying after the
test / reviewing the receipt / refreshing analysis, an LSP
`collectEvidenceContext` seam handoff packet, and a first-useful-action
projection in the status bar and hover when a workspace-matched report
already exists.

It does not run mutation testing, report killed/survived, or prove test
adequacy. Use real mutation testing, such as `cargo-mutants`, for ready-mode
confirmation.

## Settings

- `ripr.server.path`: explicit path to the `ripr` executable. Empty by default.
- `ripr.enabled`: enables saved-workspace diagnostics, hovers, status, and code
  actions. Defaults to `true`.
- `ripr.server.args`: arguments used to start the language server. Defaults to
  `["lsp", "--stdio"]`.
- `ripr.server.autoDownload`: download a matching server when needed. Defaults
  to `true`.
- `ripr.server.version`: pinned server version. Empty means match the extension
  version.
- `ripr.server.downloadBaseUrl`: override the manifest location for internal
  mirrors.
- `ripr.check.mode`: preferred editor check mode. Defaults to `draft`.
- `ripr.baseRef`: Git base ref used by context commands. Defaults to
  `origin/main`.
- `ripr.trace.server`: language-server trace setting.

## Commands

- `ripr: Restart Server`
- `ripr: Show Status`
- `ripr: Show Output`
- `ripr: Copy Finding Context`
- `ripr: Write Targeted Test - Copy Suggested Assertion`
- `ripr: Write Targeted Test - Copy Brief`
- `ripr: Agent Handoff - Copy Packet Command`
- `ripr: Agent Handoff - Copy Brief Command`
- `ripr: Verify After Test - Copy After Snapshot Command`
- `ripr: Verify After Test - Copy Verify Command`
- `ripr: Review Result - Copy Receipt Command`
- `ripr: Write Targeted Test - Open Best Related Test`
- `ripr: Open Settings`

## Preview Limitations

The `0.5.x` extension uses a universal VSIX and downloads native server
binaries from matching GitHub Releases when available. It does not auto-install
Rust tooling, run mutation tests, make automatic edits, or analyze unsaved
buffer overlays by default. Bundled platform-specific VSIXs are planned after
the downloader path is proven.
