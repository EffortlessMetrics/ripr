# Installed Rust Journey Fixture

Spec: RIPR-SPEC-0207

## Given

The 0.11 installed Rust repair journey (#4516, parents #4508/#4600, contract
#4603/RIPR-SPEC-0200, executor #4604/RIPR-SPEC-0205) needs one retained,
bounded Cargo fixture whose literal scripted journey the deterministic executor
can stamp and validate offline, so the language row is executable through the
gate before the frozen #1609 candidate and the #4510 harness make the real
installed run terminal.

This fixture commits one fresh two-source Cargo repository
(`fixtures/blind_journey_installed_rust/repository/`): `src/lib.rs` owns one
loyalty-tier predicate and `tests/tier_boundary.rs` is the single related
test. Three bound snapshots are retained:

- `base` — the boundary is exclusive (`quantity > 20`); the test covers
  quantities 19 and 21 only;
- `head` — the committed production change makes the boundary inclusive
  (`quantity >= 20`) and adds `/target/` to `.gitignore`;
- `head-no-ignore` — the same head production change without any `target/`
  ignore rule.

Every snapshot file is bound by SHA-256 in `manifest.json`; the manifest also
records the exact, reproducible git commit/tree identities (pinned author,
fixed timestamps, `core.autocrlf=false`) and the substitution contract for the
printed command templates. The expected repair is independently understandable
from source — one equality assertion `tier(20) == "gold"` — and requires no
production edit.

## When

`cargo xtask check-fixture-contracts` validates this fixture: the manifest
parses at `blind_journey_installed_rust_fixture.v1`, every recorded snapshot
digest matches the bytes on disk, every recorded git identity is well-formed,
the selected repair names exactly one edit target inside the expected edit
cage, and every scripted scenario id named by the manifest exists in the
RIPR-SPEC-0205 executor corpus. `cargo xtask blind-journey-execute` then runs
the ten scripted installed-Rust scenarios through the real executor; each
emitted packet is stamped and must be accepted by the live RIPR-SPEC-0200
validator, so a hand-edited expectation cannot make a wrong journey emit.

The positive row follows only public docs/help and literal product-emitted
commands — `ripr doctor`, `ripr pilot`, one canonical item selection, the
printed `ripr agent repair --phase before`, one bounded test edit, the exact
printed focused `cargo test --test tier_boundary` between the phases, the
printed `--phase after`, and the printed receipt route — recorded from a
foreign launch directory against the selected root, with the ordinary Cargo
build output admitted under the accepted `ignored_build_output` cage policy.

## Then

- The positive row and the second-root repeat derive `passed_blind_journey`
  and share one portable identity (concrete root spelling is retained
  evidence, never identity).
- The no-ignore-rule repository stops before any edit with the precise
  printed precondition; the skipped-verification and deleted-artifact rows
  keep verification `not_run` / honest-limitation respectively; the
  non-artifact `target/notes.rs` edit and the production edit each derive
  `unsafe_or_wrong_edit`; the failing focused test derives
  `verification_failure_visible` while static movement stays a separate axis;
  the planted workspace binary derives `candidate_identity_failure`; the
  interrupted receipt write recovers through a clean rerun and never yields a
  half-authoritative receipt.
- Every negative control keeps its own non-positive terminal result; no
  aggregate converts them into success.

## Must Not

- Do not run a candidate, launch a process, or decide a release verdict.
- Do not record any production edit as admitted: `src/lib.rs`, `Cargo.toml`
  and `target/notes.rs` stay forbidden in every positive row.
- Do not let static movement, project verification, or receipt issuance
  imply one another; the evidence axes stay separate in every scenario.
- Keep the pinned static vocabulary only; this fixture records no dynamic or
  mutation-testing verdicts and no coverage claims.
- Do not count any scripted executor success as installed usefulness, blind
  qualification, candidate selection or parent acceptance.
