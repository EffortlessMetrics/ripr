# RIPR-SPEC-0209: Installed Python journey fixture

Status: proposed

Owner: test-infra

Created: 2026-10-03

Linked issues:

- #4518 (this slice: the literal installed Python repair journey)
- #4508 (parent installed-journey matrix; Python row)
- #4600 (blind installed-agent acceptance; consumes the row)
- #3797 (blind journey authority)
- #4510 (shared candidate/process harness; admission authority, referenced not
  copied)
- #4604 / RIPR-SPEC-0205 (executor consumer; the corpus rows this fixture
  names must execute through its gate)
- #4603 / RIPR-SPEC-0200 (blind-journey contract; every emitted packet is
  stamped and validated through it)
- #4516 / RIPR-SPEC-0207 (sibling installed-Rust journey fixture; the pattern
  this fixture reuses)

Support-tier impact:

- None. The fixture is a retained flat-layout Python package plus one
  manifest; it runs no process, edits nothing, calls no provider and collects
  no telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. The fixture adds committed text/JSON under `fixtures/` covered by the
  `fixtures/**` allowlist; the dedicated `check-fixture-contracts` validator
  owns its contract and reads only the committed bytes.

## Problem

RIPR-SPEC-0205 (#4604) made the #4516/#4518/#4519 literal journeys
*scriptable* but deliberately left them *unauthored*, and RIPR-SPEC-0207
(#4516) delivered the Rust language row. #4518 owns the Python language row:
one bounded flat-layout package whose committed production boundary change
and related test that misses the exact boundary are retained, the printed
verification command must run in the documented project environment
(`python -m pytest`, not a bare executable that selects another
interpreter), and repair-card, LSP, gap-artifact and dogfood consumers must
understand the current typed command while preserving explicitly supported
historical artifacts. Without a retained, digest-bound fixture, a scripted
Python row could claim a journey over a repository the gate never sees, and
the #4508/#4600 consumers would have no fixture identity to reference.

## Behavior

`fixtures/blind_journey_installed_python/` commits the fixture:

- `repository/base`, `repository/head` — two bounded snapshots of one
  flat-layout package (`pyproject.toml`, `pricing/__init__.py`,
  `pricing/core.py`, `tests/test_pricing.py`). The committed production
  change moves one numeric equality boundary from exclusive (`units > 20`)
  to inclusive (`units >= 20`); the related test covers quantities 19 and 21
  and misses exactly 20, so the expected repair is one equality assertion
  and no production edit, independently understandable from source. The
  package is never installed editable and carries no path-repair conftest,
  so the module-form `python -m pytest tests/test_pricing.py` succeeds in
  the clean shape while the bare `pytest tests/test_pricing.py` console
  script fails with an import error — the environment-binding control the
  acceptance law requires.
- `manifest.json` (`blind_journey_installed_python_fixture.v1`) binds every
  snapshot file by SHA-256, records the reproducible git commit/tree
  identities (pinned author, fixed timestamps, `core.autocrlf=false`), the
  journey answer surface (eligible item, quiet neighbor, expected edit cage,
  forbidden edits, discriminator family), the environment-binding contract
  (module-form current command; bare form retained only as a historical
  artifact at documented strength), the printed command templates with their
  substitution contract, the eleven scripted scenario ids, and the control
  mapping.
- `SPEC.md` states the Given/When/Then/Must-Not fixture contract.

`fixtures/blind_journey_execute/corpus.json` gains the eleven
`installed_python_*` scripted journeys (#4518's positive row and its ten
failure controls, foreign-CWD decoy included). They execute through the
unchanged RIPR-SPEC-0205 executor: the event chain, per-kind digest
presence, observation binding and derived terminals are exactly the
consumer's, so a hand-edited expectation cannot make a wrong journey emit.
The derived terminals are the honest machine results —
`passed_blind_journey` for the positive row, the historical-bare-form
compatibility row and the interrupted-receipt recovery;
`verification_failure_visible` for the bare-command import failure and the
failing focused test with static movement kept on its own axis;
`verification_not_run_visible` for the no-environment stop-before-edit and
the skipped-verification rows; `unsafe_or_wrong_edit` for the production
edit attempt; `honest_limitation` for the receipt-path mismatch and the
deleted before-artifact rows that name their printed recovery; and
`candidate_identity_failure` for the planted decoy interpreter or workspace
candidate.

The dedicated validator `validate_blind_journey_installed_python_fixture`
(`cargo xtask check-fixture-contracts`) owns the fixture contract: the
manifest schema, the per-file digest bindings recomputed against the bytes
on disk, the well-formed recorded git identities, the edit-cage invariants
(selected repair target inside the cage; cage and forbidden edits
disjoint), the environment-binding invariant (focused verification command
equals the documented module form; bare form recorded and never the
recommendation), and the requirement that every scripted scenario id the
fixture names exists in the committed executor corpus with its candidate
identity bound to the manifest snapshots.

## Required Evidence

- `cargo xtask check-fixture-contracts` recomputes every snapshot digest and
  cross-checks the manifest against the committed executor corpus on every
  policy pass.
- `cargo xtask blind-journey-execute` executes all eleven installed-Python
  rows through the real executor in CI, requires the live RIPR-SPEC-0200
  validator to accept every emitted packet, and writes
  `target/ripr/reports/blind-journey-execute.{md,json}`.
- The executor decision receipt
  `metrics/blind-journey-execute/executor-receipt.json` binds the enlarged
  corpus scenario count and records the fixture-binding limitation plus the
  still-not-exercised real process-interrupt combination.
- `cargo test -p xtask installed_python_fixture` covers the validator units:
  committed-manifest digest binding, corpus cross-reference,
  unknown-scenario refusal, drifted-digest refusal, unlisted-snapshot-file
  refusal, git-identity well-formedness, scenario candidate snapshot
  binding, and the bare-form-must-not-become-the-recommendation invariant.

## Non-Goals

- No candidate execution, install, packaging, process admission or release
  verdict; #4510 stays the harness authority and the frozen #1609 candidate
  run stays the real execution authority.
- No product command or repair-implementation change; implementation owners
  (#4485, #4307/#4306) keep code ownership and a discovered defect returns
  to the narrow owner.
- No Rust or TypeScript rows; #4516/#4519 stay the sibling journey owners.
- No blind-operator assessment; #4600 keeps that authority and the scripted
  prompt surfaces stay generic and contamination-free.
- No claim that every Python packaging or test-runner configuration is
  supported, no claim that `python` selects the correct interpreter outside
  the documented environment, and no mutation-adequacy or runtime-mutation
  result anywhere in the rows.

## Acceptance Examples

1. The positive row follows only public docs/help and literal
   product-emitted commands from `ripr doctor` through the printed
   module-form `python -m pytest tests/test_pricing.py`, the printed
   `--phase after` and the printed receipt route; every emitted packet is
   stamped and accepted by the live validator with `passed_blind_journey`.
2. The bare `pytest tests/test_pricing.py` command in the clean flat-layout
   shape fails with an import error and derives
   `verification_failure_visible` — the environment-binding control
   discriminates — while the module form in the positive row passes.
3. The no-activated-environment row stops before any edit with the precise
   printed prerequisite and derives `verification_not_run_visible`; the
   harness never silently repairs PATH/PYTHONPATH.
4. The production edit derives `unsafe_or_wrong_edit`; the selected test
   file remains the only admitted user-authored edit in the positive rows.
5. The failing focused test derives `verification_failure_visible` while
   static movement stays on its own axis and receipt closure stays blocked;
   the skipped-verification row keeps verification `not_run` even though
   static movement improved.
6. The receipt-path mismatch row stops with `honest_limitation` naming the
   printed recovery; the displayed receipt path and the command `--out`
   target agree in every positive row.
7. The historical bare-form artifact is accepted at documented strength for
   replay only, the unsupported variant fails closed, and the current
   module form stays the recommendation.
8. The planted decoy interpreter or workspace candidate derives
   `candidate_identity_failure`; the foreign-CWD decoy row keeps the
   selected root authoritative.
9. A manifest whose snapshot bytes drift, whose git identities are
   ill-formed, whose focused command is the bare form, or which names a
   scenario the corpus does not contain fails `check-fixture-contracts`.

## Test Mapping

- `xtask/src/fixture_contracts/general_validators.rs`
  (`installed_python_fixture_tests`): committed manifest binds the snapshot
  bytes; committed scenarios exist in the executor corpus; an unknown
  scenario id is reported; a drifted snapshot digest is rejected; an
  unlisted snapshot file is rejected; every named scenario candidate binds
  the manifest snapshots; git identity binding accepts only lowercase
  40-hex; a bare-form focused command is rejected.
- `xtask/src/reports/blind_journey_execute.rs` gate tests: the enlarged
  committed corpus and receipt validate against the live executor and the
  required-scenario coverage cannot be dropped.
- `cargo xtask blind-journey-execute` and `cargo xtask check-fixture-contracts`
  are themselves the CI-executable proof; hosted CI is the execution
  authority for every step that would need a built binary.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `fixtures/blind_journey_installed_python/` | retained snapshot repository, digest-bound manifest, fixture SPEC |
| `fixtures/blind_journey_execute/corpus.json` | eleven scripted `installed_python_*` journey scenarios with committed executor expectations |
| `metrics/blind-journey-execute/executor-receipt.json` | ratification receipt binding the 45-scenario corpus with fixture limitations |
| `xtask/src/blind_journey_execute.rs` | required-scenario coverage extended to the eleven installed-Python rows |
| `xtask/src/fixture_contracts/general_validators.rs` | dedicated fixture validator with digest recomputation, environment-binding invariant and corpus cross-reference |
| `xtask/src/reports/fixtures.rs` | manifest-only fixture directory registration |
| `docs/specs/README.md` + `.ripr/traceability.toml` + `policy/doc-artifacts.toml` | spec registration and test traceability |

## Metrics

- `blind_journey_execute_scenarios` (45, including 11 installed-Python rows)
- `blind_journey_execute_positive_scenarios` (9, including the three
  installed-Python positive rows)
- installed-Python row terminals by control (positive, verification-failure
  visible, not-run visible, cage refusal, honest-limitation,
  candidate-identity-failure)
- fixture snapshot digest binding failures (target: 0)
