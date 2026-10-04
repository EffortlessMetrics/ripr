# Canonical product-gate plan

Issue #3330 establishes the typed authority for ordinary product gates before
any workflow migration. The implementation lives in
`xtask/src/product_gate_plan.rs` and is intentionally pure: it describes gate
meaning and applicability without selecting a runner, invoking a provider, or
changing required-check routing.

## Current producer inventory

The current producer is `.github/workflows/rust-gates.yml`, called by the
CX43, CPX42, CX53, and GitHub-hosted routes in `routed-rust.yml`. The required
commands have separate named steps with stable IDs; they are not copied into
each runner route. Runner identity and matrix shape are route details, not
product meaning.

| Canonical gate | Current command | Role | Surface |
| --- | --- | --- | --- |
| `product.rust.formatting` | `cargo fmt --check` | required | Rust |
| `product.rust.workspace_check` | `cargo check --workspace --all-targets` | required | Rust |
| `product.rust.clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | required | Rust |
| `product.rust.workspace_tests` | `cargo nextest run --workspace --profile ci` | required | Rust |
| `product.rust.workspace_doc_tests` | `cargo test --workspace --doc` | required | Rust |
| `product.repository.precommit` | `cargo xtask precommit` | required | repository policy |
| `product.evidence.promotion_honesty` | `cargo xtask check-evidence-promotion-honesty` | required | evidence |
| `product.repository.agent_skills` | `cargo xtask check-agent-skills` | required | repository policy |
| `product.repository.dependencies` | `cargo xtask check-dependencies` | required | repository policy |
| `product.repository.process_policy` | `cargo xtask check-process-policy` | required | repository policy |
| `product.repository.network_policy` | `cargo xtask check-network-policy` | required | repository policy |
| `product.evidence.goldens` | `cargo xtask goldens check` | required | evidence |
| `product.evidence.fixtures` | `cargo xtask fixtures` | required | evidence |

The Rust test contract is `canonical_nextest_plus_cargo_doc` (#3825,
`TEST_RUNNER_CONTRACT` in `xtask/src/product_gate_plan.rs`). It is
intentionally dual:

| Subject | Required owner | Features | Evidence retained |
| --- | --- | --- | --- |
| lib, bin, integration, and example test binaries | `cargo nextest run --workspace --profile ci` | default | fresh `junit.xml` naming at least one test, plus `run-context.txt` with checkout SHA, tool versions, and blob identities of `Cargo.lock`, `.config/nextest.toml`, and `rust-gates.yml` |
| Rust doctests | `cargo test --workspace --doc` | default | job log only |
| `lang-perl` and other non-default-feature tests | not in the required lane | all / perl / no-default | advisory Test Analytics, the `Perl and release proof` job, and the Windows advisory feature matrix |

Nextest cannot execute doctests, so a green nextest row never stands in for the
doctest row, and neither row claims non-default-feature subjects. The `ci`
profile pins `retries = 0` and may not declare a `default-filter` or
per-test overrides, so every test selected by default is required; the
`framed_lsp_`/`editor_agent_loop_` skip in `.config/nextest.toml` is a local
iteration hint and is never applied in CI. Nextest exit 0 without a fresh JUnit
report naming at least one test fails the step, so an empty selection cannot
render green. A change to the nextest config, the workflow, or `Cargo.lock`
changes the blob identities recorded in `run-context.txt`, so an older receipt
cannot be read as evidence for the new inputs.

Focused tests in `product_gate_plan.rs` read the real workflow, nextest
config, and this table. Each of these fails `cargo nextest run`:

- removing, filtering, or respelling a required runner row, or adding any
  other single logical line (backslash continuations joined) that invokes
  `cargo test`/`cargo t`/`cargo nextest run`/`r`, including behind wrappers,
  leading flags, or a `+toolchain`;
- an `if:`, `continue-on-error:`, or `shell:` key (bare or quoted) on a runner
  step; an `if:` or `continue-on-error:` on any job; a `defaults:` block; or a
  doctest step that is anything other than exactly
  `run: cargo test --workspace --doc`;
- any `NEXTEST_*`, `RUSTDOCFLAGS`, or `CARGO_TARGET_*_RUNNER` variable in the
  workflow;
- a `default-filter`, `overrides`, non-zero or table `retries`, or a
  different JUnit path anywhere in the parsed nextest config, quoted or inline;
- restating a different command in this table.

The `Required Rust tests` step body itself is executed under
`bash -eo pipefail` with the runners stubbed: a zero-test, leading-zero, junk,
missing, or stale-only report fails a green run, and a failing run keeps its
exit code in the step result and in `run-context.txt`.

These are text- and step-level controls over one workflow file, not a YAML or
shell interpreter. They do not resolve repository-defined Cargo aliases, read
runner-host or caller environment, catch control flow added to the nextest
step beyond the executed scenarios, assert the blob identities that
`run-context.txt` records, or guard a doctest run that selects zero doctests.
The all-feature
Test Analytics replay remains advisory telemetry and does not substitute for
either required gate.

The following current workflow producers are deliberately not ordinary
product-gate rows: advisory reports, uploaded artifacts, PR summaries,
coverage telemetry, release readiness, package listing, publish dry-runs, and
scheduled or release-only qualification. They remain operational or
release-specific evidence until a separate contract promotes them.

## Execution order and diagnostics

Formatting remains before CI-tool installation and hosted-cache restoration.
After setup, repository precommit, promotion honesty, agent-skill, dependency,
process, and network checks run before the later workspace check, Clippy, and
nextest stages. The first xtask invocation still builds xtask and its normal
dependencies; this is not a zero-build preflight. Every command in the inventory
runs once at the workflow level, without suppressing failures. A failing policy
step prevents the later stages from starting rather than spending those stages
before discovering the same policy error.

The required nextest step retains #3852's `rust-tests` ID, explicit `ci` profile,
fresh JUnit/context generation, original exit status, and two-file upload
unchanged. This companion does not replace that evidence implementation or
schedule another test suite.

Each named step exposes its own timing and outcome in the Actions job view and
workflow-jobs API. `Write gate summary` runs with `always()` and records the
workflow commit, subject head, run/attempt, route, and the actual outcomes for
all twelve product gates plus PR evidence. A push skips PR evidence by design;
`skipped`, `cancelled`, and missing (`not_reported`) outcomes are never promoted
to success. A terminated or unavailable runner can still prevent any summary
from being written; the summary is not independent execution proof.

`Ripr Rust Small Result` remains the required check. Runner routing,
scratch-tempfail fallback, advisory reporting, artifact selection, and job
budgets are unchanged. The summary does not decide release readiness, establish
cargo-test/nextest equivalence (#3825), or claim a measured speedup.

`xtask/tests/rust_gate_workflow_contract.rs` guards the command inventory,
required-step shape, execution order, and outcome bindings. It includes
negative controls for omitted or duplicated commands, optional or suppressed
tests, late policy checks, failure-only summaries, and fabricated success.
Run it with `cargo test -p xtask --test rust_gate_workflow_contract`.

## Selection boundary

`ProductGatePlan::for_subject` selects applicable rows when selector authority
is present. A missing selector authority or an external-tree trust class
selects the complete route with an explicit reason. It never returns an empty
green plan for an unknown subject. A selected gate claims only the proposition
listed in its definition; its non-claim is explicit and remains separate from
the command that happens to produce it.

This is observational groundwork. Existing workflows remain authoritative, and
future migration must compare their selected rows with this plan before any
required check is rerouted.
