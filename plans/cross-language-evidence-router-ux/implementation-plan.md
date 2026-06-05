# Cross-Language Evidence Router UX Plan

Status: active; first three implementation work items complete
Owner: language-adapter-swarm
Plan artifact: RIPR-PLAN-0063
Linked proposal: n/a
Linked specs: RIPR-SPEC-0027, RIPR-SPEC-0062, RIPR-SPEC-0063
Linked ADRs: n/a
Active goal: `cross-language-evidence-router-ux` in `.ripr/goals/active.toml`

## Current State

The 0.8.1 TypeScript/Bun preview patch has closed the calibrated Bun Blob /
ArrayBuffer proof path. The repo has TypeScript discriminator facts,
stable-byte oracle classification, bridge hints, cross-language grip projection,
TypeScript placement ranking, oracle graph corpus rows, witness routing,
bridge-unknown limitation handling, route-quality and calibration reports,
dogfood receipts, a runbook, and a closeout boundary.

The patch proof packet for that closed 0.8.1 path is recorded in
`docs/handoffs/2026-06-05-0.8.1-typescript-bun-preview-patch-proof.md`.
`cargo xtask bun-ub-preview-summary` now writes the compact advisory JSON and
Markdown summary for the current Bun UB route states. The TypeScript preview
card now projects a nested Bun cross-language advisory packet for configured
routes. The next implementation slice is `output/stable-byte-proof-mode`.

This plan turns that bounded path into a repeatable mixed TypeScript plus Rust
operating loop. It does not reopen generic TypeScript support and it does not
promote preview evidence.

Hard boundaries:

- no `tsc`, `tsserver`, Bun, Jest, Vitest, Miri, mutation, provider, generated
  test, or autonomous source-edit dependency;
- no public repair packet, default gate, badge, baseline, RIPR Zero,
  support-tier, release, publish, tag, signing, marketplace, or install-doc
  authority from this plan;
- no `ripr check --profile` flag; operators use `ripr.toml`, `ripr doctor`, and
  `ripr check`;
- bridge-unknown remains an inspect-bridge route, not a test-edit route;
- missing TypeScript placement is suggested only when typed placement evidence
  exists.

## Work Item: release/typescript-bun-preview-patch-proof

Status: done
Linked proposal: n/a
Linked spec: RIPR-SPEC-0062, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: output/bun-ub-preview-summary
Blocked by: n/a

### Goal

Turn the closed 0.8.1 TypeScript/Bun preview patch into a patch-cut proof
packet without performing a release.

### Production Delta

Add a documentation or report artifact that records the current proof commands,
observed results, included behavior, preview/advisory authority, and explicit
non-claims.

### Non-Goals

No tag, publish, release note, support-tier promotion, gate, badge, generated
test, runtime Bun execution, or source release authority.

### Acceptance

- The proof packet records the Bun Blob / ArrayBuffer calibrated states.
- The packet records `copy_to_unshared`, MarkdownObject, and FFI panic-boundary
  follow-up status from existing receipts.
- The packet says all TypeScript/Bun evidence remains preview/advisory.
- The packet says `repair_packet_ready = false` for cross-language preview rows.
- Validation results are recorded as pass, fail, or not run.

### Proof Commands

