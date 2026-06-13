# RIPR-SPEC-0087: TypeScript preview→actionable repair-packet contract (0085 §PR7)

Status: proposed

Owner: product / swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- ADR 0008 — Rust-native `oxc` parser adoption for the TypeScript adapter

Linked plan:

- `docs/IMPLEMENTATION_CAMPAIGNS.md` — TypeScript evidence-adapter wave

Linked specs:

- RIPR-SPEC-0085 — TypeScript Evidence Adapter Contract. **This spec is PR 7
  of the 0085 campaign.** It depends on the producers landed by §PR4
  (named-limitation taxonomy / `static_limit.rs`), §PR5 (oracle metadata /
  `oracle.rs`, `types.rs`), and §PR6 (ownership-resolution limitations /
  `related_tests.rs`). It is the *only* slice of 0085 that flips a TypeScript
  finding from `repair_packet_ready: false` to `true`.

Linked PRs:

- #1151 — behavior-preserving decomposition of `typescript.rs`

Support-tier impact:

- **No tier change. TypeScript stays `preview`.** Flipping
  `repair_packet_ready: true` for a *complete-contract* finding does not
  promote the language. The flipped finding still carries
  `authority_boundary: "preview_advisory_only"` and
  `language_status: "preview"`; it is *delegatable* but never *gate
  authority*. Promotion to a higher support tier still requires dogfood
  evidence plus a TypeScript route-quality slice, governed by
  [support tiers](../status/SUPPORT_TIERS.md).
- The contract is fail-closed by construction, inheriting 0085's invariant:
  an analysis produces exactly one of a **complete bounded repair packet** or
  a **named limitation**. PR 7 only adds the final transition for the
  complete case. When in doubt, the rule is **UNDER-emit (stay preview) over
  OVER-emit (flip actionable)**.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`, the spec index
  (`docs/specs/README.md`), and `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers. No public-API symbol additions; `repair_packet_ready` already
  exists in the `preview_actionability` JSON shape and is documented in
  `docs/OUTPUT_SCHEMA.md`.
- This is an **additive, documented behavior change** to an existing JSON
  field's *value distribution*, not a schema change. `schema_version` does
  **not** bump (boolean field value flips for one new condition; no field is
  added or removed from the wire shape).

---

## Problem

Today every TypeScript/JavaScript preview finding is emitted with
`repair_packet_ready: false`. The producers landed in §PR4–§PR6 already
extract the evidence facts the Rust agent-packet contract requires — related
test owner/file, oracle shape, a framework-aware `verify_command`, raw
evidence refs, and confidence — but the value of `repair_packet_ready` is
**hardcoded `false`** at `crates/ripr/src/output/preview_actionability.rs:63`.
There is no code path that flips it true.

The result is that even a TypeScript finding that has *everything the Rust
packet needs* is presented as advisory-only and cannot be safely delegated.
Conversely, the absence of a flip is currently the *only* thing protecting us
from over-emitting: the moment a flip exists, every over-emit failure mode in
the §PR7 adversary audit becomes live.

PR 7 must close exactly one gap: allow a TypeScript finding whose repair
packet is **provably complete, evidence-backed, and non-dynamic** to flip
`repair_packet_ready: true` — and *nothing else* — by **reusing the same
completeness validator the Rust packet uses**, never a parallel TypeScript
path.

### Non-goals

- No new language promotion. Preview stays preview.
- No new oracle, owner, framework, or discriminator producer beyond the four
  packet-completion fields named in §3. PR 7 is a *projection + gate* slice,
  not an evidence-extraction slice.
- No mutation execution, no runtime confirmation, no coverage.
- No second validator. If the Rust validator and the TypeScript flip ever
  disagree about completeness, that is a bug in this spec's implementation.
- No flip for static-limit, already-observed, missing-context,
  ambiguous-related-test, or missing-target-shape findings. Only the
  `incomplete_repair_packet` category is eligible to *become*
  `complete_repair_packet`.

