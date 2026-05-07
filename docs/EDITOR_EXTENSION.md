# Editor Extension

The VS Code extension is a separate artifact from the Rust crate.

```text
Rust crate:
  ripr

VS Code extension:
  EffortlessMetrics.ripr

Open VSX extension:
  EffortlessMetrics.ripr
```

The `0.3.x` extension is a universal VSIX preview client. It resolves the
server in this order:

```text
1. ripr.server.path
2. bundled server binary, if present
3. downloaded cached server binary
4. verified first-run download from GitHub Releases
5. ripr on PATH
6. actionable error
```

It does not yet publish platform-specific VSIXs with bundled native binaries.

## Location

```text
editors/vscode/
```

This directory is intentionally outside the Cargo workspace. It is a Node/VS
Code extension package, not a Rust package.

## Requirements

The extension can provision the matching server automatically. Manual install is
still supported for offline or controlled environments:

```bash
cargo install ripr
```

## Install Paths

Normal editor installs should not require a separate `cargo install ripr` step.
Use one of these surfaces:

- VS Code Marketplace: install `EffortlessMetrics.ripr`.
- Open VSX: install `EffortlessMetrics.ripr`.
- Local VSIX smoke: run `npm run package`, then install
  `editors/vscode/dist/ripr-0.3.1.vsix`.

On activation, the extension resolves a configured, bundled, cached,
downloaded, or PATH server and writes the selected source to the `ripr` output
channel. `cargo install ripr` remains the manual fallback for offline, pinned,
or controlled environments.

## Settings

- `ripr.server.path`: explicit path to the `ripr` executable. Empty by default.
- `ripr.server.args`: arguments used to start the language server. Defaults to
  `["lsp", "--stdio"]`.
- `ripr.server.autoDownload`: automatically download a matching server when
  needed. Defaults to `true`.
- `ripr.server.version`: pinned server version. Empty means match the extension
  version.
- `ripr.server.downloadBaseUrl`: override the manifest base URL for internal
  mirrors.
- `ripr.check.mode`: preferred editor check mode for LSP diagnostics and
  context commands. Defaults to `draft`.
- `ripr.baseRef`: Git base ref used by LSP diagnostics and context commands.
  Defaults to `origin/main`.
- `ripr.trace.server`: language-server trace setting.

The extension passes `ripr.check.mode` and `ripr.baseRef` to the language server
as initialization options. Changing server, check, base-ref, or trace settings
restarts the client so the next diagnostic refresh uses the new configuration.

## Defaults-First Stance

The editor surface follows the defaults-first adoption contract in
[RIPR-SPEC-0009](specs/RIPR-SPEC-0009-defaults-first-adoption.md): diagnostics,
hovers, targeted-test briefs, context packets, best related-test navigation,
and refresh status should be discoverable without forcing users to understand
every report artifact first.

The current LSP model remains saved-workspace analysis. Unsaved-buffer overlays
are not enabled by default. The defaults-first target is useful bounded editor
feedback without requiring `ripr init`: saved-workspace diagnostics, hovers,
briefs, related-test navigation, and refresh status are available through
built-in defaults. `ripr init` is optional; when a team commits `ripr.toml`,
that repo policy makes the same defaults explicit and reviewable, or tunes them
quieter.

## Commands

- `ripr: Restart Server`
- `ripr: Show Output`
- `ripr: Copy Finding Context`
- `ripr: Copy Suggested Assertion`
- `ripr: Copy Targeted Test Brief`
- `ripr: Open Related Test`
- `ripr: Open Settings`

### Copy Finding Context

The `ripr: Copy Finding Context` command first attempts to collect context
through the running LSP server via `workspace/executeCommand` with
`ripr.collectContext`. If the server has a matching analysis snapshot, it
returns a JSON context packet directly. If the LSP command is unavailable or
returns no result, the extension falls back to shelling out to the `ripr`
CLI (`ripr context --at <selector> --json`).

The code action `Copy ripr context packet` includes `finding_id` and
`probe_id` from the diagnostic data so the server can resolve the finding
without re-running workspace analysis.

### Seam Code Actions

When seam diagnostics are enabled and a diagnostic carries `seam_id`, the LSP
server can provide seam-aware code actions:

- `Copy seam packet`: copies the server-owned agent seam packet for the
  selected seam through `ripr.collectContext`.
- `Copy targeted test brief`: copies a plain-language work order for adding one
  focused test from the same seam packet guidance.
- `Copy suggested assertion`: copies a concrete assertion suggestion from the
  seam packet.
- `Open best related test`: opens the strongest related test to imitate when
  one is available, then falls back to the highest-confidence related test.
- `Refresh ripr analysis`: asks the LSP server to refresh diagnostics with
  `ripr.refresh`.

The assertion and related-test actions are conditional. `Copy suggested
assertion` is shown only when the seam has a concrete assertion suggestion, and
`Open best related test` is shown only when the current analysis snapshot can
resolve a related test location. The targeted-test brief remains available for seam
diagnostics even when there is no concrete assertion snippet, because it can
still summarize the missing discriminator, suggested file/name, candidate value,
and imitation or avoid patterns. Refresh remains available even when no
diagnostic is selected.