```bash
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo test -p xtask typescript_bun_ub_calibration -- --test-threads=1
cargo test -p xtask bun_ub_calibration -- --test-threads=1
cargo test -p xtask dogfood_bun_ub_cross_language -- --test-threads=1
cargo test -p ripr typescript_preview_card_projects_bun_cross_language_grip -- --test-threads=1
cargo xtask bun-ub-calibration
cargo xtask dogfood
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the proof packet. No runtime state or generated behavior changes need
rollback.

### Notes

This item is release-prep evidence only. Actual release, tag, publish, signing,
or install-doc updates require separate explicit release authorization.

## Work Item: output/bun-ub-preview-summary

Status: done
Linked proposal: n/a
Linked spec: RIPR-SPEC-0063
Linked ADR: n/a
Blocks: agent/bun-cross-language-advisory-packet
Blocked by: release/typescript-bun-preview-patch-proof

### Goal

Make the Bun UB preview readable on the first screen by summarizing calibrated
routes and current cross-language states.

### Production Delta

Add a compact JSON and Markdown summary built from existing route-quality,
calibration, and dogfood data.

### Non-Goals

No analyzer behavior, new bridge inference, public repair packet, generated
test, source edit, gate, badge, or support-tier promotion.

### Acceptance

- The summary lists calibrated routes and counts for
  `rust_ungripped_ts_discriminated`,
  `rust_ungripped_ts_missing_discriminator`, `bridge_unknown`,
  `ts_mention_not_observer`, and named static limitations.
- The summary includes `authority = preview_advisory_only`.
- The summary includes public packet exclusions and confirms
  `repair_packet_ready = false`.
- The summary can be read without raw preview-card JSON.

### Proof Commands

```bash
cargo test -p xtask bun_ub_preview_summary -- --test-threads=1
cargo xtask bun-ub-calibration
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the summary renderer, tests, and any generated golden/report updates.

### Notes

This is output/report work only. It adds `cargo xtask bun-ub-preview-summary`
and report-packet index discovery for
`target/ripr/reports/bun-ub-preview-summary.{json,md}` without widening the
analyzer.

## Work Item: agent/bun-cross-language-advisory-packet

Status: done
Linked proposal: n/a
Linked spec: RIPR-SPEC-0058, RIPR-SPEC-0061, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: output/stable-byte-proof-mode
Blocked by: output/bun-ub-preview-summary

### Goal

Emit a bounded packet that Claude Code or another coding agent can use to act
narrowly, or stop when evidence is incomplete.

### Production Delta

Add JSON and human output for configured Bun cross-language advisory packets:
missing-discriminator rows can describe the TypeScript file and shape; unknown
bridge rows must describe the missing bridge leg and stop condition.

### Non-Goals

No public repair packet, autonomous source edit, generated test, verify command,
receipt command, allowed edit surface, support-tier promotion, or provider
integration.

### Acceptance

- Packets include state, Rust seam, TypeScript file when eligible, missing
  discriminators, suggested shape, bridge confidence, missing graph legs,
  authority boundary, `must_not_change`, stop condition, and raw evidence refs.
- `bridge_unknown` emits an inspect-bridge packet, not a test-edit packet.
- `repair_packet_ready` remains `false`.
- Missing placement is emitted only when placement evidence exists.

### Proof Commands

```bash
cargo test -p ripr bun_cross_language_agent_packet -- --test-threads=1
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert packet fields, renderers, tests, and output contracts for this packet.

### Notes

This packet is an advisory task-shaping surface. It must remain distinct from
RIPR-SPEC-0061 public actionability. It is projected through JSON and human
output only, keeps `repair_packet_ready = false`, names `bridge_unknown` as an
inspect-bridge stop condition, and only emits TypeScript placement when
placement evidence exists.

## Work Item: output/stable-byte-proof-mode

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0063
Linked ADR: n/a
Blocks: fixtures/bun-node-fs-scalar-write-profile, fixtures/bun-write-helper-gated-profile
Blocked by: agent/bun-cross-language-advisory-packet

### Goal

Project the proof mode a Bun stable-byte reviewer should use without claiming
runtime proof.

### Production Delta

Add advisory `proof_mode` projection for configured cross-language preview rows:
`observable_red_green`, `mutation_plus_miri`, `helper_gated`,
`bridge_unknown`, and `static_limitation`.

### Non-Goals

No runtime execution, Miri execution, mutation execution, generated tests,
source edits, UB proof, gate, badge, or support-tier promotion.

### Acceptance

- Each calibrated row has one proof mode and a short reason.
- `bridge_unknown` proof mode is used when the binding or FFI leg is missing.
- Named static limitations do not become test-edit tasks.
- Proof mode appears in JSON and human report output.

### Proof Commands

```bash
cargo test -p ripr stable_byte_proof_mode -- --test-threads=1
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert proof-mode fields, renderers, tests, and output contracts.

