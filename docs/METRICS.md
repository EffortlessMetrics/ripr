# Metrics

`ripr` metrics exist to track whether static findings are becoming more useful,
more truthful, and less noisy. They are not vanity counters.

## North Star

The primary product metric is:

```text
time from changed behavior to useful targeted test intent
```

A useful targeted test intent names:

- the changed behavior
- the missing or weak discriminator
- related existing tests, if any
- missing activation values, if relevant
- suggested oracle shape
- stop reasons for unknowns

## Capability Metrics

| Metric | Why it matters | Target direction |
| --- | --- | --- |
| Fixture pass rate | Guards behavior contracts. | Up |
| Golden output drift count | Shows intentional vs accidental output changes. | Down |
| Probe ownership precision | Prevents wrong related-test evidence. | Up |
| Oracle kind recognition rate | Shows how often assertions become usable facts. | Up |
| Strong-vs-weak oracle distinction rate | Tracks discriminator quality. | Up |
| Flow sink identification rate | Shows propagation evidence quality. | Up |
| Activation value extraction rate | Tracks boundary and input modeling progress. | Up |
| Unknowns with stop reasons | Keeps static limits explicit. | Up to 100% |
| Static runtime by mode | Protects editor and CI latency. | Down or bounded |
| LSP diagnostic refresh latency | Protects live feedback. | Down |

## Quality Metrics

| Metric | Why it matters | Target direction |
| --- | --- | --- |
| Findings acted on | Measures developer trust. | Up |
| Suppression rate by rule | Identifies noisy rules. | Down after config improves |
| Reopened or reverted findings | Catches misleading guidance. | Down |
| False-positive rate of top findings | Protects trust. | Down |
| Survived-mutant recall after calibration | Tests whether static gaps predict real mutation outcomes. | Up |
| Top-N precision after calibration | Keeps reports useful. | Up |

## Engineering Metrics

| Metric | Why it matters | Target direction |
| --- | --- | --- |
| `panic` / `unwrap` / `expect` count in production code | Enforces fallible, user-facing behavior. | Down to 0 |
| `panic` / `unwrap` / `expect` count in tests | Keeps tests explicit and maintainable. | Down to 0 |
| CI duration | Protects contribution loop. | Bounded |
| Flaky test count | Protects trust in gates. | 0 |
| Public dependency count | Keeps install and maintenance risk low. | Justified |

## Current Baseline

The following baseline was observed during the planning-doc pass:

```text
Production/test panic-family inventory:
  1 production expect() call site in crates/ripr/src/lsp.rs.
  13 test unwrap() call sites across CLI, analysis, and LSP tests.
  4 additional string-pattern matches intentionally detect unwrap/expect usage
    in analyzed Rust code and are not panic-family call sites.

CI:
  Rust job runs fmt, check, clippy, test, package list, and publish dry-run.
  VS Code job runs npm ci, compile, and package.

Fixture lab:
  Not yet present.

Root changelog:
  Added by PR 0.

ADR/spec scaffolding:
  Added by PR 0.
```

Future PRs should replace this prose baseline with generated or scripted counts
once the fixture lab and metrics commands exist.

## Anti-Metrics

These are useful context but bad primary goals:

- number of warnings
- number of generated probes
- coverage percentage
- whole-workspace graph size
- number of lines of code

The product should optimize evidence quality, not raw output volume.
