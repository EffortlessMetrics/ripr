# RIPR-SPEC-0138: LSP CodeLens Refresh Lifecycle

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#2032](https://github.com/EffortlessMetrics/ripr-swarm/issues/2032) -
  negotiate `workspace.codeLens.refreshSupport` and refresh only on semantic
  lens-view changes.

Support-tier impact:

- No tier change. This spec adds one advisory lifecycle request so clients
  re-pull the display-only lenses defined by
  [RIPR-SPEC-0100](RIPR-SPEC-0100-lsp-related-test-codelens.md). It does not
  change any classification, finding set, ExposureClass, probe family,
  confidence score, `repair_packet_ready` authority, output schema version,
  or pass/fail behavior. Lens freshness is presentation, never a semantic or
  evidence authority.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

Relationship to RIPR-SPEC-0100:

- RIPR-SPEC-0100 owns lens *emission* (one display-only lens per finding from
  the cached snapshot; `resolve_provider: false`). This spec owns only the
  *refresh lifecycle* (when the server asks the client to re-request lenses).
  RIPR-SPEC-0100 is not amended: no statement in it conflicts, and the
  unconditional `code_lens_provider` advertisement is unchanged here.

## Problem

The server emits advisory CodeLens items from the current completed analysis
snapshot, but never tells the client when that snapshot changed. A client
that caches lenses can keep stale titles after a new snapshot commits, a
config change, a root transition, or a dirty/save transition. The standard
LSP mechanism is the server-originated `workspace/codeLens/refresh` request,
gated on the client capability `workspace.codeLens.refreshSupport`; until now
the server neither negotiated the capability nor sent the request.

## Behavior

### Capability negotiation

At `initialize` the server reads
`capabilities.workspace.codeLens.refreshSupport` (absent or `false` means
unsupported) into a session boolean, exactly like the existing diagnostic
refresh negotiation. Negotiation is from client capabilities only, never
inferred from the client name. Clients without the capability receive no
`workspace/codeLens/refresh` request; they keep their normal re-request
behavior and the honest status/hover fallbacks.

### Lens-view identity

The visible lens view is identified by a deterministic `LensViewIdentity`
computed from a committed snapshot's structured fields:

- per finding: finding id, canonical document URI, lens line (0-based),
  related-test count, classification class label, preview marker
  (`language_status == Preview`), and static limitation kind;
- the snapshot's input identity (`input_identity_id()`).

Deliberately excluded: wall-clock fields (snapshot age, refresh duration) and
rendered title text. The `· as of Xs ago` title suffix changes on every
snapshot without changing the semantic lens view, so comparing titles would
falsely trigger a refresh on every commit; only structured inputs are
compared. Findings order does not affect the identity.

### Refresh emission

After a snapshot commits in the publish path (the single choke point that
already covers snapshot completion, config change, root transition, and
dirty/save quarantine), the server compares the new lens-view identity with
the identity covered by the last sent request. It sends one
`workspace/codeLens/refresh` request only when the identity changed. The
identity comparison is the coalescing: a byte-identical re-commit sends no
request, and no timers are introduced.

A refresh request never triggers analysis, refresh scheduling, configuration
reload, or source access by itself. It is presentation-only: it never
strengthens producer actionability and never creates a repair route.

### Cleared analysis state

A root transition that clears analysis state (root removed, unavailable, or
otherwise no longer analysis-capable) makes every visible lens stale: the
visible view becomes the empty view, and the server records the cleared
identity and sends one `workspace/codeLens/refresh` for it — after the
root-transition guard is released, under the same scheduling discipline as
the deferred configuration pull. The recording is coalesced identically: a
view that is already cleared sends nothing.

### Failure behavior

A failed or declined request is logged (`window/logMessage`, warning) as a
bounded optional-client failure, never an analysis failure. There is no
retry: the new identity is recorded when the request is attempted, so the
next commit compares against the attempted view. A client failure cannot
mutate snapshot, evidence, or identity state beyond that recording.

## Required Evidence

- `code_lens_refresh_support: Mutex<bool>` on `Backend`, negotiated at
  `initialize` by `client_supports_code_lens_refresh(params)` reading
  `capabilities.workspace.codeLens.refreshSupport`.
- `LensViewIdentity` in `crates/ripr/src/lsp/lens.rs` with the field list
  above, plus `lens_view_identity(snapshot)`; wall-clock fields excluded by
  construction.
