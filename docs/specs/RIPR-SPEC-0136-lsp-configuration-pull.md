# RIPR-SPEC-0136: LSP Configuration Pull

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#2031](https://github.com/EffortlessMetrics/ripr-swarm/issues/2031) - add a
  server-originated `workspace/configuration` pull model with a documented
  fallback for LSP session configuration.
- [#2303](https://github.com/EffortlessMetrics/ripr-swarm/issues/2303) -
  amendment: add a sixth governed session key, `gitTimeoutMs`, the
  cooperative per-invocation git deadline for the refresh path.

Support-tier impact:

- No tier change. This spec adds one LSP transport for the governed session
  keys (five at ratification; the #2303 amendment adds a sixth,
  `gitTimeoutMs`) plus additive status disclosure fields. It does not
  change any classification, finding set, ExposureClass, probe family,
  confidence score, `repair_packet_ready` authority, output schema version,
  or pass/fail behavior.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

The LSP server learns session configuration only from initialization options
and pushed `workspace/didChangeConfiguration` settings. Clients that support
the standard `workspace/configuration` pull model expect the server to ask
for its configuration section; for those clients, pushed settings may never
arrive and initialization options are a one-shot, restart-only channel. The
server needs a capability-negotiated pull transport with an honest startup
window and a documented fallback, without widening the governed key set.

## Behavior

### Negotiation

At `initialize` the server negotiates a `ConfigurationMode` from client
capabilities only — never from the client name:

- `pull`: `capabilities.workspace.configuration` is `true`.
- `push_fallback`: pull is unavailable but the client advertises
  `workspace/didChangeConfiguration` support.
- `initialization_only`: the client neither pulls nor advertises push
  support.

### Pull

In pull mode the server sends, from the `initialized` handler (never from
`initialize`, where client requests are rejected with `-32002`), one
`workspace/configuration` request for the bounded section:

```text
ConfigurationItem { scope_uri: <selected root URI>, section: "ripr" }
```

The section contains exactly the governed session keys allowlisted for
initialization options: `baseRef`, `checkMode`, `includeUnchangedTests`,
`seamDiagnostics`, `diagnosticProfile`, and — per the #2303 amendment —
`gitTimeoutMs`.

The response is validated before anything is applied:

- the array must contain exactly one item matching the single requested
  section;
- a `null` item means the client holds no `ripr` settings (valid, no
  overrides);
- an object item fails the whole pull (fail-closed) when a supported key has
  the wrong JSON type or an unknown enum literal; unsupported keys are
  outside the governed section and ignored;
- each string value is bounded (4096 bytes): pulled settings arrive outside
  the initialization-options ingress bound (RIPR-SPEC transport bounds,
  #2034), so an oversized value fails the pull instead of being stored and
  re-rendered.

### Precedence (amends RIPR-SPEC-0007, scoped)

In pull mode, the governed keys resolve per key:

```text
valid pulled setting > initialization option > ripr.toml > built-in default
```

Initialization options are the compatibility fallback for keys the pull did
not return. Pulled values are retained as a distinct layer (like retained
session options) so a repository-config reload re-applies this contract. In
push-fallback and initialization-only modes the RIPR-SPEC-0007 precedence is
unchanged: initialization options and pushed values win over `ripr.toml`.

### Change notifications and epoch guard

`workspace/didChangeConfiguration` in pull mode does not apply pushed values
directly. It invalidates the retained pulled layer by bumping a pull epoch
(mirroring `workspace_root_epoch`) and schedules ONE coalesced re-pull: at
most one request is in flight, and a schedule request arriving during a pull
collapses into a single queued re-pull. Responses arriving for an older epoch
are dropped. A re-pull whose validated values leave the effective settings
semantically unchanged does not reschedule analysis; the retained layer is
still updated so precedence and source disclosure stay correct.

### Startup-window honesty

Until the first pull resolves, the analysis status payload discloses the
pull state as `pending`; with no single selected workspace root the pull is
`deferred` (analysis is already blocked there, and the pull is retried when
a single root is selected). Pulled settings are scoped to the root URI they
were pulled for: any change of the effective root bumps the pull epoch so
an in-flight response for the old root is dropped, and a transition that
lands on an analysis-capable root — including an `A → unavailable → B`
sequence — schedules exactly one re-pull scoped to that root. A failed or
malformed pull discloses a typed
state (`config_pull_failed` / `config_pull_invalid`, mirroring the
`AnalysisFailure { kind, message }` pattern) with a recovery route, while
the last-known-good pulled layer is retained as stale. Defaults never
masquerade as accepted requested settings: the status payload carries the
negotiated `configuration_mode`, a per-field source map
(`pulled` | `initialization` | `repo` | `default`) for the governed keys
(six per the #2303 amendment), and the last pull state, epoch, failure, and
recovery route. All status fields are additive and snake_case.

A pull is a pure LSP round-trip: it never launches analysis, git, network
beyond the LSP connection, or edits. Applying changed effective settings
reschedules analysis only through the existing config-reload invalidation
path, exactly as a pushed configuration change does today.

### Fallback

Clients without `workspace.configuration` support keep today's behavior:
`workspace/didChangeConfiguration` keeps applying pushed values
(push fallback), and clients that neither pull nor push usable values run
initialization-only. Each transport can supply exactly the same governed
keys (six per the #2303 amendment); no transport can set anything else.

### Cooperative git invocation deadline (#2303 amendment)

The sixth governed key, `gitTimeoutMs` (non-negative integer, default
`30000`), bounds every git invocation the LSP refresh path spawns: the
scheduler's Git-input resolution probe and the `git diff` load inside
analysis. It is a resource bound, not an analysis input — it never changes
what the analysis computes, only whether a hung git invocation aborts the
run. The CLI passes no deadline and stays unbounded, byte-identical to the
pre-#2303 behavior.

Semantics:

- every git spawn goes through the shared `crates/ripr/src/git.rs`
  authority, which polls the child on a short interval, checks cooperative
  analysis cancellation each tick (a superseded refresh kills a hung git),
  drains piped stdout/stderr so a verbose child cannot deadlock against the
  wait, and kills + reaps the child on deadline expiry;
- an expired deadline yields the named, matchable error
  `git_invocation_timeout`; a zero deadline fails before spawning;
- a diff load that fails with the named timeout commits a LIMITED snapshot
  — zero findings plus one typed failed `diff` component outcome
  (`kind: git_invocation_timeout`, `findings_trustworthy: false`, recovery
  `retry ripr.refreshDiagnostics`) so `run_status` derives `limited` — instead
  of dropping the refresh with no snapshot. Only the named timeout error
  converts; every other analysis failure keeps the no-snapshot path;
- a scheduler probe that exceeds the deadline fails closed exactly like an
  unresolvable ref (`Unresolved` / loader-default with no commit) — a
  timed-out probe is never mistaken for a resolved input;
- initialization and pushed values are lenient (a malformed `gitTimeoutMs`
  is ignored, keeping the current value); pulled values validate
  fail-closed (a non-integer fails the whole pull). `gitTimeoutMs` has no
  `ripr.toml` slot, so its disclosed source is only `pulled`,
  `initialization`, or `default`.

## Required Evidence

- Capability predicate `client_supports_configuration_pull` and mode
  negotiation in `crates/ripr/src/lsp/capabilities.rs`, from
  `capabilities.workspace.configuration` only.
- Validated retained pulled layer in `crates/ripr/src/lsp/config.rs` with
  the per-key precedence contract and reload preservation.
- Pull scheduling, epoch guard, coalescing, and deferred-pull retry in
  `crates/ripr/src/lsp/backend.rs`; typed `ConfigPullState` in
  `crates/ripr/src/lsp/state.rs`.
- Additive `input_authority` status disclosure: `configuration_mode`,
  `session_value_sources`, and `configuration_pull` (state, epoch, failure,
  recovery route).
- End-to-end duplex-socket test where a fake client answers
  `workspace/configuration`, plus a malformed-response re-pull test.

## Non-Goals

- A full typed client feature profile (that is #1987's scope).
- New output schema versions, or changes to `ripr.toml` parsing. (The
  ratified text also excluded new governed configuration keys; the #2303
  amendment deliberately adds exactly one, `gitTimeoutMs`, with the
  semantics above.)
- Dynamic registration for `workspace/didChangeConfiguration`.
- Per-resource `scope_uri` pulls; the scope is always the selected root URI.
- Editor extension behavior changes; the extension keeps sending
  initialization options and pushed settings. (The #2303 amendment adds one
  additive contributed setting declaration, `ripr.gitTimeoutMs`, so
  pull-mode clients can serve the key; it wires no new extension logic.)

## Acceptance Examples

### Pull overrides initialization for returned keys

```text
Given the client advertises workspace.configuration = true,
and initializationOptions checkMode = "fast",
when the initialized pull returns {"checkMode": "ready"},
then the session uses ready mode,
and input_authority.session_value_sources.check_mode is "pulled".
```

### Initialization options fill keys the pull did not return

```text
Given the same session,
when the pull returns only {"checkMode": "ready"},
then baseRef still comes from initializationOptions,
and session_value_sources.base_ref is "initialization".
```

### Malformed pull fails closed and retains last-known-good

```text
Given a session with an applied pull,
when a re-pull returns {"checkMode": 42},
then the pull state discloses failed with kind config_pull_invalid,
the last-known-good pulled layer stays in effect,
and analysis is not rescheduled by the pull.
```

### Fallback is unchanged without pull support

```text
Given a client without workspace.configuration support,
when the session starts,
then configuration_mode is push_fallback or initialization_only,
didChangeConfiguration keeps applying pushed values,
and no workspace/configuration request is ever sent.
```

### Git deadline applies, discloses, and fails honestly (#2303 amendment)

```text
Given initializationOptions gitTimeoutMs = 45000,
when the session applies the option,
then the refresh path bounds each git invocation at 45s,
and input_authority.session_value_sources.git_timeout_ms is "initialization".

Given a pull returns {"gitTimeoutMs": "45000"} (wrong JSON type),
when the response is validated,
then the whole pull fails closed and the previous deadline stays in effect.

Given a diff-load git invocation exceeds the configured deadline,
when the refresh would previously have dropped with no snapshot,
then a limited snapshot commits with zero findings,
and one failed diff component outcome with kind git_invocation_timeout,
findings_trustworthy false, and recovery "retry ripr.refreshDiagnostics",
and run_status derives "limited".

Given a scheduler Git-input probe exceeds the configured deadline,
when the probe cannot complete,
then the record fails closed as Unresolved (or loader-default with no
commit), never as a fabricated resolved input.
```

## Test Mapping

- `crates/ripr/src/lsp/capabilities.rs::tests::configuration_mode_follows_workspace_capabilities`
  — negotiation from capabilities only; empty capabilities are
  initialization-only.
- `crates/ripr/src/lsp/config.rs::tests::validated_pulled_options_*` —
  item-count, null-item, type/enum validation, fail-closed cases.
- `crates/ripr/src/lsp/config.rs::tests::pulled_settings_override_initialization_options_for_returned_keys`
  and `empty_pull_layer_restores_initialization_and_repo_precedence` — the
  precedence contract.
- `crates/ripr/src/lsp/config.rs::tests::repository_reload_preserves_pulled_overrides`
  and `session_option_change_preserves_pulled_layer` — retained-layer
  semantics across reloads.
- `crates/ripr/src/lsp/config.rs::tests::effective_settings_eq_ignores_layer_representation`
  — the no-reschedule guard.
- `crates/ripr/src/lsp/config.rs::tests::session_value_sources_disclose_per_field_origin`
  — per-field source disclosure.
- `crates/ripr/src/lsp/tests.rs::initialization_only_mode_discloses_transport_and_value_sources`
  and `pull_mode_is_pending_until_the_first_pull_resolves` — status
  disclosure and startup-window honesty.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_configuration_pull_applies_and_discloses_pull_state`
  — end-to-end pull over a duplex socket: bounded section request, applied
  state, no analysis launched, coalesced re-pull on
  `workspace/didChangeConfiguration`, typed malformed-pull disclosure with
  retained last-known-good.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_deferred_configuration_pull_runs_after_root_transition_guard_release`
  — the deferred pull is scheduled only after the root-transition guard is
  released: an ambiguous-root start defers the pull, selecting a single root
  runs it with the new root's `scopeUri`, and a settings-changing apply must
  reach the refresh path without deadlocking on the transition guard.
- #2303 amendment:
  `crates/ripr/src/git.rs::tests::deadline_kills_and_reaps_a_hung_invocation`,
  `zero_deadline_errors_before_spawning`,
  `cancellation_wins_over_a_long_deadline`, and
  `output_larger_than_the_pipe_buffer_does_not_deadlock` — the shared
  deadline-aware git invocation contract.
- `crates/ripr/src/lsp/config.rs::tests::git_timeout_ms_applies_from_initialization_options`,
  `git_timeout_ms_defaults_to_thirty_seconds`,
  `git_timeout_ms_malformed_initialization_value_keeps_default`,
  `validated_pulled_options_rejects_wrong_typed_git_timeout_ms`,
  `session_value_sources_disclose_git_timeout_ms_origin`, and
  `effective_settings_eq_compares_git_timeout` — the sixth governed key.
- `crates/ripr/src/lsp/diagnostics.rs::git_timeout_error_converts_to_a_committed_limited_snapshot`
  and `non_timeout_analysis_errors_are_not_converted` — the limited-snapshot
  conversion and its named-error guard.
- `crates/ripr/src/lsp/git_inputs.rs::tests::timed_out_probe_fails_closed_as_unresolved`
  — the scheduler probe fail-closed contract.
- `crates/ripr/src/analysis/diff/load.rs::tests::zero_deadline_diff_load_fails_with_the_named_timeout_error`
  and `zero_deadline_base_probes_fail_closed_instead_of_hanging` — the
  diff-load named error and probe fail-closed behavior.
- `crates/ripr/src/app/check.rs::tests::cli_check_input_carries_no_git_deadline`
  — the CLI no-drift contract (no deadline, unbounded, byte-identical).

## Implementation Mapping

- `crates/ripr/src/lsp/capabilities.rs` — `ConfigurationMode`,
  `client_supports_configuration_pull`, `configuration_mode`.
- `crates/ripr/src/lsp/config.rs` — retained `pulled_options` layer,
  `validated_pulled_options`, `with_pulled_options`,
  `effective_settings_eq`, `session_value_sources`.
- `crates/ripr/src/lsp/state.rs` — `ConfigPullState` with failure and
  recovery-route disclosure.
- `crates/ripr/src/lsp/backend.rs` — mode negotiation at `initialize`, first
  pull at `initialized`, coalesced re-pull with epoch guard, deferred-pull
  retry on root selection, additive `input_authority` status fields.
- #2303 amendment: `crates/ripr/src/git.rs` — the shared deadline-aware git
  invocation authority (`run_git_output_with_deadline`,
  `GIT_INVOCATION_TIMEOUT_PREFIX`, `poll_child`);
  `crates/ripr/src/analysis/diff/load.rs` — deadline threading through the
  diff-load exports; `crates/ripr/src/lsp/diagnostics.rs` — the
  limited-snapshot conversion; `crates/ripr/src/lsp/git_inputs.rs` — the
  fail-closed probe deadline; `crates/ripr/src/lsp/config.rs` — the
  `gitTimeoutMs` governed key.

## Metrics

- Unit and integration tests listed above pass under `cargo test -p ripr`.
- `cargo xtask goldens check` remains clean: the status payload change is
  additive and touches no CLI goldens.
- `cargo xtask check-static-language` clean: disclosure strings use
  conservative language only.
