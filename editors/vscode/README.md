# ripr: Static Mutation Exposure

Preview VS Code extension for `ripr`, the static RIPR mutation-exposure analyzer
for Rust/Cargo workspaces.

The extension starts `ripr lsp` and surfaces static exposure diagnostics from
the local `ripr` executable.

## Requirements

Install `ripr` first:

```bash
cargo install ripr
```

This extension requires `ripr 0.1.0` or newer on `PATH`, or a configured
`ripr.server.path`.

## What ripr Does

`ripr` scans changed Rust code for mutation-shaped probes and reports whether
tests appear to contain the discriminators needed to expose the changed
behavior.

It does not run mutation testing, report killed/survived, or prove test
adequacy. Use real mutation testing, such as `cargo-mutants`, for ready-mode
confirmation.

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

## Preview Limitations

The `0.1.x` extension is intentionally PATH-based. It does not bundle native
server binaries, auto-install Rust tooling, or download release artifacts.
