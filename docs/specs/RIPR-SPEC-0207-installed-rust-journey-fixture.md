# RIPR-SPEC-0207: Installed Rust journey fixture

Status: proposed

Owner: test-infra

Created: 2026-10-03

Linked issues:

- #4516 (this slice: the literal installed Rust repair journey)
- #4508 (parent installed-journey matrix; Rust row)
- #4600 (blind installed-agent acceptance; consumes the row)
- #3797 (blind journey authority)
- #4510 (shared candidate/process harness; admission authority, referenced not
  copied)
- #4604 / RIPR-SPEC-0205 (executor consumer; the corpus rows this fixture
  names must execute through its gate)
- #4603 / RIPR-SPEC-0200 (blind-journey contract; every emitted packet is
  stamped and validated through it)

Support-tier impact:

- None. The fixture is a retained two-source Cargo repository plus one
  manifest; it runs no process, edits nothing, calls no provider and collects
  no telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. The fixture adds committed text/JSON under `fixtures/`; the dedicated
  `check-fixture-contracts` validator owns its contract and reads only the
  committed bytes.

## Problem

RIPR-SPEC-0205 (#4604) made the #4516/#4518/#4519 literal journeys
*scriptable* but deliberately left them *unauthored*: its corpus uses generic
items (`item-a`, `quiet-neighbor`, `example.invalid` identities), records no
real repository, and binds no retained fixture. #4516 owns the Rust language
row: one bounded Cargo repository with a committed production boundary change
and a related test that misses the exact boundary, plus the ten failure
controls, following only public docs/help and literal product-emitted
commands. Without a retained, digest-bound fixture, a scripted Rust row could
claim a journey over a repository the gate never sees, and the #4508/#4600
consumers would have no fixture identity to reference.

## Behavior

`fixtures/blind_journey_installed_rust/` commits the fixture:

- `repository/base`, `repository/head`, `repository/head-no-ignore` — three
  bounded snapshots of one Cargo repository (`Cargo.toml`, `src/lib.rs`,
  `tests/tier_boundary.rs`, plus `/target/` `.gitignore` on `head` only). The
  committed production change moves one equality boundary from exclusive
  (`quantity > 20`) to inclusive (`quantity >= 20`); the related test covers
  quantities 19 and 21 and misses exactly 20, so the expected repair is one
  equality assertion and no production edit, independently understandable
  from source.
- `manifest.json` (`blind_journey_installed_rust_fixture.v1`) binds every
  snapshot file by SHA-256, records the reproducible git commit/tree
  identities (pinned author, fixed timestamps, `core.autocrlf=false`), the
  journey answer surface (eligible item, quiet neighbor, expected edit cage,
  forbidden edits, discriminator family), the printed command templates with
  their substitution contract, the ten scripted scenario ids, and the
  control mapping.
- `SPEC.md` states the Given/When/Then/Must-Not fixture contract.

`fixtures/blind_journey_execute/corpus.json` gains the ten `installed_rust_*`
scripted journeys (#4516's positive row and its ten failure controls,
foreign-CWD decoy and second-root repeat included). They execute through the
unchanged RIPR-SPEC-0205 executor: the event chain, per-kind digest presence,
observation binding and derived terminals are exactly the consumer's, so a
hand-edited expectation cannot make a wrong journey emit. The derived
terminals are the honest machine results — `passed_blind_journey` for the
positive row, the second-root repeat and the interrupted-receipt recovery;
`verification_not_run_visible` for the no-ignore stop-before-edit and the
skipped-verification rows; `unsafe_or_wrong_edit` for the `target/notes.rs`
and production-source attempts; `verification_failure_visible` for the
failing focused test with static movement kept on its own axis;
`honest_limitation` for the deleted before-artifact row that names its
printed recovery; and `candidate_identity_failure` for the planted workspace
binary.

The dedicated validator `validate_blind_journey_installed_rust_fixture`
(`cargo xtask check-fixture-contracts`) owns the fixture contract: the
manifest schema, the per-file digest bindings recomputed against the bytes on
disk, the well-formed recorded git identities, the edit-cage invariants
(selected repair target inside the cage; cage and forbidden edits disjoint),
and the requirement that every scripted scenario id the fixture names exists
in the committed executor corpus.

The receipt template and three positive scripted receipt inputs include the
workflow verification artifact, the language's authored seam identity and JSON
output required by the public parser. The existing public CLI receipt-attempt
control projects each committed template onto a genuine Rust repair attempt and
checks receipt binding plus incomplete-argument refusals. That parser/control
proof does not execute the authored installed-language fixture or establish blind
acceptance; the real candidate-bound installed journey remains separate.

## Required Evidence

- `cargo xtask check-fixture-contracts` recomputes every snapshot digest and
  cross-checks the manifest against the committed executor corpus on every
  policy pass.
- `cargo xtask blind-journey-execute` executes all ten installed-Rust rows
  through the real executor in CI, requires the live RIPR-SPEC-0200
  validator to accept every emitted packet, and writes
  `target/ripr/reports/blind-journey-execute.{md,json}`.
- The executor decision receipt
  `metrics/blind-journey-execute/executor-receipt.json` binds the enlarged
  corpus scenario count and records the fixture-binding limitation plus the
  still-not-exercised real process-interrupt combination.
- `cargo test -p xtask installed_rust_fixture` covers the validator units:
  committed-manifest digest binding, corpus cross-reference, unknown-scenario
  refusal, drifted-digest refusal and git-identity well-formedness.

## Non-Goals

- No candidate execution, install, packaging, process admission or release
  verdict; #4510 stays the harness authority and the frozen #1609 candidate
  run stays the real execution authority.
- No product command or repair-implementation change; implementation owners
  (#4441, #4365, #4307/#4306) keep code ownership and a discovered defect
  returns to the narrow owner.
- No Python or TypeScript rows; #4518/#4519 stay the sibling journey owners.
- No blind-operator assessment; #4600 keeps that authority and the scripted
  prompt surfaces stay generic and contamination-free.
- No claim that every Cargo repository shape is supported and no
  mutation-adequacy or runtime-mutation result anywhere in the rows.

## Acceptance Examples

1. The positive row follows only public docs/help and literal
   product-emitted commands from `ripr doctor` through the printed focused
   `cargo test --test tier_boundary`, the printed `--phase after` and the
   printed receipt route; every emitted packet is stamped and accepted by the
   live validator with `passed_blind_journey`.
2. The repository without a `target/` ignore rule stops before any edit with
   the precise printed precondition and derives `verification_not_run_visible`
   — never a terminal failure after the printed test command.
3. The `target/notes.rs` note file and the production-source edit each derive
   `unsafe_or_wrong_edit`; the selected test file remains the only admitted
   user-authored edit in the positive rows.
4. The failing focused test derives `verification_failure_visible` while the
   static movement stays on its own axis and receipt closure stays blocked;
   the skipped-verification row keeps verification `not_run` even though
   static movement improved.
5. The deleted before-artifact row stops with `honest_limitation`, names the
   printed recovery route, and claims no static or receipt state.
6. The planted workspace binary derives `candidate_identity_failure`; the
   foreign-CWD decoy row and the second-root repeat keep the selected root
   authoritative and share one portable identity.
7. A manifest whose snapshot bytes drift, whose git identities are
   ill-formed, or which names a scenario the corpus does not contain fails
   `check-fixture-contracts`.

## Test Mapping

- `xtask/src/fixture_contracts/general_validators.rs`
  (`installed_rust_fixture_tests`): committed manifest binds the snapshot
  bytes; committed scenarios exist in the executor corpus; an unknown
  scenario id is reported; a drifted snapshot digest is rejected; an
  unlisted snapshot file is rejected; every named scenario candidate binds
  the manifest snapshots; git identity binding accepts only lowercase
  40-hex.
- `xtask/src/reports/blind_journey_execute.rs` gate tests: the enlarged
  committed corpus and receipt validate against the live executor and the
  required-scenario coverage cannot be dropped.
- `cargo xtask blind-journey-execute` and `cargo xtask check-fixture-contracts`
  are themselves the CI-executable proof; hosted CI is the execution
  authority for every step that would need a built binary.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `fixtures/blind_journey_installed_rust/` | retained snapshot repository, digest-bound manifest, fixture SPEC |
| `fixtures/blind_journey_execute/corpus.json` | ten scripted `installed_rust_*` journey scenarios with committed executor expectations |
| `metrics/blind-journey-execute/executor-receipt.json` | ratification receipt binding the 34-scenario corpus with fixture limitations |
| `xtask/src/blind_journey_execute.rs` | required-scenario coverage extended to the ten installed-Rust rows |
| `xtask/src/fixture_contracts/general_validators.rs` | dedicated fixture validator with digest recomputation and corpus cross-reference |
| `xtask/src/reports/fixtures.rs` | manifest-only fixture directory registration |
| `docs/specs/README.md` + `.ripr/traceability.toml` | spec registration and test traceability |

## Metrics

- `blind_journey_execute_scenarios` (34, including 10 installed-Rust rows)
- `blind_journey_execute_positive_scenarios` (6, including the two
  installed-Rust positive rows)
- installed-Rust row terminals by control (positive, not-run, cage refusal,
  verification-failure-visible, honest-limitation, candidate-identity-failure)
- fixture snapshot digest binding failures (target: 0)
