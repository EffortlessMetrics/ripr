# Test Taxonomy

`ripr` tests should prove behavior sharply enough that a regression would fail
for the right reason.

| Test type | Purpose | Required for |
| --- | --- | --- |
| Unit | Small pure logic and parser helpers. | domain rules, classifier decisions, low-level parsing |
| Contract | Public API and output shape. | JSON DTOs, config, CLI flags |
| Fixture BDD | End-to-end analyzer behavior on mini workspaces. | analyzer capability changes |
| Golden | Stable user-facing output. | human, JSON, context, LSP, GitHub, SARIF output |
| Invariant | Repo safety properties. | static language, unknown stop reasons, no panic-family debt |
| Metamorphic | Stability under behavior-preserving edits. | whitespace, comments, reordered tests, renamed locals |
| Regression | A specific bug never returns. | review comments, CI failures, bug reports |
| Dogfood | `ripr` on `ripr`. | product quality loop |
| Extension smoke | VS Code client and server path. | provisioning, startup, diagnostics |
| Calibration | Static prediction compared with real mutation data. | cargo-mutants integration |

## Required Proof By Change Type

Classifier or analyzer semantics:

- fixture BDD
- golden JSON
- golden human output when human output changes
- invariant tests when language or unknown behavior changes

JSON or context schema:

- schema docs
- golden JSON or context output
- compatibility note

LSP behavior:

- LSP diagnostic or hover expectation
- extension compile/package gates

Docs-only changes:

- markdown link check
- docs index consistency

Automation or policy changes:

- `xtask` tests when logic is non-trivial
- dry-run transcript or CI evidence

## Test Oracle Standard

Avoid weak analyzer tests such as only checking:

- command exits successfully
- output contains one loose substring
- findings are non-empty

Prefer exact assertions on:

- exposure class
- probe family
- missing discriminator
- related test evidence
- oracle kind and strength
- stop reasons

`cargo xtask test-oracle-report` writes the current advisory baseline to
`target/ripr/reports/test-oracles.md` and
`target/ripr/reports/test-oracles.json`. The report classifies detected Rust
tests as strong, medium, weak, or smoke so the repo can measure weak-oracle
debt before making it a blocking policy.