---

## Behavior

For a TypeScript/JavaScript preview finding, `repair_packet_ready` flips from
`false` to `true` if and only if:

1. All G-A through G-F preconditions hold (see §1.2).
2. A `GapRecord` projected from the finding passes the shared
   `validate_agent_gap_record_packet` validator (see §1.1).

When flipped:

- `actionability_category` becomes `"complete_repair_packet"`.
- `gap_state` becomes `"actionable"`.
- `missing_actionability_fields` is cleared to `[]`.
- `authority_boundary` stays `"preview_advisory_only"` (no language promotion).

In every other case — static limit, dynamic oracle, heuristic relation, missing
context, no verify command, cross-language bridge — the finding stays preview
with a **named** reason surfaced in `actionability_category` or
`why_not_actionable`. There is no silent non-actionable path.

## Required Evidence

- A TypeScript finding that satisfies G-A–G-F: category
  `incomplete_repair_packet`, non-dynamic concrete oracle,
  import-aware/owner-call related test, discoverable `verify_command` from
  `package.json`, non-empty `missing_discriminators`.
- A `GapRecord` projected by `typescript_gap_record_for` carrying all six
  fields validated by `validate_agent_gap_record_packet`: `projection_eligibility`,
  `repair_route`, `verification_commands`, `repairability`, `allowed_edit_surface`,
  `receipt_command`.
- A `receipt_command` of the form `ripr outcome … target/ripr/receipts/<gap_id>.json`
  (no external provider, no fabricated command).
