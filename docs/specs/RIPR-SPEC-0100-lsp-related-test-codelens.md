# RIPR-SPEC-0100: LSP Related-Test CodeLens

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- None

Linked PRs:

- None yet

Support-tier impact:

- Adds an advisory, display-only LSP codeLens above each changed symbol citing
  the static related-test count from the cached analysis snapshot. Informational
  surface only; no new diagnostic, no gate, no tier change.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, or LSP servers.
- New source module `crates/ripr/src/lsp/lens.rs` (pure helpers, no I/O).
- Adds `CodeLensOptions { resolve_provider: Some(false) }` to
  `initialize_result()`; no resolve round-trip.
- No JSON output schema change; no schema version bump.
- Register this spec in `policy/doc-artifacts.toml`.

## Problem

The LSP cockpit surfaces diagnostics, hover, code actions, and commands but
gives no inline summary of how many related tests ripr found for a changed
symbol. Developers must open the repair-packet or read the JSON output to
learn the count. A VSCode codeLens is a low-friction, display-only way to
surface that count directly above the symbol without adding a diagnostic or
changing the gate posture.

## Behavior

### Advisory-only contract

The codeLens is strictly informational:

- It is **never** a diagnostic. Clients MUST NOT treat absence of a lens as an
  error or gate failure.
- The lens count comes exclusively from the cached analysis snapshot; there is
  no live re-analysis triggered by the codeLens request.
- When no snapshot is available (`snapshot == None`) the handler returns an
  empty list — never a fabricated zero-count lens.
- When `N == 0`, the title reads `"ripr: no related tests found (CLASS)"`.
  The word "no related tests found" is the correct static phrase; the
  runtime-mutation-testing synonym is never used.

### Honesty rules

The lens title MUST comply with the static-language vocabulary:

| Condition | Required phrasing |
|---|---|
| `snapshot == None` | Return empty `Vec` (no lens at all) |
| `N == 0` | `"ripr: no related tests found (CLASS)"` |
| `N >= 1`, class is `exposed` | `"ripr: N related tests (exposed) · static, cached"` |
| `N >= 1`, class is not `exposed` | `"ripr: N related tests, no static discriminator (CLASS) · static, cached"` |
| Any TS/Python finding (preview) | Prefix with `"preview: "` |

Forbidden words in any codeLens title (enforced by `check-static-language`):
`killed`, `survived`, `proven`, `adequate`, `covered`, `passing`, and the
runtime-mutation-testing synonym for "no related tests found".

### Server capabilities

`initialize_result()` in `lsp/capabilities.rs` advertises:

```json
"codeLensProvider": { "resolveProvider": false }
```

No `workDoneProgress` support is needed. `resolve_provider: Some(false)` is
the only field in `CodeLensOptions` (tower-lsp-server 0.23.0 / ls-types 0.0.6).

### Data source

Count = `finding.related_tests.len()` on the cached `AnalysisSnapshot`.
No projection, no inference, no diagnostic count — strictly the populated
`Vec<RelatedTest>` from the last `check_workspace` run.

### URI matching

`code_lens_response(uri, snapshot)` filters the snapshot's `findings` list
to those whose `probe.location.file` resolves (relative to `snapshot.root`)
to the same URI as `params.text_document.uri`, using the existing
`file_uri_for_path` + `file_uris_match` helpers from `lsp/uri.rs`.

### Range

The lens is placed at the line reported by `finding.probe.location.line`
(1-indexed; converted to 0-indexed for LSP). Character range is 0..0 (start
of line) — the standard VS Code position for a codeLens.

### Command object

Each `CodeLens` carries `command: Some(Command { title, command: String::new(), arguments: None })`.
A `Command` with an empty `command` string is display-only in VS Code; the
client shows the title as a read-only annotation without registering a handler.
`data: None` (resolve is disabled).

## Architecture

```
lsp/backend.rs          <- thin handler (mutex + delegate)
lsp/lens.rs             <- pure helpers: code_lens_response, finding_to_code_lens,
                           finding_matches_uri, related_test_lens_title
lsp/capabilities.rs     <- CodeLensOptions advertisement
lsp/uri.rs              <- reused unchanged
```

`lsp/lens.rs` imports only from `domain/` (via `super::` through `lsp/`)
and from `lsp/uri.rs` (same module). It does not import from `analysis/`,
`output/`, or `app.rs`. This is consistent with the LSP module boundary
enforced by `cargo xtask check-architecture`.

## Required Evidence

Unit tests in `crates/ripr/src/lsp/lens.rs` (pure helpers):

1. `code_lens_response_no_snapshot_emits_nothing` — when `snapshot == None` the
   handler must return empty `Vec`, not a fabricated 0-count lens.

