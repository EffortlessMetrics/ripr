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

The `0.2.x` extension is a universal VSIX preview client. It resolves the
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
- `ripr.check.mode`: preferred editor check mode. Defaults to `instant`.
- `ripr.baseRef`: Git base ref used by context commands. Defaults to
  `origin/main`.
- `ripr.trace.server`: language-server trace setting.

## Commands

- `ripr: Restart Server`
- `ripr: Show Output`
- `ripr: Copy Finding Context`
- `ripr: Open Settings`

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
code --install-extension dist/ripr-0.2.0.vsix --force
```

Manual smoke:

```text
Open a Rust workspace with Cargo.toml.
Confirm the extension activates.
Confirm ripr lsp --stdio starts.
Confirm diagnostics can arrive from the server.
Confirm missing ripr path gives a useful message.
Confirm restart and output commands work.
```

## Current Limitations

The preview extension does not yet provide:

- bundled native server binaries
- platform-specific VSIX packages
- automatic Rust or Cargo installation
- deep editor UI beyond LSP diagnostics and basic commands
