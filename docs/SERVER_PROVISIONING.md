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

Before activation commits to any candidate, the extension first checks
`ripr --version`, then runs a bounded standard-LSP compatibility session over
`ripr lsp --stdio`: framed `initialize`, `initialized`, `shutdown`, and `exit`.
Each response must use JSON-RPC 2.0, the active request id, and exactly one of a
structurally valid `result` or `error`. Initialize must identify a versioned
`ripr` server, select UTF-16, and advertise full saved synchronization, hover,
code actions, pull diagnostics, workspace folders, and the complete command set
the extension executes. Only code-action resolve and work-done progress remain
typed optional evidence because the client has fallbacks for their absence.
Shutdown succeeds only with a `null` result. A failed probe never becomes the
active client, and resolver fallback continues only along the existing order
above. Workspace Trust remains outside and above resolution, so an untrusted
workspace spawns neither this probe nor the active server.

Every terminal path after spawn owns process-tree cleanup, including a direct
child that exits successfully or unsuccessfully before cleanup runs. POSIX
probes lead a dedicated process group. Windows probes run inside a kill-on-close
Job Object whose owner relays raw stdio, so descendants cannot escape merely
because the direct process exits before `taskkill /T` can address its PID.

This compatibility evidence is activation state, separate from the completed
install receipt and its byte-integrity evidence. It does not attest producer
provenance or use a private/experimental capability as the standard-LSP
baseline.

## Downloaded Server Cache

Downloaded servers are stored under the VS Code global storage directory:

```text
servers/
  <version>/
    <rust-target>/
      ripr(.exe)
      install-receipt.json
```

The default server version is the extension version. Users can pin a different
server with `ripr.server.version`. The configured value must be a canonical
semantic version such as `1.2.3` or `1.2.3-rc.1`; path separators, rooted paths,
drive/UNC forms, and `.` / `..` aliases are rejected before URL or filesystem
use.

Each version/target install uses a unique temporary sibling directory and a
per-version/target lock. The extension verifies the manifest version and
archive digest, extracts and probes the staged executable, records its digest
and reported binary version, writes a completed `install-receipt.json`, and
then atomically renames the validated directory into the final cache path.
Concurrent extension hosts converge on that one completed installation.

A cached executable is eligible only when the receipt has
`installationState: "complete"`, its requested/manifest version, target, and
executable name match the request, and its current executable SHA-256 matches
the receipt. Binary-only, partial, malformed, or tampered directories are not
probed as cache candidates. A failed install for a new version leaves an
already completed prior version unchanged. Contenders never reclaim an
existing lock based only on age; they fail closed after a bounded wait rather
than risk deleting a replacement owner's lock.

The install receipt establishes local completion and byte integrity. It is not
a producer provenance attestation; release provenance verification remains a
separate downstream trust boundary.

## Manifest

The extension downloads a manifest from GitHub Releases unless
`ripr.server.downloadBaseUrl` is set:

```text
https://github.com/EffortlessMetrics/ripr/releases/download/v<VERSION>/ripr-server-manifest-v<VERSION>.json
```

The manifest shape is:

```json
{
  "version": "0.7.0",
  "assets": {
    "x86_64-pc-windows-msvc": {
      "url": "https://github.com/EffortlessMetrics/ripr/releases/download/v0.7.0/ripr-server-v0.7.0-x86_64-pc-windows-msvc.zip",
      "sha256": "..."
    }
  }
}
```

The checksum is for the downloaded archive. The extension verifies the archive
before extraction and admits the result only after the staged binary's
`ripr --version` probe and completed-receipt validation pass.

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

For `v0.7.0`, the release proof must publish the same asset family and extend
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
behavior. The `v0.7.0` release proof verifies the server archive path and local
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
