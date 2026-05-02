# Goal-Mode Execution

Goal mode is the execution style for long-running `ripr` implementation work.
It is intentionally stricter than ordinary issue picking: one goal becomes one
scoped PR with a defined stop condition, evidence package, and command set.

Use goal mode after the repo operating rails are strong enough that a coding
agent can run:

```bash
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
```

and receive either a reviewable branch or specific repair guidance.

## Contract

Each goal-mode task must define:

- goal
- scope
- production delta
- evidence/support delta
- single acceptance criterion
- required commands
- non-goals

The task should not say "make the analyzer better." It should name one
capability or one architecture seam.

## Task Template

```text
Goal:
<one capability or seam>

Scope:
One scoped production behavior, public contract, or architecture seam.
Large fixture, docs, and golden support is fine when it supports this one goal.

Production delta:
<exact module, command, or contract being changed>

Evidence/support delta:
<specs, fixtures, tests, goldens, metrics, docs, ADRs, or learnings expected>

Acceptance:
<single reviewable claim>

Required commands:
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
git diff --check

If analyzer output changes:
cargo xtask fixtures
cargo xtask goldens check

If extension files change:
cd editors/vscode
npm ci
npm run compile
npm run package

Non-goals:
<explicit exclusions>

Do not:
- add panic-family shortcuts
- use mutation-runtime outcome language in static output
- add non-Rust implementation files outside allowlisted surfaces
- add shell scripts instead of xtask commands
- add new crates without an approved workspace-shape decision
```

## Stop Conditions

A goal-mode PR is done when:

- the acceptance criterion passes
- the production delta remains scoped
- expected evidence artifacts are present
- required commands pass or the remaining blocker is documented
- the PR summary explains reviewer focus
- non-goals remain out of scope

If the goal cannot be completed cleanly, stop by recording the blocker in the
relevant doc, spec, or learning note. Do not silently broaden the PR.

## Readiness Gate

Before switching from repo-automation work to analyzer implementation work, the
repo should have the following commands available or intentionally scheduled:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
cargo xtask fixtures
cargo xtask goldens check
cargo xtask metrics
```

Reports should land under:

```text
target/ripr/reports/
```

The practical cutoff is not perfect automation. The cutoff is enough automation
that future analyzer PRs are forced toward scoped production deltas, explicit
evidence, and useful repair guidance.

## Goal Queue

The implementation queue after the operating rails is:

| Order | Goal | Purpose |
| ---: | --- | --- |
| 1 | `analysis/file-facts-model` | Introduce a fact model while preserving current scanner behavior. |
| 2 | `analysis/syntax-adapter-mvp` | Add a parser adapter boundary before parser-specific logic spreads. |
| 3 | `analysis/ast-test-oracle-extraction` | Extract tests and assertions from syntax-backed facts. |
| 4 | `analysis/ast-probe-ownership` | Map diff spans to stable owner symbols. |
| 5 | `analysis/ast-probe-generation` | Generate current probe families from syntax facts. |
| 6 | `analysis/oracle-strength-v2` | Make oracle kind and strength explicit and probe-relative. |
| 7 | `analysis/local-delta-flow-v1` | Name locally visible sinks for changed behavior. |
| 8 | `analysis/activation-value-modeling-v1` | Detect observed inputs and missing boundary or variant values. |
| 9 | `output/evidence-first-output` | Make CLI output the reference explanation for each finding. |
| 10 | `lsp/evidence-hover-actions` | Add finding-specific diagnostics, hover evidence, and actions. |
| 11 | `context/agent-context-v2` | Emit a compact test-writing brief from CLI and LSP. |
| 12 | `config/ripr-config-v1` | Let repositories describe topology and oracle conventions. |
| 13 | `ci/sarif-ci-policy` | Add SARIF and opt-in CI policy modes. |
| 14 | `calibration/cargo-mutants-scaffold` | Import real mutation results for offline calibration. |
| 15 | `cache/persistent-cache-v1` | Cache stable facts after the fact model earns caching. |

## First Implementation Goal

Start with `analysis/file-facts-model`.

Goal:

```text
Introduce internal fact DTOs while preserving current scanner behavior.
```

Production delta:

```text
Add FileFacts, FunctionFact, TestFact, OracleFact, CallFact, ReturnFact,
LiteralFact, and related IDs or ranges where needed.
```

Acceptance:

```text
Existing sample findings and current fixture expectations are unchanged.
Analysis consumes file facts instead of ad hoc scanner structures where the
change can be made safely.
No parser dependency is added yet.
```

Non-goals:

```text
No parser integration.
No HIR or MIR.
No oracle-strength redesign.
No output schema change unless unavoidable.
```

This goal creates the seam for syntax extraction without mixing parser choice,
semantic flow, or output redesign into the same PR.

## Second Implementation Goal

Follow with `analysis/syntax-adapter-mvp`.

Goal:

```text
Add the syntax adapter boundary.
```

Production delta:

```rust
trait RustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, RiprError>;
}
```

Acceptance:

```text
Current lexical behavior remains available.
The adapter boundary is tested.
No analyzer output changes unless explicitly documented with evidence.
```

Non-goals:

```text
Do not replace test extraction yet.
Do not replace probe generation yet.
```

After these two goals, the analyzer can move to AST-backed oracle extraction,
probe ownership, and probe generation with smaller production deltas.
