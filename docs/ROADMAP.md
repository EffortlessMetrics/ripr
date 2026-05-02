# Roadmap

This roadmap is the product plan for moving `ripr` from a published alpha to a
live static exposure analyzer that developers and agents can rely on during a
pull request.

The goal state is this loop:

```text
Developer changes Rust behavior
-> ripr detects the changed behavior
-> ripr identifies the missing or weak discriminator
-> editor shows a precise diagnostic
-> hover explains the evidence path
-> code action emits agent-ready test intent
-> human or agent adds a targeted test
-> finding closes or downgrades
-> real mutation confirms later when the PR is ready
```

`ripr` stays focused on static oracle-gap analysis for diff-derived mutation
probes. It does not become a full mutation runner, a coverage dashboard, a proof
system, a second rust-analyzer, or a generic test generator.

## Current Position

The current alpha has the product shape in place:

- one published package: `ripr`
- one CLI binary: `ripr`
- one shared analysis engine
- human, JSON, and GitHub output
- an experimental LSP sidecar
- extension-managed server provisioning
- analysis modes that change indexing scope

Mode scope is intentionally cost-aware:

| Mode | Current scope |
| --- | --- |
| `instant` | Changed Rust files only. |
| `draft` | Rust files in packages touched by the diff. |
| `fast` | Package-local scope for now. |
| `deep` | Whole workspace. |
| `ready` | Whole workspace static preflight before separate mutation confirmation. |

The main bottleneck is now analyzer truth. The existing syntax-first scanner is
good enough for alpha feedback, but not enough for trustworthy diagnostics,
hover evidence, or agent-ready test briefs.

## Strategic Sequence

The load-bearing path is:

```text
quality rails
-> traceability
-> capability metrics
-> fixture/golden tooling
-> dogfooding checks
-> install verification
-> fixture lab
-> file facts
-> syntax facts
-> probe ownership
-> probe generation
-> local flow facts
-> oracle facts
-> activation/value facts
-> evidence findings
-> LSP evidence loop
-> agent context
-> repository config
-> calibration
-> cache
```

The analyzer path is:

```text
fixture lab
-> file facts
-> syntax facts
-> probe ownership
-> probe generation
-> local flow facts
-> oracle facts
-> activation/value facts
-> evidence findings
-> LSP evidence loop
-> agent context
-> repository config
-> calibration
-> cache
```

Do not skip ahead to MIR, Charon, a hard HIR dependency, SQLite-first storage,
large dashboards, broad LSP features, or more probe families before the current
probe families are grounded in better facts.

## Quality Rail Sequence

Before large analyzer work, add the repo machinery that makes future PRs easy to
write and review:

| Order | PR | Purpose |
| ---: | --- | --- |
| Q1 | `engineering-doctrine-rails` | Scope PRs by production risk, separate production and evidence deltas, add issue templates, capability matrix, traceability seed, and first policy checks. |
| Q2 | `rust-first-file-policy` | Deny unapproved non-Rust programming files, executable bits, and workflow shell sprawl through allowlisted policy checks. |
| Q3 | `spec-fixture-contracts` | Require agent-readable spec sections and BDD fixture contracts before fixture/golden work expands. |
| Q4 | `automation-guardrails` | Require allowlisted generated files, dependency surfaces, process spawning, and network behavior. |
| Q5 | `shape-fix-pr` | Add safe local PR normalization and report writing through `cargo xtask shape` and `cargo xtask fix-pr`. |
| Q6 | `pr-summary` | Generate a reviewer packet from changed paths, policy exceptions, and suggested focus areas. |
| Q7 | `check-pr-precommit` | Add obvious local gates for cheap pre-commit checks and review readiness checks. |
| Q8 | `guided-check-reports` | Make policy checks emit repair briefs with fix kind, why-it-matters text, commands, and exception templates. |
| Q9 | `ci-report-artifacts` | Upload `target/ripr/reports` from CI so failed runs still produce reviewer guidance. |
| Q10 | `fixture-golden-scaffolding` | Add fixture/golden conventions plus scaffold/check/bless commands. |
| Q11 | `traceability-spec-id-checks` | Validate behavior manifests, spec IDs, fixture links, and drift warnings. |
| Q12 | `capability-metrics-report` | Generate capability and quality metrics artifacts from fixtures and traceability. |
| Q13 | `architecture-boundary-check` | Add workspace-shape, public API, and module-boundary checks that preserve one crate with strong internal seams. |
| Q14 | `dogfood-report` | Add focused `ripr`-on-`ripr` reports as CI artifacts without blocking by default. |

These PRs should remain narrow production changes. Most of their size may be
evidence, docs, templates, allowlists, or generated scaffolding. That is
intentional: future analyzer PRs should start with a clear spec, fixture,
golden-output path, metric, and mechanical check.

These rails are also agent-context infrastructure. Long-running agent work
should not depend on a single chat transcript. Roadmap items, specs,
traceability, capability status, metrics, ADRs, and learnings are the durable
handoff surface that lets an agent resume, subset the next slice, and finish one
reviewable PR without guessing.

