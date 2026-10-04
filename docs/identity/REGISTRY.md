# RIPR governed identity registry

Generated from `crates/ripr/src/domain/identity`. Do not edit by hand.

This table is the human projection of the machine-readable registry. It names
authorities and compatibility posture. It does not prove that consumers have
migrated, and it does not change actionability, currentness, transport
behavior, or support claims.

| Identity | Owner | Class | Invalidation | Canonical fields | Aliases |
| --- | --- | --- | --- | --- | --- |
| `ActionId` | `(fingerprint in lsp::action_contract::action_data; typed wrapper is #4806)` (#1892/#1661/#4806) | `session-bound` | Action class, addressed canonical identity, command id, or stable action name change. Title text must not. | `action_id` | — |
| `AnalysisAttemptId` | `lsp refresh generation echoed as snapshot_id (not #1602 handle)` (#1603/#1765/#4807) | `session-bound` | A new accepted refresh. Scheduler coalescing must not mint a completed snapshot handle. | `snapshot_id` | — |
| `CanonicalItemId` | `analysis::canonical_gap::CanonicalGapIdentity` (#1932/#4804 (records PR-era Lane 1 gap identity; typed wrapper is #4805)) | `portable` | Owner, seam kind, flow sink, missing discriminator, or assertion shape change. Line numbers are locators, not identity. | `canonical_gap_id` | `gap_id` |
| `CommandId` | `domain::command_spec::CommandSpec.command_id` (#1617/#1754/#4808) | `portable` | Semantic argv, role, or policy change. Display-string-only changes must not. | `command_id` | — |
| `CompletedAnalysisSnapshotId` | `analysis_outcome::AnalysisIdentity.snapshot_identity` (#1765/#1602/#4807) | `snapshot-bound` | Any completed-analysis input or result commitment change. Request order and scheduler generation do not define this id. | `snapshot_identity` | — |
| `ContinuationId` | `(request continuation_id / success continuation_identity; typed wrapper is #4807)` (#1603/#1899/#4807) | `transport-bound` | Snapshot, profile, budget, or page-order change. | `continuation_id` | `continuation_identity` |
| `DiagnosticCodeId` | `LSP Diagnostic.code / diagnostic data diagnostic_id` (#1892/#4804 (distinct from DiagnosticResultId)) | `transport-bound` | Diagnostic code change. Result-id payload digest must not be substituted. | `diagnostic_id` | — |
| `DiagnosticResultId` | `lsp::diagnostics::DiagnosticResultIdCache` (#1565/#1566/#4807) | `transport-bound` | Snapshot, document, delivery selection, or diagnostic payload change. | — | — |
| `EditInstructionId` | `(not yet typed; #4806)` (#1571/#1895/#4806) | `snapshot-bound` | Any source, range, or document-version change. Cannot survive an edit. | — | — |
| `FeedbackReceiptId` | `domain::feedback::FeedbackReceipt.feedback_id / identity.receipt_id` (#4585/#4804 (same field name as repair ReceiptId; different authority)) | `snapshot-bound` | Referenced snapshot or judgment identity change. Timestamp is outside payload identity. | `receipt_id` | — |
| `FindingId` | `domain::support::ProbeId` (#1932 (producer finding/probe identity; typed CanonicalItemId family is #4805)) | `portable` | Changed owner or probe family. Display titles do not participate. | `finding_id` | — |
| `InputIdentity` | `lsp::input_identity::LspAnalysisInputIdentity` (#1642/#1671/#2000/#4804) | `repository-bound` | Any listed semantic input change. Absolute checkout spelling is recorded separately as root_identity containment evidence. | `input_identity` | — |
| `InstructionInstanceId` | `(not yet typed; #4805)` (#1663/#1894/#4805) | `snapshot-bound` | Snapshot or source-fact change. Semantic meaning can remain stable while this instance does not. | — | — |
| `InstructionSemanticId` | `(not yet typed; #4805)` (#1663/#1894/#4805) | `portable` | The portable next-action concept changes. Client name, display text, and snapshot identity must not. | — | — |
| `ReceiptId` | `(agent/repair evidence receipt; typed wrapper is #4808)` (#1941/#4808) | `snapshot-bound` | Exact head, command/proof identity, or movement-fact change. | — | — |
| `RepairAttemptId` | `app::repair_attempt::RepairAttemptId` (#2927/#3511/#4808) | `repository-bound` | A new reserved attempt directory. May span several completed snapshots. Must not alias AnalysisAttemptId or CompletedAnalysisSnapshotId. | `repair_attempt_id` | `attempt_id` |
| `SeamLocationId` | `analysis::seams::SeamId` (#1932 (location-tied seam identity; distinct from CanonicalItemId)) | `repository-bound` | Source location or seam span change. Canonical gap grouping must not be used as a substitute. | `seam_id` | — |

## Relationships

- `ActionId` parents: `InstructionInstanceId`, `CommandId`; children: —
- `CanonicalItemId` parents: —; children: `FindingId`, `InstructionSemanticId`
- `CommandId` parents: —; children: `ActionId`, `ReceiptId`
- `CompletedAnalysisSnapshotId` parents: `InputIdentity`; children: `InstructionInstanceId`, `DiagnosticResultId`, `ContinuationId`, `FeedbackReceiptId`
- `ContinuationId` parents: `CompletedAnalysisSnapshotId`; children: —
- `DiagnosticResultId` parents: `CompletedAnalysisSnapshotId`; children: —
- `EditInstructionId` parents: `InstructionInstanceId`; children: —
- `FeedbackReceiptId` parents: `CompletedAnalysisSnapshotId`; children: —
- `FindingId` parents: `CanonicalItemId`; children: —
- `InputIdentity` parents: —; children: `CompletedAnalysisSnapshotId`
- `InstructionInstanceId` parents: `InstructionSemanticId`, `CompletedAnalysisSnapshotId`; children: `ActionId`, `EditInstructionId`
- `InstructionSemanticId` parents: `CanonicalItemId`; children: `InstructionInstanceId`
- `ReceiptId` parents: `RepairAttemptId`, `CommandId`; children: —
- `RepairAttemptId` parents: —; children: `ReceiptId`

## Adjacent identity-shaped fields

- `after_artifact_identity` on `schemas/ripr/repair-assurance.schema.json` — Repair artifact digest identity; tracked so the scanner cannot treat it as a new taxonomy id
- `attempt_id` on `crates/ripr/src/output/feedback.rs` — Caller-supplied feedback --attempt slot; must not collapse RepairAttemptId and AnalysisAttemptId
- `before_artifact_identity` on `schemas/ripr/repair-assurance.schema.json` — Repair artifact digest identity; tracked so the scanner cannot treat it as a new taxonomy id
- `diff_identity` on `schemas/ripr/check.schema.json` — Git candidate diff digest (#3278); not an agentic-LSP item/instruction/action identity
- `feedback_id` on `crates/ripr/src/output/feedback.rs` — Idempotency/store key for a usefulness receipt, not ReceiptId
- `registration_id` on `schemas/ripr/check.schema.json` — Test harness registration identity; not a CanonicalItemId
