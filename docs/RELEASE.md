# Release

This document is the release checklist for publishing `ripr`.

## Release History Snapshot

Use this quick ledger to keep release intent aligned with `CHANGELOG.md`:

- `0.1.0` (2026-05-01): first publishable alpha baseline for static RIPR
  exposure analysis.
- `0.2.0` (2026-05-01): editor/server distribution path, release archives,
  checksum manifest, and published VSIX/Open VSX extension.
- `0.3.0` (2026-05-02): syntax-backed analyzer + evidence-quality foundation,
  broader automation/gates, LSP state surfaces, and CI/release hardening.
- `0.4.0` (unreleased target): Campaign 4B completion is included; Campaign 5A
  usability/precision/calibration is in progress; Campaign 5B
  operationalization remains blocked on 5A outputs.

For detailed user-visible notes and category-level deltas, see the root
`CHANGELOG.md`.

## Preconditions

- The release branch has been reviewed and merged.
- The version in `crates/ripr/Cargo.toml` is correct.
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

## Runtime Smoke

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
```

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
```

Tag the release:

```bash
git tag v0.3.0
git push origin v0.3.0
```

Update docs or release notes if the install command or package metadata changed.
