# Upstream Tool Substrate Standard

`ripr` standardizes on a small set of upstream engines, but keeps the
repo-facing control surface in `xtask`. The goal is stable repo policy and
review evidence, not a public contract around every third-party binary.

Core doctrine:

```text
Make xtask the repo surface.
Make upstream tools the engine room.
```

This keeps policy encoded in Rust automation, keeps exceptions receipted, and
lets CI optimize evidence per minute without scattering repo rules through
workflow YAML or ad hoc scripts.

## Repo-facing surface

Contributors and agents should prefer the `cargo xtask ...` command that names
the repo intent. Upstream tool command lines are implementation details unless a
specialized local investigation needs them.

Existing repo-facing commands already cover the main PR surface:

```bash
cargo xtask check-pr
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask ripr-pr
cargo xtask check-dependencies
cargo xtask check-supply-chain
cargo xtask check-workflows
cargo xtask policy-report
```

Target wrappers may be added when the underlying lane becomes active:

```bash
cargo xtask allow-check
cargo xtask allow-diff
cargo xtask unsafe-review-pr
cargo xtask test-pr
cargo xtask test-docs
cargo xtask coverage
cargo xtask mutation-targeted
cargo xtask miri-targeted
cargo xtask semver-check
cargo xtask check-toml
```

Adding a wrapper should preserve the same rule: the wrapper describes the repo
contract and the upstream tool remains replaceable engine-room machinery.

## Standard upstream engines

| Plane | Standard substrate | Repo role |
| --- | --- | --- |
| Syntax and codemods | `ast-grep`; Rust analyzer crates for Rust authority | Find syntactic candidates cheaply; use Rust-aware data when identity must survive refactors. |
| Workspace graph | `cargo_metadata`, `guppy` | Inventory packages and targets, plan reverse-dependency and risk-pack lanes, and route CI/release work. |
| Test execution | `cargo-nextest`, `cargo test --doc` | Run ordinary Rust tests quickly while keeping doctests on Cargo's runner. |
| Coverage | `cargo-llvm-cov`, Codecov | Produce execution-surface telemetry and artifacts without treating coverage as test-discriminator adequacy. |
| Static mutation exposure | `ripr` | Produce PR-time changed-behavior exposure evidence, weak-oracle findings, repair packets, and review summaries. |
| Runtime mutation backstop | `cargo-mutants` | Confirm selected high-risk seams on targeted, nightly, or release lanes; do not impose full mutation on ordinary PRs. |
| Unsafe and UB witnesses | `unsafe-review`, Miri | Make unsafe changes reviewable statically, then run concrete UB witnesses only when targeted risk warrants it. |
| Source exceptions | `cargo-allow` | Own durable exception receipts for source and workflow policy. |
| Dependency trust | `cargo-deny`, `cargo-vet`, RustSec / `cargo-audit`, `cargo-auditable` | Gate dependency policy, advisory state, audit evidence, and shipped-binary dependency inspection. |
| Public API / release | `cargo-semver-checks`, rustdoc JSON | Check release compatibility and support custom public-surface reports. |
| Workflow policy | `actionlint`, `zizmor` | Separate workflow correctness from workflow security posture, with repo exceptions receipted by policy. |
| Text and config hygiene | `taplo`, `typos`, Markdown link/style tooling | Keep TOML, spelling, and documentation mechanics stable once dictionaries and policies are baselined. |
| Workspace hygiene | `cargo-udeps`; `cargo-hakari` only when justified | Run unused-dependency cleanup on scheduled/manual lanes; only add feature-unification machinery for measured duplicate-build pain. |
| CI cache | `Swatinem/rust-cache`; `sccache` only when justified | Restore/cache Rust builds economically; add compiler cache infrastructure only for large or self-hosted runner economics. |

## Authority boundaries

- `ast-grep` finds candidates; Rust-aware tooling decides authoritative Rust
  policy identity.
- `git ls-files -z` is the normal source inventory for tracked-file policy;
  broader filesystem walks need an explicit reason.
- Coverage is execution-surface context. It must not be phrased as
  discriminator adequacy, merge approval, or release readiness.
- `ripr` reports static mutation-exposure evidence. It must not report runtime
  mutation outcomes.
- `cargo-mutants` and Miri are runtime backstops for targeted, nightly, and
  release lanes, not default full-workspace PR taxes.
- `unsafe-review` records reviewable safety evidence. It does not prove memory
  safety or UB-free behavior.
- `cargo-deny` is the normal dependency policy gate; `cargo-vet` is a maturity
  layer for durable third-party audit evidence.
- `cargo-semver-checks` is the default public API compatibility gate; rustdoc
  JSON is for custom product facts.

## Adoption posture

Default PR lanes should stay cheap and repo-shaped. Heavy engines belong behind
risk routing, scheduled lanes, release lanes, or explicit labels. When a new
upstream tool is introduced, update the xtask command catalog, policy receipts,
and CI documentation so contributors keep invoking repo intent instead of a
scattered set of raw tool commands.