The operating model for those rails is documented in
[PR automation](PR_AUTOMATION.md): deterministic cleanup is shaped locally,
non-negotiable rules are checked, and judgment-required issues produce repair
briefs. Codex Goals campaign work is documented in
[Codex Goals](CODEX_GOALS.md), [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md),
and [Scoped PR contract](SCOPED_PR_CONTRACT.md).

## Operating-System Cutoff

Do not wait for perfect automation before analyzer work. The cutoff is enough
repo machinery that a future analyzer PR is pushed toward one production delta,
one evidence package, and an actionable PR summary.

Required before deeper analyzer mode:

- PR summary and reviewer packet
- `precommit` and `check-pr` command surface
- guided reports for existing policy checks
- CI report artifacts
- fixture and golden command scaffolding
- behavior manifest and spec ID checks
- capability metrics report
- architecture and workspace-shape guard

Nice later, not blocking:

- test-oracle quality report
- full docs-as-tests suite
- auto-labeling
- learning-required triggers
- public API compatibility checker
- local hook polish

## PR Queue

| Order | PR | Purpose | Release target |
| ---: | --- | --- | --- |
| 0 | `planning-and-tracking-docs` | Put the product plan, metrics, and contribution rules in-repo. | `0.2.x` |
| 1 | `engineering-doctrine-rails` | Make scoped evidence-heavy PRs mechanical with templates, traceability, capability status, and first policy checks. | `0.2.x` |
| 2 | `rust-first-file-policy` | Keep implementation and automation Rust-first by allowlisting non-Rust programming surfaces and checking workflow shell budgets. | `0.2.x` |
| 3 | `spec-fixture-contracts` | Make specs and fixtures mechanically checkable with required sections and BDD fixture contracts. | `0.2.x` |
| 4 | `automation-guardrails` | Require allowlisted generated files, dependency surfaces, process spawning, and network behavior. | `0.2.x` |
| 5 | `shape-fix-pr` | Add safe local PR normalization and report writing through `cargo xtask shape` and `cargo xtask fix-pr`. | `0.2.x` |
| 6 | `pr-summary` | Generate a reviewer packet from changed paths, policy exceptions, and suggested focus areas. | `0.2.x` |
| 7 | `automation-path-docs` | Document the fix/check/guide model and Codex Goals campaign handoff before the remaining rails. | `0.2.x` |
| 8 | `check-pr-precommit` | Add `cargo xtask precommit` and `cargo xtask check-pr` as the obvious local gates. | `0.2.x` |
| 9 | `guided-check-reports` | Make existing policy checks write actionable Markdown repair briefs. | `0.2.x` |
| 10 | `ci-report-artifacts` | Upload generated PR reports from CI. | `0.2.x` |
| 11 | `verify-one-click-extension-install` | Verify the normal editor install path without requiring `cargo install ripr`. | `0.2.x` |
| 12 | `fixture-golden-scaffolding` | Add fixture/golden structure, scaffold command, check command, and bless command. | `0.3.0` |
| 13 | `traceability-spec-id-checks` | Validate spec IDs, behavior manifests, fixture links, and drift warnings. | `0.3.0` |
| 14 | `fixture-laboratory` | Create golden fixtures and invariants before changing the analyzer. | `0.3.0` |
| 15 | `capability-metrics-report` | Generate capability, quality, engineering, and latency metrics artifacts. | `0.3.0` |
| 16 | `architecture-boundary-check` | Enforce internal module boundaries while keeping one published crate. | `0.3.x` |
| 17 | `dogfood-report` | Emit focused `ripr`-on-`ripr` reports as non-blocking artifacts. | `0.3.x` |
| 18 | `file-facts-model` | Introduce a fact model while preserving current scanner behavior. | `0.3.0` |
| 19 | `syntax-adapter-mvp` | Add a parser adapter boundary and syntax-backed file facts. | `0.3.0` |
| 20 | `ast-test-oracle-extraction` | Extract tests and assertions from syntax nodes. | `0.3.0` |
| 21 | `ast-probe-ownership` | Map diff spans to changed syntax nodes and stable owner symbols. | `0.3.0` |
| 22 | `ast-probe-generation` | Generate predicate, return, error, field, and call probes from syntax. | `0.3.0` |
| 23 | `oracle-strength-v2` | Distinguish exact, weak, smoke, snapshot, mock, and unknown oracles. | `0.4.0` |
| 24 | `local-delta-flow-v1` | Name return, field, error, and effect sinks for changed behavior. | `0.4.0` |
| 25 | `activation-value-modeling-v1` | Detect observed values and missing boundary or variant inputs. | `0.4.0` |
| 26 | `evidence-first-output` | Make CLI output the reference explanation for each finding. | `0.4.0` |
| 27 | `lsp-evidence-hover-actions` | Add finding-specific diagnostics, hover evidence, and code actions. | `0.5.0` |
| 28 | `agent-context-v2` | Emit a compact test-writing brief from CLI and LSP. | `0.5.0` |
| 29 | `ripr-config-v1` | Add topology, oracle, snapshot, mock, and external-boundary config. | `0.6.0` |
| 30 | `suppression-v1` | Add reasoned, visible suppressions with optional expiry. | `0.6.0` |
| 31 | `sarif-ci-policy` | Add SARIF and opt-in CI policy modes. | `0.6.0` |
| 32 | `cargo-mutants-calibration-scaffold` | Import real mutation results for offline calibration. | `0.7.0` |
| 33 | `persistent-cache-v1` | Cache stable facts after the fact model is worth caching. | `0.8.0` |

