# RIPR-SPEC-0078: LSP Top-Limitation Command

Status: proposed

Owner: product / swarm

Created: 2026-06-11

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `ripr.collectTopLimitation` as a new LSP
  `executeCommand`. No existing contract is modified. No language,
  surface, or evidence class is promoted to a stronger support tier.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- None. The command is advisory-only. It reads the in-memory snapshot
  and returns a structured limitation packet. No files are written or
  read from disk.

## Problem

When `ripr.collectRepairPacket` returns null or a sentinel an agent knows
a repair packet is unavailable, but does not know why. Without a
structured reason the agent must guess which part of the analyzer route
is incomplete — leading to wasted effort or incorrect repair attempts.

`ripr.collectTopLimitation` answers the complementary question: what is
the top named limitation blocking analysis, and what analyzer route must
be implemented to remove it?

## Behavior

`ripr.collectTopLimitation` is a new LSP `executeCommand` that:

1. Locks `latest_analysis` briefly.
2. If no snapshot exists yet, returns the `no_limitation` sentinel (see
   Outputs). This is a meaningful "no blockers detected" answer, not null.
3. If `gap_artifact_rejections` is empty on the snapshot, returns null
   (the first rejection is absent; no limitation to report).
4. Otherwise builds the full limitation packet from the first
   `GapArtifactRejection` using `rejection.as_str()` (limitation
   category) and `workspace_status_rejection_repair` (repair_route,
   why_not_actionable). Does not duplicate the rejection taxonomy.

### `unlock_condition` equivalence

`unlock_condition` is set equal to `repair_route`. This is honest: the
route the analyzer must implement is exactly the condition that unlocks
the gap. Agents should treat the two fields as equivalent — one is
present for readability.

### `sample_sources` payload

`GapArtifactRejection` variants that carry a `String` payload emit that
payload as a one-element list. Unit and `&str`-reason variants emit an
empty list. gap_id-bearing sample sources are a deferred follow-up; the
rejection enum does not carry a gap_id.

String-payload variants: `DisabledLanguage`, `MalformedCommandPayload`,
`OutOfWorkspacePath`, `UnavailableLanguage`, `UnsupportedKind`,
`UnsupportedSchema`, `UnsupportedStaticLimitKind`, `WrongRoot`.

Empty-payload variants: `MalformedArtifact`, `MissingIdentity`,
`StaleArtifact`.

### `non_claims` table

Static per-category assertions about what the packet does not claim.
Uses static-language vocabulary only.

| Category | non_claims |
| --- | --- |
| `disabled_language`, `unavailable_language` | "not a Rust repair packet", "does not indicate the behavior is reachable", "does not indicate tests are absent" |
| `wrong_root`, `out_of_workspace_path` | "not a repair packet", "path resolution required before exposure can be assessed" |
| `stale_artifact`, `missing_identity`, `malformed_artifact`, `malformed_command_payload` | "not a repair packet", "artifact regeneration required before exposure can be assessed" |
| `unsupported_schema`, `unsupported_kind`, `unsupported_static_limit_kind` | "not a repair packet", "ripr upgrade required before exposure can be assessed" |
| (unknown) | "not a repair packet" |

## Non-Goals

- Does not run mutants, execute tests, or modify any file.
- Does not replace `ripr.collectRepairPacket` or `ripr.collectWorkspaceStatus`.
- Does not expose the full rejection list — only the top (first) rejection.
- Does not fabricate gap_ids; sample_sources is payload-only.

## Required Evidence

- An `AnalysisSnapshot` in memory with at least one `GapArtifactRejection`.

## Inputs

- No arguments. The command reads the in-memory snapshot.

## Outputs

Full limitation packet (when snapshot exists and has rejections):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "top_limitation",
  "limitation_category": "disabled_language",
  "repair_route": "enable_language_in_config",
  "why_not_actionable": "gap artifact language is not enabled in ripr config",
  "sample_sources": ["typescript"],
  "unlock_condition": "enable_language_in_config",
  "non_claims": [
    "not a Rust repair packet",
    "does not indicate the behavior is reachable",
    "does not indicate tests are absent"
  ],
  "limits_note": "Static evidence only; advisory, not a gate decision."
}
```

No-limitation sentinel (when no snapshot exists):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "top_limitation",
  "status": "no_limitation"
}
```

Null (when snapshot exists but `gap_artifact_rejections` is empty): the
command returns null.

## Acceptance Examples

1. No snapshot → `Some(value)` with `status == "no_limitation"`, not null.
2. Snapshot with `DisabledLanguage("typescript")` rejection → full
   packet with `limitation_category == "disabled_language"`, non-empty
   `repair_route`, non-empty `why_not_actionable`, `non_claims` is array,
   `limits_note` present, no mutation-runtime vocabulary.
3. Capabilities list contains exactly 7 commands including
   `ripr.collectTopLimitation`.

## Test Mapping

- `crates/ripr/src/lsp/tests.rs::execute_command_collect_top_limitation_no_snapshot_returns_no_limitation`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_top_limitation_with_rejection_returns_limitation`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_top_limitation_registered_in_capabilities`

## Implementation Mapping

- `crates/ripr/src/lsp.rs` — `COLLECT_TOP_LIMITATION_COMMAND` constant.
- `crates/ripr/src/lsp/capabilities.rs` — command registered (7 total after RIPR-SPEC-0081).
- `crates/ripr/src/lsp/backend.rs` — `collect_top_limitation`,
  `limitation_sample_sources`, `limitation_non_claims`.

## CI Proof

- `cargo build -p ripr` clean.
- `cargo test -p ripr lsp` (incl. new tests + 6-command capabilities).

## Metrics

- Gate: all three acceptance tests pass.
- Promote to accepted when the command ships in a tagged release and an
  agent exercises it end-to-end.

## Failure Modes

- No snapshot → `no_limitation` sentinel (safe fallback, not null).
- No rejections in snapshot → null (nothing to report).
- Lock failure → null (graceful degradation; never panics).