2. `code_lens_response_filters_to_requested_file` — snapshot has findings for two
   different files; request for file A must return only file A's lens.

3. `code_lens_response_zero_related_tests_is_honest` — `N == 0`,
   class `no_static_path`. Title must contain "no related tests found" and the
   class label; must NOT contain the runtime-mutation-testing synonym for absence
   of related tests.

4. `code_lens_response_reports_real_related_test_count` — `N == 3`,
   class `exposed`. Title must contain "3" and "exposed". Lens placed at
   0-indexed line 9 (probe at line 10).

5. `code_lens_response_preview_finding_softens` — `N == 1`, class `exposed`,
   `language_status == Preview` (TypeScript). Title must contain "preview".

6. `code_lens_response_uncertain_class_does_not_claim_grip` — `N == 2`,
   class `reachable_unrevealed`. Title must contain "no static discriminator"
   and "reachable_unrevealed".

7. `code_lens_response_static_language_clean` — iterates all seven `ExposureClass`
   variants plus preview, checks every generated title against
   `lens_title_is_static_language_clean`.

Unit tests in `crates/ripr/src/lsp/tests.rs` (capability + wiring):

8. `capabilities_advertise_code_lens_provider` — `initialize_result()` must
   return `code_lens_provider == Some(CodeLensOptions { resolve_provider: Some(false) })`.

9. `backend_code_lens_handler_delegates_to_lens_helper` — builds a real
   `AnalysisSnapshot` with one finding, calls `code_lens_response` directly
   (the pure helper that the async handler delegates to), verifies at least one
   lens is returned and its title cites "1" and passes the static-language check.

No new golden fixture files are required (the lens is LSP-only; `check --json`
output is unchanged).

## Non-Goals

- No resolve round-trip: `resolve_provider` is `false`; no `code_lens_resolve` handler.
- No VS Code extension changes for MVP; the lens appears automatically once the
  server advertises the capability.
- No live re-analysis on `textDocument/codeLens`; the handler reads the cached
  snapshot only.
- No per-lens navigation: clicking a lens does nothing (empty command string).
  A future PR may wire `OPEN_RELATED_TEST_COMMAND` to the lens data field.
- No lens invalidation on file save; the existing snapshot-refresh model covers
  this transparently.
- No aggregation lens: one lens per finding (matching the current diagnostics
  model). Aggregation into a single per-file count is a separate UI concern.
- No Python-specific label differentiation beyond the preview prefix; the same
  softening logic covers both TS and Python for now.
- No JSON output schema changes; codeLens is LSP-only.

## Acceptance Examples

### Snapshot absent — no fabricated lens

```
request:  textDocument/codeLens for any URI
state:    latest_analysis == None (no refresh has run)
result:   [] (empty array, not a 0-count lens)
```

### Rust Exposed, 3 related tests

```
finding:  class=exposed, related_tests.len()=3, language_status=None
title:    "ripr: 3 related tests (exposed) · static, cached"
```

### Rust NoStaticPath, 0 related tests

```
finding:  class=no_static_path, related_tests.len()=0, language_status=None
title:    "ripr: no related tests found (no_static_path) · static, cached"
```

### TypeScript Preview, Exposed, 1 related test

```
finding:  class=exposed, related_tests.len()=1, language_status=Some(Preview)
title:    "preview: 1 related tests (preview, exposed) · static, cached"
```

### URI mismatch — filter works

```
snapshot: one finding for src/a.rs, one for src/b.rs
request:  textDocument/codeLens for src/a.rs
result:   [lens for a.rs only]
```

## Test Mapping

- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_no_snapshot_emits_nothing`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_filters_to_requested_file`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_zero_related_tests_is_honest`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_reports_real_related_test_count`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_preview_finding_softens`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_uncertain_class_does_not_claim_grip`
- `crates/ripr/src/lsp/lens.rs::tests::code_lens_response_static_language_clean`
- `crates/ripr/src/lsp/tests.rs::capabilities_advertise_code_lens_provider`
- `crates/ripr/src/lsp/tests.rs::backend_code_lens_handler_delegates_to_lens_helper`

## Implementation Mapping

- `crates/ripr/src/lsp/lens.rs` — `code_lens_response`, `finding_to_code_lens`, `finding_matches_uri`, `related_test_lens_title`, `lens_title_is_static_language_clean` (cfg(test))
- `crates/ripr/src/lsp/capabilities.rs` — `CodeLensOptions { resolve_provider: Some(false) }` in `initialize_result()`
- `crates/ripr/src/lsp/backend.rs` — `code_lens` handler delegating to `code_lens_response`
- `crates/ripr/src/lsp.rs` — `mod lens;` module declaration

## Metrics

- `lsp_code_lens_advisory_count_per_document` — advisory count of lenses returned per codeLens request