## Missing Server Behavior

If no usable server can be resolved, the extension shows:

```text
ripr server is not available. Enable automatic download, install with `cargo install ripr`, or set `ripr.server.path`.
```

Actions:

- Open Settings
- Copy Install Command
- Retry

The extension does not auto-install Rust or Cargo. It only downloads verified
release archives when `ripr.server.autoDownload` is enabled.

## Local Gates

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
npm run test:e2e
code --install-extension dist/ripr-0.3.1.vsix --force
```

Manual smoke:

```text
Open a Rust workspace with Cargo.toml.
Confirm the extension activates.
Open the ripr output channel.
Confirm the resolved server source is logged.
Confirm ripr lsp --stdio starts.
Confirm diagnostics can arrive from saved-workspace analysis.
Confirm hover evidence, Copy Targeted Test Brief, and Open Best Related Test are available on seam diagnostics when the analysis snapshot includes the required data.
Confirm missing-server state gives the documented actionable message.
Confirm Restart Server, Show Output, and Open Settings work.
```

## Diagnostic Refresh Model

The preview LSP server currently analyzes the saved workspace diff. It stores
open document text for future hover/actions work, but unsaved edits are not yet
used as analyzer overlays.

Diagnostics refresh when a document opens, when a document is saved, or when the
`ripr.refresh` LSP command runs. Text changes update server document state but
do not trigger full workspace analysis until the change is saved or refreshed
explicitly.

The server logs refresh lifecycle messages to the LSP output stream. A normal
refresh logs when analysis starts and when it completes. Completion logs include
the refresh duration, total diagnostic count, finding count, seam diagnostic
count, published file count, and cleared file count. If a newer refresh
supersedes an older one, the older result is not published.

Refresh failures clear previously published diagnostics and log a warning with
the failure reason. Normal refreshes and one-off failures do not show user-facing
popups; the output stream is the intended place to inspect refresh state.

## Diagnostic Data

LSP diagnostics include a stable JSON `data` payload for editor commands:

```json
{
  "schema_version": "0.1",
  "finding_id": "probe:src/pricing.rs:88:predicate",
  "probe_id": "probe:src/pricing.rs:88:predicate",
  "classification": "weakly_exposed",
  "probe_family": "predicate",
  "confidence": 0.75,
  "source_range": {
    "file": "src/pricing.rs",
    "line": 88,
    "column": 1
  }
}
```

Diagnostics remain advisory. `exposed`, `propagation_unknown`, and
`static_unknown` findings are informational; weak or missing exposure findings
are warnings.

## Hover Content

When the cursor is on a diagnostic range and a matching analysis snapshot is
available, the hover renders evidence-rich finding content:

```text
**ripr** `weakly_exposed`

Add an exact boundary assertion.

## RIPR Evidence

* reach yes: related tests found
* infection yes: predicate can alter branch behavior
* propagation yes: branch influences return value
* observation weak: return value asserted
* discriminator weak: boundary value missing

## Related Tests

- `tests/pricing.rs:12` `discount_boundary_is_exact` — strong exact_value oracle: assert_eq!(total, expected)

## Weakness

- no equality-boundary case was found

---
Analysis snapshot: generated 2 seconds ago; last refresh took 138 ms.
```

Fallback behavior preserves three levels:

1. **Snapshot + matching finding** — evidence-rich hover with RIPR stage
   summaries, related tests, weakness notes, and snapshot age.
2. **Diagnostic without matching finding** — diagnostic-only hover showing the
   classification, message, and finding or probe identifiers.
3. **No diagnostic at position** — generic guidance hover.

Seam hovers use the same snapshot footer when a matching seam diagnostic is
available. They also include a `Why this diagnostic?` section that makes the
static classification legible:

```text
## Why this diagnostic?
Grip class: `weakly_gripped` — the current static evidence has a weak discriminator or a named missing discriminator.

Strong evidence:
- reach yes: related tests reach discounted_total
- observation yes: exact value assertion exists

Weak / missing evidence:
- discrimination weak: equality boundary missing
- missing discriminator `discount_threshold (equality boundary)`: observed values do not include the equality-boundary case

Recommended next move: Add an exact-value assertion for the equality boundary.
```

This keeps saved-workspace staleness visible without claiming that unsaved
document text has been analyzed.

## VS Code Extension Tests

Smoke tests run inside a real VS Code instance via `@vscode/test-electron`:

```bash
cd editors/vscode
npm ci
npm run test:e2e
```

The suite activates the extension in a fixture Rust workspace and verifies
command registration, defaults-first `draft` mode, LSP-first seam context
collection with CLI fallback, targeted-test brief copying, suggested assertion
copying, related-test opening, malformed command argument handling, and
`restartServer` callability. CI runs the suite headless with `xvfb-run`.

## Current Limitations

The preview extension does not yet provide:

- bundled native server binaries
- platform-specific VSIX packages
- automatic Rust or Cargo installation
- deep editor UI beyond diagnostics, evidence hovers, code actions, and basic
  commands
- unsaved-buffer analysis overlays
