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
- repo seam inventory, test-grip evidence, agent seam packets, LSP seam code
  actions, cached seam fact layers, and advisory cargo-mutants calibration

Mode scope is intentionally cost-aware:

| Mode | Current scope |
| --- | --- |
| `instant` | Changed Rust files only. |
| `draft` | Rust files in packages touched by the diff. |
| `fast` | Package-local scope for now. |
| `deep` | Whole workspace. |
| `ready` | Whole workspace static preflight before separate mutation confirmation. |

The main bottleneck is now proving the hot sidecar path stays responsive without
serving stale evidence. Campaign 5A closed the cache, precision, actionability,
and calibration loop for repo seam evidence. Campaign 5B landed repository
configuration, SARIF/CI policy, and seam-native badge count mapping. Campaign 6
then completed the internal module SRP refactor chain through #405 without
changing output schemas, public API, SARIF, badge, or saved-workspace LSP
behavior. Campaign 7 then pinned the defaults/config baseline, added the
operator cockpit report, made the generated GitHub Action upload pilot/report
artifacts with optional SARIF rendering/upload, documented and verified the
existing VS Code install path and command coverage, and added the public example
corpus for the defaults-first operator path. Install and release-path proof is
now complete for the crate install path, public GitHub Release server assets,
and VSIX package path.
Campaign 7 is closed; the closeout audit demonstrates the installed
boundary-gap seam packet, outcome receipt, and optional calibration loop.
Campaign 8 is closed; the runtime calibration fixture expansion now has a
checked supplied-data sample for the main static/runtime agreement buckets, and
runtime mutation vocabulary remains confined to explicit calibration reports.
Campaign 9 is closed; bounded repo-exposure latency reporting now records cache
collection, cache load hit/miss/corrupt state, file-fact cache reuse, cold
compute, cache store, and total phase timing without changing repo-exposure
outputs. File-fact warm reuse improved the measured index-build subphase, and
the repo evidence hot path now uses indexed related-test candidates, lazy
value-resolution facts, and owned classification handoff. Current proof shows a
bounded cold run can fill the classified-seam cache, after which the default
30-second repo-exposure latency report passes on JSON and Markdown cache hits.
The budget-aware pilot path has first-screen Markdown and terminal copy that
states the inspected seam, why it matters, the focused test to write, and the
before/after commands. Campaign 10 closed after aligning the saved-workspace
diagnostic, evidence, packet/brief, focused-test, after-snapshot, verify,
receipt, cockpit, CI, and install loop. Its release-readiness gate proves the
installed CLI, boundary-gap `pilot`, `outcome`, `agent verify`,
`agent receipt`, latency, LSP cockpit, advisory workflow, VSIX path, and
known-limit surfaces. Campaign 11 closed the LLM work loop: `ripr agent status`
is a read-only lens over existing artifacts, loop command templates are
centralized for the current CLI/LSP/cockpit/CI surfaces, workflow manifests,
provenance-backed receipts, next-action guidance, review summaries, fixtures,
and generated CI packet uploads are pinned, and the LLM operator guide documents
the source-edit-free human and external-agent path. Campaign 12 closed the
First-Hour UX lane: after the LLM work-loop command and artifact state
stabilized, the current product risk was making the VS Code extension and
generated GitHub workflow explain the top recommendation without requiring
users to inspect CLI reports first. The PR guidance annotation contract is
pinned, the editor now has
a first-run status path for server/workspace/analysis state, and seam diagnostic
actions are titled around inspect, targeted-test, agent-handoff, verify, review,
and refresh intent, and the generated GitHub workflow now writes a
reviewer-oriented advisory summary before artifact download. The generated
workflow smoke fixture pins the CI first screen, artifact packet, optional
SARIF gates, badge output, and PR guidance annotation hook. Campaign 12
closed after the first-hour docs routed users by VS Code, CI, CLI, and
agent/reviewer path. Campaign 13 closed PR Review Guidance:
`ripr review-comments` now writes the already-specified
`target/ripr/review/comments.json` report, generated CI runs that producer
before the existing non-blocking summary and changed-line check-annotation
consumers, guidance placement and suppression cases are fixture-pinned, and
[PR review guidance](PR_REVIEW_GUIDANCE.md) documents the command, CI behavior,
summary-only fallback, inline-comment opt-in boundary, and static-evidence
limits. Campaign 14 is closed as Recommendation Calibration: RIPR-SPEC-0013
pins the recommendation calibration report contract for measuring whether
completed recommendation surfaces are actionable, correctly placed, properly
suppressed or capped, and correlated with before/after static movement without
telemetry, generated tests, runtime mutation execution, or default CI blocking.
The PR-shaped calibration corpus, local outcome receipts, and advisory
`cargo xtask recommendation-calibration` report are now checked, and
[Recommendation calibration](RECOMMENDATION_CALIBRATION.md) documents how to
read metrics, receipts, placement quality, suppression correctness, static
movement buckets, and advisory limits. The closeout handoff records the PR
chain and deferred policy boundary. Campaign 15 is closed as Calibrated Gate
Policy: RIPR-SPEC-0014 pins optional policy gates after measured signal
quality, `ripr gate evaluate` writes the read-only decision report,
`fixtures/calibrated-gate-cases` pins the decision matrix, and generated
GitHub workflows run gate evaluation only when `RIPR_GATE_MODE` is explicitly
configured. [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) documents the
operating model. Gates remain explicit, advisory by default, and separate from
runtime mutation vocabulary. The [Campaign 15
closeout](handoffs/2026-05-08-campaign-15-closeout.md) records the final proof
and defers the next product campaign to an explicit decision. Campaign 16 is
now active as Gate Adoption UX: the goal is not more policy machinery, but
making explicit gate adoption safe through copyable generated-CI examples,
visible waiver workflows, baseline creation and refresh guidance, first-screen
gate summaries, repo-local dogfood receipts, and guidance for when blocking is
earned by local evidence. The generated-CI examples for default advisory
posture, `visible-only`, `acknowledgeable`, `baseline-check`, and
`calibrated-gate` are now documented, and the `ripr-waive` reviewer workflow
now keeps acknowledgements visible and separate from suppressions. Baseline
creation and refresh guidance now frames baselines as visible historical debt
ledgers that teams can shrink toward RIPR 0 under configured scope, and the
generated CI summary now shows gate mode, status, labels, waiver, baseline,
calibration, blocking reason, and artifact paths at a glance. Checked
repo-local dogfood gate receipts now show visible-only, acknowledged,
baseline-existing, baseline-new, missing-baseline, and explicit
calibrated-gate decisions from checked evidence while preserving non-blocking
generated CI defaults. [Gate blocking readiness](GATE_BLOCKING_READINESS.md)
now documents when to stay advisory, require acknowledgement, use
baseline-check, or enable calibrated blocking. Campaign 16 closeout is the next
ready adoption slice. Editor Evidence UX is queued as a separate Lane 3 editor
campaign, not a replacement for the active gate-adoption manifest; it should
begin after Gate Adoption UX closes or after an explicit parallel-lane decision.

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
-> repo seam inventory and test grip
-> seam fact cache
-> related-test, value, and oracle-shape precision
-> seam-native LSP actions
-> runtime calibration import
-> repository config
-> SARIF and CI policy
-> seam-native badge counts
-> operationalization closeout
-> Campaign 6 stack audit
-> Campaign 6 modularization closeout
-> defaults-first operator adoption
-> runtime calibration fixture expansion
-> Campaign 8 closeout
-> hot-sidecar latency proof
-> editor-agent integration
-> editor-agent release readiness proof
-> LLM work loop
-> first-hour UX
-> PR review guidance
-> recommendation calibration
-> calibrated gate policy
-> gate adoption UX
-> editor evidence UX
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
| 27 | `test-efficiency-test-fact-ledger` | Emit advisory per-test ledgers from existing owner, oracle, value, and evidence facts. | `0.4.x` |
| 28 | `test-efficiency-vacuous-signal-v1` | Classify likely vacuous, smoke-only, broad-oracle, opaque, and circular test signals with evidence. | `0.4.x` |
| 29 | `test-efficiency-duplicate-discriminator-v1` | Group tests with duplicate owner, activation, oracle, and sink evidence. | `0.4.x` |
| 30 | `lsp-evidence-hover-actions` | Add finding-specific diagnostics, hover evidence, and code actions. | `0.5.0` |
| 31 | `agent-context-v2` | Emit a compact test-writing brief from CLI and LSP. | `0.5.0` |
| 32 | `ripr-config-v1` | Add topology, oracle, snapshot, mock, severity, suppression, and seam-diagnostic config. | `0.6.0` |
| 33 | `suppression-v1` | Add reasoned, visible suppressions with optional expiry. | `0.6.0` |
| 34 | `sarif-ci-policy` | Add SARIF and opt-in CI policy modes. | `0.6.0` |
| 35 | `seam-native-count-mapping` | Remap `ripr` and `ripr+` badge artifacts onto seam-native unresolved gap counts. | `0.6.x` |
| done | `repo-seam-facts-cache` | Cache seam fact layers after the fact model became stable enough. | Campaign 5A |
| done | `cargo-mutants-calibration-scaffold` | Import real mutation results for offline calibration. | Campaign 5A |

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