- A `canonical_gap_id` of the form `gap:typescript:<family>:<fp8>`, with path
  normalized `\`→`/`.
- Behavioral fixtures for the complete case (flips) and one per each of the
  F1–F16 failure families (stays non-actionable, reason named).

## Non-Goals

- No new language promotion. Preview stays `preview_advisory_only`.
- No new oracle, owner, framework, or discriminator producer beyond the four
  packet-completion fields named in §3. This is a projection + gate slice,
  not an evidence-extraction slice.
- No mutation execution, no runtime confirmation, no coverage.
- No second validator. The flip calls `validate_agent_gap_record_packet`
  exclusively; any parallel TypeScript completeness check is a bug.
- No flip for static-limit, already-observed, missing-context,
  ambiguous-related-test, or missing-target-shape findings. Only
  `incomplete_repair_packet` is eligible to become `complete_repair_packet`.

---

## 1. The actionability gate (exact flip condition)

### 1.1 Single authority: reuse `validate_agent_gap_record_packet`

A TypeScript finding flips `repair_packet_ready: true` **iff** a `GapRecord`
projected from that finding passes the *existing, shared* Rust completeness
validator:

```
crates/ripr/src/output/agent_seam_packets.rs::validate_agent_gap_record_packet
```

PR 7 **must not** introduce a parallel TypeScript validator, mirror, or
inline re-implementation. The TypeScript path builds a `GapRecord` from
preview evidence and calls the *same* `validate_agent_gap_record_packet`. If
it returns `Ok(())`, the finding is complete; otherwise it stays preview.
This is the load-bearing architectural constraint of the spec.

The shared validator enforces (verbatim, `agent_seam_packets.rs:839–871`):

1. `record.projection_eligibility["agent_packet"]` present **and**
   `projection_eligible(record, "agent_packet") == true`.
2. `record.repair_route` is `Some(_)`.
3. `record.verification_commands` is non-empty.
4. `record.repairability == "repairable"` **OR**
   `route.route_kind == "InspectStaticLimit"`.
5. `allowed_edit_surface_for_gap_route(route)` returns a non-empty `Vec`.
6. `record.receipt_command` is `Some(non-empty)`.

### 1.2 TypeScript-specific preconditions BEFORE the validator (fail-closed)

The validator is necessary but **not sufficient** for preview safety. Before
a TypeScript finding is even projected into a `GapRecord`, ALL of the
following must hold, or the finding stays preview with its current
non-actionable category from `typescript_actionability_for`
(`crates/ripr/src/analysis/language/typescript/actionability.rs`):

- **G-A. Category is `incomplete_repair_packet`.** Only the terminal
  `incomplete_repair_packet` branch (actionability.rs:152) is eligible. Every
  earlier branch (`static_limitation`, `strong_oracle_observed`,
  `missing_context`, `ambiguous_related_test`, `missing_target_shape`) stays
  preview by construction — those branches assert evidence is *missing*, and
  PR 7 does not touch them.
- **G-B. `language_status == Preview` and `language ∈ {TypeScript,
  JavaScript}`.** Inherited from `preview_actionability_for` (the function is
  a no-op for other languages).
- **G-C. Non-dynamic oracle.** The borrowed assertion's
  `expected_value_or_variant` is `Some(concrete literal)` and
  `has_dynamic_matcher_arg == false` (§PR5 oracle metadata, `types.rs`,
  `oracle.rs:575`). A finding whose only oracle is dynamic
  (`expect(x).toBe(someVar)`, snapshot, `toBeDefined`) is **not** eligible —
  it routes to `missing_target_shape`, which stays preview.
- **G-D. Oracle-eligible, token-aware relation.**
  `has_oracle_eligible_relation == true` — the related test is linked by
  owner-call / import-aware / receiver-aware evidence, not heuristic-only.
  Heuristic-only relations route to `ambiguous_related_test` (preview).
- **G-E. Non-empty `missing_discriminators` resolved.** A safe target
  discriminator/observer shape is named (the `missing_target_shape` guard at
  actionability.rs:128 has already passed).
- **G-F. No unresolved cross-language oracle visibility / test target.** Bun
  cross-language bridge verdicts that emit
  `route_cross_language_oracle_visibility_limitation`
  (`typescript/bun_bridge.rs`) are *named limitations*; such findings stay
  preview.

If G-A through G-F all hold, the finding has *exactly* the evidence the Rust
packet requires except the four projection fields named in §3. PR 7 produces
those four fields, builds the `GapRecord`, and calls the shared validator.

### 1.3 Result of the gate

- Validator `Ok(())` **and** G-A–G-F hold ⇒ set
  `repair_packet_ready: true`, set `actionability_category:
  "complete_repair_packet"`, set `gap_state: "actionable"`, clear
  `missing_actionability_fields` to `[]`. `authority_boundary` stays
  `"preview_advisory_only"`.
- Validator `Err(reason)` ⇒ finding stays preview. The validator's `reason`
  string is surfaced in `why_not_actionable` so the missing field is named.
  `repair_packet_ready` stays `false`. `actionability_category` stays
  `incomplete_repair_packet` (or downgrades per the named missing field).

There is no third outcome.

---

## 2. Where the flip is implemented

`crates/ripr/src/output/preview_actionability.rs` currently hardcodes
`repair_packet_ready: false` at line 63. PR 7 replaces that literal with a
computed value driven by §1:

```rust
// preview_actionability.rs (PR 7)
let packet = typescript_gap_record_for(finding); // §3 projection, returns Option<GapRecord>
let repair_packet_ready = packet
    .as_ref()
    .is_some_and(|record| validate_agent_gap_record_packet(record).is_ok());
