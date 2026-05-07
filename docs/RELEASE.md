# Release

This document is the release checklist for publishing `ripr`.

## Preconditions

- The release branch has been reviewed and merged.
- The version in `crates/ripr/Cargo.toml` is correct.
- For the defaults-first public install line, the version is newer than
  `0.3.0`; `0.3.0` predates `ripr pilot` and `ripr outcome`.
- The root workspace uses Rust edition `2024`.
- The root workspace `rust-version` is `1.93`.
- `repository` and `homepage` point at `https://github.com/EffortlessMetrics/ripr/`.
- The README says `ripr` is alpha software and does not claim mutation execution.

## Local Gates

Run from the repository root:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

For the defaults-first install path, also run the local install proof from
[Installation verification](INSTALLATION_VERIFICATION.md).

## Runtime Smoke

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- pilot --root fixtures/boundary_gap/input --out target/ripr/release-smoke/pilot
cargo run -p ripr -- outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
```

## Install And Release Proof

Before calling an install or release-path PR complete, verify the crate package,
the local install path, the extension package, and the published server assets:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo install --path crates/ripr --locked --force --root target/ripr/install-smoke
target/ripr/install-smoke/bin/ripr --version
target/ripr/install-smoke/bin/ripr doctor
npm --prefix editors/vscode run package
```

For a published release, confirm that GitHub Releases contains the VSIX, server
manifest, server archives, and checksums:

```bash
gh release list --repo EffortlessMetrics/ripr --limit 5
gh release view v0.3.0 --repo EffortlessMetrics/ripr --json name,tagName,publishedAt,assets,url,isDraft,isPrerelease
gh release download v0.3.0 --repo EffortlessMetrics/ripr --pattern 'ripr-server-v0.3.0-x86_64-pc-windows-msvc.zip' --pattern 'ripr-server-manifest-v0.3.0.json' --dir target/ripr/release-smoke --clobber
```

The Campaign 7 release/install-polish pass verified `v0.3.0` as the latest
public release on May 6, 2026. That release has `ripr-v0.3.0.vsix`, a server
manifest, per-target server archives, checksums, and a Windows server archive
whose manifest checksum matched the downloaded ZIP. The extracted server ran
`ripr --version`, `ripr lsp --version`, and `ripr doctor`.

That `v0.3.0` proof covers packaging and server provisioning. It does not
verify the defaults-first public install loop because `v0.3.0` predates
`ripr pilot` and `ripr outcome`; use
[Installation verification](INSTALLATION_VERIFICATION.md) for the `0.3.1`
public-install smoke.

## Name Gate

Immediately before the first real publish:

```bash
cargo search ripr --limit 5
```

Then check the crates.io API:

```bash
curl -i https://crates.io/api/v1/crates/ripr
```

If `ripr` is taken, stop. Do not publish under a fallback name without a naming
decision.

## Publish

```bash
cargo login
cargo publish -p ripr
```

Cargo may time out while polling the registry index after upload. If that
happens, check crates.io manually before retrying.

## Post-Publish

```bash
cargo install ripr
ripr --version
ripr doctor
ripr pilot --root fixtures/boundary_gap/input --out target/ripr/install-smoke-cratesio/pilot
ripr outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

Tag the release:

```bash
git tag v0.3.1
git push origin v0.3.1
```

Update docs or release notes if the install command or package metadata changed.
