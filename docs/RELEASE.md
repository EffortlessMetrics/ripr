# Release

This document is the release checklist for publishing `ripr`.

Run the [Release copy checklist](RELEASE_COPY_CHECKLIST.md) before finalizing
the GitHub Release body, triggering `publish-extension.yml`, or running
`cargo publish`. It captures the public-surface rules (release body vs.
process narrative, marketplace metadata, install truth, badge freshness,
public vocabulary, asset verification) that the v0.5.0 release surfaced.

## Preconditions

- The release branch has been reviewed and merged.
- The version in `crates/ripr/Cargo.toml` is correct.
- For the defaults-first public install line, the version is newer than
  `0.3.0`; `0.3.0` predates `ripr pilot` and `ripr outcome`.
- The root workspace uses Rust edition `2024`.
- The root workspace `rust-version` is `1.95`.
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
cargo xtask check-product-copy
cargo xtask check-generated-clean
cargo xtask release-readiness --version 0.7.0
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

For the defaults-first install path, also run the local install proof from
[Installation verification](INSTALLATION_VERIFICATION.md).

For 0.7.x release claims about finding alignment or evidence accuracy, also
run the Lane 1 evidence reports:

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
```

The release claim is supported only when the Lane 1 audit coverage section
reports zero `static_unknown` items without named limitations, zero actionable
canonical items without repair routes, and zero actionable canonical items
without verify commands. Raw finding counts are diagnostic context; actionable
aligned items are the user-facing work count.

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
target/ripr/install-smoke/bin/ripr first-pr --help
target/ripr/install-smoke/bin/ripr doctor
target/ripr/install-smoke/bin/ripr pilot --root fixtures/boundary_gap/input --out target/ripr/install-smoke/pilot
target/ripr/install-smoke/bin/ripr outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
npm --prefix editors/vscode ci
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run package
```

For first-run adoption releases, also confirm the generated GitHub workflow
summary includes `#### First-run status`, the `missing_start_here` recovery
state, and the `target/ripr/reports/start-here.md` front door. Confirm
`editors/vscode/package.json` contributes `ripr: Start Current Repair` so the
installed editor path exposes the same repair loop.

For a published release, confirm that GitHub Releases contains the VSIX, server
manifest, server archives, and checksums:

```bash
gh release list --repo EffortlessMetrics/ripr --limit 5
gh release view v0.7.0 --repo EffortlessMetrics/ripr --json name,tagName,publishedAt,assets,url,isDraft,isPrerelease
gh release download v0.7.0 --repo EffortlessMetrics/ripr --pattern 'ripr-server-v0.7.0-x86_64-pc-windows-msvc.zip' --pattern 'ripr-server-manifest-v0.7.0.json' --dir target/ripr/release-smoke --clobber
```

For the `v0.7.0` release, the GitHub Release must have the VSIX, server
manifest, per-target server archives, checksums, and a server archive whose
manifest checksum matches the downloaded archive. The extracted server must run
`ripr --version`, `ripr lsp --version`, `ripr pilot`, `ripr outcome`, and
`ripr agent verify`. When an agent verify JSON artifact is available, also run
`ripr agent receipt` for the top seam.

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
cargo install ripr --version 0.7.0 --locked --root target/ripr/install-smoke-cratesio --force
target/ripr/install-smoke-cratesio/bin/ripr --version
target/ripr/install-smoke-cratesio/bin/ripr first-pr --help
target/ripr/install-smoke-cratesio/bin/ripr doctor
target/ripr/install-smoke-cratesio/bin/ripr pilot --root fixtures/boundary_gap/input --out target/ripr/install-smoke-cratesio/pilot
target/ripr/install-smoke-cratesio/bin/ripr outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
mkdir -p target/ripr/install-smoke-cratesio/agent
target/ripr/install-smoke-cratesio/bin/ripr agent verify --root . --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json --json > target/ripr/install-smoke-cratesio/agent/agent-verify.json
target/ripr/install-smoke-cratesio/bin/ripr agent receipt --root . --verify-json target/ripr/install-smoke-cratesio/agent/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/install-smoke-cratesio/agent/agent-receipt.json
```

Tag the release:

```bash
git tag v0.7.0
git push origin v0.7.0
```

Update docs or release notes if the install command or package metadata changed.

## Public-Surface Copy

Before publishing, run the
[Release copy checklist](RELEASE_COPY_CHECKLIST.md). It covers the GitHub
Release body, marketplace metadata, README install commands, badge freshness
disclosure, public vocabulary, and asset/dependent-channel verification.
For 0.7.0, also compare GitHub About and repository topics with the current
release-readiness handoff before changing repository metadata. Repository
metadata should describe static mutation-exposure analysis for test-gap review
without implying runtime mutation execution, generated tests, provider-backed
analysis, default blocking, or stable preview-language gate authority.

## Recovery

If a release workflow fails after the tag has been pushed, prefer
fix-forward over retagging. The tag is the release-prep snapshot; the
release workflows can be rerun against `main` (or any commit that contains
the fix) using `workflow_dispatch`, and uploaded assets attach to the
existing GitHub Release rather than replacing it.

1. Open a focused fix PR on `main` that reproduces the failure as a test
   and fixes only the broken path. Merge it.
2. Rerun the failed workflow via `workflow_dispatch` with the same
   `version` input as the tag, for example
   `gh workflow run release-server-binaries.yml -f version=0.7.0`. The
   asset names continue to use the original version, so they overlay
   correctly on the existing Release.
3. After server assets are present and verified, rerun any downstream
   workflow that was gated on them, for example
   `gh workflow run publish-extension.yml -f version=0.7.0`.
4. Do not retag and do not delete the GitHub Release. Leave the tag at
   the release-prep commit; the fix-forward commit is on `main` and any
   subsequent point release will include it.
5. Update the GitHub Release body to document the recovery if the failure
   was user-visible. crates.io publish remains a manual step and should
   only run once asset verification is complete.

This pattern is what the `v0.5.0` release used after the initial Windows
server-archive failure; see CHANGELOG `Release recovery (v0.5.0)` for the
record.