```

`typescript_gap_record_for` returns `None` whenever any §1.2 precondition
(G-A–G-F) fails, so a `None` packet ⇒ `repair_packet_ready: false`
automatically. This keeps the fail-closed default literally encoded: the flip
is opt-in per finding, the false case requires no positive proof.

The projection lives in the TypeScript analysis module
(`crates/ripr/src/analysis/language/typescript/`) and is consumed by
`output/preview_actionability.rs`; the *validator* it calls is the existing
shared function in `output/agent_seam_packets.rs`. No new module.

---

## 3. Required fields: existing producers vs. new producers

The Rust gap-record packet requires the fields below. PR 7 must wire each to
a **real** producer. No fabricated fields. Status is grounded in the §PR2–§PR6
producer audit.

### 3.1 Already satisfied (reuse, do not re-produce)

| Packet field | Producer (existing) | Evidence fact |
| --- | --- | --- |
| `repair_route.target_file` / `related_test` | `typescript/related_tests.rs` | `TypeScriptTest.file`, ranked by `sort_related_candidates()` |
| `repair_route.assertion_shape` / observer shape | `typescript/oracle.rs` | `TypeScriptAssertion` (matcher, oracle_kind, oracle_strength, observed_expression, expected_value_or_variant) |
| `verification_commands` / `verify_command` | `typescript/package.rs` | `verify_command_for_discovery()` (jest/vitest/bun/node --test, runner-aware) emitted as `typescript_verify_command` |
| `evidence_ids` / `raw_evidence_refs` | `typescript/actionability.rs` | `typescript_raw_evidence_ref()` (file, line, kind, source_id, owner) |
| `confidence` | `typescript/oracle.rs`, `package.rs` | `oracle_confidence` + `typescript_package_confidence` |
| `anchor.{file,line,owner}` | `typescript/related_tests.rs`, owner inference | probe location + `TypeScriptOwner.name` |

### 3.2 NEW producers PR 7 must add (each REAL, none fabricated)

| Packet field | Why missing today | Producer PR 7 adds | Real basis (no fabrication) |
| --- | --- | --- | --- |
| `receipt_command` | validator rejects on empty (§1.1 cond. 6); actionability.rs lists it missing at lines 96/119/142/168 | `typescript_receipt_command(finding)` deriving a `ripr outcome --before … --after … --out target/ripr/receipts/<canonical_gap_id>.targeted-test-outcome.json` | Mirrors the Rust receipt shape (`render_agent_gap_record_packet_json`); the `<gap_id>` slug is the canonical gap id from this same projection — no new external command, no provider call |
| `must_not_change` | actionability.rs lists missing at line 168 | `typescript_must_not_change(language_status)` returning the **same** constant boundary list the Rust packet uses, **including** the preview-language clause | Reuses `gap_record_packet_do_not_do` semantics; for preview it MUST include "Do not treat preview-language evidence as gate authority." |
| `allowed_edit_surface` | actionability.rs lists missing at lines 97/120/143/169 | NOT a new TS producer — derived by the **shared** `allowed_edit_surface_for_gap_route(route)` from `repair_route.target_file`/`related_test` | Reuses the existing shared function (agent_seam_packets.rs:906); PR 7 only ensures the projected `repair_route` carries a tokenizable test file so the shared fn returns non-empty |
| `canonical_gap_id` | no TS producer; actionability.rs lists missing at line 162 | `typescript_canonical_gap_id(finding)` = content-addressed `gap:typescript:<probe_family>:<fp8>` derived from the existing content-addressed finding id | MUST normalize `\`→`/` before hashing (per content-addressed-ids learning) so Windows-blessed goldens match Linux CI; reuses the finding's existing SHA-256 fp8, no new hash domain |

`repair_kind` / `route.route_kind`: derived by the shared `task_for_gap_route`
mapping from the projected `repair_route.route_kind`. PR 7 sets
`route_kind` from the resolved discriminator (G-E): a missing
boundary/value/error assertion ⇒ `AddBoundaryAssertion` /
`AddValueAssertion` / `AddErrorDiscriminator` consistent with the Rust route
taxonomy. No new TS-only route kind is invented.

`repairability` must be set to `"repairable"` only when G-C–G-E hold (a
concrete assertion can be written); otherwise the projection returns `None`.

---

## 4. Fail-closed invariants (from the §PR7 over-emit audit)

For **every** over-emit failure mode, the finding stays preview or becomes a
**named** limitation — never silently actionable. Each invariant below is a
guard that MUST hold for the flip; failing it ⇒ `repair_packet_ready: false`.

| # | Failure mode (over-emit) | Fail-closed invariant | Enforcement |
| --- | --- | --- | --- |
| F1 | Dynamic/non-literal oracle borrowed as proof | `expected_value_or_variant.is_some() && !has_dynamic_matcher_arg` | G-C; else `missing_target_shape` (preview) |
| F2 | Snapshot / `toMatchSnapshot` / `toBeDefined` treated as exact | oracle_kind ∈ exact set only; snapshot/loose kinds excluded | G-C; oracle.rs metadata |
| F3 | Heuristic-only related test borrowed | `has_oracle_eligible_relation == true` | G-D; else `ambiguous_related_test` (preview) |
| F4 | Cross-package / monorepo test that doesn't exercise the owner | related test resolved same-package; workspace_root filtered before ranking | related_tests.rs ownership filter; else `missing_context` (preview) |
| F5 | Guessed/templated `verify_command` | command produced by `verify_command_for_discovery()` from real framework/runner discovery, non-empty | validator cond. 3; package.rs |
| F6 | Missing receipt ⇒ unrecordable repair | `receipt_command` `Some(non-empty)` | validator cond. 6 (§1.1); §3.2 |
| F7 | Receipt delegates to external provider | receipt is a `ripr outcome …` invocation only; reject any `curl`/`http`/provider-name pattern | `typescript_receipt_command` constructs a fixed `ripr outcome` shape; no interpolation of free text |
| F8 | Implicit / unbounded edit surface | `allowed_edit_surface_for_gap_route(route)` non-empty AND tokenizes to a single test file | validator cond. 5 (shared fn) |
| F9 | Edit surface points at production (not test) file | route.target_file is the *test* file; production file lands in `forbidden_files` | `forbidden_files_for_gap_record` (shared) |
| F10 | Static limit buried, flip taken anyway | static_limit short-circuits to `static_limitation` BEFORE oracle/relation checks | actionability.rs:51 early return; G-A excludes it |
| F11 | Unresolved cross-language oracle visibility | bridge limitation routes to named limitation, never actionable | G-F; bun_bridge.rs |
| F12 | Already-observed (strong oracle) re-emitted as a gap | `Exposed` ⇒ `strong_oracle_observed`, empty missing_fields, no packet | actionability.rs:65; G-A excludes it |
| F13 | Line-keyed / non-content-addressed `canonical_gap_id` | id is `gap:typescript:<family>:<fp8>` from content-addressed finding id; path normalized `\`→`/` | §3.2; content-addressed-ids invariant |
| F14 | Preview evidence presented as gate authority | `authority_boundary == "preview_advisory_only"` even when flipped; `must_not_change` includes preview clause | §1.3; §3.2 `must_not_change` |
| F15 | Projection-eligibility override / bypass | `projection_eligibility["agent_packet"].eligible == true` required | validator cond. 1 (§1.1) |
| F16 | Confidence-only flip | confidence is a ranking signal, never the flip decider; flip requires the full validator + G-A–G-F | §1; confidence not in gate predicate |

Any invariant that cannot be proven for a finding ⇒ the finding stays preview.
The validator's `Err` reason is surfaced in `why_not_actionable` so the named
missing field is visible to the operator.

---

## 5. Fixtures

One COMPLETE-contract fixture that flips actionable, plus one fixture per
failure family that stays NON-actionable. All live under `fixtures/` with
input diff + expected output, run by `cargo xtask fixtures` and checked by
`cargo xtask goldens check`.

### 5.1 Complete-contract (flips `repair_packet_ready: true`)

- **`ts_repair_packet_complete`** — a diff changing a boundary condition in a
  TS owner (`applyDiscount`, `if (amount >= threshold)`), with:
  a same-package Jest/Vitest test importing and calling the owner
  (token-aware relation, G-D); an exact oracle `expect(applyDiscount(100,
  100)).toBe(90)` with concrete `expected_value_or_variant` (G-C); a
  discoverable `package.json` yielding a real `verify_command` (F5); a named
  missing boundary discriminator (G-E).
  Expected output: `repair_packet_ready: true`,
  `actionability_category: "complete_repair_packet"`,
  `gap_state: "actionable"`, `missing_actionability_fields: []`,
  `authority_boundary: "preview_advisory_only"`,
  `must_not_change` includes the preview clause,
  `canonical_gap_id` matches `gap:typescript:.*:[0-9a-f]{8}`,
  `receipt_command` is a `ripr outcome … target/ripr/receipts/…` string,
  `allowed_edit_surface` = the single test file,
  `forbidden_files` includes the production file.

### 5.2 Stay-non-actionable (one per failure family)

| Fixture | Models | Expected (stays preview) |
| --- | --- | --- |
| `ts_dynamic_oracle` | F1/F2 | `repair_packet_ready: false`, category `missing_target_shape` |
| `ts_heuristic_relation` | F3 | false, category `ambiguous_related_test` |
| `ts_cross_package_test` | F4 | false, category `missing_context` |
| `ts_no_verify_command` | F5 | false, validator reason in `why_not_actionable` |
| `ts_static_limit` | F10 | false, gap_state `static_limitation` (named) |
| `ts_cross_language_bridge_limit` | F11 | false, named cross-language limitation |
| `ts_already_observed` | F12 | false, category `strong_oracle_observed` |

Each non-actionable fixture must assert `repair_packet_ready: false` and that
the reason it stays preview is **named** (a category and/or validator reason),
not silent.

---

## 6. Additive / golden expectations

- **Most existing TS findings stay `false`.** The flip is gated on G-A–G-F +
  the shared validator, which the overwhelming majority of preview findings
  fail (dynamic oracles, heuristic relations, missing receipts). Existing
  golden expectations for `repair_packet_ready` remain `false` and must NOT
  drift.
- **Exactly the genuinely-complete case flips.** Only fixtures that satisfy
  the full contract (e.g. `ts_repair_packet_complete`) change from `false` to
  `true`. This is an **intended, documented behavior change**, recorded via
  `cargo xtask goldens bless ts_repair_packet_complete --reason
  "RIPR-SPEC-0087 §PR7: complete TS repair packet now actionable"` — it is
  NOT drift.
- **No schema bump.** `repair_packet_ready` is an existing boolean in the
  `preview_actionability` JSON object (preview_actionability.rs:79;
  `docs/OUTPUT_SCHEMA.md`). Only its *value* changes for the new complete
  case. `schema_version` is unchanged. `docs/OUTPUT_SCHEMA.md` is updated to
  document the flip condition and the new `complete_repair_packet` category
  value (additive enum value, not a shape change).
- **Goldens loop coverage.** The repo-exposure JSON output path produces zero
  drift (no TS preview findings flip there); the `check --diff --json` golden
  for the complete fixture is the single intended diff.
- Update `docs/OUTPUT_SCHEMA.md`, `docs/CAPABILITY_MATRIX.md` (TypeScript
  actionability row), and `docs/LEARNINGS.md` (the "flip reuses the Rust
  validator, never a parallel path" rule).

---

## 7. Verification bar

### 7.1 Full local gate list (must pass before PR)

```
cargo fmt --check
cargo check --workspace --all-targets
RUSTFLAGS="-D warnings" cargo build -p xtask -p ripr   # CI lib-only -D warnings mirror
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-static-language        # 'actionable' is NOT a banned static-output word; 'killed/survived/proven/adequate' MUST NOT appear
cargo xtask check-output-contracts
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-architecture           # validator stays in output/, projection in analysis/typescript/
cargo xtask check-public-api             # no new public symbols
cargo xtask check-traceability           # RIPR-SPEC-0087 → tests → code mapped
cargo xtask check-doc-index
cargo xtask check-spec-format
cargo xtask check-capabilities
cargo xtask precommit
cargo xtask check-pr
```

CI gate (`routed-rust.yml`) must be green; the only required check is
**"Ripr Rust Small Result"**. Because this PR touches Rust production code, it
is NOT docs-only and runs the full routed-rust lane (it will not be skipped by
`detect-docs-only`/`docs-gate`).

### 7.2 Static-language note

`actionable` / `complete_repair_packet` / `repair_packet_ready` are allowed
vocabulary (they are packet-readiness terms, not mutation verdicts).
`check-static-language` must still pass: the forbidden words
`killed`/`survived`/`untested`/`proven`/`adequate` MUST NOT appear in any new
output string, fixture expectation, or evidence line.

### 7.3 Behavioral repro (RUN the commands, do not grep)

```bash
# Complete case IS actionable:
cargo run -p ripr -- check --diff fixtures/ts_repair_packet_complete/input.diff --json \
  | jq '.findings[] | select(.language=="typescript") | .preview_actionability.repair_packet_ready'