### Notes

This item should not decide whether a particular Bun bug is observable. It
records the calibrated proof strategy encoded by the fixture or corpus row.

## Work Item: fixtures/bun-node-fs-scalar-write-profile

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0062, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: analysis/configured-bridge-inventory, dogfood/live-bun-stable-byte-receipts
Blocked by: output/stable-byte-proof-mode

### Goal

Add the first manifest-only non-Blob stable-byte profile for a node:fs scalar
write route.

### Production Delta

Add fixture or corpus rows that define the Rust seam, TypeScript witness path,
discriminator vocabulary, proof mode, bridge requirement, expected limitations,
and non-claims before analyzer behavior changes.

### Non-Goals

No analyzer behavior, bridge inference, runtime Bun execution, generated tests,
source edits, public repair packet, gate, badge, or support-tier promotion.

### Acceptance

- The profile is marked `manifest_only`.
- The suggested witness path is recorded as
  `test/js/node/fs/fs.test.ts` only when the row has typed placement evidence.
- `proof_mode = observable_red_green`.
- Missing bridge or oracle evidence remains a named limitation.
- The fixture contract rejects actionability fields.

### Proof Commands

```bash
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the manifest rows, validator expectations, and docs references.

### Notes

This PR establishes the target plate for a future analyzer route. It should not
try to solve node:fs reachability.

## Work Item: fixtures/bun-write-helper-gated-profile

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0062, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: analysis/configured-bridge-inventory, dogfood/live-bun-stable-byte-receipts
Blocked by: output/stable-byte-proof-mode

### Goal

Model a Bun.write stable-byte route that is real but blocked by a helper or
upstream primitive.

### Production Delta

Add fixture or corpus rows for a helper-gated profile with explicit unblock
condition, proof mode, bridge status, and non-actionability.

### Non-Goals

No analyzer behavior, runtime Bun execution, generated tests, source edits,
public repair packet, gate, badge, support-tier promotion, or speculative test
placement.

### Acceptance

- The row records `proof_mode = helper_gated` or `bridge_unknown`.
- The unblock condition is explicit.
- No suggested test file is emitted unless bridge and observer evidence are
  credited.
- `repair_packet_ready = false`.
- Public repair packet fields remain absent.

### Proof Commands

```bash
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the manifest rows, validator expectations, and docs references.

### Notes

This item turns "blocked" into a named route rather than a silent omission.

## Work Item: analysis/configured-bridge-inventory

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0062, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: dogfood/live-bun-stable-byte-receipts
Blocked by: fixtures/bun-node-fs-scalar-write-profile, fixtures/bun-write-helper-gated-profile

### Goal

Show configured bridge profiles and missing future surfaces without building a
full Bun binding graph.

### Production Delta

Add a report-only configured bridge inventory that lists credited configured
bridges and future or missing surfaces.

### Non-Goals

No new reachability inference, no analyzer behavior, no public repair packets,
no placement from missing inventory rows, no gates, badges, or support-tier
promotion.

### Acceptance

- Configured bridge profiles list Blob ArrayBuffer, copy_to_unshared, and
  MarkdownObject routes.
- Future or missing surfaces list node:fs scalar write, Bun.write, S3, or other
  configured placeholders only when backed by corpus metadata.
- The report distinguishes configured, manifest-only, bridge-unknown, and
  future surfaces.
- Missing inventory entries do not become repair tasks.

### Proof Commands

