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
gh workflow run release-server-binaries.yml -f version=0.8.0
```

The workflow builds:

```text
x86_64-pc-windows-msvc
x86_64-apple-darwin
aarch64-apple-darwin
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

## Exact-candidate qualification (read-only)

Use `.github/workflows/server-archive-qualification.yml` when archive shape
must be checked before any publication decision. It requires an immutable
40-character `candidate_sha`; an optional `candidate_tag` must use the
`ripr-release-MAJOR.MINOR.PATCH` format and may be lightweight or annotated,
but must resolve to the same commit. Every matrix job fetches that
SHA, builds the existing five-target server matrix, verifies the archive
checksum, extracts the flat package, and checks both the archive label and the
candidate-built binary's `--version` command, including that the requested
qualification version matches the candidate package version. The manifest job verifies
`SHA256SUMS` and emits a
machine-readable and Markdown qualification receipt.

Dispatch with the already selected candidate identity:

```bash
gh workflow run server-archive-qualification.yml \
  -f candidate_sha=<40-character-candidate-sha> \
  -f candidate_tag=<optional-immutable-tag> \
  -f version=<version>
```

This workflow has `contents: read`, does not call `release-upload-assets`, and
does not use `GH_TOKEN`, `github.token`, or repository secrets. When a tag is
supplied, it verifies the fixed public detail endpoint
`/repos/EffortlessMetrics/ripr-swarm/rulesets/20661783` without credentials;
the response must contain the expected ruleset id, active tag target, singleton
`refs/tags/ripr-release-*` include, empty exclude, and both update/deletion
rules. Bounded HTTP retries report status, rate-limit, and response-digest
diagnostics on every failure, then fail closed if the endpoint or shape is
unavailable. Its only writes are scoped
GitHub Actions artifacts containing the archives, manifest, checksums, and
qualification receipt. An Actions artifact is rehearsal evidence, not a
GitHub Release asset and not publication proof. The existing
`release-server-binaries.yml` workflow remains the separate publication
authority and must not be used as the qualification receipt.


Packaging and manifest assembly intentionally live in Rust-first automation:

```bash
cargo xtask release-server-archive --version <VERSION> --target <target> --executable <ripr-or-ripr.exe> --archive <zip-or-tar.gz>
cargo xtask release-server-manifest --version <VERSION> --repository <owner/repo>
cargo xtask release-upload-assets --version <VERSION>
```

The workflow should only orchestrate those commands instead of keeping archive,
checksum, manifest, or upload branching logic in shell or PowerShell.

and uploads these assets to the matching GitHub Release:

```text
ripr-server-v<VERSION>-<target>.zip
ripr-server-v<VERSION>-<target>.tar.gz
ripr-server-manifest-v<VERSION>.json
SHA256SUMS
```

The `SHA256SUMS` sidecar is `sha256sum -c SHA256SUMS`-compatible (one
`<sha256>  <file_name>` line per asset). Releases through `v0.7.0` published the
same manifest under the legacy name `checksums.txt`; the content format is
unchanged.

Each server archive contains:

```text
ripr(.exe)
LICENSE-MIT
LICENSE-APACHE
README-server.txt
```

## Release Proof

The last verified public release line before 0.8.0 execution is `v0.7.0`,
published on May 20, 2026:

- The GitHub Release has `ripr-0.7.0.vsix`.
- The release has `ripr-server-manifest-v0.7.0.json`.
- The release has `checksums.txt`.
- The release has server archives and `.sha256` files for each supported
  target:
  - `x86_64-pc-windows-msvc`;
  - `x86_64-apple-darwin`;
  - `aarch64-apple-darwin`;
  - `x86_64-unknown-linux-gnu`;
  - `aarch64-unknown-linux-gnu`.
- The installed public loop was verified for `doctor`, `pilot`, `outcome`,
  `agent verify`, and `agent receipt`; see
  [Installation verification](INSTALLATION_VERIFICATION.md).
- Future releases must refresh the same VSIX, manifest, checksum, and
  per-target server-archive asset family before publication.

The historical `v0.3.1` release was verified on May 7, 2026:

- `ripr v0.3.1` was the public GitHub Release at that time.
- The release has `ripr-0.3.1.vsix`.
- The release has `ripr-server-manifest-v0.3.1.json`.
- The release has server archives and `.sha256` files for each supported
  target.
- The Windows archive checksum matched the manifest entry for
  `x86_64-pc-windows-msvc`.
- The extracted Windows server ran `ripr --version`, `ripr lsp --version`,
  `ripr pilot`, and `ripr outcome`.

That proof covered server archive shape for the then-current public release and
the defaults-first `ripr pilot` and `ripr outcome` public-install smoke; see
[Installation verification](INSTALLATION_VERIFICATION.md).

The historical `v0.4.0` release was verified on May 7, 2026:

- `ripr-server-manifest-v0.4.0.json`, `checksums.txt`, per-target server
  archives, per-target `.sha256` files, and `ripr-0.4.0.vsix` were present on
  the GitHub Release.
- The Windows archive checksum matched the manifest entry for
  `x86_64-pc-windows-msvc`.
- The extracted Windows server ran `ripr --version`, `ripr lsp --version`,
  `ripr pilot`, `ripr outcome`, `ripr agent verify`, and
  `ripr agent receipt`.

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
code --install-extension dist/ripr-0.8.0.vsix --force
```

For the defaults-first release line, also run the server archive smoke from
[Installation verification](INSTALLATION_VERIFICATION.md): the extracted server
binary must report the release version and run `ripr pilot` against the checked
boundary-gap fixture.

## Notes

The extension verifies archive SHA-256 before extraction. It still keeps
`ripr.server.path` and PATH fallback for offline installs, pinned binaries, and
enterprise-managed environments.