# MUST print: true   (exactly one finding)

cargo run -p ripr -- check --diff fixtures/ts_repair_packet_complete/input.diff --json \
  | jq -r '.findings[] | .preview_actionability | "\(.actionability_category) \(.authority_boundary)"'
# MUST print: complete_repair_packet preview_advisory_only

# Every incomplete case is NOT actionable:
for f in ts_dynamic_oracle ts_heuristic_relation ts_cross_package_test \
         ts_no_verify_command ts_static_limit ts_cross_language_bridge_limit \
         ts_already_observed; do
  cargo run -p ripr -- check --diff fixtures/$f/input.diff --json \
    | jq -e '[.findings[].preview_actionability.repair_packet_ready] | all(. == false)' >/dev/null \
    && echo "$f: NOT actionable (correct)" || echo "$f: OVER-EMIT BUG"
done
# MUST print "NOT actionable (correct)" for every fixture
```

The behavioral bar is a hard AND: the complete case prints `true` and the
`complete_repair_packet`/`preview_advisory_only` pair, and *every* incomplete
case prints all-`false`. A single over-emit fails the spec.

### 7.4 Validator-parity test (the architectural assertion)

A unit test must construct the `GapRecord` the TS projection emits for the
complete fixture and assert `validate_agent_gap_record_packet(&record) ==
Ok(())`, and for each incomplete fixture assert it returns the expected `Err`
reason. This proves the flip is driven by the *shared* validator and not a
parallel TS path — if someone later forks the logic, this test breaks.

---

## Acceptance criteria

1. `repair_packet_ready` flips `true` for the complete fixture and **only**
   when `validate_agent_gap_record_packet` returns `Ok(())` on a `GapRecord`
   projected from a TypeScript finding satisfying G-A–G-F.
2. No parallel/mirror TypeScript completeness validator exists; the flip
   calls `agent_seam_packets::validate_agent_gap_record_packet`.
3. The four new producers (`receipt_command`, `must_not_change`,
   `canonical_gap_id`, and a tokenizable `repair_route` feeding the shared
   `allowed_edit_surface_for_gap_route`) are real and grounded in §3.2.
4. Every F1–F16 over-emit mode keeps the finding preview or named-limitation.
5. Fixtures: one flips, seven stay non-actionable, each with a named reason.
6. Goldens: only the complete fixture's golden changes (blessed with reason);
   `schema_version` unchanged; no other drift.
7. Full §7.1 gate list and §7.3 behavioral repro pass; §7.4 parity test
   passes.

---

## Acceptance Examples

### Complete repair packet (flips `repair_packet_ready: true`)

```text
input:  TypeScript owner applyDiscount, predicate change (> → >=),
        direct import-aware related test with concrete literal oracle
        toBeGreaterThan(50), discoverable package.json (jest/npm),
        named missing discriminator (amount == threshold)