```bash
cargo test -p xtask configured_bridge_inventory -- --test-threads=1
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the inventory report, tests, and docs references.

### Notes

This item makes configuration debt visible while deliberately avoiding a full
binding graph.

## Work Item: dogfood/live-bun-stable-byte-receipts

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0063
Linked ADR: n/a
Blocks: docs/bun-ub-first-run-polish, docs/post-081-support-decision
Blocked by: analysis/configured-bridge-inventory

### Goal

Prove that the cross-language router reduces real Bun review work, not just
fixture drift.

### Production Delta

Add dogfood receipts for live or live-shaped Bun stable-byte cases with tool
verdict, manual verdict, route-quality state, placement verdict, proof mode,
and operator note.

### Non-Goals

No runtime Bun execution, generated tests, source edits, provider calls, public
repair packets, gates, badges, support-tier promotion, or UB proof claim.

### Acceptance

- Receipts include at least one credited witness, one missing discriminator,
  one mention-only rejection, one bridge-unknown case, and one named static
  limitation.
- Receipts include manual verdicts and what work the output saved or avoided.
- Wrong-language placement regressions are absent.
- The report repeats preview/advisory authority.

### Proof Commands

```bash
cargo test -p xtask live_bun_stable_byte_dogfood -- --test-threads=1
cargo xtask dogfood
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the dogfood receipts, validators, and report updates.

### Notes

Dogfood should record disagreement honestly. A useful disagreement is evidence
for the next profile or bridge slice.

## Work Item: docs/bun-ub-first-run-polish

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0063
Linked ADR: n/a
Blocks: docs/post-081-support-decision
Blocked by: dogfood/live-bun-stable-byte-receipts

### Goal

Make the Bun UB TypeScript preview loop copy-pasteable for an operator who did
not watch the lane land.

### Production Delta

Update the runbook and docs index for the actual command surface, report
artifacts, failure modes, and next actions.

### Non-Goals

No new command-line flag, analyzer behavior, generated tests, source edits,
runtime execution, public repair packet, gate, badge, or support-tier
promotion.

### Acceptance

- The runbook starts from `ripr.toml`, `ripr doctor`, and `ripr check`.
- Failure modes have next actions: missing config, missing discriminator,
  mention-only, bridge-unknown, and named static limitation.
- The compact summary, agent packet, proof mode, bridge inventory, and dogfood
  receipts are linked when they exist.
- Known limits are explicit.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the runbook and docs index changes.

### Notes

Do not document commands before they exist. Planned commands belong in this
plan, not in the operator runbook.

## Work Item: docs/post-081-support-decision

Status: ready
Linked proposal: n/a
Linked spec: RIPR-SPEC-0030, RIPR-SPEC-0063
Linked ADR: n/a
Blocks: n/a
Blocked by: dogfood/live-bun-stable-byte-receipts, docs/bun-ub-first-run-polish

### Goal

Decide whether the post-0.8.1 TypeScript/Bun claim stays preview/advisory or
has enough evidence to request a later scoped promotion.

### Production Delta

Add a support-decision note or closeout that lists evidence, gaps, non-claims,
and the next required proof for any stronger claim.

### Non-Goals

No support-tier promotion without a separate accepted promotion contract; no
release, publish, tag, signing, marketplace, install-doc, gate, badge, baseline,
RIPR Zero, runtime execution, generated test, or source edit authority.

### Acceptance

- The likely current claim remains:

  ```text
  TypeScript/JavaScript remain opt-in preview.

  For calibrated Bun stable-byte cases, ripr can provide advisory
  cross-language evidence that distinguishes TS-discriminated,
  missing-discriminator, mention-only, bridge-unknown, and named-limitation
  states.
  ```

- The note explicitly rejects claims of TypeScript stable support, Bun UB proof,
  runtime execution, generated tests, default gates, public repair packets, and
  full Bun binding graph coverage.
- Any promotion proposal lists missing proof and requires a separate PR.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the support-decision note and index updates.

### Notes

This is a decision point, not a promotion by itself.
