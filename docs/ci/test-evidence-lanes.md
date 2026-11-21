# Test Evidence Lanes

This document defines the evidence lane split for `ripr`. Each lane type runs at
a different cost point; the lanes do not target different signal classes.

## The core principle

`ripr` is the PR-time **static mutation-exposure analysis** filter. It catches
the same class of signal mutation testing catches — weak test/oracle exposure
on changed behavior — by reading the diff at draft time instead of running
mutants:

> For the behavior changed in this diff, do the current tests appear to contain
> a discriminator that would notice if that behavior were wrong?

Mutation testing answers the same question with execution and remains the
slower runtime backstop for targeted, nightly, and release lanes when the change
is ready for execution-backed confirmation. The lane split is about *when* and
*how expensively* the signal is taken, not about parallel evidence streams.

Running `ripr` itself demonstrates disciplined CI economics. Every lane that
runs here must be lean enough that running it often is obviously worthwhile.

## Lane split

### Default PR lane

Runs on every PR synchronize event. It is blocking, bounded, and owned by the
routed reusable Rust gate workflow.

```text
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo xtask precommit
+ routed lane-only repository invariants
+ bounded PR evidence
```

The default lane does not also run full instrumented coverage, all-feature Test
Analytics, VS Code E2E, a duplicate MSRV build, or release packaging.

### Targeted PR lane

Runs when the operator selects an additional evidence family:

```text
coverage label
  + cargo llvm-cov --workspace --all-features

full-ci label
  + coverage
  + all-feature Test Analytics and doc tests
  + named MSRV proof
  + VS Code compile/package/E2E
  + Perl and release-surface proof

release-check label
  + Perl adapter proof
  + package list
  + publish dry-run
  + release-readiness
```

Mutation in a targeted lane should be scoped to the changed analyzer, gate, or
classifier surface. It is not a full mutation matrix run.

### Main and manual advisory lane

Pushes to `main` and explicit manual dispatches refresh the broader standing
evidence without taxing every pull request:

```text
coverage report
Test Analytics JUnit and doc-test proof
future Clippy readiness
source-of-truth and security posture
```

These workflows keep their advisory semantics. Selection is intentional; a
missing or unusable evidence artifact may still fail its own lane rather than
claiming that evidence exists.

### Nightly lane

Scheduled proof may be slower and is not a PR tax:

```text
full mutation matrix
deeper coverage report
dogfood/report drift check
full fixture suite
```

### Release lane

Runs on push to `main`, explicit release selection, or release automation:

```text
default required lane
Perl adapter proof
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo xtask release-readiness --version <version>
output/schema contracts
VS Code extension package smoke
```

## What is not acceptable

- Gating ordinary Rust PRs behind VS Code extension E2E tests.
- Running release-surface proof on every PR.
- Running full coverage and Test Analytics merely because a PR exists.
- Running full mutation as a default PR gate.
- Enabling a soft gate before advisory data exists.
- Enforcing learned budgets before `ci-actuals.json` has accumulated history.
- Treating `ripr` findings as blocking before calibration demonstrates an
  acceptable false-positive rate.

## Mutation doctrine

`ripr` is not a replacement for mutation testing. It is the PR-time exposure
filter.

Mutation should be:

- targeted on PRs that touch the analyzer, gate, or classifier;
- broader on nightly and release proof;
- never hidden inside an ordinary default PR as an invisible tax.

The output language boundary is enforced by `cargo xtask check-static-language`:

- Allowed static output: `exposed`, `weakly_exposed`, `reachable_unrevealed`,
  `no_static_path`, `infection_unknown`, `propagation_unknown`,
  `static_unknown`.
- Forbidden in static output: `killed`, `survived`, `untested`, `proven`,
  `adequate`.

See `docs/ci/ripr-mutation-boundary.md` for the full boundary doctrine.

## LEM budget bands

See `docs/ci/lem-budgeting.md` for the Local Evidence Minutes planning unit and
band definitions. The default PR lane targets the `medium` band. Release-surface
lanes target the `release` band and require `full-ci` or `release-check` label
acknowledgement.

## See also

- [`docs/ci/ripr-mutation-boundary.md`](ripr-mutation-boundary.md) — mutation boundary.
- [`docs/ci/rust-1.95-quality-rollout.md`](rust-1.95-quality-rollout.md) — 0.6.0 release-shaping anchor.
- [`docs/ci/verification-ladder.md`](verification-ladder.md) — PR verification ladder.
- [`docs/ci/lem-budgeting.md`](lem-budgeting.md) — LEM budget bands.
- [`docs/ci/labels.md`](labels.md) — CI label registry.
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — verification economics.
- [`docs/ci/current-state.md`](current-state.md) — current CI implementation state.
