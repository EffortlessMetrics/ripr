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

The `0.1.x` extension is a preview client that starts the `ripr` executable from
`PATH` or from `ripr.server.path`. It does not bundle native `ripr` binaries.

## Location

```text
editors/vscode/
```

This directory is intentionally outside the Cargo workspace. It is a Node/VS
Code extension package, not a Rust package.

## Requirements

Install the Rust CLI first:

```bash
cargo install ripr
```

The extension requires `ripr 0.1.0` or newer.

## Settings

- `ripr.server.path`: path to the `ripr` executable. Defaults to `ripr`.
- `ripr.server.args`: arguments used to start the language server. Defaults to
  `["lsp"]`.
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

If the executable is missing, the extension shows:

```text
ripr executable not found. Install with `cargo install ripr`, or set `ripr.server.path`.
```

Actions:

- Open Settings
- Copy Install Command

The extension does not auto-install Rust, Cargo, or `ripr`.

## Local Gates

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.1.0.vsix --force
```

Manual smoke:

```text
Open a Rust workspace with Cargo.toml.
Confirm the extension activates.
Confirm ripr lsp starts.
Confirm diagnostics can arrive from the server.
Confirm missing ripr path gives a useful message.
Confirm restart and output commands work.
```

## Current Limitations

The preview extension does not yet provide:

- bundled native server binaries
- automatic binary downloads
- platform-specific VSIX packages
- automatic Rust or Cargo installation
- deep editor UI beyond LSP diagnostics and basic commands

