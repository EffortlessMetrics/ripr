# Testing

Tests should prove product behavior, not implementation trivia. For analyzer and
output work, use BDD-shaped names and fixtures that make the behavior question
plain:

```text
given_changed_boundary_when_equal_value_is_missing_then_reports_weak_exposure
```

Behavior changes should have a three-way match:

```text
spec -> test -> code
```

See [Spec-test-code traceability](SPEC_TEST_CODE.md) for the expected mapping.
See [Test taxonomy](TEST_TAXONOMY.md) for required proof levels by change type.

Run everything:

```bash
cargo xtask shape
cargo xtask pr-summary
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo xtask ci-fast
cargo xtask ci-full
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
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

## Error-Handling Bar

The target rule is:

```text
No panic, unwrap, expect, todo, or unimplemented in production or tests.
```

New tests should return `Result` when setup can fail and should use explicit
assertions. Existing panic-family usage is tracked engineering debt and should
be paid down in scoped PRs rather than copied into new tests.

## Golden Output

When changing user-visible output, update or add golden coverage for:

- human output
- JSON output
- context packets
- LSP diagnostic shape, when applicable

Golden updates must preserve the static language boundary: draft static output
does not use mutation-runtime terms such as `killed` or `survived`.
