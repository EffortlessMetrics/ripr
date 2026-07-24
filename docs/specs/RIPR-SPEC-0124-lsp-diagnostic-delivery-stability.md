# RIPR-SPEC-0124: LSP Diagnostic Delivery Stability

Status: accepted

Owner: product / swarm

Created: 2026-07-12

Linked issues:

- #1565
- #1573

## Problem

The saved-workspace LSP path must not republish an equivalent diagnostic
payload merely because a refresh, traversal, or map iteration occurred again.
This is a server-side delivery and cache-stability contract. It does not claim
any particular editor or agent prompt-cache improvement.

## Behavior

Before publication, ripr canonicalizes each URI's diagnostics:

- diagnostics are ordered by stable diagnostic identity, range, and serialized
  fallback fields;
- exact duplicate payloads are removed;
- the complete current per-URI snapshot is retained for the next comparison;
- only URIs whose canonical payload changed are published; and
- only URIs that disappeared are cleared.

Each diagnostic carries `data.diagnostic_id`. The identity is derived from a
canonical gap, or from stable semantic fields such as repository-relative file,
owner, probe family, sink, and expression. It must not contain a workspace
absolute root, wall-clock time, refresh generation, duration, or map order.
Visible LSP ranges remain locators and may change when a line moves.

Navigation fields may retain absolute file URIs where the protocol requires
them. A normalized payload digest treats those URIs and other path-bearing
fields as repository-relative `repo://` values so equivalent checkouts can be
compared.

Refresh telemetry reports computed files, published files, unchanged files,
cleared files, published payload bytes, and suppressed payload bytes.

`did_change` remains document-state only. Analysis and publication continue to
advance on the saved-workspace open/save/close and explicit refresh paths.

## Required Evidence

- An unchanged refresh produces no publish calls for unchanged URIs.
- A changed URI is published without republishing unchanged URIs.
- Removed URIs are cleared without clearing surviving URIs.
- Diagnostic ordering and duplicate removal are deterministic.
- Equivalent roots produce equal diagnostic identities and equal normalized
  payload digests.
- The existing LSP fixture contract includes the additive identity field.
- Refresh logs expose published versus suppressed file and byte counts.

## Non-Goals

- No client-side prompt-cache or editor performance claim.
- No diagnostic classification, gate-policy, or severity-policy change.
- No analysis of unsettled buffers or new debounce behavior.
- No pull-diagnostics protocol implementation; that is #1566.

## Acceptance Examples

1. Repeating a saved-workspace refresh with the same canonical diagnostics
   produces no publish or clear calls.
2. Changing one diagnostic URI publishes only that URI and retains the other
   URI's cached payload.
3. Moving an equivalent checkout root changes navigation URIs but not the
   diagnostic identity or normalized payload digest.
4. A removed diagnostic URI receives one clear call and surviving URIs are not
   cleared.

## Test Mapping

- Refresh-plan tests cover unchanged, changed, and removed URI behavior.
- Delivery tests cover stable sorting, exact duplicate removal, and equivalent
  root digest normalization.
- The boundary-gap LSP fixture pins the additive diagnostic identity field.

## Claim boundary

This contract proves only deterministic server-side diagnostic delivery
behavior. It does not prove editor transport behavior, prompt-cache hit rate,
analysis latency, runtime mutation outcomes, test adequacy, or correctness.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/lsp/diagnostics.rs` | identity, canonicalization, normalized digest |
| `crates/ripr/src/lsp/backend.rs` | per-URI refresh comparison and telemetry |
| `crates/ripr/src/lsp/tests.rs` | publication-plan regression coverage |
| `fixtures/boundary_gap/expected/lsp-diagnostics.json` | wire-shape fixture |

## Metrics

- `lsp_diagnostic_computed_files`
- `lsp_diagnostic_published_files`
- `lsp_diagnostic_unchanged_files`
- `lsp_diagnostic_cleared_files`
- `lsp_diagnostic_published_payload_bytes`
- `lsp_diagnostic_suppressed_payload_bytes`
