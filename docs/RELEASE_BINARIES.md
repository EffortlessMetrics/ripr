# Server Binary Release

The VS Code/Open VSX extension can self-provision only when GitHub Releases has
native `ripr` server archives and a manifest.

## Workflow

Use:

```text
.github/workflows/release-server-binaries.yml
```

Manual dispatch:

```bash
gh workflow run release-server-binaries.yml -f version=0.3.1
```

The workflow builds:

```text
x86_64-pc-windows-msvc
x86_64-apple-darwin
aarch64-apple-darwin
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

and uploads these assets to the matching GitHub Release:

```text
ripr-server-v<VERSION>-<target>.zip
ripr-server-v<VERSION>-<target>.tar.gz
ripr-server-manifest-v<VERSION>.json
checksums.txt
```

Each server archive contains:

```text
ripr(.exe)
LICENSE-MIT
LICENSE-APACHE
README-server.txt
```

## Published Release Proof

The `v0.3.1` release was verified on May 7, 2026:

- `ripr v0.3.1` is the latest GitHub Release.
- The release has `ripr-0.3.1.vsix`.
- The release has `ripr-server-manifest-v0.3.1.json`.
- The release has server archives and `.sha256` files for each supported
  target.
- The Windows archive checksum matched the manifest entry for
  `x86_64-pc-windows-msvc`.
- The extracted Windows server ran `ripr --version`, `ripr lsp --version`,
  `ripr pilot`, and `ripr outcome`.

That proof covers server archive shape for the current public release and the
defaults-first `ripr pilot` and `ripr outcome` public-install smoke; see
[Installation verification](INSTALLATION_VERIFICATION.md).

## Local Verification

After downloading a release asset for the current platform:

```bash
ripr --version
ripr lsp --version
```

Then install the local VSIX and open a Rust workspace, which exercises
`ripr lsp --stdio` through proper LSP framing:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.3.1.vsix --force
```

For the defaults-first release line, also run the server archive smoke from
[Installation verification](INSTALLATION_VERIFICATION.md): the extracted server
binary must report the release version and run `ripr pilot` against the checked
boundary-gap fixture.

## Notes

The extension verifies archive SHA-256 before extraction. It still keeps
`ripr.server.path` and PATH fallback for offline installs, pinned binaries, and
enterprise-managed environments.
