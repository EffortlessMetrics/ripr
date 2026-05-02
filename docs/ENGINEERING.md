# Engineering Rules

This document records the engineering bar for `ripr` implementation PRs.

## Product-First Scope

Every implementation PR should be traceable to the product contract:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Avoid broad infrastructure unless it makes that answer more precise, faster, or
more actionable.

## Scoped Evidence-Heavy PRs

Optimize PRs for narrow production risk, not low line count.

A good `ripr` PR changes one production behavior, public contract, or
architectural seam and carries the full evidence package needed to review that
change. Evidence can include specs, fixtures, tests, golden outputs, docs,
metrics, ADRs, learnings, and traceability.

This means a fixture-heavy or docs-heavy PR can be scoped even when it has many
changed lines. It also means a small code diff can be unscoped if it changes
multiple contracts or unrelated behaviors.

Every behavior PR should identify:

- production delta
- evidence delta
- one acceptance criterion
- explicit non-goals

## Architecture

Keep one published package:

```text
Package: ripr
Binary: ripr
Library: ripr
Automation: xtask, unpublished
```

Use internal seams:

- `domain`: exposure concepts, probes, RIPR evidence, oracle strength, classes
- `app`: use-case orchestration and public library API
- `analysis`: diff, indexing, facts, probes, classification
- `output`: human, JSON, GitHub, future SARIF
- `cli`: command adapter
- `lsp`: editor protocol adapter

Do not split crates until an external contract makes the boundary real.

## SRP and Modularity

Prefer small modules whose names describe the problem concept they own.

Good ownership examples:

- diff parsing owns hunks and changed ranges
- syntax adapter owns parser integration
- fact extraction owns syntax-to-fact conversion
- probe generation owns changed-fact-to-probe conversion
- classifier owns fact/probe-to-finding decisions
- output adapters own rendering only

Avoid modules that mix parsing, analysis, classification, and rendering.

## Error Handling

Target rule:

```text
No panic, unwrap, expect, todo, or unimplemented in production or tests.
```

This target allows ordinary test assertions such as `assert!` and `assert_eq!`.
It forbids accidental panic paths, unwrap-style setup shortcuts, and
panic-driven control flow.

Use typed errors or `Result<_, String>` where the existing codebase has not yet
introduced a richer error type. Tests should return `Result` and use explicit
assertions instead of unwrap-style failure.

Exceptions require a narrow comment explaining why failure is impossible and why
the exception is better than propagating an error. The preferred long-term count
is zero exceptions.

Fixture projects may intentionally contain `unwrap`, `expect`, `panic!`, or
`#[should_panic]` only when the fixture spec says that behavior is being
analyzed as a smoke, panic, or weak-oracle shape.

## Modern Rust

Use Rust 2024 and the workspace minimum Rust version. Prefer standard library
types and clear ownership before adding dependencies. Keep `unsafe_code =
"forbid"`.

When adding types, encode domain states directly instead of passing loosely
typed strings through the analyzer. Keep fallible boundaries explicit with
`Result`, and keep rendering concerns out of domain and analysis types.

## Rust-First Implementation

Rust is the default implementation language for repo automation, production
logic, test harnesses, fixture runners, release checks, and policy checks.

Non-Rust programming files are allowlisted exceptions. They are allowed only for
approved surfaces such as the VS Code extension, GitHub Actions declarations,
fixtures, generated outputs, documentation examples, or assets.

If a contributor adds a non-Rust programming file, the PR must explain:

- why Rust or `xtask` is not the right place
- which approved surface owns the file
- whether the file is production, test, fixture, generated, config, or docs
- what CI check covers it

See [File policy](FILE_POLICY.md).

## PR Shaping Automation

Repo automation has three responsibilities:

- shape: safely normalize files where no judgment is required
- check: enforce non-negotiable rules
- guide: report exact repair steps when judgment is required

The operating model is documented in [PR automation](PR_AUTOMATION.md). The
short version is:

```text
deterministic cleanup -> automated shaping
non-negotiable rules -> check failures
judgment-required exceptions -> repair briefs
```

`cargo xtask shape` is allowed to mutate only safe local artifacts:

- run `cargo fmt`
- sort `.ripr/*.txt` and `policy/*.txt` allowlist entries
- ensure `target/ripr/reports`
- write `target/ripr/reports/shape.md`

It must not bless goldens, add policy exceptions, change schema versions, add
dependencies, or accept public output drift.

`cargo xtask pr-summary` writes `target/ripr/reports/pr-summary.md` by
classifying changed paths from git diff and status.

`cargo xtask fix-pr` currently runs safe shaping, refreshes `pr-summary`, and
writes a local fix-pr report.

Long-running implementation work should use the scoped task model in
[Goal-mode execution](GOAL_MODE.md).

## Testing Style

Use BDD-shaped test names and fixtures:

```text
given_changed_boundary_when_only_smoke_oracle_exists_then_reports_weak_exposure
```

Each behavior should have:

- a spec entry
- one or more tests that cite the spec ID or fixture name
- implementation code in the matching module
- golden output when user-visible output changes
- a capability or quality metric when capability status changes

## Documentation

Use Diataxis deliberately:

- tutorials: teach first successful use
- how-to guides: solve concrete tasks
- reference: define commands, schemas, config, and enums
- explanation: record model, architecture, ADRs, and tradeoffs

The README should stay problem-first and should surface the most important
metrics, current capability state, and next-step docs.

## Dogfooding

When `ripr` can analyze a behavior shape that exists in its own codebase, add a
fixture or smoke command that uses this repository as an example. Dogfooding
should produce focused evidence and tests, not broad self-analysis dashboards.

## Output Language

Static output may use:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Static output must not claim:

- `killed`
- `survived`
- `untested`
- `proven`
- `adequate`

Real mutation data can be reported only in explicit calibration output.
