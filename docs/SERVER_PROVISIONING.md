# Server Provisioning

The editor extension owns server provisioning. Users should be able to install
the extension, open a Rust/Cargo workspace, and get `ripr` diagnostics without
running `cargo install ripr` first.

## Resolution Order

The VS Code/Open VSX extension resolves the server in this order:

```text
1. ripr.server.path
2. bundled server binary, if present
3. downloaded cached server binary
4. verified first-run download from GitHub Releases
5. ripr on PATH
6. actionable error
```

`ripr.server.path` is an override for pinned or enterprise-managed binaries. The
PATH fallback remains useful for local development and offline installs.

## Downloaded Server Cache

Downloaded servers are stored under the VS Code global storage directory:

```text
servers/
  <version>/
    <rust-target>/
      ripr(.exe)
      sha256.txt
```

The default server version is the extension version. Users can pin a different
server with `ripr.server.version`.

## Manifest

The extension downloads a manifest from GitHub Releases unless
`ripr.server.downloadBaseUrl` is set:

```text
https://github.com/EffortlessMetrics/ripr/releases/download/v<VERSION>/ripr-server-manifest-v<VERSION>.json
```

The manifest shape is:

```json
{
  "version": "0.4.0",
  "assets": {
    "x86_64-pc-windows-msvc": {
      "url": "https://github.com/EffortlessMetrics/ripr/releases/download/v0.4.0/ripr-server-v0.4.0-x86_64-pc-windows-msvc.zip",
      "sha256": "..."
    }
  }
}
```

The checksum is for the downloaded archive. The extension verifies the archive
before extraction and only starts the extracted binary after `ripr --version`
passes.

## Previous Public Release Proof

The `v0.3.1` GitHub Release verified the default extension server-provisioning
shape:

```text
ripr-0.3.1.vsix
ripr-server-manifest-v0.3.1.json
ripr-server-v0.3.1-x86_64-pc-windows-msvc.zip
ripr-server-v0.3.1-x86_64-unknown-linux-gnu.tar.gz
ripr-server-v0.3.1-aarch64-unknown-linux-gnu.tar.gz
ripr-server-v0.3.1-x86_64-apple-darwin.tar.gz
ripr-server-v0.3.1-aarch64-apple-darwin.tar.gz
checksums.txt
```

The release/install proof downloaded the Windows server archive, matched its
SHA-256 against the manifest, extracted it, and ran `ripr --version`,
`ripr lsp --version`, `ripr pilot`, and `ripr outcome`.

For `v0.4.0`, the release proof must publish the same asset family and extend
the extracted server smoke through `ripr agent verify` and
`ripr agent receipt`.

## Supported Targets

The first binary release workflow builds these targets:

```text
x86_64-pc-windows-msvc
x86_64-apple-darwin
aarch64-apple-darwin
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

Alpine and musl targets are intentionally separate. If no compatible prebuilt
server exists, users can set `ripr.server.path` or install `ripr` manually.

## Verification

For local extension smoke before release:

```bash
npm --prefix editors/vscode ci
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run package
npm --prefix editors/vscode run test:e2e
```

The e2e suite runs in a fixture Rust workspace and covers extension activation,
defaults-first `draft` mode, command registration, LSP-first seam context
collection with CLI fallback, targeted-test brief copying, suggested assertion
copying, related-test opening, malformed command arguments, and restart
behavior. The `v0.4.0` release proof verifies the server archive path and local
VSIX package path for current provisioning. Defaults-first public install proof
for `ripr pilot`, `ripr outcome`, `ripr agent verify`, and
`ripr agent receipt` is covered by
[Installation verification](INSTALLATION_VERIFICATION.md).

## Future Bundled VSIXs

The universal VSIX plus downloader is the first one-click path. Platform-specific
VSIXs can come later:

```text
win32-x64
linux-x64
linux-arm64
darwin-x64
darwin-arm64
```

When those exist, bundled binaries should remain ahead of downloaded binaries in
the resolution order, with auto-download retained as fallback/update machinery.
