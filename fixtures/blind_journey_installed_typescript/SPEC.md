# Installed TypeScript Journey Fixture

Spec: RIPR-SPEC-0210

## Given

The 0.11 installed TypeScript repair journey (#4519, parents #4508/#4600,
contract #4603/RIPR-SPEC-0200, executor #4604/RIPR-SPEC-0205) needs one
retained, bounded npm/Vitest fixture whose literal scripted journey the
deterministic executor can stamp and validate offline, so the language row is
executable through the gate before the frozen #1609 candidate and the #4510
harness make the real installed run terminal.

This fixture commits one fresh small npm project
(`fixtures/blind_journey_installed_typescript/repository/`): `src/pricing.ts`
owns one discount-threshold comparison and `test/pricing.test.ts` is the
single related Vitest test. `package.json` selects the `vitest run` script and
the retained `package-lock.json` root entry backs the npm-runner selection.
Ten bound snapshots are retained:

- `base` — the boundary is exclusive (`subtotal > DISCOUNT_THRESHOLD`); the
  test covers subtotals 9999 and 10001 only;
- `head` — the committed production change makes the boundary inclusive
  (`subtotal >= DISCOUNT_THRESHOLD`);
- eight negative variants — `variant-let-binding`, `variant-reassignment`,
  `variant-shadowing`, `variant-imported-constant`,
  `variant-object-namespace-write`, `variant-enum-computed-initializer`,
  `variant-nonliteral-arithmetic` and `variant-reach-barrier` — the same
  changed comparison expressed through each rebindable, imported, shadowed,
  computed, nonliteral or control-flow-barrier shape the accepted #4429
  implementation contract leaves unresolved.

Every snapshot file is bound by SHA-256 in `manifest.json`; the manifest also
records the exact, reproducible git commit/tree identities (pinned author,
fixed timestamps, `core.autocrlf=false`), the recorded npm/Node/Vitest
environment identities and the substitution contract for the printed command
templates. The expected repair is independently understandable from source —
one equality assertion `discountedTotal(10000) === 9500` — and requires no
production edit.

## When

`cargo xtask check-fixture-contracts` validates this fixture: the manifest
parses at `blind_journey_installed_typescript_fixture.v1`, every recorded
snapshot digest matches the bytes on disk, every recorded git identity is
well-formed, the selected repair names exactly one edit target inside the
expected edit cage, and every scripted scenario id named by the manifest
exists in the RIPR-SPEC-0205 executor corpus and binds the recorded snapshot
identities through the manifest's explicit per-scenario
`scenario_snapshot_bindings` map — each negative variant is bound to its own
variant snapshot, never to `base` or to another variant. `cargo xtask
blind-journey-execute` then runs the sixteen scripted
installed-TypeScript scenarios through the real executor; each emitted packet
is stamped and must be accepted by the live RIPR-SPEC-0200 validator, so a
hand-edited expectation cannot make a wrong journey emit.

The positive row follows only public docs/help and literal product-emitted
commands — `ripr doctor`, `ripr check`, `ripr pilot`, `ripr agent status`, one
canonical item selection, the printed `ripr agent repair --phase before` that
emits the constant-boundary packet naming the owner, the equality input and
the executable `vitest run test/pricing.test.ts` command, one bounded test
edit, the exact printed Vitest command between the phases, the printed
`--phase after`, and the printed receipt route — recorded from a foreign
launch directory against the selected root.

## Then

- The positive row, the interrupted-receipt recovery and the second-root
  repeat derive `passed_blind_journey` and share one portable identity
  (concrete root spelling is retained evidence, never identity).
- Each negative variant stops before any edit: packet readiness stays false
  with the precise printed reason for its shape, and the row derives
  `verification_not_run_visible` without a delegatable packet or fabricated
  receipt.
- The failing printed Vitest command derives
  `verification_failure_visible` while static movement stays a separate axis;
  the skipped-verification row keeps verification `not_run` despite static
  movement; the cyclic pilot/agent-status route derives
  `product_discoverability_failure` with the exact blocked input; the deleted
  before-artifact row stops with `honest_limitation` naming the printed
  recovery; the planted workspace binary derives
  `candidate_identity_failure`; the interrupted receipt write recovers
  through a clean rerun and never yields a half-authoritative receipt.
- Every negative control keeps its own non-positive terminal result; no
  aggregate converts them into success.

## Must Not

- Do not run a candidate, launch a process, or decide a release verdict.
- Do not record any production edit as admitted: `src/pricing.ts`,
  `src/constants.ts` and `package.json` stay forbidden in every positive row.
- Do not let static movement, project verification, or receipt issuance
  imply one another; the evidence axes stay separate in every scenario.
- Keep the pinned static vocabulary only; this fixture records no dynamic or
  mutation-testing verdicts and no coverage claims.
- Do not count any scripted executor success as installed usefulness, blind
  qualification, candidate selection or parent acceptance.
