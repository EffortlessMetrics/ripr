# Implementation Campaigns

This is the campaign-level plan for Codex Goals and long-context contributor
work. Campaigns are larger than one PR. Each campaign has an objective, an end
state, and work items that should each follow the
[scoped PR contract](SCOPED_PR_CONTRACT.md).

The operational checklist remains in [Implementation plan](IMPLEMENTATION_PLAN.md).
The machine-readable active campaign is `.ripr/goals/active.toml`.

## Campaign 1: Agentic DevEx Foundation

Campaign ID: `agentic-devex-foundation`

Objective:

```text
Make the repo safe for autonomous Codex Goals work and human review.
```

Why it matters:

`ripr` is being built for long-context, agent-assisted implementation. The repo
must reject ambiguous PRs before review and produce enough receipts for humans
to evaluate trusted change instead of chat transcripts.

End state:

- architecture guard exists
- output-contract checks exist
- first behavior fixtures exist
- docs-as-tests baseline exists
- test-oracle report exists
- dogfood report exists
- Codex Goals campaign docs exist

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `policy/architecture-guard` | done | Workspace, architecture, and public API guardrails exist. |
| `output/output-contract-check` | done | Output contract registry checks exist. |
| `docs/docs-index-checks` | done | Docs index checks exist. |
| `docs/codex-goals-campaigns` | done | Clarify Codex Goals as multi-PR campaigns. |
| `docs/readme-state-and-link-checks` | done | README state and repo-local Markdown links are checked. |
| `goals/manifest-check` | done | Active campaign manifest is validated and reportable. |
| `fixtures/runner-comparison-v1` | done | Fixture and golden commands run `ripr` and compare actual outputs. |
| `fixtures/first-two-goldens` | done | `boundary_gap` and `weak_error_oracle` fixtures exist with JSON and human goldens. |
| `testing/test-oracle-report` | ready | Report strength of `ripr`'s own test oracles. |
| `dogfood/static-self-check` | ready | Produce focused `ripr`-on-`ripr` reports. |

Dependencies:

- Do not start analyzer rewrites until fixture and golden scaffolding can record
  behavior.
- Do not treat test-oracle reports as blocking until baseline debt is measured.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
cargo xtask fixtures
cargo xtask goldens check
cargo xtask metrics
```

Blocking conditions:

- policy exception required
- architecture exception required
- output schema change required
- golden blessing needed without explicit review scope
- campaign item depends on an unmerged non-stackable PR

Review policy:

Work items should usually produce one scoped PR. Independent docs or reporting
items may be stackable when the campaign manifest marks them that way.

## Campaign 2: Syntax-Backed Analyzer Foundation

Objective:

```text
Move the analyzer from lexical facts to syntax-backed facts.
```

Why it matters:

Current analyzer behavior still has line-oriented surfaces. `ripr` needs a
stable fact model and parser adapter boundary before replacing lexical checks.

End state:

- `FileFacts` model exists
- syntax adapter boundary exists
- tests and oracles are extracted from syntax-backed facts
- probes attach to stable owner symbols
- current probe families are generated from syntax facts

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `analysis/file-facts-model` | ready | Introduce fact DTOs while preserving scanner behavior. |
| `analysis/syntax-adapter-mvp` | blocked | Depends on reviewed fact model. |
| `analysis/ast-test-oracle-extraction` | blocked | Depends on syntax adapter boundary. |
| `analysis/ast-probe-ownership` | blocked | Depends on syntax node and owner facts. |
| `analysis/ast-probe-generation` | blocked | Depends on ownership and syntax-backed facts. |

Dependencies:

- `analysis/file-facts-model` should merge before syntax adapter work.
- Analyzer work items are non-stackable unless the manifest explicitly says
  otherwise.

Commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask pr-summary
```

Blocking conditions:

- output drift without golden evidence
- parser dependency requiring scope expansion
- architecture exception required
- missing stop reason for new unknowns

Review policy:

Each analyzer work item should include spec, fixture or test, output contract
evidence when user-visible output changes, metrics movement when capability
status changes, and a clear non-goal list.

## Campaign 3: Evidence Quality

Objective:

```text
Make findings explain changed behavior, oracle strength, propagation, activation,
and unknown stop reasons with enough precision to guide test work.
```

End state:

- oracle kind and strength are probe-relative
- local delta flow can name visible sinks
- activation modeling can name observed and missing discriminator values
- output is evidence-first
- unknown findings include stop reasons across surfaces

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `analysis/oracle-strength-v2` | blocked | Depends on syntax-backed oracle extraction. |
| `analysis/local-delta-flow-v1` | blocked | Depends on owner and syntax facts. |
| `analysis/activation-value-modeling-v1` | blocked | Depends on value facts and fixture coverage. |
| `output/evidence-first-output` | blocked | Depends on stable evidence fields. |
| `output/unknown-stop-reason-invariant` | ready | Can start once output-contract expectations exist. |

## Campaign 4: Editor and Agent Loop

Objective:

```text
Turn findings into editor and agent actions that help produce targeted tests.
```

End state:

- LSP diagnostics carry finding and probe IDs
- hovers show evidence for the selected finding
- code actions can copy context packets or open related tests
- context packets include missing values and assertion shapes

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `lsp/evidence-hover-actions` | blocked | Depends on evidence-first output shape. |
| `context/agent-context-v2` | blocked | Depends on stable evidence and oracle fields. |
| `docs/how-to-use-agent-context` | blocked | Depends on context v2. |

## Campaign 5: Adoption and Calibration

Objective:

```text
Make `ripr` practical in repositories, CI, and offline calibration loops.
```

End state:

- repository config exists
- SARIF and CI policy modes exist
- cargo-mutants calibration scaffold exists
- persistent cache exists after fact model earns caching

Work items:

| Work item | Status | Notes |
| --- | --- | --- |
| `config/ripr-config-v1` | blocked | Depends on stable analyzer conventions. |
| `ci/sarif-ci-policy` | blocked | Depends on output contract stability. |
| `calibration/cargo-mutants-scaffold` | blocked | Depends on improved static facts. |
| `cache/persistent-cache-v1` | blocked | Depends on stable fact model. |
