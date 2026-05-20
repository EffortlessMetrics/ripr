# Installation Verification

Use this checklist for `release/install-polish` and for every release that
claims the defaults-first operator loop.

The install promise is:

```text
cargo install ripr
ripr pilot
# add one focused test
ripr outcome --before <before.repo-exposure.json> --after <after.repo-exposure.json>
ripr agent verify --before <before.repo-exposure.json> --after <after.repo-exposure.json> --json
ripr agent receipt --verify-json <agent-verify.json> --seam-id <seam_id> --json
```

`ripr init` may materialize repo policy, but it must not be required for the
first useful CLI, editor, or CI experience.

## Last Published Release Proof

The `ripr 0.6.0` release was published and verified on 2026-05-18. The
verified public loop covers `ripr first-pr`, zero-config `ripr pilot`,
`ripr outcome`, `ripr agent verify`, `ripr agent receipt`, saved-workspace
editor actions, first-run/start-here guidance, operator cockpit status, and
generated non-blocking CI artifacts.

Post-publish proof covered:

- crates.io serving `ripr 0.6.0`;
- public `cargo install ripr --version 0.6.0 --locked`;
- installed CLI smoke for `doctor`, `first-pr --help`, `first-pr`, `pilot`,
  `outcome`, `agent verify`, and `agent receipt`;
- GitHub Release `v0.6.0` with VSIX, server manifest, checksums, and all
  supported server archives (Windows, Linux x86_64/aarch64, macOS
  x86_64/aarch64);
- VS Marketplace serving `EffortlessMetrics.ripr@0.6.0`;
- Open VSX serving `EffortlessMetrics.ripr@0.6.0`.

Release execution also verified the Windows server archive checksum, the
downloaded VSIX package version, the Open VSX badge/listing routes, and the
public-copy boundary. Isolated marketplace install smoke remains useful
post-release hygiene, but the 0.6.0 public availability claim is already
verified by the release closeout.

## Next Release-Candidate Proof

Use this section for the next unreleased line. Until the maintainer tags,
publishes, and verifies new public artifacts, do not claim crates.io, GitHub
Release, VS Marketplace, or Open VSX availability for that next version.

Pre-publish proof should record:

- local package and publish dry-run success;
- path-installed `ripr --version` and `ripr first-pr --help`;
- generated CI dry-run with start-here/advisory gate boundaries;
- VSIX packaging success;
- one external-adopter smoke showing an installed binary can find one Rust
  repairable gap, produce a bounded packet or no-action state, add one focused
  proof outside `ripr`, and verify static movement.

## Previous Release Proof

The `ripr 0.5.0` release was published and verified on 2026-05-10. The
verified public loop covered zero-config `ripr pilot`, `ripr outcome`,
`ripr agent verify`, `ripr agent receipt`, saved-workspace editor actions,
operator cockpit status, and generated non-blocking CI artifacts.

Post-publish proof covered:

- crates.io serving `ripr 0.5.0`;
- public `cargo install ripr --version 0.5.0 --locked`;
- installed CLI smoke for `doctor`, `pilot`, `outcome`, `agent verify`, and
  `agent receipt`;
- GitHub Release `v0.5.0` with VSIX, server manifest, checksums, and all
  supported server archives (Windows, Linux x86_64/aarch64, macOS
  x86_64/aarch64);
- VS Marketplace serving `EffortlessMetrics.ripr@0.5.0`;
- Open VSX serving `EffortlessMetrics.ripr@0.5.0`.

## Older Release Proof

The `ripr 0.4.0` release was published and verified on 2026-05-07. The
verified public loop covered zero-config `ripr pilot`, `ripr outcome`,
`ripr agent verify`, `ripr agent receipt`, saved-workspace editor actions,
operator cockpit status, and generated non-blocking CI artifacts.

Post-publish proof covered:

- crates.io serving `ripr 0.4.0`;
- public `cargo install ripr --version 0.4.0 --locked`;
- installed CLI smoke for `doctor`, `pilot`, `outcome`, `agent verify`, and
  `agent receipt`;
- GitHub Release `v0.4.0` with VSIX, server manifest, checksums, and all
  supported server archives;
- Windows server archive checksum matching the manifest;
- extracted Windows server smoke for `ripr --version`, `ripr lsp --version`,
  `pilot`, `outcome`, `agent verify`, and `agent receipt`;
- VS Marketplace serving `EffortlessMetrics.ripr@0.4.0`;
- Open VSX serving `EffortlessMetrics.ripr@0.4.0`;
- isolated VS Code install smoke from VS Marketplace and from the Open VSX
  VSIX download.

## Pre-Publish Local Proof

Run these before publishing:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Install from the checked-out package into a local temp root and exercise the
operator loop with checked examples. The checked fixture is only the repeatable
release smoke input; the installed binary must not depend on the `ripr` source
checkout for normal use.

