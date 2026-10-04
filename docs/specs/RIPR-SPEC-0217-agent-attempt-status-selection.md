# RIPR-SPEC-0217: Repair-attempt discovery selection and fresh-process resume

Status: proposed

Owner: product-agent

Created: 2026-10-03

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4798 — attempt discovery, selection, and fresh-process resume (this spec
  implements its one-PR objective)
- #4797 / RIPR-SPEC-0195 — typed store identity and resolver (consumed)
- #2927 — parent repair-attempt transaction
- #4799 — crash/retry matrix (later; not absorbed)
- #2928 — VS Code shell (consumer of the shared DTO; not absorbed)
- #1702 — real-repair pilot (consumer; not absorbed)

Linked PRs:

- None yet

Support-tier impact:

- None. This spec makes one existing command (`ripr agent status`) able to
  select one attempt exactly. It does not promote a language, editor surface,
  gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

Durable `RepairAttempt` records already enumerate through
`ripr agent status`, and #4797 / RIPR-SPEC-0195 made the store identity
typed and explicit. But the only way to act on one attempt was the
inventory surface: it lists every attempt and selects a next command only
when that choice is unambiguous. There was no exact-selection route: a
fresh process could not select one attempt by ID and receive one typed
current state plus one exact next or recovery action. Scripting a resume
required parsing human prose, and a malformed or stale attempt had no
typed result at all.

## Behavior

`ripr agent status --attempt <id>` selects exactly one attempt from the
resolved store and reports one typed state. The selection law:

- Discovery enumerates only manifests in the resolved #4797 store; the
  selected-attempt view resolves the same store identity and never searches
  parent directories, sibling worktrees, newest-mtime folders, or another
  store. An attempt ID from one store is `corrupt_or_unavailable` through
  another, never silently resolved.
- Every attempt row validates independently. A missing, malformed, or
  unbound selected attempt is the typed `corrupt_or_unavailable` result
  naming the refused artifact — not an opaque error, not a generic clean
  result, and never state borrowed from the store's other rows.
- No attempt, one attempt, and several attempts are different typed
  results: the inventory document lists zero, one, or many rows and refuses
  to guess between several active attempts; the selected document describes
  exactly the requested ID.
- `--attempt <id>` is the only way one attempt is targeted; several active
  attempts never select newest, first, same seam, most recently modified,
  or first workspace folder.
- Human Markdown and normalized JSON derive from one DTO, so the two
  surfaces agree on state, ordering, next action, and claim boundary, and
  reordered directory traversal produces byte-stable output (the store is
  ordered by attempt identity).

The one-attempt DTO projects the #4798 status model while keeping
operational state, static movement, focused execution, edit-cage verdict,
receipt strength, and currentness as separate facts:

- `state` is the manifest's operational state (`prepared`,
  `awaiting_edit`, `ready_to_finish`, `stale`, `incomparable`, `failed`).
- `status_class` is the resume vocabulary: `awaiting_edit`, `prepared`,
  `finished_current`, `finished_historical`, `stale`, `incomparable`,
  `failed`, `limited`, `corrupt_or_unavailable`,
  `legacy_compatibility_only`. The class is never stronger than the
  receipt the attempt actually retained:
  - `finished_current` / `finished_historical` require a digest-bound
    attempt-local receipt whose reading shows the gap closed, split by
    whether HEAD is still the head the after phase recorded. A moved HEAD
    downgrades to `finished_historical`; historical validity and current
    applicability are reported separately and the retained result is never
    presented as current proof.
  - `limited` covers a receipt that does not show the gap closed, a
    finished attempt whose currentness cannot be read, and an awaiting
    attempt whose HEAD cannot be related to its prepared head.
  - `corrupt_or_unavailable` covers declared-but-missing, tampered, or
    unbound attempt-local terminal evidence. Status never falls back to
    another attempt's one-slot compatibility receipt.
  - `legacy_compatibility_only` covers manifests without
    `terminal_artifacts`: any reading travels only through the one-slot
    compatibility projection, which is exactly the strength it earned.
- `currentness` is `current`, `historical`, or `unknown`, derived from
  HEAD alone and reported separately from the class.
- `next_action` is one exact next or recovery command: the after phase's
  retained command for `awaiting_edit`; a typed restart or head-recovery
  command for `stale`, `incomparable`, `failed`, `prepared`, gap-open
  `limited`, and recovery routes; absent for terminal classes and for
  states where no honest action names itself.
- `claim_boundary`, `limitations`, and `non_claims` carry the read-only
  non-claim (status never finishes, restarts, rewrites, or deletes an
  attempt by inspecting it), the retained-evidence non-claim, the
  manifest's own limitations/non-claims, and the store resolver's
  non-claims.

The command remains read-only end to end: it runs no analysis, executes
nothing a returned route names, writes no file, and introduces no
execution or mutation authority.

## Required Evidence

