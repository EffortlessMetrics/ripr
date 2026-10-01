# Coverage Reports

Coverage reporting is **execution-surface evidence only**. It shows whether changed code paths executed during testing, but does not prove correctness, conformance, safety, completeness, or mutation adequacy.

## What coverage measures

`ripr`'s coverage reports use `cargo-llvm-cov` to generate line and branch coverage over:
- Product code: `crates/ripr/src/`
- Automation code: `xtask/src/`

Coverage artifacts are excluded for:
- `target/` (build output)
- `fixtures/**/target/` (fixture build output)
- `editors/vscode/**/node_modules/` (extension dependencies)
- `xtask/src/reports/release.rs` (release automation)

## Claim boundaries

The Codecov badge and coverage reports do **not** prove:
- Test discriminator adequacy (coverage does not equal mutation killing)
- Seam classification completeness
- Oracle strength across all five RIPR stages
- Static analysis correctness (dynamic mutation testing required)
- Reproducibility across different versions of `cargo-llvm-cov`

Coverage is **advisory** and does not block merges. Codecov status checks are informational by default.

## Baseline targets

As of 2026-05-07, the project coverage baseline is 75.5%:
- **Product code** (`crates/ripr/src/`): 94.8% coverage; target 94% (project), 94% (patch)
- **Automation** (`xtask/src/`): 59% coverage; target 59% (project), 75% (patch)

Thresholds:
- **Project**: 1% for product, 1% for automation
- **Patch**: 3% for product, 10% for automation

## Manual verification

Use `workflow_dispatch` on the `Coverage` workflow to verify after changing:
- `.github/workflows/coverage.yml`
- `codecov.yml`
- Coverage-related policy entries
- Test topology that may affect `cargo-llvm-cov`

Expected artifacts after a coverage run:
- `lcov.info`, uploaded by GitHub Actions as the `rust-lcov` artifact.

Codecov upload requires the `CODECOV_TOKEN` secret and runs only for trusted
pushes, manual dispatches, and same-repo pull requests. Fork pull requests still
generate `lcov.info` and upload the `rust-lcov` artifact, but skip Codecov
upload because repository secrets are unavailable.

### Validate locally

To generate coverage and inspect artifacts locally, run the same supported
recipe as `.github/workflows/coverage.yml`: export the instrumentation
environment first, clean after that environment, run a plain instrumented
build and plain test run, then report. The explicit absolute
`RIPR_TEST_BINARY` binds the analyzer-consuming xtask tests to the actual
instrumented executable; a configured coverage target that does not name the
instrumented analyzer fails closed instead of substituting an ordinary
binary. `report` takes no build flags and, at this virtual workspace root,
covers all members even without `--workspace`.

```bash
llvm_cov_env="$(cargo llvm-cov show-env --export-prefix)"
eval "$llvm_cov_env"
cargo llvm-cov clean --workspace
cargo build --workspace --all-features
export RIPR_TEST_BINARY="$CARGO_LLVM_COV_TARGET_DIR/debug/ripr"
test -f "$RIPR_TEST_BINARY"
"$RIPR_TEST_BINARY" --version
sha256sum "$RIPR_TEST_BINARY"
cargo test --workspace --all-features --tests
cargo llvm-cov report --lcov --output-path lcov.info
ls -lh lcov.info
```

The show-env output is captured in a standalone assignment before `eval`
so a producer failure propagates under `set -e` instead of being masked
by `eval`.

To view the LCOV report in a browser (if you have `genhtml` installed):

```bash
genhtml lcov.info -o coverage-report/
open coverage-report/index.html
```

## Future calibration

Threshold ratcheting should follow the strategy documented in [IMPLEMENTATION_CAMPAIGNS.md](../IMPLEMENTATION_CAMPAIGNS.md). Allow real data to accumulate on `main` before raising targets or making Codecov status blocking for branch protection.