output: repair_packet_ready: true
        actionability_category: complete_repair_packet
        gap_state: actionable
        missing_actionability_fields: []
        authority_boundary: preview_advisory_only
```

### Dynamic oracle (stays non-actionable, F1/F2)

```text
input:  related test uses expect(result).toBe(expected) where expected is a variable
output: repair_packet_ready: false
        actionability_category: strong_oracle_observed (oracle_strength Strong → Exposed)
```

### Heuristic relation only (stays non-actionable, F3)

```text
input:  related test only has heuristic name proximity (no direct import)
output: repair_packet_ready: false
        actionability_category: ambiguous_related_test
```

### No test context (stays non-actionable, F4)

```text
input:  no related test found for the changed owner
output: repair_packet_ready: false
        actionability_category: missing_context
```

### No verify command (stays non-actionable, F5)

```text
input:  no package.json present → no typescript_verify_command evidence
output: repair_packet_ready: false
        actionability_category: incomplete_repair_packet
        (projection returns None — no verify command)
```

### Static limit (stays non-actionable, F10)

```text
input:  changed line uses computed member invocation handlers[key]()
output: repair_packet_ready: false
        actionability_category: dynamic_dispatch
        gap_state: static_limitation
```

### Cross-language bridge / mock limit (stays non-actionable, F11)

```text
input:  test file uses vi.mock() → mocked_module static limit
output: repair_packet_ready: false
        actionability_category: mocked_module
        gap_state: static_limitation