```bash
cargo install --path crates/ripr --locked --root target/ripr/install-smoke-path --force
target/ripr/install-smoke-path/bin/ripr --version
target/ripr/install-smoke-path/bin/ripr first-pr --help
target/ripr/install-smoke-path/bin/ripr pilot \
  --root fixtures/boundary_gap/input \
  --out target/ripr/install-smoke-path/pilot
target/ripr/install-smoke-path/bin/ripr outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
mkdir -p target/ripr/install-smoke-path/agent
target/ripr/install-smoke-path/bin/ripr agent verify \
  --root . \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json \
  --json > target/ripr/install-smoke-path/agent/agent-verify.json
target/ripr/install-smoke-path/bin/ripr agent receipt \
  --root . \
  --verify-json target/ripr/install-smoke-path/agent/agent-verify.json \
  --seam-id 67fc764ba37d77bd \
  --json \
  --out target/ripr/install-smoke-path/agent/agent-receipt.json
```

On Windows, use `target\ripr\install-smoke-path\bin\ripr.exe`.

Also confirm the generated CI and editor first-run front doors before release:

```bash
cargo xtask release-readiness --version 0.7.0
```

The readiness report must show that generated GitHub CI includes `#### First-run
status`, `missing_start_here`, and `target/ripr/reports/start-here.md`; it must
also verify that the VS Code manifest contributes `ripr: Start Current Repair`.

## Public Cargo Install Proof

After publishing to crates.io, run the same smoke from an isolated install root.
Use the version being verified so an older cached or latest crate cannot mask a
release mistake:

```bash
cargo install ripr --version 0.7.0 --locked --root target/ripr/install-smoke-cratesio --force
target/ripr/install-smoke-cratesio/bin/ripr --version
target/ripr/install-smoke-cratesio/bin/ripr first-pr --help
target/ripr/install-smoke-cratesio/bin/ripr pilot \
  --root fixtures/boundary_gap/input \
  --out target/ripr/install-smoke-cratesio/pilot
target/ripr/install-smoke-cratesio/bin/ripr outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
mkdir -p target/ripr/install-smoke-cratesio/agent
target/ripr/install-smoke-cratesio/bin/ripr agent verify \
  --root . \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json \
  --json > target/ripr/install-smoke-cratesio/agent/agent-verify.json
target/ripr/install-smoke-cratesio/bin/ripr agent receipt \
  --root . \
  --verify-json target/ripr/install-smoke-cratesio/agent/agent-verify.json \
  --seam-id 67fc764ba37d77bd \
  --json \
  --out target/ripr/install-smoke-cratesio/agent/agent-receipt.json
```

The installed version must be the release being verified. If crates.io still
serves an older version, the defaults-first public install is not verified.
The fixture paths are for this repository's release smoke; a user should be
able to run the same commands against any Rust/Cargo workspace without checking
out the `ripr` source.

## GitHub Release Server Proof

After the server-binary workflow runs for the release tag, verify that the
GitHub Release has:

```text
ripr-server-manifest-v<VERSION>.json
checksums.txt
ripr-server-v<VERSION>-x86_64-pc-windows-msvc.zip
ripr-server-v<VERSION>-x86_64-unknown-linux-gnu.tar.gz
ripr-server-v<VERSION>-aarch64-unknown-linux-gnu.tar.gz
ripr-server-v<VERSION>-x86_64-apple-darwin.tar.gz
ripr-server-v<VERSION>-aarch64-apple-darwin.tar.gz
```

Then download the archive for the current platform, verify the SHA-256 from the
manifest or sidecar checksum, extract it, and run:

```bash
ripr --version
ripr lsp --version
ripr pilot --root fixtures/boundary_gap/input --out target/ripr/release-server-smoke/pilot
ripr outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
mkdir -p target/ripr/release-server-smoke/agent
ripr agent verify \
  --root . \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json \
  --json > target/ripr/release-server-smoke/agent/agent-verify.json
ripr agent receipt \
  --root . \
  --verify-json target/ripr/release-server-smoke/agent/agent-verify.json \
  --seam-id 67fc764ba37d77bd \
  --json \
  --out target/ripr/release-server-smoke/agent/agent-receipt.json
```

The VS Code extension relies on these assets for first-run self-provisioning.

## VS Code Package Proof

Package the extension from the repository root:

```bash
npm --prefix editors/vscode run package
```

The VSIX version must match `editors/vscode/package.json`. Before publishing the
extension, confirm that the matching GitHub Release server manifest is already
available. The extension resolves the server in this order:

```text
ripr.server.path
bundled server binary, if present
downloaded cached server binary
verified first-run download from GitHub Releases
ripr on PATH
actionable error
```

Normal editor installs should not require `cargo install ripr`; that remains a
fallback for offline, pinned, or controlled environments.

## Known Limits

These limits are intentional for the defaults-first release:

- no runtime mutation execution by default;
- no CI blocking by default;
- no runtime mutation outcome language in static outputs;
- no unsaved-buffer analysis overlays by default;
- no automatic Rust or Cargo installation in the editor extension;
- no platform-specific VSIX packages with bundled native binaries yet.