- `last_lens_view_identity: Mutex<Option<LensViewIdentity>>` on `Backend`;
  `note_lens_view_for_refresh` records a new view and reports whether it
  changed (the coalescing decision), and
  `request_code_lens_refresh_if_view_changed` sends one request per changed
  view, gated on the negotiated capability, failure log-only with no retry.
- Unit tests: capability parser (true / false / absent); identity ignores
  wall-clock-only differences; identity tracks count, class, line, preview
  marker, and input identity; findings order is not semantic.
- In-process tests: an unsupported client records and attempts no refresh; a
  supported client records the first view and advances it on a semantic
  (classification) change.
- Framed duplex test: a supported client receives exactly one
  `workspace/codeLens/refresh` after the first snapshot commit (the test
  client answers the request), no request after a byte-identical re-commit,
  and exactly one after a semantic lens-view change.

## Non-Goals

- Gating the `code_lens_provider` advertisement on refresh support (a
  separate behavior change; lenses remain poll-able without refresh).
- CodeLens resolve, edits, or any change to lens titles or content
  (RIPR-SPEC-0100 owns emission).
- A typed `ClientFeatureProfile` (#1987 has not landed; this spec adds the
  one negotiated boolean consistent with the current ad-hoc pattern).
- Refresh scheduling, timers, or debounce beyond identity comparison.
- Editor extension changes; client behavior is out of scope.

## Acceptance Examples

### Unsupported clients receive no request

```text
Given a client that did not advertise workspace.codeLens.refreshSupport,
when a snapshot commits,
then no workspace/codeLens/refresh request is sent,
and the client keeps its normal re-request behavior.
```

### First commit sends one request

```text
Given a client that advertised refreshSupport,
when the first snapshot with findings commits,
then exactly one workspace/codeLens/refresh request arrives.
```

### Byte-identical re-commit sends nothing

```text
Given a supported client whose first snapshot already committed,
when an explicit refresh re-commits byte-identical analysis state,
then the rendered title age changed but no new request is sent.
```

### Semantic change sends one request

```text
Given a supported client with a committed lens view,
when a new snapshot changes a finding's classification or related-test
count,
then exactly one new workspace/codeLens/refresh request arrives.
```

### Failure is bounded

```text
Given a supported client,
when the refresh request fails or is declined,
then a warning is logged, no retry is attempted,
and analysis state is unchanged.
```

## Test Mapping

- `crates/ripr/src/lsp/capabilities.rs::tests::code_lens_refresh_follows_workspace_capability`
  — refreshSupport true / false / absent negotiation.
- `crates/ripr/src/lsp/lens.rs::tests::lens_view_identity_ignores_wall_clock_age`
  — a wall-clock-only snapshot difference does not change the identity.
- `crates/ripr/src/lsp/lens.rs::tests::lens_view_identity_tracks_semantic_lens_fields`
  — count, class, line, preview marker, and input identity are semantic;
  findings order is not.
- `crates/ripr/src/lsp/tests.rs::code_lens_refresh_is_not_attempted_for_unsupported_clients`
  — unsupported client records and attempts no refresh.
- `crates/ripr/src/lsp/tests.rs::code_lens_refresh_tracks_semantic_view_changes_for_supported_clients`
  — supported client records the first view and advances it on a
  classification change.
- `crates/ripr/src/lsp/tests.rs::framed_code_lens_refresh_follows_semantic_lens_view_changes`
  — framed duplex: exactly one request after the first commit, none after a
  byte-identical re-commit, exactly one after a semantic lens-view change;
  the test client answers each request.

## Implementation Mapping

- `crates/ripr/src/lsp/capabilities.rs` —
  `client_supports_code_lens_refresh` negotiation parser.
- `crates/ripr/src/lsp/lens.rs` — `LensViewIdentity` and
  `lens_view_identity`.
- `crates/ripr/src/lsp/backend.rs` — `code_lens_refresh_support` session
  boolean, `last_lens_view_identity`, `initialize` negotiation,
  `note_lens_view_for_refresh`, and the post-commit
  `request_code_lens_refresh_if_view_changed` call in the publish path.

## Metrics

- Unit and integration tests listed above pass under `cargo test -p ripr`.
- `cargo xtask goldens check` remains clean: the change is LSP-only and
  additive; it touches no CLI goldens.
- `cargo xtask check-static-language` clean: log and disclosure strings use
  conservative language only.