- Focused `cargo test -p ripr --lib agent_status` and
  `cargo test -p ripr --lib repair_attempt` selectors run with nonzero
  subjects.
- Unit tests cover exact selection resuming an awaiting attempt, the typed
  `corrupt_or_unavailable` result for a missing attempt, selected-store
  isolation for explicit stores, the stale downgrade after HEAD movement,
  and byte-stable normalized JSON across repeated reads.
- Built-binary integration tests cover the full fresh-process journey
  (prepare → resume → external test-only edit → finish → retained
  terminal read), the `finished_historical` downgrade after HEAD movement,
  same-seam attempts staying distinct by ID, an explicit non-default store
  across processes, one malformed row keeping the store listing fail-closed
  while valid rows stay resumable by exact selection, tampered terminal
  evidence never falling back to another attempt's projection, a deleted
  retained verify artifact making the whole terminal record unavailable, and
  a legacy manifest reporting at compatibility strength only.
- `cargo xtask check-output-contracts`, `check-fixture-contracts`,
  `check-static-language`, `check-capabilities`, and `check-traceability`
  pass on the candidate.

## Non-Goals

- Start/Continue/Finish editor UX (#2928 owns it; this spec's DTO is the
  reusable surface it consumes)
- Packaged CLI/VSIX qualification (#2929)
- Real-attempt denominators and pilot measurement (#1702, #1560, #1579)
- Automatic edit, verification execution, mutation execution, cleanup
  daemon, or background watcher
- Command-catalog or `help --json` ownership (#1613 remains the authority)
- The crash/retry/compatibility matrix (#4799)
- Support-tier, release, or publication action

## Acceptance Examples

1. A fresh process runs `ripr agent status --root R --attempt A --json` on
   an attempt prepared by an exited process and reads `awaiting_edit`, the
   exact after command the before phase recorded, and the read-only claim
   boundary.
2. After the external test-only edit and a finish in a third process, a
   fourth process reads `finished_current` with the digest-bound retained
   receipt and no next action.
3. After HEAD moves, the same attempt reads `finished_historical`: the
   retained result stays readable and is explicitly not current proof.
4. Two same-seam awaiting attempts each resume by their own ID; the
   inventory surface refuses to guess between them.
5. An explicit-store attempt is `corrupt_or_unavailable` through the
   default store and `awaiting_edit` through `--store`, with `--store`
   repeated on the next action.
6. A manifest with tampered terminal evidence reads
   `corrupt_or_unavailable` even while the one-slot compatibility
   projection still matches the attempt.

## Test Mapping

- `crates/ripr/src/app/agent_status.rs::tests::agent_attempt_status_selects_and_resumes_an_awaiting_attempt`
- `crates/ripr/src/app/agent_status.rs::tests::agent_attempt_status_types_a_missing_attempt_instead_of_erroring`
- `crates/ripr/src/app/agent_status.rs::tests::agent_attempt_status_uses_only_the_selected_store`
- `crates/ripr/src/app/agent_status.rs::tests::agent_attempt_status_never_claims_resumable_past_a_moved_head`
- `crates/ripr/src/app/agent_status.rs::tests::agent_attempt_status_renders_byte_stable_across_repeated_reads`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_parses_exact_attempt_selection_and_rejects_empty`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_resumes_and_finishes_one_attempt_across_fresh_processes`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_reports_historical_after_head_moves`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_keeps_same_seam_attempts_distinct_by_id`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_round_trips_an_explicit_store`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_types_a_malformed_row_and_isolates_valid_rows`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_never_reconstructs_a_tampered_result`
- `crates/ripr/tests/cli_smoke.rs::agent_status_attempt_reports_legacy_manifest_at_compatibility_strength`
- `crates/ripr/tests/cli_help_hierarchy.rs::agent_status_help_names_the_selected_store_and_exact_attempt_selection`

## Implementation Mapping

- `crates/ripr/src/app/agent_status.rs` — the one-attempt DTO, the
  status-class projection, next/recovery action selection, and the human
  and JSON renderers over that one DTO
- `crates/ripr/src/app/repair_attempt/store.rs` — no behavior change; the
  existing typed resolver is consumed (`location_class`, `limitations`
  surfaced on the DTO)
- `crates/ripr/src/cli/agent.rs` — `--attempt` on `agent status`
- `crates/ripr/src/cli/commands/agent.rs` — the exact-selection run path
- `crates/ripr/src/cli/help/agent.rs` — documented `--attempt`

## CI Proof

- Focused `cargo test -p ripr --lib agent_status`
- Focused `cargo test -p ripr --lib repair_attempt`
- Focused `cargo test -p ripr --test cli_smoke -- agent_status`
- `cargo xtask check-output-contracts`
- `cargo xtask check-traceability`
- `cargo xtask precommit` on the candidate

## Metrics

- None new. This spec projects existing attempt-store and receipt
  authorities; it adds no counters and claims no measurement.
