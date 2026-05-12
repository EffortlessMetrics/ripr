# Test Evidence Lanes

This document defines the evidence lane split for `ripr`. Each lane type runs at a different
cost point; the lanes do not target different signal classes.

## The core principle

`ripr` is the PR-time **static mutation-exposure analysis** filter. It catches the same class
of signal mutation testing catches — weak test/oracle exposure on changed behavior — by reading
the diff at draft time instead of running mutants:

> For the behavior changed in this diff, do the current tests appear to contain a discriminator
> that would notice if that behavior were wrong?

Mutation testing answers the same question with execution and remains the slower runtime
backstop for targeted, nightly, and release lanes when the change is ready for execution-backed
confirmation. The lane split is about *when* and *how expensively* the signal is taken, not
about parallel evidence streams.

Running `ripr` itself demonstrates disciplined CI economics. Every lane that runs here must be
lean enough that running it often is obviously worthwhile.

## Lane split

### Default PR lane

Runs on every PR synchronize event. Must be cheap, blocking, and fast.

```text
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-static-language
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-dependencies
cargo xtask check-supply-chain
```

Advisory on default PRs (non-blocking, upload artifacts):

```text
ripr self-dogfood (advisory; not a gate until calibrated)
coverage report (advisory; Codecov status is informational)
test analytics baseline delta
```

### Targeted PR lane

Runs when changed paths touch the analyzer, gate, or report surfaces, or when the `full-ci`
or `release-check` label is applied.

```text
default PR lane
+ mutation for changed analyzer/gate/report owner surface
+ coverage when label coverage/full-ci
```

Mutation in this lane is targeted and scoped to the changed surface — it is not a full
mutation matrix run.

### Nightly lane

Runs on a schedule. May be slower; not a PR tax.

```text
full mutation matrix
deeper coverage report
dogfood/report drift check
full fixture suite
```

### Release lane

Runs on push to `main`, on explicit dispatch, or on release tag. Blocks release.

```text
default PR lane (all required gates)
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo doc --workspace --no-deps
cargo xtask release-readiness --version <version>
output/schema contracts
VS Code extension package smoke
```

## What is not acceptable

- Gating ordinary Rust PRs behind VS Code extension e2e tests.
- Running release-surface proof (package dry-run, VSIX) on every PR.
- Running full mutation as a default PR gate.
- Enabling a soft gate before advisory data exists.
- Enforcing learned budgets before `ci-actuals.json` has accumulated history.
- Treating `ripr` findings as blocking before calibration demonstrates acceptable
  false-positive rate.

## Mutation doctrine

`ripr` is not a replacement for mutation testing. It is the PR-time exposure filter.

Mutation should be:

- Targeted on PRs that touch the analyzer, gate, or classifier.
- Broader on nightly/release.
- Never hidden inside an ordinary default PR as an invisible tax.

The output language boundary is enforced by `cargo xtask check-static-language`:

- Allowed static output: `exposed`, `weakly_exposed`, `reachable_unrevealed`,
  `no_static_path`, `infection_unknown`, `propagation_unknown`, `static_unknown`.
- Forbidden in static output: `killed`, `survived`, `untested`, `proven`, `adequate`.

See `docs/ci/ripr-mutation-boundary.md` for the full boundary doctrine.

## LEM budget bands

See `docs/ci/lem-budgeting.md` for the Local Evidence Minutes planning unit and band
definitions. The default PR lane targets the `standard` band. Release-surface lanes target the
`elevated` band and require `full-ci` or `release-check` label acknowledgement.

## See also

- [`docs/ci/ripr-mutation-boundary.md`](ripr-mutation-boundary.md) — mutation boundary.
- [`docs/ci/rust-1.95-quality-rollout.md`](rust-1.95-quality-rollout.md) — 0.5.1 rollout anchor.
- [`docs/ci/verification-ladder.md`](verification-ladder.md) — PR verification ladder.
- [`docs/ci/lem-budgeting.md`](lem-budgeting.md) — LEM budget bands.
- [`docs/ci/labels.md`](labels.md) — CI label registry.
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — verification economics.
- [`docs/ci/current-state.md`](current-state.md) — current CI implementation state.
