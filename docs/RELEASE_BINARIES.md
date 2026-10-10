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
ripr-server-v<VERSION>-<target>.<zip-or-tar.gz>.sha256
ripr-server-manifest-v<VERSION>.json
SHA256SUMS
```

The assembly step also retains the internal, non-publication evidence packet
`ripr-server-assembly-v<VERSION>.receipt.json`. It records the common build
identity, one validated per-target receipt/archive mapping, and the manifest
and `SHA256SUMS` digests. It is excluded from `SHA256SUMS` and is not a
release asset; downstream provenance and placement-independent subject
selection consume this evidence.


### Product identity and release placement

Archive and manifest `--version` inputs are **product versions**, such as
`0.11.0`. A channel suffix remains rejected by their canonical admission:
the same product archives and manifest must keep the same subject names at RC
and stable placements.

The existing producer commands also accept `--release-version <placement>`
instead of `--version`. This caller adapter reads the checked-out `ripr`
crate version through the existing workspace-inheritance reader, admits the
suffix-free source product, and binds the placement to that exact product.
It accepts the matching stable version/tag or `v<product>-rc.<positive integer>`
(with an optional leading `v`); leading-zero, mismatched, ordinary-prerelease,
ambiguous and missing inputs reject before producer filesystem changes.
It then calls the unchanged archive/manifest producers with the product.
This adapter is identity binding only; it grants no event or publication authority.

For example, on a source checkout whose product version is `0.11.0`, the
nonpublishing invocation shape is:

```text
cargo xtask release-server-archive --release-version v0.11.0-rc.2 --target <target> --executable <name> --archive <zip|tar.gz>
cargo xtask release-server-manifest --release-version v0.11.0-rc.2 --repository <owner/repo>
```

The names remain `ripr-server-v0.11.0-<target>.*` and
`ripr-server-manifest-v0.11.0.json`. These are synthetic invocation examples,
not an authorized or frozen RC. Current development `alpha.2` metadata cannot
stand in for a final suffix-free product checkout.

The legacy `release-upload-assets` CLI stops channel-suffixed inputs **before**
calling GitHub. Repairing producer placement transport does not make that
opportunistic create/clobber uploader an RC publisher. Exact RC event authority,
validated subjects and publication transport remain #1631/#1644/#1646 work.
Existing explicit product producer invocations and stable upload behavior remain
unchanged. Do not use the publication workflow as qualification proof.

## Final server subject preparation (nonpublishing)

After accepted archive/manifest assembly, prepare the exact current upload set:

```bash
cargo xtask release-final-server-subjects \
  --version <product-version> --repository EffortlessMetrics/ripr \
  --candidate-sha <40-lowercase-hex-sha> --candidate-tree <40-lowercase-hex-tree> \
  --dist dist --out target/ripr/final-server-subjects
```

The output parent must exist and the output directory must be fresh and outside
staging. This command reads staged bytes and writes local preparation evidence;
it never invokes a signer, verifier, publication tool, or credential request.
Its receipt always states `provenance_verified=false` and
`release_upload_eligible=false`. It does not gate or authorize the existing live
publisher yet, and a prepared JSON file cannot unlock publication.

The command reuses the canonical assembler's read-only rendering to validate
the receipt set and compare the final manifest, `SHA256SUMS`, and assembly receipt
byte-for-byte. It independently hashes the files, requires the expected source
SHA/tree/repository and locked release build contract, and binds every path from
the existing uploader allowlist to an accepted role. Observed unknown, missing,
changed, nonregular, symlinked, or aliased inputs reject with bounded JSON/Markdown
observations retained before the command fails. Metadata inputs are bounded at
4 MiB each and staging at 64 entries. The stable single-link/file-identity
controls currently require a Unix assembly host, matching the Ubuntu manifest
runner; other hosts reject rather than claim an unavailable alias check. All
five configured artifact target platforms remain in the subject set.

Input consistency is limited to observed snapshots. The command takes its
initial directory snapshot before the canonical assembler reads, then takes and
compares a final snapshot after those reads and before writing any preparation
packet. These checks do not provide an atomic filesystem snapshot or resistance
to hostile concurrent filesystem mutation: paths are reopened, staging is not
locked, and changes can occur and revert between observations or occur after the
final check. Run against exclusively controlled, quiescent staging. A later
signer or uploader must independently bind the bytes it actually consumes.

The current uploader selects **twelve** public files: five archives, five
per-archive `.sha256` sidecars, one manifest, and `SHA256SUMS`. Issue #1502's
seven-subject description omits the legacy sidecars. Preparation preserves and
explicitly inventories those five additional current upload paths. Removing
them would require a separately reviewed public-subject contract change.
Build/assembly receipts and the preparation outputs remain nonpublic.

The fresh output contains `final-server-subjects.json`,
`final-server-provenance-inputs.json`, `final-server-subjects.sha256`, and
`final-server-subjects.receipt.{json,md}`. The external subject-checksum list
includes the digest of the public `SHA256SUMS` bytes without modifying or making
that public file self-referential. The machine receipt is installed last and
binds the exact raw bytes of all three preparation files through
`inventory_sha256`, `provenance_inputs_sha256`, and `subject_checksums_sha256`.
A rejected packet has only its diagnostic receipt, with those digest fields
explicitly `null` and no inventory, provenance-input, or subject-checksum file
from a successful generation. These byte commitments provide no signing or
publication authority.

This is the inventory/rehearsal portion of #1502, which remains open. The next
transition must select a reviewed full-SHA producer action, separately establish
the narrow signing/OIDC permission boundary, execute and genuinely verify every
final subject with exact repository/workflow/ref/SHA/name/digest constraints,
and make live upload require that terminal admission. Synthetic fixtures and
these preparation receipts cannot substitute for it. Replay the producer on the
eventual history-preserving #1768 integrated source head before candidate-bound
qualification; this command does not freeze or qualify that candidate.

The release-server evidence contracts are versioned independently of release
placement: per-target build receipts use schema `0.2`, the assembled manifest
uses schema `2`, and the internal assembly receipt uses schema `0.2`.
Manifest assembly accepts only per-target receipts with schema `0.2`, validates
the platform-neutral compiler release/commit identity across runner hosts, and
retains host-specific `rustc -vV` text only as per-target evidence. The
publication command selects the archives, archive sidecars, versioned
manifest, and `SHA256SUMS` explicitly; receipt files remain downstream
evidence and are never release assets.

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

### Exact VSIX attachment transport

`cargo xtask release-upload-vsix --tag <tag>` consumes exactly one regular
`dist/*.vsix` file and requires an existing GitHub Release. It reads the asset
inventory before deciding whether to upload, never replaces an existing asset,
and independently downloads and SHA-256 compares the resulting asset. An
existing equal asset is verification-only; conflict, unavailable inventory,
malformed inventory, upload failure, or verification failure stops the command.
The command requires explicit release approval and is never part of a local
validation recipe. Unit tests use a fake transport and make no GitHub requests.

The workflow passes the tag through `RIPR_RELEASE_TAG` as argument data and
builds the helper before supplying the publication token to the execution step.
Downloaded verification bytes remain in a uniquely named temporary directory.
This transport does not admit the final release candidate, authorize a channel,
or replace the publication-convergence work in #1489/#1646.
