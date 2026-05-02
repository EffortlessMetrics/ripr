# ripr: Static Mutation Exposure

[![VS Marketplace](https://img.shields.io/badge/VS%20Marketplace-live%20listing-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-1%20install-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Version](https://img.shields.io/open-vsx/v/EffortlessMetrics/ripr?label=Open%20VSX)](https://open-vsx.org/extension/EffortlessMetrics/ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)

<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-02 from the public VS Marketplace listing. Refresh from publisher metrics after publish. Do not use live VS Marketplace Shields routes. -->

Preview VS Code extension for `ripr`, the static RIPR mutation-exposure analyzer
for Rust/Cargo workspaces.

The extension starts `ripr lsp --stdio` and surfaces static exposure diagnostics
from a resolved `ripr` server.

## Requirements

The extension can download and cache the matching `ripr` server binary from
GitHub Releases on first activation. Manual installation is still supported for
offline, pinned, or enterprise-controlled environments.

## What ripr Does

`ripr` scans changed Rust code for mutation-shaped probes and reports whether
tests appear to contain the discriminators needed to expose the changed
behavior.

It does not run mutation testing, report killed/survived, or prove test
adequacy. Use real mutation testing, such as `cargo-mutants`, for ready-mode
confirmation.

## Settings

- `ripr.server.path`: explicit path to the `ripr` executable. Empty by default.
- `ripr.server.args`: arguments used to start the language server. Defaults to
  `["lsp", "--stdio"]`.
- `ripr.server.autoDownload`: download a matching server when needed. Defaults
  to `true`.
- `ripr.server.version`: pinned server version. Empty means match the extension
  version.
- `ripr.server.downloadBaseUrl`: override the manifest location for internal
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

## Preview Limitations

The `0.2.x` extension uses a universal VSIX and downloads native server binaries
when available. It does not auto-install Rust tooling. Bundled platform-specific
VSIXs are planned after the downloader path is proven.
