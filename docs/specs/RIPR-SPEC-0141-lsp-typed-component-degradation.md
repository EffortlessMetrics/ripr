# RIPR-SPEC-0141: LSP Typed Component-Outcome Degradation Model

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#1997](https://github.com/EffortlessMetrics/ripr-swarm/issues/1997) -
  replace hidden LSP stderr degradation with typed snapshot limitations and
  client-visible recovery.
- [#1910](https://github.com/EffortlessMetrics/ripr-swarm/issues/1910) -
  seam, gap-ledger, and causal failures were written to hidden stderr
  (parent).
- [#1939](https://github.com/EffortlessMetrics/ripr-swarm/issues/1939) -
  duplicated run-status logic drifted (parent); this spec keeps exactly one
  run-status aggregation on the LSP path.
- [#1960](https://github.com/EffortlessMetrics/ripr-swarm/issues/1960) -
  reliability convergence program (parent).

Support-tier impact:

- No tier change. This spec changes how the LSP server records and discloses
  optional-component degradation. It does not change any classification,
  finding set, ExposureClass, probe family, confidence score,
  `repair_packet_ready` authority, output schema version, or pass/fail
  behavior. The `ripr/analysisStatus` payload gains one additive
  `components` array; CLI output contracts are untouched.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

Several LSP refresh-path failure modes reported degradation only through
process stderr (`eprintln!`): a seam-inventory walk failure, gap decision
ledger read/parse/validation failures, and a causal-projection load warning
were invisible to any client that had not opened a terminal, and the
committed snapshot carried no record that a component had failed. A GUI user
saw a full-looking run with silently missing evidence. The gap-ledger
projection also re-read and re-validated the ledger a second time after the
workspace gap-artifact report had already judged it — a second
interpretation of the same artifact that could drift from the first.

## Behavior

### Typed bounded component-outcome record

Every refresh records one bounded `ComponentOutcome` per optional analysis
component on the committed `AnalysisSnapshot`:

```text
component: diff | seam_inventory | gap_ledger | causal_projection | cache
           (report deferred until a producer exists)
state: complete | unavailable | limited | failed | deferred
       (cancelled deferred until a producer exists)
kind: stable machine code for the specific outcome (optional)
message: normalized, path-redacted, size-bounded detail (optional)
findings_trustworthy: whether ordinary diff-scoped findings remain trustworthy
recovery: the safe recovery action a client may name (optional)
snapshot_identity: the input/snapshot identity that produced the record
```

Rules:

- The snapshot is the single typed authority. Diagnostics severity policy,
  hover, code actions, standard work-done progress, `ripr/analysisStatus`,
  and workspace status all read the same records.
- Messages are bounded and path-redacted at construction
  (`bounded_message`), so no client surface can emit unbounded error text or
  absolute workspace paths.
- `report` and `cancelled` are deferred vocabulary members: they are
  intentionally absent from the code until a real producer exists (the
  closed code vocabulary today is `diff | seam_inventory | gap_ledger |
  causal_projection | cache` and `complete | unavailable | limited |
  failed | deferred`). Out-of-scope test-file projection suppression is
  disclosed typed via the existing `out_of_scope_test_file_findings` count,
  not a fabricated report outcome. A cancelled attempt never commits a
  snapshot, so it never emits a spurious component failure either. Adding
  either member is a contract change owned by the PR that lands its
  producer.
- An absent optional artifact (no gap decision ledger, no canonical delta)
  is a normal state and records no outcome.

### Single run-status aggregation

`derive_run_status` in `crates/ripr/src/lsp/diagnostics.rs` is the only
run-status aggregation on the LSP path. Workspace status, document/workspace
diagnostic result identities, health, and progress ends all derive from it.
Precedence (first match wins):

```text
stale                  (StaleArtifact gap-artifact rejection)
cache_limited          (any other gap-artifact rejection)
limited_partial_scope  (bounded partition of an over-budget diff)
limited                (per-finding or gap-artifact static limits)
limited                (any component outcome in limited or failed state)
seams_deferred         (interactive deferral, RIPR-SPEC-0105)
full
```

A degraded optional component therefore never presents as `full`: finding
WARNINGs downgrade to INFORMATION and gap-record diagnostics stay suppressed
under the existing limited-run policy, while ordinary diff-scoped findings
remain published and usable (`findings_trustworthy: true`).

### Client-visible degradation reporting

- `window/logMessage` receives one concise WARNING per distinct degradation
  signature, naming each degraded component, its bounded detail, and its
  recovery route. A byte-identical repeated degradation on later refreshes
  does not warn again. When the signature clears, one INFO recovery line is
  logged.
- `ripr/analysisStatus` (and the embedded copy in workspace status) exposes
  the typed records under `components`, so push (notification) and pull
  (command) surfaces agree.
- Standard work-done progress (#1971) ends `limited` — never `complete` —
  when any component is degraded, because the progress end derives from the
  same shared run status.
- `window/showMessage` remains reserved for reviewed hard setup/analysis
  failures. Routine partial evidence stays in status/output channels, and
  passive diagnostics carry no per-file status spam.
- Hard failure (no snapshot), recoverable limitation, cancellation, and
  deferment stay distinct: a failed attempt records `AnalysisFailure` on
  health; a degraded component records `limited`/`failed`; a cancelled
  attempt records the cancelled attempt state with no component outcome;
  interactive deferral records `deferred`, which is disclosed but not a
  degradation and never warns.
- CLI/non-LSP stderr behavior is independently governed and unchanged.

## Required Evidence

- `ComponentOutcome`, `AnalysisComponent`, `ComponentState`,
  `degradation_signature`, `degradation_log_message`, and `bounded_message`
  in `crates/ripr/src/lsp/component_outcome.rs`.
- Per-refresh outcome construction for diff, cache, causal projection, gap
  ledger, and seam inventory, plus the single read/validate/parse gap-ledger
  loader, in `crates/ripr/src/lsp/diagnostics.rs`.
- The `component_outcomes` field on `AnalysisSnapshot` in
  `crates/ripr/src/lsp/state.rs`.
- The deduplicated `window/logMessage` warning and recovery line, the
  `components` array on `ripr/analysisStatus`, and the async
  `collect_context_packet` log warning in
  `crates/ripr/src/lsp/backend.rs`.
- The framed duplex degradation test and the no-stderr-fallback source guard
  in `crates/ripr/src/lsp/tests.rs`, with the fake client answering every
  server-originated request.

## Non-Goals

- Retry policy, progress implementation changes, diagnostic-budget algorithm
  changes, or analyzer evidence fixes; this spec reports truthfully and does
  not manufacture missing evidence.
- `window/showMessage` adoption for routine partial evidence.
- CLI/non-LSP stderr governance (independent by design).
- Deduplicating any run-status logic outside the LSP path; the LSP path has
  exactly one aggregation after this spec.
- Output schema version bumps; the `components` array is additive to the
  LSP-internal status payload.

## Acceptance Examples

### Seam inventory failure with usable diff findings

```text
Given a refresh whose seam inventory walk fails,
when the snapshot commits,
then the seam_inventory outcome is failed with kind seam_inventory_failed
  and a bounded message and the recovery route retry ripr.refreshDiagnostics,
and the run status is limited,
and ordinary diff-scoped findings remain published with
  findings_trustworthy: true,
and progress ends limited, and exactly one WARNING log message names the
  component and its recovery.
```

### Gap-ledger read or parse failure

```text
Given a workspace whose gap decision ledger cannot be read or parsed,
when the refresh commits,
then the gap_ledger outcome is failed with a typed kind
  (gap_ledger_read_failed, gap_ledger_parse_failed, gap_ledger_wrong_kind,
  gap_records_parse_failed, or the validation rejection code),
and the run status is not full,
and the failure is recorded once from the single shared ledger load.
```

### Causal projection failure

```text
Given a canonical delta artifact that exists but cannot be loaded,
when the refresh commits,
then the causal_projection outcome is failed with kind
  causal_projection_unusable and a recovery route,
and the run status is limited, and the client sees one WARNING log message.
```

### Diff-component guard stops (timeout and oversized scope)

```text
Given a refresh whose diff load exceeds the cooperative git deadline
  (git_invocation_timeout, #2303),
when the conversion fires,
then the diff outcome is failed with the named kind and
  findings_trustworthy: false, the run status is limited, zero findings
  and zero diagnostics are committed, and the recovery route is
  retry ripr.refreshDiagnostics.

Given a refresh whose diff exceeds the fail-closed scope guard
  (diff_scope_oversized, #2299),
when the conversion fires,
then the diff outcome is failed with the named kind and
  findings_trustworthy: false, the run status is limited, and exactly
  ONE workspace-scoped warning diagnostic (code
  ripr-scope-diff-oversized) anchored at the workspace root URI carries
  the guard's bounded first line (kind, actual counts, split guidance),
  while the CLI keeps its non-zero exit and unchanged error text.
  Only the raw, unwrapped guard error converts; the distinct
  repo_scope_oversized guard and wrapped lookalikes do not.
```

### Repeated identical degradation does not spam

```text
Given a committed snapshot with a degraded component,
when the next refresh reproduces the byte-identical degradation,
then no second WARNING log message is emitted,
and the status payload still reports the degraded component.
```

### Recovery on next refresh

```text
Given a session whose previous refresh logged a component degradation,
when a later refresh commits with every recorded component complete,
then one INFO recovery log message is emitted,
and the status payload reports the complete outcomes.
```

### Cancellation emits no spurious failure

```text
Given a refresh attempt that is cancelled or superseded,
when the attempt ends,
then no component outcome is recorded as failed for the cancelled attempt,
and progress ends cancelled or superseded, never failed.
```

## Test Mapping

- `crates/ripr/src/lsp/component_outcome.rs::tests::degraded_states_are_exactly_limited_and_failed`
  — degradation vocabulary.
- `crates/ripr/src/lsp/component_outcome.rs::tests::failed_outcome_bounds_and_redacts_message`
  — bounded, path-redacted messages.
- `crates/ripr/src/lsp/component_outcome.rs::tests::status_payload_names_component_state_kind_and_recovery`
  — typed status payload shape.
- `crates/ripr/src/lsp/component_outcome.rs::tests::degradation_signature_is_none_without_degradation_and_stable_when_degraded`
  — dedup signature stability.
- `crates/ripr/src/lsp/component_outcome.rs::tests::degradation_log_message_names_component_and_recovery`
  — concise warning text.
- `crates/ripr/src/lsp/diagnostics.rs::tests::derive_run_status_marks_degraded_component_run_limited`
  — single aggregation: degraded component yields `limited`, precedence
  preserved.
- `crates/ripr/src/lsp/diagnostics.rs::tests::load_gap_ledger_records_types_every_failure_mode`
  — one ledger interpretation feeding both the outcome and the projection.
- `crates/ripr/src/lsp/tests.rs::lsp_production_sources_have_no_stderr_degradation_fallback`
  — guard: no `eprintln!` remains in LSP production sources.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_component_degradation_is_typed_logged_and_recovers`
  — framed duplex: typed status components, one WARNING per distinct
  degradation, limited progress end, findings still published, dedup on
  repeat, INFO recovery line after the artifact is repaired.
- Existing pins `framed_lsp_protocol_smoke_logs_successful_refresh_completion`,
  `boundary_gap_lsp_diagnostics_match_fixture_expectation`, and the
  `work_done_progress_*` suite — behavior preservation for clean runs.

## Implementation Mapping

- `crates/ripr/src/lsp/component_outcome.rs` — the typed record, states,
  signature, warning text, and bounded message.
- `crates/ripr/src/lsp/diagnostics.rs` — outcome construction in
  `workspace_diagnostics_with_config`, `cache_component_outcome`,
  `load_gap_ledger_records`, the pure `append_gap_record_diagnostics_with_causal`
  projection, and the extended `derive_run_status`.
- `crates/ripr/src/lsp/state.rs` — `AnalysisSnapshot.component_outcomes`.
- `crates/ripr/src/lsp/backend.rs` — `log_component_degradations` with
  `last_component_degradation` dedup state, the `components` status array,
  and the async context-packet log warning.

## Metrics

- Unit and framed tests listed above pass under
  `cargo test -p ripr lsp::`.
- `cargo xtask goldens check` remains clean: LSP degradation reporting
  touches no CLI golden.
- `cargo xtask check-static-language` and
  `cargo xtask check-output-contracts` remain clean: conservative vocabulary
  and no CLI output-contract change.
