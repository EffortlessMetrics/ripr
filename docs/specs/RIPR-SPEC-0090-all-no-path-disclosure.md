# RIPR-SPEC-0090: All-No-Path Disclosure

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1185 — Human output is opaque when every finding is no-path/unknown

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds an advisory disclosure line to human output
  when a `ripr check` run produces findings but every finding is
  `no_static_path`, `infection_unknown`, `propagation_unknown`, or
  `static_unknown` (zero `exposed`/`weakly_exposed`/`reachable_unrevealed`).
  It does not promote any feature to a higher support tier, does not change
  pass/fail authority, and does not alter what the analyzer classifies.
- The disclosure is additive human output only. JSON shape is not modified.
  No schema version bump. Claim boundaries remain governed by the canonical
  ledger in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When `ripr check` produces findings but every finding is `no_static_path` or
an unknown class, the human output shows N individual "no path" or "unknown"
finding sections with no summary-level commentary. A user scanning the output
cannot easily tell that the tool found NO exposable evidence for any changed
expression — they see a wall of individual no-path entries and must read each
one to understand the aggregate situation.

This is an honesty gap at the aggregate level: the individual finding sections
correctly say "no static path found," but there is no single sentence that
says "and this is true for ALL N changed expressions in this diff."

The fix is one advisory line at the aggregate level, emitted only when the
all-no-path condition holds. If ANY finding is exposed/weakly_exposed/
reachable_unrevealed, the per-finding output already carries the signal and
the aggregate disclosure must not fire (to avoid over-claiming).

## Behavior

### Trigger conditions

The disclosure fires when ALL of the following are true:

1. `summary.findings > 0` (there are findings — zero findings is a different
   case handled by the no-probes message and RIPR-SPEC-0083).
2. `summary.exposed == 0` AND `summary.weakly_exposed == 0` AND
   `summary.reachable_unrevealed == 0` — no finding is exposed or weakly
   reached.
3. `(summary.no_static_path + summary.infection_unknown +
   summary.propagation_unknown + summary.static_unknown) == summary.findings`
   — every finding is in a no-path or unknown class.

The disclosure does NOT fire when:

- There are zero findings (the no-probes message handles that case).
- Any finding is `exposed`, `weakly_exposed`, or `reachable_unrevealed` — the
  per-finding output already carries the signal.

### Human output

When the condition holds, the following advisory note is appended after the
findings loop, before any preview-language advisories (RIPR-SPEC-0082):

```
Note: ripr found no static test path for any of the N changed expression(s) in this diff. This is not a coverage assessment — it means no co-located test was found that statically discriminates the changed behavior.
```

Where N is the total no-path/unknown count
(`no_static_path + infection_unknown + propagation_unknown + static_unknown`).

The note does not change the exit code or pass/fail status.

### JSON output

No change. The disclosure is human-only. No new fields, no schema version
bump. The JSON `check.json` shape is unchanged.

### Non-claims

- This spec does NOT change the exit code or gate authority.
- The disclosure is NOT a coverage assessment. It is a pure absence-of-path
  statement: no co-located test was found that statically discriminates the
  changed behavior.
- This spec does NOT imply the code is safe, tests pass, or behavior is
  correct.
- This spec does NOT change what the analyzer classifies.

## Non-Goals

- Disclosure in JSON, SARIF, GitHub, badge, or repo-exposure output formats.
- Changing the per-finding output for no-path/unknown findings.
- Runtime mutation testing, coverage measurement, or correctness claims.
- Aggregating findings into a single entry or removing per-finding detail.

## Required Evidence

- `CheckOutput.summary` fields: `findings`, `exposed`, `weakly_exposed`,
  `reachable_unrevealed`, `no_static_path`, `infection_unknown`,
  `propagation_unknown`, `static_unknown`. These fields already exist in
  `crates/ripr/src/domain/summary.rs` — no new fields needed.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| `summary.findings` | yes | Gate: only fire when findings > 0 |
| `summary.exposed`, `summary.weakly_exposed`, `summary.reachable_unrevealed` | yes | Suppress disclosure when any finding is reached |
| `summary.no_static_path + .infection_unknown + .propagation_unknown + .static_unknown` | yes | Count for the disclosure message |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| Human text `Note:` line | None | Additive; absent when condition not met; does not change exit code |
| JSON `check.json` | None | Unchanged; no new fields; no schema bump |

## Acceptance Examples

1. **Primary case**: diff has 1 changed expression, no tests — human output
   includes `Note: ripr found no static test path for any of the 1 changed
   expression(s) in this diff.`
2. **Multi-finding all-no-path**: diff has 5 changed expressions, all
   no-path — disclosure shows count 5.
3. **Mixed case (do NOT emit)**: diff has 3 findings, 1 is `exposed`, 2 are
   `no_static_path` — disclosure does NOT appear (per-finding output carries
   the signal).
4. **Zero findings (do NOT emit)**: diff analyzed, 0 findings — disclosure
   does NOT appear (the no-probes message handles that case).
5. **JSON unchanged**: `ripr check --json` on an all-no-path diff — JSON
   output shape is identical to the existing no-path pattern; no new fields;
   no schema version bump.

## Test Mapping

- `crates/ripr/src/output/human.rs::tests::render_emits_all_no_path_disclosure_when_all_findings_are_no_path`
- `crates/ripr/src/output/human.rs::tests::render_emits_all_no_path_disclosure_for_infection_unknown_findings`
- `crates/ripr/src/output/human.rs::tests::render_omits_all_no_path_disclosure_when_exposed_finding_exists`
- `crates/ripr/src/output/human.rs::tests::render_omits_all_no_path_disclosure_when_weakly_exposed_finding_exists`
- `crates/ripr/src/output/human.rs::tests::render_omits_all_no_path_disclosure_when_zero_findings`
- `crates/ripr/src/output/human.rs::tests::render_all_no_path_disclosure_uses_finding_count_not_probe_count`
- `crates/ripr/src/output/human.rs::tests::render_all_no_path_disclosure_uses_conservative_static_language`
- `fixtures/all_no_path_disclosure` (golden fixture)

## Implementation Mapping

- `crates/ripr/src/output/human.rs` — new function
  `render_all_no_path_disclosure(out, output)`, called in `render_with_config`
  after the findings loop and before `render_preview_language_advisories`.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr` — all pass including the disclosure tests.
- `cargo clippy -p ripr -p xtask --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-output-contracts` pass.
- `cargo xtask goldens check` pass.
- `cargo xtask fixtures all_no_path_disclosure` pass.
- Behavioral repro (a): `ripr check --diff fixtures/all_no_path_disclosure/diff.patch`
  on an all-no-path diff → human output includes
  `Note: ripr found no static test path for any of the 1 changed expression(s)`
  and JSON output is unchanged.
- Behavioral repro (b): `ripr check --diff crates/ripr/examples/sample/example.diff`
  on a diff with exposed findings → human output does NOT include the
  all-no-path disclosure.

## Metrics

- Gate: all disclosure acceptance tests pass, including the golden fixture.
- Promote to accepted when real-world usage confirms the all-no-path wall-of-N
  result is no longer opaque to users.
