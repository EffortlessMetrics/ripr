# Publishing

`ripr` is intentionally a single published package.

The full release checklist lives in [RELEASE.md](RELEASE.md). This page keeps
the short command sequence.

Before publishing:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Then publish:

```bash
cargo publish -p ripr
```

Verify `repository` and `homepage` point at the canonical GitHub repository before publishing.
