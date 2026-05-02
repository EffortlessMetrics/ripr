# Contributing

`ripr` is built PR by PR. Each PR should have a narrow production delta and a
complete evidence package, so reviewers can evaluate behavior, risk, and
traceability without reconstructing intent from chat history.

## Product Contract

Before changing code, check the product question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Changes that do not improve the precision, speed, usability, calibration, or
maintainability of that answer should be deferred.

## PR Shape

Prefer one PR per capability step from [Implementation plan](docs/IMPLEMENTATION_PLAN.md).

Each PR should include:

- scoped implementation or documentation changes
- tests or documented verification
- relevant docs updates
- changelog entry when behavior, workflow, or public docs change
- traceability from spec to tests to code for behavior changes

## Scoped Evidence-Heavy PRs

PR size is measured by production risk, not line count.

A scoped PR changes one production behavior, public contract, or architectural
seam, then includes the complete evidence package needed to make that change
reviewable: specs, fixtures, tests, golden outputs, docs, metrics, ADRs,
learnings, and traceability.

A large PR can be scoped when the production delta is narrow and most of the
diff is supporting evidence. A small PR can still be too large when it mixes
unrelated behaviors, changes multiple public contracts, or touches multiple
architectural seams without one shared acceptance criterion.

Every PR should make three things visible:

- production delta: what behavior, contract, or seam changed
- evidence delta: what specs, tests, fixtures, goldens, docs, metrics, ADRs, or
  learnings support it
- acceptance criterion: what single reviewable claim the PR proves

Prefer:

- narrow production delta
- large evidence delta when needed
- clear spec -> test -> code mapping
- deterministic golden output
- explicit metrics movement
- documented non-goals

Avoid:

- unrelated production behavior changes
- schema changes bundled with analyzer rewrites unless one acceptance criterion
  requires both
- LSP or UI changes bundled with classifier changes unless they share one
  finding contract
- cleanup mixed with behavior changes

## Review Checklist

Before requesting review:

- [ ] Scope matches one roadmap or implementation-plan item.
- [ ] Production delta and evidence delta are both explicit.
- [ ] New behavior has a spec entry or updates an existing spec.
- [ ] Tests use BDD-shaped names or fixture names that explain the behavior.
- [ ] Output changes update golden expectations and schema docs.
- [ ] Static output avoids mutation-runtime outcome language.
- [ ] Unknowns include stop reasons where applicable.
- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in
      production or test code.
- [ ] CI-relevant docs or workflows were updated when gates changed.
- [ ] New non-Rust programming files are covered by the file policy allowlist.
- [ ] New generated files, dependency surfaces, process spawning, and network
      behavior are covered by policy allowlists.
- [ ] `cargo xtask shape` or `cargo xtask fix-pr` was run before final checks.

## PR Shaping Commands

The shape/check/guide model is documented in
[PR automation](docs/PR_AUTOMATION.md).

Use `shape` when the tree needs safe normalization:

```bash
cargo xtask shape
```

It can run `cargo fmt`, sort policy allowlists, create
`target/ripr/reports`, and write `target/ripr/reports/shape.md`.

Use `fix-pr` as the current safe repair entrypoint:

```bash
cargo xtask fix-pr
```

It runs `shape`, refreshes `target/ripr/reports/pr-summary.md`, and writes
`target/ripr/reports/fix-pr.md`. Future automation also expands the repair
brief for judgment-required issues.

Generate the reviewer packet directly with:

```bash
cargo xtask pr-summary
```

The summary classifies changed paths into production, evidence, policy,
workflow, extension, and public-contract surfaces.

Run the cheap local guardrail with:

```bash
cargo xtask precommit
```

Run the review-ready non-release gate with:

```bash
cargo xtask check-pr
```

Validate fixture and golden scaffolding with:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
```

Long-running Codex Goals campaigns should follow
[Codex Goals](docs/CODEX_GOALS.md), [Implementation campaigns](docs/IMPLEMENTATION_CAMPAIGNS.md),
and the [Scoped PR contract](docs/SCOPED_PR_CONTRACT.md).

## Required Rust Gates

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask metrics
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo xtask check-static-language
cargo xtask check-no-panic-family
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
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

When the worktree is intentionally dirty during local review, Cargo packaging
commands may require `--allow-dirty`. A branch is not ready to merge until the
plain commands pass on a committed tree.

## Required Extension Gates

For changes under `editors/vscode`:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

## Documentation

Use the documentation map in [Documentation system](docs/DOCUMENTATION.md).

For behavior changes, update:

- [Specs](docs/specs/README.md)
- [Spec-test-code traceability](docs/SPEC_TEST_CODE.md)
- [Testing](docs/TESTING.md)
- output or config reference docs when public shapes change

For decisions, add or update an ADR.

For repo knowledge, update [Learnings](docs/LEARNINGS.md).
