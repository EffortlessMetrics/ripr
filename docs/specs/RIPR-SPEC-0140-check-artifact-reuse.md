# RIPR-SPEC-0140: Check-Artifact Reuse for explain/context

Status: accepted

Owner: product / app

Created: 2026-07-22

Linked issues:

- [#2107](https://github.com/EffortlessMetrics/ripr-swarm/issues/2107) —
  `ripr explain` / `ripr context` re-run the full pipeline per invocation.

Linked proposals:

- [RIPR-PROP-0020](../proposals/RIPR-PROP-0020-check-artifact-reuse.md) —
  accepted design contract for this spec.

Support-tier impact:

- No tier change. A reused artifact is the same analysis result the
  producing `check` run computed; reuse changes wall time, not evidence.
  The artifact is a local, disposable derivative and must never feed a
  support-tier row, gate, badge, or proof route.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, process spawns, network access,
  workflow, or hook changes. The artifact is a local file the user names
  explicitly.

## Problem

The documented three-step flow — `check`, then `explain`, then `context`,
each with the same `--diff` — re-parses the diff and re-runs the entire
pipeline on every invocation (`explain_finding_with_config` and
`collect_context_with_config` each call `check_workspace_with_config` and
then select one finding). On a real diff in a large workspace this doubles
or triples wall time versus reusing the prior result, and the user must
re-supply identical scope flags three times.

`check --json` cannot be the reuse source: it is a one-way, lossy render
projection (related tests capped at 8, probe owner conditionally omitted,
severity baked at render time, no diff identity, no `Finding` serde
round-trip). Explaining from it would silently explain less than the
analysis knew.

## Behavior

### Explicit artifact pair, no implicit cache

`ripr check` gains `--write-artifact <path>`; `ripr explain` and
`ripr context` gain `--from <path>`. Both directions are explicit user
intent. There is no implicit cross-invocation cache and no background
write.

`--write-artifact` records a diff-scoped findings run. It fails closed
with a named limitation when combined with `--gap-ledger`, a repo-scoped
format, or managed `[perl] producer` packet generation without an explicit
`--perl-facts` packet (the generated packet is produced inside the
analysis run, after the CLI-level input is captured, so it cannot join the
recorded identity; resolving it at reuse time would re-run the producer
and defeat reuse). An explicit `--perl-facts <path>` packet is recorded
(path plus content hash) and remains supported. A failed artifact write
fails the command: the user explicitly requested the artifact.

A `--worktree` producing run is supported: its diff source is recorded as
`worktree` (the requested base, or none for dynamic default-base
resolution), and reuse re-resolves the base-to-live-working-tree diff
through git and re-hashes it. Worktree drift between write and reuse
changes the diff bytes and fails closed on `diff_bytes_hash`.

### `CheckArtifactV1` envelope, full-fidelity findings

The artifact is a schema-versioned JSON envelope
(`schema_version = "ripr-check-artifact-v1"`) with `tool`, `analyzer_version`,
an `identity` block, and the complete `Finding` set — serde derives on the
domain probe types, including the uncapped related-tests list and the
probe owner. The existing `check --json` render is unchanged.

Writes are atomic: a uniquely named temp file in the destination directory
(same filesystem, so the rename is atomic), flushed and fsynced before the
rename, and unlinked on any failure. A repeated `--write-artifact` to the
same path replaces it via rename (last writer wins); concurrent writers
use distinct temp names (pid + clock + process-local sequence) so one
writer's failure cannot tear another's artifact.

The envelope stays at `ripr-check-artifact-v1` for the `worktree` diff
source: adding an enum variant is additive — artifacts written before the
variant existed still parse under v1, and an older binary reading a new
`worktree` artifact fails closed on serde's unknown-variant error instead
of misreading it.

A `worktree` recording with no explicit base re-resolves the default base
at load time; if the default base advanced between write and reuse the
diff bytes differ and reuse fails closed on `diff_bytes_hash` — the same
accepted hazard as a `base_head` recording with dynamic default-base
resolution.

`--from` load follows the receipt read-path precedent: parse, validate
structure and `schema_version`, fail closed with a named error on any
deviation (missing or unreadable file, invalid JSON, missing or
unsupported `schema_version`, unknown or missing fields, wrong `tool`).

### Identity gate, fail closed

The artifact embeds an input identity computed at check time:

- `diff_source` — `diff_file` (canonicalized `--diff` path), `base_head`
  (requested base plus `HEAD`), or `worktree` (requested base for the
  base-to-live-working-tree diff), plus `diff_bytes_hash`, the hash of the
  exact diff bytes the analysis consumed;
- `root` (canonicalized), `mode` (resolved analysis effort profile), and
  `enabled_languages` (resolved, sorted, including the explicit
  `--perl-facts` opt-in);
- `analysis_options` — the complete `CheckInput` option surface that flows
  into `analysis_options_from_input_and_config` and is not already
  recorded elsewhere: today `include_unchanged_tests` and
  `perl_facts_path` with the packet's content hash (the Perl adapter reads
  findings-changing facts from that file). The extraction destructures
  `AnalysisOptions` without a `..` rest pattern, so a future analysis
  input option fails compilation until it is explicitly classified;
