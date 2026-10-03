# RIPR-SPEC-0210: Installed TypeScript journey fixture

Status: proposed

Owner: test-infra

Created: 2026-10-04

Linked issues:

- #4519 (this slice: the literal installed TypeScript repair journey)
- #4508 (parent installed-journey matrix; TypeScript row)
- #4600 (blind installed-agent acceptance; consumes the row)
- #3797 (blind journey authority)
- #4510 (shared candidate/process harness; admission authority, referenced not
  copied)
- #4604 / RIPR-SPEC-0205 (executor consumer; the corpus rows this fixture
  names must execute through its gate)
- #4603 / RIPR-SPEC-0200 (blind-journey contract; every emitted packet is
  stamped and validated through it)
- #4410 / #4429 (accepted TypeScript same-file constant-threshold
  implementation authority; defects return to its owner)

Support-tier impact:

- None. The fixture is a retained small npm/Vitest repository plus one
  manifest; it runs no process, edits nothing, calls no provider and collects
  no telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. The fixture adds committed text/JSON under `fixtures/`; the dedicated
  `check-fixture-contracts` validator owns its contract and reads only the
  committed bytes.

## Problem

RIPR-SPEC-0205 (#4604) made the #4516/#4518/#4519 literal journeys
*scriptable* but deliberately left them *unauthored*, and RIPR-SPEC-0207
(#5126) delivered the Rust language row. #4519 owns the TypeScript language
row: one bounded npm/Vitest repository with a committed production boundary
change and a related test that misses the exact boundary, plus every
rebindable, imported, shadowed, computed and nonliteral negative shape the
accepted #4429 implementation contract leaves unresolved, following only
public docs/help and literal product-emitted commands. Without a retained,
digest-bound fixture, a scripted TypeScript row could claim a journey over a
repository the gate never sees, and the #4508/#4600 consumers would have no
fixture identity to reference.

## Behavior

`fixtures/blind_journey_installed_typescript/` commits the fixture:

- `repository/base` and `repository/head` — two bounded snapshots of one
  small npm project (`package.json`, `package-lock.json`, `src/pricing.ts`,
  `test/pricing.test.ts`). The committed production change moves one
  discount-threshold comparison from exclusive (`subtotal > 10000`) to
  inclusive (`subtotal >= 10000`); the related Vitest test covers subtotals
  9999 and 10001 and misses exactly 10000, so the expected repair is one
  equality assertion and no production edit, independently understandable
  from source. The retained `package-lock.json` root entry backs the
  npm-runner selection that makes the printed `vitest run test/pricing.test.ts`
  command executable in the documented npm environment.
- `repository/variant-*` — eight negative snapshots of the same changed
  comparison through each shape the accepted contract fails closed on:
  rebindable `let` binding, reassignment, shadowing, imported constant,
  object/namespace write, enum or computed initializer, nonliteral
  arithmetic, and the control-flow reach barrier.
- `manifest.json` (`blind_journey_installed_typescript_fixture.v1`) binds
  every snapshot file by SHA-256, records the reproducible git commit/tree
  identities (pinned author, fixed timestamps, `core.autocrlf=false`), the
  recorded Node/npm/Vitest environment identities, the journey answer
  surface (eligible item, quiet neighbor, expected edit cage, forbidden
  edits, discriminator family), the printed command templates with their
  substitution contract, the sixteen scripted scenario ids, and the control
  mapping.
- `SPEC.md` states the Given/When/Then/Must-Not fixture contract.

`fixtures/blind_journey_execute/corpus.json` gains the sixteen
`installed_typescript_*` scripted journeys (#4519's positive row, its
failure controls, one row per negative variant, foreign-CWD decoy and
second-root repeat included). They execute through the unchanged
RIPR-SPEC-0205 executor: the event chain, per-kind digest presence,
observation binding and derived terminals are exactly the consumer's, so a
hand-edited expectation cannot make a wrong journey emit. The derived
terminals are the honest machine results — `passed_blind_journey` for the
positive row, the second-root repeat and the interrupted-receipt recovery;
`verification_not_run_visible` for the eight negative variants (packet
readiness false with the precise printed reason, never a delegatable packet
or fabricated receipt) and the skipped-verification row;
`verification_failure_visible` for the failing printed Vitest command with
static movement kept on its own axis; `product_discoverability_failure` for
the cyclic pilot/agent-status route; `honest_limitation` for the deleted
before-artifact row that names its printed recovery; and
`candidate_identity_failure` for the planted workspace binary.

The dedicated validator `validate_blind_journey_installed_typescript_fixture`
(`cargo xtask check-fixture-contracts`) owns the fixture contract: the
manifest schema, the per-file digest bindings recomputed against the bytes
on disk, the well-formed recorded git identities, the edit-cage invariants
(selected repair target inside the cage; cage and forbidden edits
disjoint), the requirement that every scripted scenario id the fixture names
exists in the committed executor corpus, and the requirement that every
named scenario's candidate identities bind the manifest snapshot its
`scenario_snapshot_bindings` entry designates — never the unchanged `base`
snapshot and never another variant.

## Required Evidence

- `cargo xtask check-fixture-contracts` recomputes every snapshot digest and
  cross-checks the manifest against the committed executor corpus on every
  policy pass.
- `cargo xtask blind-journey-execute` executes all sixteen
  installed-TypeScript rows through the real executor in CI, requires the
  live RIPR-SPEC-0200 validator to accept every emitted packet, and writes
  `target/ripr/reports/blind-journey-execute.{md,json}`.
- The executor decision receipt
  `metrics/blind-journey-execute/executor-receipt.json` binds the enlarged
  corpus scenario count and records the fixture-binding limitation.
- `cargo test -p xtask installed_typescript_fixture` covers the validator
  units: committed-manifest digest binding, corpus cross-reference,
  unknown-scenario refusal, drifted-digest refusal, unlisted-snapshot-file
  refusal, scenario designated-snapshot binding (including wrong-variant and
  missing-binding refusal) and git-identity well-formedness.

## Non-Goals

- No candidate execution, install, packaging, process admission or release
  verdict; #4510 stays the harness authority and the frozen #1609 candidate
  run stays the real execution authority.
- No product command or repair-implementation change; #4410/#4429 keep code
  ownership and a discovered defect returns to the narrow owner.
- No Rust or Python rows; #4516/#4518 stay the sibling journey owners.
- No blind-operator assessment; #4600 keeps that authority and the scripted
  prompt surfaces stay generic and contamination-free.
- No claim that every TypeScript project or boundary expression is supported
  and no mutation-adequacy or runtime-mutation result anywhere in the rows.

## Acceptance Examples

1. The positive row follows only public docs/help and literal
   product-emitted commands from `ripr doctor` through `ripr check`,
   `ripr pilot`, `ripr agent status`, the printed `ripr agent repair
   --phase before` that emits the packet naming `src/pricing.ts` owner,
   the 10000 equality input and the executable `vitest run
   test/pricing.test.ts` command, the bounded edit, the exact printed
   Vitest command, the printed `--phase after` and the printed receipt
   route; every emitted packet is stamped and accepted by the live
   validator with `passed_blind_journey`.
2. Each negative variant stops before any edit: packet readiness stays
   false with the precise printed reason for its binding or reach shape,
   and the row derives `verification_not_run_visible` without a delegatable
   packet or fabricated receipt.
3. The failing printed Vitest command derives `verification_failure_visible`
   while the static movement stays on its own axis and receipt closure
   stays blocked; the skipped-verification row keeps verification
   `not_run` even though static movement improved.
4. The cyclic pilot/agent-status route derives
   `product_discoverability_failure` with the exact blocked public input;
   the deleted before-artifact row stops with `honest_limitation`, names
   the printed recovery route, and claims no static or receipt state.
5. The planted workspace binary derives `candidate_identity_failure`; the
   foreign-CWD decoy row and the second-root repeat keep the selected root
   authoritative and share one portable identity.
6. A manifest whose snapshot bytes drift, whose git identities are
   ill-formed, which names a scenario the corpus does not contain, or whose
   scenario candidate binds a snapshot other than its designated entry fails
   `check-fixture-contracts`.

## Test Mapping

- `xtask/src/fixture_contracts/general_validators.rs`
  (`installed_typescript_fixture_tests`): committed manifest binds the
  snapshot bytes; committed scenarios exist in the executor corpus; an
  unknown scenario id is reported; a drifted snapshot digest is rejected; an
  unlisted snapshot file is rejected; every named scenario candidate binds
  the manifest snapshots; a candidate bound to the wrong variant snapshot or
  missing its designated-snapshot binding is rejected; git identity binding
  accepts only lowercase 40-hex.
- `xtask/src/reports/blind_journey_execute.rs` gate tests: the enlarged
  committed corpus and receipt validate against the live executor and the
  required-scenario coverage cannot be dropped.
- `cargo xtask blind-journey-execute` and `cargo xtask check-fixture-contracts`
  are themselves the CI-executable proof; hosted CI is the execution
  authority for every step that would need a built binary.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `fixtures/blind_journey_installed_typescript/` | retained snapshot repository, digest-bound manifest, fixture SPEC |
| `fixtures/blind_journey_execute/corpus.json` | sixteen scripted `installed_typescript_*` journey scenarios with committed executor expectations |
| `metrics/blind-journey-execute/executor-receipt.json` | ratification receipt binding the 61-scenario corpus with fixture limitations |
| `xtask/src/blind_journey_execute.rs` | required-scenario coverage extended to the sixteen installed-TypeScript rows |
| `xtask/src/fixture_contracts/general_validators.rs` | dedicated fixture validator with digest recomputation and corpus cross-reference |
| `xtask/src/reports/fixtures.rs` | manifest-only fixture directory registration |
| `docs/specs/README.md` + `.ripr/traceability.toml` | spec registration and test traceability |

## Metrics

- `blind_journey_execute_scenarios` (61, including 16 installed-TypeScript
  rows)
- `blind_journey_execute_positive_scenarios` (12, including the three
  installed-TypeScript positive rows)
- installed-TypeScript row terminals by control (positive, not-run,
  verification-failure-visible, discoverability-failure,
  honest-limitation, candidate-identity-failure)
- fixture snapshot digest binding failures (target: 0)
