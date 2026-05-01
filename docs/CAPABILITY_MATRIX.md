# Capability Matrix

This matrix tracks what `ripr` can currently do, which artifacts prove it, and
which roadmap item should move it next. It is intentionally capability-focused:
line count, warning count, and raw probe volume are not product success metrics.

Status values:

- `planned`: designed but not implemented
- `alpha`: implemented with limited syntax-first evidence
- `stable`: fixture-backed and expected to be reliable within documented scope
- `calibrated`: compared against real mutation outcomes

| Capability | Status | Spec | Current evidence | Next checkpoint | Metric |
| --- | --- | --- | --- | --- | --- |
| Static exposure loop | `alpha` | `RIPR-SPEC-0001` | sample diff, current CLI tests | fixture laboratory | findings include changed behavior, class, evidence, and next step |
| Predicate probes | `alpha` | `RIPR-SPEC-0001` | syntax-first probe generation | `fixture-laboratory`, `ast-probe-generation` | missing boundary discriminator detection |
| Error-path probes | `alpha` | `RIPR-SPEC-0001` | sample `error_path` explain/context command | `oracle-strength-v2` | broad error checks distinguished from exact variants |
| Return-value probes | `alpha` | `RIPR-SPEC-0001` | syntax-first scanner | `ast-probe-generation` | exact vs smoke return oracle distinction |
| Call-deletion probes | `alpha` | `RIPR-SPEC-0001` | syntax-first scanner | `ast-probe-generation` | side-effect or call boundary candidate named |
| Fixture laboratory | `planned` | `RIPR-SPEC-0002` | fixture spec only | `fixture-golden-scaffolding` | fixture pass rate |
| Golden JSON output | `planned` | `RIPR-SPEC-0002` | schema reference | `fixture-golden-scaffolding` | golden output drift count |
| Golden human output | `planned` | `RIPR-SPEC-0002` | human output renderer | `fixture-golden-scaffolding` | golden output drift count |
| Context packet v1 | `alpha` | `RIPR-SPEC-0003` | CLI context command | `agent-context-v2` | packet includes missing discriminator and related tests |
| Agent context v2 | `planned` | `RIPR-SPEC-0003` | agent context spec | `agent-context-v2` | packet includes missing values and suggested assertions |
| Analysis modes | `alpha` | `RIPR-SPEC-0001` | mode scope tests | capability metrics report | runtime and scope by mode |
| LSP diagnostics | `alpha` | `RIPR-SPEC-0001` | experimental sidecar | `lsp-evidence-hover-actions` | finding/probe metadata in diagnostics |
| Parser-backed syntax facts | `planned` | future spec | roadmap | `syntax-adapter-mvp` | syntax extraction parity |
| AST-backed test/oracle extraction | `planned` | future spec | roadmap | `ast-test-oracle-extraction` | oracle kind recognition rate |
| AST-backed probe ownership | `planned` | future spec | roadmap | `ast-probe-ownership` | duplicate symbols do not cross-link tests |
| Local delta flow | `planned` | future spec | roadmap | `local-delta-flow-v1` | flow sink identification rate |
| Activation/value modeling | `planned` | future spec | roadmap | `activation-value-modeling-v1` | detected and missing value facts |
| Repository config | `planned` | future spec | roadmap | `ripr-config-v1` | configured oracle/topology rules applied |
| Suppression | `planned` | future spec | roadmap | `suppression-v1` | suppression rate and visible suppressed findings |
| cargo-mutants calibration | `planned` | future spec | roadmap | `cargo-mutants-calibration-scaffold` | static class vs real mutation outcome report |

## Update Rules

Update this file when a PR changes capability status, adds fixtures, changes a
public output contract, or adds a measurable acceptance condition.

README should show only the headline capability snapshot. This file is the
deeper tracking surface for contributors and agents.
