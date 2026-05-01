# Publishing

`ripr` is intentionally a single published package.

Before publishing:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Then publish:

```bash
cargo publish -p ripr
```

Verify `repository` and `homepage` point at the canonical GitHub repository before publishing.