### `0.4.0` - Editor Agent Integration

Ship:

- LSP copy commands for agent packet, brief, after-snapshot, verify, and receipt
- operator cockpit status for before/after snapshots, agent verify JSON, agent
  receipt JSON, movement counts, and missing-input next commands
- one canonical editor-agent loop fixture that pins diagnostics, actions, agent
  brief, agent packet, verify, receipt, and cockpit output
- generated CI artifacts for the non-blocking editor-agent loop
- first-hour docs centered on `ripr pilot`, one focused test, after snapshot,
  `ripr outcome`, `ripr agent verify`, `ripr agent receipt`, editor, CI, and
  known limits
- release-readiness proof for installed CLI, packaged VSIX, package dry-run, and
  known-limits surfaces after the loop is pinned

Success condition:

```text
saved-workspace diagnostic -> packet/brief -> focused test -> after snapshot -> agent verify -> agent receipt -> cockpit/CI artifact
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

Campaign 5A shipped the advisory scaffold:

- `cargo-mutants` import
- static-vs-real mutation reports

Future calibration work can add:

- family-specific precision measurements
- explicit calibration fixtures or bounded runtime artifacts

Success condition:

```text
ripr can compare static exposure classes with real mutation results when explicit mutation data is present.
```

### `0.8.0` - Hot Sidecar

Campaign 5A shipped the first seam fact cache. Future hot-sidecar work can add:

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
