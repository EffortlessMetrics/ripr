# Release

This document is the release checklist for publishing `ripr`.

## Preconditions

- The release branch has been reviewed and merged.
- The version in `crates/ripr/Cargo.toml` is correct.
- The root workspace uses Rust edition `2024`.
- The root workspace `rust-version` is `1.92`.
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
git tag v0.1.0
git push origin v0.1.0
```

Update docs or release notes if the install command or package metadata changed.

