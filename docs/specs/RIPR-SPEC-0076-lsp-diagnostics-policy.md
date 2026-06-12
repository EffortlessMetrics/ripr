# RIPR-SPEC-0076: LSP Diagnostics Severity Policy

Status: accepted

Owner: product / swarm

Created: 2026-06-11

Linked proposal:

- None

Linked ADRs:

- None

Linked plan:

- None

Linked issues:

- None

Linked PRs:

- None

Support-tier impact:

- None. This spec records the severity-mapping policy for LSP diagnostics.
  It does not promote any language, surface, or evidence class to a stronger
  support tier.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Nothing new beyond the spec itself: no new crate, binary, dependency,
  parser, runtime executor, LSP server, artifact type, or workflow.

## Problem

The LSP sidecar emitted WARNING for any repairable gap record and for
finding classes that map to `ConfigSeverity::Warning` — including advisory
findings (preview language, static limit) that carry no complete repair
packet. A WARNING without an actionable, packet-complete repair route is
decoration noise that reduces editor signal quality and violates the
cockpit principle of RIPR-SPEC-0069.

## Behavior

### Severity rules

**Finding diagnostics** (`diagnostic_for_finding_with_config`):

A finding is advisory when it has `static_limit_kind.is_some()` or
`language_status == Some(LanguageStatus::Preview)`.
Advisory findings are clamped to INFORMATION (never WARNING) because they
lack a complete repair packet by definition.

**Gap-record diagnostics** (`gap_record_diagnostic_severity`):

A gap record has a complete repair packet when:
- `repairability == "repairable"`, AND
- `verification_commands` is non-empty, AND
- `receipt_command.is_some()`.

A gap record is advisory when `language_status == "preview"` or
`static_limit_kind.is_some()`.

Severity:
- WARNING only when `has_complete_packet && !is_advisory`.
- INFORMATION otherwise.

**Seam diagnostics** (structural grip-class signals, exempt):

Seam diagnostics carry structural grip-class signals, not repair packets.
The WARNING/INFORMATION mapping is owned by `SeverityConfig` (per
`diagnostic_severity_for_grip_class_with_config`). No repair-packet
completeness check applies. This exception is documented in the code.

### Limited / stale run policy

When `snapshot_run_status` returns anything other than `"full"`
(i.e. `"stale"`, `"cache_limited"`, or `"limited"`):

- Finding diagnostics that would be WARNING are downgraded to INFORMATION.
- Seam diagnostics that would be WARNING are downgraded to INFORMATION.
- Gap-record diagnostics are suppressed entirely (none emitted).

The limited state is surfaced by `ripr.collectWorkspaceStatus`, not by
per-file diagnostic spam.

`snapshot_run_status` is computed from findings (any `static_limit_kind`
present → `"limited"`) and gap-artifact rejections
(`StaleArtifact` → `"stale"`, any other rejection → `"cache_limited"`,
nothing → `"full"`).

## Non-Goals

- HINT severity (needs a `ConfigSeverity::Hint` variant) — deferred.
- ERROR-for-regressed-gap (needs ledger diffing) — do not implement.
- No config knob for the WARNING threshold on gap records — v1 is
  hardcoded to the packet-completeness rule.

## Required Evidence

- Spec registered in `docs/specs/README.md` and `policy/doc-artifacts.toml`.
- 7 reject-list tests in
  `crates/ripr/src/lsp/diagnostics.rs::diagnostic_policy_tests`.

## Acceptance Examples

- A finding with `static_limit_kind = Some(DynamicDispatch)` and class `WeaklyExposed`
  emits a diagnostic with severity INFORMATION, not WARNING, regardless of the base
  `ConfigSeverity::Warning` mapping for that class.
- A finding with `language_status = Some(Preview)` emits INFORMATION even when the
  exposure class maps to WARNING by default config.
- A gap record with `repairability = "repairable"`, non-empty `verification_commands`,
  and `receipt_command = Some(...)`, and `language_status = "stable"` emits WARNING.
- The same gap record with `receipt_command = None` emits INFORMATION.
- A gap record with `language_status = "preview"` and an otherwise complete packet
  emits INFORMATION.
- When `snapshot_run_status` returns `"stale"` (a `StaleArtifact` rejection is
  present), no gap-record diagnostics are emitted and finding/seam WARNINGs are
  downgraded to INFORMATION.

## Test Mapping

- `no_warning_for_finding_with_static_limit` — WeaklyExposed + static_limit_kind=Some → INFORMATION.
- `no_warning_for_preview_finding` — WeaklyExposed + language_status=Preview → INFORMATION.
- `warning_only_when_gap_record_has_complete_packet` — complete packet → WARNING; missing verify or receipt → INFORMATION.
- `no_warning_for_preview_gap_record` — complete packet + language_status=preview → INFORMATION.
- `no_warning_for_static_limit_gap_record` — complete packet + static_limit_kind=Some → INFORMATION.
- `limited_run_downgrades_finding_warnings` — snapshot with static_limit finding → run_status="limited" → finding WARNING downgrades to INFORMATION.
- `stale_run_suppresses_gap_record_diagnostics` — StaleArtifact rejection → run_status="stale" → gap records suppressed (is_full_run=false).

## Implementation Mapping

- `crates/ripr/src/lsp/diagnostics.rs` — `finding_is_advisory`,
  `gap_record_has_complete_packet`, `gap_record_is_advisory`,
  `gap_record_diagnostic_severity`, `snapshot_run_status`, and the
  assembly policy in `workspace_diagnostics_with_config`.

## Metrics

- `lsp_diagnostics_policy_warning_rate_advisory_zero` — no advisory finding
  or gap record emits WARNING (enforced by test reject list).
- `unit_test_pass_rate`