```

### Already-observed strong oracle (stays non-actionable, F12)

```text
input:  related test uses toBe(true) with oracle_strength Strong
output: repair_packet_ready: false
        actionability_category: strong_oracle_observed
        gap_state: already_observed
```

## Test Mapping

The §7.4 validator-parity tests live in:

```text
crates/ripr/src/output/typescript_packet_projection.rs::tests::
  validator_parity_complete_finding_passes_shared_validator
  validator_parity_missing_verify_command_returns_none
  validator_parity_missing_oracle_expected_returns_none
  validator_parity_dynamic_oracle_returns_none
  validator_parity_wrong_category_returns_none
  validator_parity_no_related_tests_returns_none
  validator_parity_no_missing_discriminators_returns_none
  validator_parity_cross_language_bridge_returns_none
  canonical_gap_id_derives_from_finding_id
  canonical_gap_id_normalizes_backslashes
  receipt_command_is_ripr_outcome_shape
  must_not_change_includes_preview_clause_via_shared_function
```

The eight fixtures serve as behavioral integration tests:

```text
fixtures/ts_repair_packet_complete   — complete contract (flips)
fixtures/ts_dynamic_oracle           — F1/F2: strong oracle + dynamic arg
fixtures/ts_heuristic_relation       — F3: heuristic relation only
fixtures/ts_cross_package_test       — F4: no related test
fixtures/ts_no_verify_command        — F5: no verify command
fixtures/ts_static_limit             — F10: dynamic dispatch static limit
fixtures/ts_cross_language_bridge_limit — F11: mocked module static limit
fixtures/ts_already_observed         — F12: strong exact oracle observed
```

## Implementation Mapping

The implementation adds two modules and modifies one:

```text
NEW  crates/ripr/src/output/typescript_packet_projection.rs
     (projection: typescript_gap_record_for, G-A–G-F checks,
      canonical_gap_id, receipt_command, route_kind mapping)

MOD  crates/ripr/src/output/preview_actionability.rs
     (replaces hardcoded false at line 63 with computed repair_packet_ready
      via typescript_gap_record_for + validate_agent_gap_record_packet)

MOD  crates/ripr/src/output/mod.rs
     (registers typescript_packet_projection module)
```

The shared validator lives in (not modified):

```text
crates/ripr/src/output/agent_seam_packets.rs::validate_agent_gap_record_packet
```

## Metrics

- `typescript_actionable_repair_packet_contract_status_proposed` — tracks
  the proposed status of this contract. The single behavioral metric is the
  binary `repair_packet_ready` value: exactly one fixture flips `true`
  (`ts_repair_packet_complete`), and all others stay `false`. Real
  route-quality metrics remain deferred to the TypeScript route-quality PR.