- `config_identity_version` and `config_identity_hash` — a closed,
  versioned allowlist contract over `ripr.toml`: the finding-affecting
  fields (`oracles.*`, `typescript.resolve_tsconfig_paths`, `perl.*`),
  canonically serialized with defaults materialized, sorted, and hashed.
  The classifier (`RiprConfig::check_artifact_identity_fields`)
  destructures every config struct without a `..` rest pattern, so an
  unclassified new field fails compilation; a unit test pins the role of
  every field. Any PR that adds a finding-affecting config field must
  classify it and bump `config_identity_version` in the same PR.
  Render-only knobs (severity display, `reports.max_related_tests`),
  LSP-only fields, fields not consumed by the diff-check pipeline
  (`profiles.bun_ub`, `suppressions.path`), and loader container metadata
  are excluded; fields already recorded elsewhere in the identity
  (`analysis.mode`, `analysis.include_unchanged_tests`,
  `languages.enabled`) are marked as captured, not hashed twice;
- `analyzer_version` (the writing binary's version) and the envelope
  `schema_version`.

The CLI has no diagnostic-profile surface today (that concept is LSP-only,
`lsp/input_identity.rs`); `mode` is the analysis-effort profile the CLI
identity records. If a CLI diagnostic profile lands, it must join the
identity under an envelope schema-version bump.

`explain --from` / `context --from` do not require re-supplying scope
flags: the recorded diff source is re-resolved and re-hashed (a recorded
`--diff` path is re-read; a recorded base/head pair or base-to-worktree
diff is re-resolved through git), and the root, mode, languages, analysis
options, config identity, and analyzer version are recomputed from the
invocation's working directory and config files. Scope flags passed alongside `--from`
(`--diff`, `--base`) are assertions verified against the recording, never
overrides. `explain` and `context` also accept `--mode` and
`--no-unchanged-tests`: both feed the identity recomputation (and the
fresh analysis when `--from` is absent), so an artifact recorded with a
non-default mode or with unchanged tests excluded is consumable when the
same value resolves on the reuse side (flag or `ripr.toml`) — a mismatch
is a typed error naming `mode` or
`analysis_options.include_unchanged_tests`. On any mismatch — or when the
recorded diff source no longer exists or cannot be re-resolved — the
command fails with a typed error naming every mismatched identity field.
There is no silent fallback to recompute and no "close enough" matching.

The identity gate commits to the diff bytes, config, and option surface —
not to full repository content. A source or test file edited between the
write and the reuse does not trip the gate. This is an accepted,
documented limitation: the artifact is an explicit, local, disposable
derivative under user control, and the gate exists to prevent reusing an
artifact across *invocation* changes (different diff, mode, languages, or
analysis-relevant config), not to fingerprint the repository.

### Selection and rendering are unchanged

`--from` replaces only the pipeline re-run. Finding selection
(`select_finding`) and rendering (`render_finding_with_config`,
`render_context_packet`) operate on the loaded findings exactly as on
freshly computed ones, so a reused explanation is byte-identical to a
recomputed one given the same render options. Render-time knobs
(`--max-related-tests`, severity display, output format) are not part of
the identity and are honored fresh at render time — including
`--max-related-tests` beyond the `check --json` render cap, because the
artifact stores the uncapped related-tests list.

## Required Evidence

- `CheckArtifactV1` envelope and atomic write in
  `crates/ripr/src/app/check_artifact.rs`.
- Serde round-trip derives on the domain probe types in
  `crates/ripr/src/domain/probe.rs`, `classification.rs`, `language.rs`.
- Closed config-identity allowlist in `crates/ripr/src/config/model.rs`
  with the hash in `crates/ripr/src/config.rs`; the field-enumeration
  contract test in `crates/ripr/src/config/tests.rs`.
- CLI wiring in `crates/ripr/src/cli/commands.rs` (`--write-artifact`,
  `--from`, fail-closed run-shape rejections) and help text in
  `crates/ripr/src/cli/help/core.rs`.
- Unit tests in `crates/ripr/src/app/check_artifact/tests.rs` and
  end-to-end CLI tests in `crates/ripr/tests/cli_smoke.rs` (Test Mapping).

## Non-Goals

- The per-file fact cache (#1912 / #1967) — in-process, orthogonal layer.
- LSP warm refresh (#1908) — the LSP has its own input-identity model.
- Any change to the `check --json` output contract.
- Content-addressed automatic caching or cache eviction policy.
- Managed `[perl] producer` packet generation as an artifact input: the
  generated packet cannot join the recorded identity (it is produced
  inside the analysis run) and resolving it at reuse time would re-run
  the producer, defeating reuse; the combination fails closed with a
  named limitation. An explicit `--perl-facts <path>` packet is fully
  supported.
- Full-repository content fingerprinting in the identity gate (see the
  documented limitation above).
- Sharing artifacts between machines or committing them; the artifact is
  a local, disposable derivative and must never feed a support-tier row,
  gate, badge, or proof route.

## Acceptance Examples

### Write then reuse without scope flags

```text
Given `ripr check --diff X --write-artifact a.json` succeeded,
when `ripr explain --from a.json probe:<id>` runs with no scope flags,
then it re-reads and re-hashes the recorded diff file,
and renders the finding byte-identically to a fresh
`ripr explain --diff X probe:<id>` without re-running the pipeline.
```

### Identity mismatch fails closed naming the field

```text
Given the artifact above,
when the recorded diff file changes on disk,
then `ripr explain --from a.json probe:<id>` fails closed with
"cannot be reused: identity mismatch on diff_bytes_hash",
and never silently recomputes.
```

### Scope flags alongside --from are assertions

```text
Given the artifact above recorded `--diff X`,
when `ripr explain --from a.json --diff Y probe:<id>` runs,
then it fails closed naming diff_source,
and `ripr explain --from a.json --diff X probe:<id>` succeeds.
```

### Worktree write then reuse fails closed on drift

```text
Given `ripr check --worktree --base HEAD --write-artifact a.json` succeeded,
when `ripr explain --from a.json probe:<id>` runs with the worktree unchanged,
then it re-resolves and re-hashes the base-to-worktree diff and renders the
recorded finding,
and when a tracked file is edited between write and reuse,
then it fails closed with "cannot be reused: identity mismatch on diff_bytes_hash".
```

### Render knobs are honored fresh

```text
Given an artifact whose finding has 12 related tests,
when `ripr context --from a.json --at probe:<id> --max-related-tests 12`
runs,
then the packet lists all 12 related tests,
even though the `check --json` render caps related tests at 8.
```

## Test Mapping

- `crates/ripr/src/app/check_artifact/tests.rs::artifact_round_trip_preserves_full_fidelity_findings`
  — loaded findings equal the computed set exactly, including the fields
  the lossy JSON projection drops.
- `crates/ripr/src/app/check_artifact/tests.rs::explain_from_artifact_is_byte_identical_to_fresh_explain`
  and `context_from_artifact_is_byte_identical_to_fresh_context` — the
  core reuse proof.
- `crates/ripr/src/app/check_artifact/tests.rs::context_from_artifact_honors_max_related_tests_beyond_json_render_cap`
  — uncapped related tests round-trip; render knob honored fresh.
- `crates/ripr/src/app/check_artifact/tests.rs::load_fails_closed_on_mode_mismatch`,
  `load_fails_closed_on_root_mismatch`,
  `load_fails_closed_on_include_unchanged_tests_mismatch`,
  `load_fails_closed_on_diff_bytes_change`,
  `load_fails_closed_when_recorded_diff_source_is_missing`,
  `load_fails_closed_on_config_identity_mismatch`,
  `load_fails_closed_on_analyzer_version_mismatch`,
  `load_fails_closed_on_enabled_languages_mismatch`,
  `load_fails_closed_on_wrong_schema_version_and_malformed_input` — every
  mismatch class fails closed with its named field.
- `crates/ripr/src/app/check_artifact/tests.rs::scope_flags_passed_alongside_from_are_assertions`
  — assertion semantics for `--diff` / `--base`.
- `crates/ripr/src/app/check_artifact/tests.rs::perl_facts_packet_content_is_part_of_the_identity`
  — the fact packet's content hash participates.
- `crates/ripr/src/app/check_artifact/tests.rs::worktree_diff_source_wire_form_is_documented_worktree`
  — the `worktree` variant serializes in the documented snake_case wire
  vocabulary.
- `crates/ripr/src/app/check_artifact/tests.rs::worktree_artifact_round_trip_reuses_recorded_findings`
  — a matching worktree reuses the recorded finding set exactly.
- `crates/ripr/src/app/check_artifact/tests.rs::worktree_artifact_fails_closed_when_worktree_drifts`
  — worktree drift between write and reuse fails closed naming
  `diff_bytes_hash`.
- `crates/ripr/src/app/check_artifact/tests.rs::worktree_artifact_scope_flags_alongside_from_are_assertions`
  — `--base` assertion semantics against a worktree recording; a `--diff`
  assertion fails closed naming `diff_source`.
- `crates/ripr/src/app/check_artifact/tests.rs::repeated_write_replaces_artifact_atomically`
  and `concurrent_writers_never_leave_a_torn_artifact` — atomic-write
  discipline.
- `crates/ripr/src/config/tests.rs::check_artifact_identity_fields_classify_every_config_field`
  — the closed field-enumeration contract.
- `crates/ripr/src/config/tests.rs::check_artifact_config_identity_hash_tracks_finding_affecting_fields_only`
  — finding-affecting changes invalidate; render-only changes do not.
- `crates/ripr/tests/cli_smoke.rs::check_write_artifact_then_explain_and_context_reuse_byte_identical`
  — end-to-end three-step flow with byte-identical reuse.
- `crates/ripr/tests/cli_smoke.rs::explain_from_fails_closed_on_tampered_identity`
  and `check_write_artifact_rejects_unsupported_run_shapes` — CLI-level
  fail-closed paths.
- `crates/ripr/tests/cli_smoke.rs::explain_from_consumes_artifact_written_with_non_default_mode`
  — an artifact written with `--mode ready` is consumable with the same
  flag (byte-identical) and fails closed naming `mode` without it.
- `crates/ripr/tests/cli_smoke.rs::context_from_consumes_artifact_written_with_no_unchanged_tests`
  — an artifact written with `--no-unchanged-tests` is consumable with the
  same flag and fails closed naming
  `analysis_options.include_unchanged_tests` without it.
- `crates/ripr/tests/cli_smoke.rs::check_write_artifact_rejects_managed_perl_producer`
  — managed `[perl] producer` packet generation fails closed with the
  named limitation.
- `crates/ripr/tests/cli_smoke.rs::check_worktree_write_artifact_then_explain_reuse_and_drift_fails_closed`
  — end-to-end `--worktree --write-artifact` then `explain --from` reuse,
  matching and mismatched `--base` assertions, and the drift fail-closed
  path.

## Implementation Mapping

- `crates/ripr/src/app/check_artifact.rs` — envelope, atomic write,
  fail-closed load, identity build/verify, scope assertions.
- `crates/ripr/src/analysis/mod.rs` — `load_worktree_diff` re-export so the
  app layer can re-resolve a recorded `worktree` diff source.
- `crates/ripr/src/app/explain.rs` — `explain_finding_from_artifact`.
- `crates/ripr/src/app/context.rs` — `collect_context_from_artifact`.
- `crates/ripr/src/cli/commands.rs` — flag parsing, run-shape rejection,
  dispatch.
- `crates/ripr/src/config/model.rs` + `crates/ripr/src/config.rs` —
  config-identity allowlist, version, and hash.
- `crates/ripr/src/domain/probe.rs`, `classification.rs`, `language.rs` —
  serde derives enabling the full-fidelity `Finding` round-trip.

## Metrics

- Unit and integration tests listed above pass under `cargo test -p ripr`.
- `cargo xtask goldens check` remains clean: `check --json` output bytes
  are unchanged; the new flags add no output to existing flows.
- `cargo xtask check-static-language` clean: artifact and error strings
  use conservative static vocabulary only.
