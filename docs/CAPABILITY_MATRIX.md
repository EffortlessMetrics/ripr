# Capability Matrix

This matrix tracks what `ripr` can currently do, which artifacts prove it, and
which roadmap item should move it next. It is intentionally capability-focused:
line count, warning count, and raw probe volume are not product success metrics.

The machine-readable source is `metrics/capabilities.toml`. Run
`cargo xtask metrics` for generated Markdown and JSON reports under
`target/ripr/reports/`.

Status values:

- `planned`: designed but not implemented
- `alpha`: implemented with limited syntax-first evidence
- `stable`: fixture-backed and expected to be reliable within documented scope
- `calibrated`: compared against real mutation outcomes

| Capability | Status | Spec | Current evidence | Next checkpoint | Metric |
| --- | --- | --- | --- | --- | --- |
| Static exposure loop | `alpha` | `RIPR-SPEC-0001` | sample diff, current CLI tests, boundary_gap fixture, weak_error_oracle fixture, FileFacts DTOs, lexical syntax adapter boundary, ra_ap_syntax parser-backed test/oracle extraction, module-qualified and impl-qualified owner symbols, parser-backed probe shape facts, unknown stop reason invariant, probe-relative oracle kind and strength, typed local flow sink facts, observed activation value facts, missing discriminator facts | `evidence-first-output` | findings include changed behavior, class, evidence, and next step |
| Unknown stop reasons | `alpha` | `RIPR-SPEC-0001` | Campaign 3 manifest, domain unknown stop reason invariant, JSON stop_reasons output, human stop reasons output, boundary_gap fixture | `evidence-first-output` | unknown findings include explicit stop reasons |
| Predicate probes | `alpha` | `RIPR-SPEC-0001` | syntax-first probe generation, boundary_gap fixture, FileFacts DTOs, lexical syntax adapter boundary, ra_ap_syntax parser-backed test/oracle extraction, module-qualified and impl-qualified owner symbols, parser-backed predicate probe shape facts, predicate branch to return-value flow tests, missing equality discriminator facts | `evidence-first-output` | missing boundary discriminator detection |
| Error-path probes | `alpha` | `RIPR-SPEC-0001` | sample error_path explain/context command, weak_error_oracle fixture, parser-backed error-path probe shape facts, probe-relative exact error variant vs broad error oracle classification, error variant flow sink tests, missing exact error variant discriminator facts | `evidence-first-output` | broad error checks distinguished from exact variants |
| Return-value probes | `alpha` | `RIPR-SPEC-0001` | syntax-first scanner, weak_error_oracle fixture, parser-backed return and tail-expression probe shape facts, probe-relative exact value vs smoke oracle classification, binding to return-value flow tests | `activation-value-modeling-v1` | exact vs smoke return oracle distinction |
| Call-deletion probes | `alpha` | `RIPR-SPEC-0001` | syntax-first scanner, weak_error_oracle fixture, parser-backed call and side-effect probe shape facts, call effect flow sink tests | `activation-value-modeling-v1` | side-effect or call boundary candidate named |
| Fixture laboratory | `alpha` | `RIPR-SPEC-0002` | fixture spec, fixture/golden runner commands, boundary_gap fixture, weak_error_oracle fixture, FileFacts DTOs, lexical syntax adapter boundary, ra_ap_syntax parser-backed test/oracle extraction, module-qualified and impl-qualified owner symbols, parser-backed probe shape facts, oracle-strength-v2 golden blessings | `negative-metamorphic-baseline` | fixture pass rate |
| Golden JSON output | `alpha` | `RIPR-SPEC-0002` | schema reference, goldens check runner, boundary_gap check.json, weak_error_oracle check.json | `output-contract-matrix` | golden output drift count |
| Golden human output | `alpha` | `RIPR-SPEC-0002` | human output renderer, goldens check runner, boundary_gap human.txt, weak_error_oracle human.txt | `output-contract-matrix` | golden output drift count |
| Context packet v1 | `alpha` | `RIPR-SPEC-0003` | CLI context command | `agent-context-v2` | packet includes missing discriminator and related tests |
| Agent context v2 | `planned` | `RIPR-SPEC-0003` | agent context spec | `agent-context-v2` | packet includes missing values and suggested assertions |
| Analysis modes | `alpha` | `RIPR-SPEC-0001` | mode scope tests | `capability-metrics-report` | runtime and scope by mode |
| LSP diagnostics | `alpha` | `RIPR-SPEC-0001` | tower-lsp-server sidecar, stable diagnostic data payload, related test diagnostic information, diagnostic-targeted context action, diagnostic hover details, serialized refresh generations | `lsp-context-command-v1` | diagnostics carry finding/probe metadata and editor actions target the selected finding |
| Parser-backed syntax facts | `alpha` | `RIPR-SPEC-0001` | ADR 0006, ra_ap_syntax dependency, RustSyntaxAdapter parser adapter, fixture/golden output stability, module-qualified and impl-qualified owner symbols, parser-backed probe shape facts, probe-relative oracle kind and strength, typed local flow sink facts | `activation-value-modeling-v1` | syntax extraction parity |
| AST-backed test/oracle extraction | `alpha` | `RIPR-SPEC-0001` | ra_ap_syntax function extraction, parser-backed assertion macro extraction, unwrap/expect smoke-oracle tests, probe-relative OracleKind classification, fixture/golden output stability, flow sink classification tests | `activation-value-modeling-v1` | oracle kind recognition rate |
| AST-backed probe ownership | `alpha` | `RIPR-SPEC-0001` | module-qualified owner SymbolId tests, impl-qualified owner SymbolId tests, changed-line owner resolution tests, fixture/golden output stability, flow sink owner retention | `activation-value-modeling-v1` | duplicate symbols do not cross-link tests |
| AST-backed probe generation | `alpha` | `RIPR-SPEC-0001` | parser-backed predicate probe shape facts, parser-backed return and tail-expression probe shape facts, parser-backed error-path probe shape facts, parser-backed call, side-effect, field, and match probe shape facts, fixture/golden output stability, probe-relative oracle kind and strength, typed local flow sink facts | `activation-value-modeling-v1` | syntax facts generate current probe families |
| Local delta flow | `alpha` | `RIPR-SPEC-0001` | FlowSinkKind domain contract labels, FlowSinkFact attached to findings, predicate branch to return-value flow tests, error variant flow sink tests, side-effect call flow sink tests, match-arm result flow sink tests, binding to return-value flow tests, propagation_unknown stop reason regression test, missing discriminator facts tied to flow sinks | `evidence-first-output` | flow sink identification rate |
| Activation/value modeling | `alpha` | `RIPR-SPEC-0001` | ValueContext domain contract labels, observed ValueFact records, MissingDiscriminatorFact records, boundary equality BDD tests, exact error variant gap BDD tests, boundary_gap fixture, weak_error_oracle fixture, snapshot_oracle fixture | `evidence-first-output` | detected and missing value facts |
| Repository config | `planned` | future spec | roadmap | `ripr-config-v1` | configured oracle/topology rules applied |
| Suppression | `planned` | future spec | roadmap | `suppression-v1` | suppression rate and visible suppressed findings |
| cargo-mutants calibration | `planned` | future spec | roadmap | `cargo-mutants-calibration-scaffold` | static class vs real mutation outcome report |

## Update Rules

Update this file when a PR changes capability status, adds fixtures, changes a
public output contract, or adds a measurable acceptance condition.

README should show only the headline capability snapshot. This file is the
deeper tracking surface for contributors and agents.
