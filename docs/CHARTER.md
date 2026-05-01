# Charter

`ripr` is the static mutation-exposure layer for Rust.

It watches changed code, creates mutation-shaped probes, and checks whether the
current tests appear to contain the discriminators needed to observe those
changes.

It helps humans and agents add targeted tests while the pull request is still in
draft, then uses real mutation testing at ready time to confirm and calibrate.

Its value is not proving correctness. Its value is shortening the path from
"this behavior changed" to "this is the exact test oracle that should notice."

## Mission

`ripr` helps Rust developers and coding agents write tests that actually notice
changed behavior.

It performs static RIPR exposure analysis over changed Rust code, creates
mutation-shaped probes, and reports whether existing tests appear to contain the
discriminators needed to expose those changes.

It is a fast draft-mode companion to real mutation testing: static guidance
while the pull request is moving, real mutation confirmation when the change is
ready.

## Vision

The vision for `ripr` is to become the live test-adequacy layer between coverage
and mutation testing for Rust/Cargo workspaces.

Coverage tells developers what executed. Mutation testing tells developers what
survived. `ripr` tells developers, reviewers, and agents what changed behavior
appears to lack a meaningful oracle before the expensive mutation run begins.

The end state is an LSP-native sidecar that watches changed code, identifies
missing discriminators, explains the evidence path, generates agent-ready test
intent, and calibrates itself against real mutation outcomes.

## Category

The category is:

```text
Static Mutation Exposure Analysis
```

The technical definition is:

```text
Static oracle-gap analysis for diff-derived mutation probes.
```

The agent-facing definition is:

```text
A test-intent generator grounded in static mutation exposure.
```

## Ecosystem Role

| Existing layer | What it answers | Gap |
| --- | --- | --- |
| `cargo-llvm-cov` | Did code execute? | Not oracle-aware |
| selective retest tools | Which tests are impacted? | Not assertion-aware |
| `cargo-mutants` | Did real mutants survive? | Too expensive for live draft feedback |
| `rust-analyzer` | What does Rust code mean in the editor? | Does not rank mutation exposure |
| `ripr` | Does changed behavior appear exposed to a meaningful oracle? | New middle layer |

## Non-Goals

`ripr` should not become:

- a full mutation engine
- a coverage dashboard
- a proof system
- a second rust-analyzer
- a generic LLM test generator
- a whole-workspace MIR theorem project

It should avoid claims like:

- this is untested
- this mutant would survive
- no test checks this
- `ripr` proves adequacy
- `ripr` replaces mutation testing

It should use defensible language:

- no detected boundary discriminator
- no detected exact error oracle
- only weak oracle evidence found
- no static oracle path found
- static propagation stopped at an opaque boundary
- escalate to real mutation

## Operating Principles

Findings beat scores. A useful diagnostic says which changed behavior appears
weakly checked and what discriminator would close the gap.

Unknown is a valid result. For proc macros, opaque fixtures, dynamic dispatch,
async causality, generated code, feature-gated behavior, or external state,
`ripr` should say where static propagation stopped.

Fast modes stay narrow. Draft feedback should focus on changed behavior and
nearby package-local evidence. Deeper semantic analysis belongs in slower modes.

The graph is the product. The useful question is whether a changed probe can
reach a discriminating oracle, not whether syntax can be walked.

One product package comes first. Split crates only when a stable external
contract emerges.

## Success Metrics

Good metrics:

- time from changed behavior to useful test intent
- percentage of findings accepted or acted on
- mutation survivors predicted by weak/no exposure findings
- reduction in smoke-test-only agent outputs
- number of real mutation candidates avoided or prioritized
- suppression rate by rule
- false-positive rate of top findings

Bad primary metrics:

- number of warnings
- coverage percentage
- number of probes generated
- whole-workspace graph size

