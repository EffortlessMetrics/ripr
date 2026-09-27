# Server Binary Rehearsal

The VS Code/Open VSX extension can self-provision only when GitHub Releases has
native `ripr` server archives and a manifest.

## Workflow

Use:

```text
.github/workflows/release-server-binaries.yml
```

Dispatch from the selected swarm branch:

```bash
gh workflow run release-server-binaries.yml --ref <candidate-branch> -f version=<candidate-package-version>
```

This read-only entrypoint calls `server-archive-qualification.yml` with the
dispatch commit's exact SHA. It builds and verifies:

```text
x86_64-pc-windows-msvc
x86_64-apple-darwin
aarch64-apple-darwin
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

Download the `ripr-server-qualification-*` Actions artifacts to inspect all
five archives, per-target checksums, manifest, `SHA256SUMS`, and the
JSON/Markdown receipt. The machine-readable receipt records the exact swarm
SHA, version, each target archive digest and resolved Rust/Cargo toolchain,
plus the Cargo.lock, workflow, manifest, and `SHA256SUMS` digests for the
source-repo handoff. Actions artifacts expire and are not public release
assets. The prospective URLs in the rehearsal manifest name the swarm repo;
the source release must generate its own manifest for its selected source
commit and independently verify the promoted assets. Public release creation
and asset upload belong only to `EffortlessMetrics/ripr`.

## Exact-candidate qualification (read-only)

Use `.github/workflows/server-archive-qualification.yml` when archive shape
must be checked before any publication decision. It requires an immutable
40-character `candidate_sha`; an optional `candidate_tag` must use the
`ripr-release-MAJOR.MINOR.PATCH` format and may be lightweight or annotated,
but must resolve to the same commit. Every matrix job fetches that
SHA, builds the existing five-target server matrix, verifies the archive
checksum, extracts the flat package, and checks both the archive label and the
candidate-built binary's `--version` command, including that the requested
qualification version matches the candidate package version and that the
binary names the candidate commit. The manifest job verifies
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
GitHub Release asset and not publication proof. The swarm
`release-server-binaries.yml` entrypoint uses this same qualification receipt;
the source repository's separate release workflow owns publication.


Packaging and manifest assembly intentionally live in Rust-first automation:

```bash
cargo xtask release-server-archive --version <VERSION> --target <target> --executable <ripr-or-ripr.exe> --archive <zip-or-tar.gz>
cargo xtask release-server-manifest --version <VERSION> --repository <owner/repo>
```

The qualification workflow orchestrates packaging and checksums. A later,
separately authorized source-repo release may publish assets named:

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

### RC placement and the embedded manifest digest (#3798)

The server manifest is placement-neutral per distribution generation: the
generation's manifest bytes (and archive identities) are identical on the
exact stable release and on the one public RC release of the same generation,
so one raw-byte manifest SHA-256 admits both placements. Build the RC release
from the generation's version (`release-server-manifest --version 0.11.0`),
upload those bytes to the RC tag, and upload the identical bytes to the stable
tag when it is cut.

`cargo xtask release-server-manifest` also regenerates
`editors/vscode/src/serverDescriptor.ts` with that digest. The release cut
must run the manifest command and commit the refreshed descriptor **before**
packaging the extension VSIX, so the packaged extension embeds the real
digest. A generation without an embedded descriptor has no fallback row: a
prerelease request's stable absence stays terminal.

With the descriptor embedded, a clean RC extension requests its own version
(for example `0.11.0-rc.1`) and the downloader resolves:

```text
fetch the exact stable placement v<GENERATION>/ripr-server-manifest-v<GENERATION>.json
  accepted      -> selected_stable_exact
  direct 404    -> fetch the one predeclared RC placement
                   v<REQUESTED>/ripr-server-manifest-v<GENERATION>.json
                   admitted by the same digest
                -> selected_rc_exact_after_stable_absent
  anything else -> fail closed with the original typed reason
```

A redirected 404, DNS/connect/TLS failure, timeout, 401/403, 5xx, redirect
loop, malformed or wrong-digest manifest, or a target-set identity conflict is
never absence and never falls back. No other RC, `latest` query, version
range, or mutable lookup is ever derived.

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
