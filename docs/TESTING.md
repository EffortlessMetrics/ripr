# Testing

Run everything:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo xtask ci-fast
cargo xtask ci-full
```

Package check:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

The current test suite covers:

- unified diff parsing
- Rust test/assertion extraction
- JSON escaping
- simple end-to-end diff analysis
- CLI smoke behavior
