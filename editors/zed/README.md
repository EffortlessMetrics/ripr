# ripr for Zed

This extension registers ripr's language server with Zed. It starts
`ripr lsp --stdio` from your `PATH` for Rust, Python, TypeScript, TSX, and
JavaScript files. It does not download or bundle ripr; install the binary
first.

The extension is not in the Zed extension registry. Install it as a dev
extension from a checkout of this repository.

## Install

1. Install ripr so `ripr` is on your `PATH`
   ([Quickstart](../../docs/QUICKSTART.md#installation)):

   ```bash
   cargo install ripr --locked
   ```

2. Install Rust through rustup. Zed compiles the extension to WebAssembly on
   your machine; `rust-toolchain.toml` in this directory names the
   `wasm32-wasip2` target, and rustup installs it on the first build.
3. In Zed, run `zed: install dev extension` from the command palette and
   choose this `editors/zed` directory.
4. Open a repository and a changed file. Zed starts ripr next to the language's
   other servers.

If Zed cannot find `ripr`, the language server status shows the install
routes. Zed's log (`zed: open log`) records build and start failures.

## Settings

Zed passes the `lsp.ripr` entry of `settings.json` to ripr:

```json
{
  "lsp": {
    "ripr": {
      "settings": {
        "diagnosticProfile": "full",
        "baseRef": "origin/main"
      }
    }
  }
}
```

- `settings` answers ripr's `workspace/configuration` request for section
  `ripr`. Write the keys flat, as above; the extension places them under
  `ripr`. The keys are the ones every client uses:
  [Other LSP editors](../../docs/interop/other-editors-lsp.md#what-every-client-needs-to-know).
- `initialization_options` is sent as `initializationOptions` unchanged.
- `binary.arguments` and `binary.env` change how the `PATH` binary starts.
  The binary is found on the worktree's `PATH` before `binary.env` applies,
  so a `PATH` entry in `binary.env` does not help Zed find ripr. For a binary
  outside your `PATH`, use `binary.path`.
- `binary.path` makes Zed start that file directly. Zed then passes only
  `binary.arguments`, so set them too:

  ```json
  {
    "lsp": {
      "ripr": {
        "binary": {
          "path": "/opt/ripr/bin/ripr",
          "arguments": ["lsp", "--stdio"]
        }
      }
    }
  }
  ```

To turn ripr off for one language, leave it out of that language's servers:

```json
{
  "languages": {
    "Python": { "language_servers": ["!ripr", "..."] }
  }
}
```

## How it behaves in Zed

Read from Zed's source (`crates/project/src/lsp_store.rs`,
`crates/lsp/src/lsp.rs`, `crates/extension/src/extension_builder.rs` at
zed-industries/zed `bd74733`) on 2026-09-29; not run in a Zed binary.

- **One repository per server.** Zed starts one server per worktree and gives
  it that worktree's root. A project with two folders gets two ripr servers,
  so each repository is analyzed on its own. Open the repository root, not a
  subdirectory, so ripr sees the whole Git workspace.
- **Save to refresh.** ripr analyzes saved files. Unsaved edits withdraw that
  file's diagnostics until the next save.
- **Few diagnostics is normal.** The default `actionable` profile publishes
  only findings with a repair route. Set `diagnosticProfile` to `full` to see
  every finding.

The shared protocol notes, including ripr's commands and error behavior, are in
[Other LSP editors](../../docs/interop/other-editors-lsp.md).

## What was checked

- Zed's own packager (`zed-extension` from zed-industries/zed `bd74733`, the
  tool the registry uses) compiled, validated, and packaged this directory on
  2026-09-29. It reported `wasm_api_version` 0.6.0 and
  `provides: ["language-servers"]`.
- With `wasm32-wasip2` removed from the 1.95.0 toolchain, a build in this
  directory installed the target through `rust-toolchain.toml`.
- CI builds the component and runs the unit tests (`Zed extension` workflow).
  The tests cover the launch command, the missing-binary message, argument and
  environment overrides, and the `ripr` configuration section.

Not verified: loading the extension in a running Zed, diagnostics and hover in
Zed's UI, and Zed's handling of ripr's pull diagnostics after a refresh.

## Publishing

Publishing to the Zed extension registry is a release action for the
maintainers, not part of this directory. It takes a pull request to
`zed-industries/extensions` that adds this repository as a submodule under
`extensions/ripr-lsp` with `path = "editors/zed"` and the `version` from
`extension.toml`, after a manual test in Zed at the submitted commit. Zed's
publishing rules require the license files in this directory (present), an ID
without "zed" or "extension" (`ripr-lsp`), and a language server found in the
user's environment rather than bundled (as here).