## Release Frames

### `0.3.0` - Evidence Foundation

Ship:

- fixture and golden scaffolding
- fixture laboratory
- capability metrics report
- stable output DTOs
- file facts
- parser adapter MVP
- AST-backed test and oracle extraction
- AST-backed probe ownership and generation

Success condition:

```text
Existing sample findings come from fact objects instead of line-substring guesses.
```

### `0.4.0` - Exposure Truth

Ship:

- oracle strength v2
- local delta flow
- activation and value modeling
- evidence-first human output

Success condition:

```text
ripr can say what changed behavior appears to flow to and which discriminator is missing.
```

### `0.5.0` - Live Editor Loop

Ship:

- finding-specific diagnostics
- evidence hovers
- copy-context action
- open-related-tests action
- deep-check command
- agent context v2

Success condition:

```text
A developer can hover a diagnostic, understand the gap, and copy a test intent.
```

### `0.6.0` - Repository Adaptation

Ship:

- `ripr.toml` v1
- custom oracle macros
- snapshot, mock, and effect config
- reasoned suppressions
- SARIF and opt-in CI policy modes

Success condition:

```text
Real repositories can teach ripr their testing idioms without hiding findings.
```

### `0.7.0` - Calibration

Ship:

- `cargo-mutants` import
- static-vs-real mutation reports
- family-specific precision measurements

Success condition:

```text
ripr can compare static exposure classes with real mutation results when explicit mutation data is present.
```

### `0.8.0` - Hot Sidecar

Ship:

- incremental in-memory store
- file-hash invalidation
- warm fact reuse
- persisted cache when needed

Success condition:

```text
Common editor edits reclassify without rescanning the full workspace.
```

## Canonical Acceptance Scenario

Use one case as the product-in-miniature:

```rust
if amount >= discount_threshold {
    apply_discount(...)
}
```

Existing tests:

```rust
#[test]
fn premium_customer_gets_discount() {
    let quote = price(10_000);
    assert!(quote.total > Money::zero());
}

#[test]
fn small_customer_gets_no_discount() {
    let quote = price(50);
    assert_eq!(quote.discount_applied, false);
}
```

Expected diagnostic:

```text
Changed boundary has no detected equality-boundary test.
```

Expected hover:

```text
Static exposure: weakly_exposed

Changed:
  amount >= discount_threshold

Evidence:
  premium_customer_gets_discount reaches price
  detected amount values: 50, 10_000
  assertion: assert!(quote.total > Money::zero())
  oracle strength: weak

Missing:
  amount == discount_threshold
  exact assertion on discount_amount
  exact assertion on total
```

Expected context packet:

```json
{
  "task": "write_targeted_test",
  "gap": "boundary_gap",
  "arrange": "build input where amount == discount_threshold",
  "act": "call price",
  "assert": [
    "discount_applied == true",
    "discount_amount == expected_discount",
    "total == expected_total"
  ]
}
```

Expected close condition:

```text
When a related test covers amount == discount_threshold and checks the changed
outputs with exact assertions, the finding disappears or downgrades.
```

## Validation Gate Before Deeper Semantics

Before investing in HIR, MIR, Charon, persistent storage, or wider probe
families, the product should satisfy this suite:

| Area | Required evidence |
| --- | --- |
| Distribution | Marketplace and Open VSX install paths work without requiring `cargo install ripr`. |
| Distribution | Verified server download starts `ripr lsp --stdio`. |
| Analyzer | Duplicate function names do not cross-link tests. |
| Analyzer | Stacked test attributes are detected. |
| Analyzer | Multi-line assertions are extracted. |
| Analyzer | Boundary probes report missing equality values. |
| Analyzer | Error-path probes distinguish broad error checks from exact variant checks. |
| Analyzer | Return-value probes distinguish exact assertions from smoke assertions. |
| Analyzer | Local flow names at least one sink or reports a stop reason. |
| Output | Static output never uses mutation-runtime outcome language. |
| Output | Unknowns carry stop reasons. |
| LSP | Diagnostics include finding and probe metadata. |
| LSP | Hover shows evidence path and missing discriminator. |
| LSP | Code action copies the exact context packet. |
| Agent | Context packet includes related tests, missing values, and suggested assertion shape. |
| Calibration | Imported mutation results are shown only as explicit calibration data. |

## Documentation Tracking

Every significant PR should update the matching docs:

- product behavior: [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- JSON or context shape: [Output schema](OUTPUT_SCHEMA.md)
- architecture or module seams: [Architecture](ARCHITECTURE.md)
- test strategy or gates: [Testing](TESTING.md)
- roadmap status or sequencing: this file
- decisions that should not be re-litigated: [ADR directory](adr/)
- contributor learnings: [Learnings](LEARNINGS.md)
