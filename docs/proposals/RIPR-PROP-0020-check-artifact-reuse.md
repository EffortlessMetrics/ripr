# RIPR-PROP-0020: Check-artifact reuse for explain/context

Status: proposed

Owner: product / app

Created: 2026-07-22

Linked issues:

- #2107 — `ripr explain` / `ripr context` re-run the full pipeline per invocation
- #1912 / #1967 — per-file fact cache (separate, in-process layer)
- #1908 — LSP warm refresh (separate surface)

Support-tier impact:

- None. A reused artifact is the same analysis result the producing `check`
  run computed; reuse changes wall time, not evidence. Support-tier rules for
  the underlying findings are unchanged, and a stale artifact can never be
  consumed (identity gate below).

Policy impact:

- None. No new process spawns, no network, no workflow or hook change. The
  artifact is a local file the user names explicitly.

## Problem

The documented Quick Start (README.md:157-161) is a three-step flow —
`check`, then `explain`, then `context`, each with the same `--diff`. Every
invocation re-parses the diff and re-runs the entire pipeline
(`app/check.rs:35-40` → `run_analysis_with_oracle_policy` →
`run_diff_pipeline_with_oracle_policy`):

- `app/explain.rs:27-37` — `explain_finding_with_config` re-runs
  `check_workspace_with_config` and then selects one finding.
- `app/context.rs:34-52` — `collect_context_with_config` does the same.

On a real diff in a large workspace this doubles or triples wall time versus
reusing the prior result, and the user must re-supply identical scope flags
three times.

## Why `check --json` cannot be the reuse source

The findings JSON is a one-way, lossy render projection, not a round-trip
format:

- `Finding` (`domain/probe.rs:260-307`) has no `Serialize`/`Deserialize`;
  JSON is a hand-built render (`output/json/report.rs:352`).
- `related_tests` is capped at 8 in JSON (`MAX_RELATED_TESTS_PER_FINDING_JSON`,
  `output/json/report.rs:34`), so `context --max-related-tests > 8` could not
  be honored.
- `probe.owner` is conditionally omitted (`report.rs:420-430`).
- Severity is baked from the check-time config at render time
  (`report.rs:394-400`).
- The JSON carries no diff/base/head identity to gate reuse on.

Explaining from a lossy projection would silently explain less than the
analysis knew — the coverage mistake in a new costume. The reuse source must
be a faithful artifact, and the JSON contract stays untouched.

## Required decisions

### 1. Explicit artifact pair, no implicit cache

`ripr check` gains `--write-artifact <path>`; `ripr explain` and
`ripr context` gain `--from <path>`. Both directions are explicit user
intent. There is no implicit cross-invocation cache and no background writes:
an analyzer that silently reads stale state is worse than one that recomputes.

A content-addressed store under `target/ripr/cache/check-runs/` may be added
later as a separate increment; it is not required for the three-step flow and
is a non-goal here.

### 2. New serde envelope, full-fidelity findings

New `CheckArtifactV1` envelope with `schema_version`, written atomically:
the temporary file is created in the destination directory (same
filesystem, so `rename` is atomic), flushed and fsynced before the rename,
and unlinked on any failure; a repeated `--write-artifact` to the same path
replaces it via rename (last writer wins), and concurrent writers use
uniquely named temp files so one writer's failure cannot tear another's
artifact. This mirrors the seam-cache envelope discipline
(`analysis/seam_cache.rs:289-329`, `:623+`). The envelope serializes the
complete `Finding` set (serde derives on the domain probe types or a
faithful parallel DTO), including the uncapped related-tests list and the
probe owner. The existing `check --json` render is unchanged.

`--from` load follows the receipt read path precedent
(`app/receipt.rs:137-170`): parse, validate structure and `schema_version`,
fail closed with a named error on any deviation.

### 3. Identity gate, fail closed

The artifact embeds an input identity computed at check time:

- diff-bytes hash (or requested/resolved base + head + worktree hash when no
  `--diff` file is used), plus the diff *source* (`--diff` path, or the
  base/head pair) so the consumer can re-resolve it;
- root, mode, diagnostic profile, enabled languages;
- the complete `CheckInput` option surface that flows into
  `analysis_options_from_input_and_config` — today `include_unchanged_tests`
  and `perl_facts_path` (with the fact packet's content hash, since the Perl
  adapter reads findings-changing facts from that file), plus any future
  analysis input option, covered by the same closed-allowlist discipline as
  the config identity below;
- an analysis-config identity with a **closed, versioned contract**: an
  explicit allowlist of the config fields that change findings, canonically
  serialized (field name, normalized value, defaults materialized), hashed,
  and stamped with its own `config_identity_version`. Any PR that adds a
  finding-affecting config field must extend the allowlist and bump
  `config_identity_version` in the same PR; a test enumerates the config
  type's fields against the allowlist so an omitted field fails CI instead
  of silently validating stale artifacts. Render-only knobs (severity
  display, output format, `--max-related-tests`) are excluded by the same
  allowlist discipline;
- analyzer version and artifact schema version.

`explain --from` / `context --from` do **not** require re-supplying scope
flags: the artifact's recorded diff source is re-resolved and re-hashed
(a recorded `--diff` path is re-read; a recorded base/head pair is
re-resolved through git), and the current root, mode, profile, languages,
config identity, and analyzer version are recomputed from the invocation's
working directory and config files. The user may pass scope flags
(`--diff`, `--base`, `--root`) alongside `--from` to assert them
explicitly; they are then verified against the artifact instead of derived
from it. On any mismatch — or when the recorded diff source no longer
exists or cannot be re-resolved — the command fails with a typed error
naming the mismatched identity fields. There is no silent fallback to
recompute and no "close enough" matching: explaining from a stale artifact
would explain the wrong behavior, which is exactly the failure this flow
exists to avoid. Render-time config is not part of the identity and is
honored fresh at render time from the current invocation.

### 4. Selection and rendering are unchanged

`--from` replaces only the pipeline re-run. Finding selection
(`select_finding`) and rendering (`render_finding_with_config`,
`render_context_packet`) operate on the loaded `Finding`s exactly as they do
on freshly computed ones, so a reused explanation is byte-identical to a
recomputed one given the same render options.

## Non-goals

- The per-file fact cache (#1912 / #1967) — in-process, orthogonal layer.
- LSP warm refresh (#1908) — the LSP has its own input-identity model
  (`lsp/input_identity.rs`); this proposal is CLI-only.
- Any change to the `check --json` output contract.
- Content-addressed automatic caching or cache eviction policy.
- Sharing artifacts between machines or committing them; the artifact is a
  local, disposable derivative and must be documented as such.

## Alternatives considered

- **Reuse `check --json` as the cache source.** Rejected: it is a lossy
  render projection (no serde round-trip, related-tests capped at 8,
  conditional probe owner, render-time severity, no diff identity), so
  explaining from it would silently explain less than the analysis knew.
- **Implicit content-addressed cache under `target/ripr/cache/`.** Deferred:
  it adds eviction, staleness, and cross-run policy questions the three-step
  flow does not need. The explicit `--write-artifact`/`--from` pair keeps
  reuse under user intent; the store can layer on later without changing
  this contract.
- **Do nothing (keep recomputing).** Rejected for large workspaces: the
  documented Quick Start triples wall time for no evidence gain.

## Risks

- **Stale-artifact misuse.** Mitigated by the fail-closed identity gate: a
  mismatched diff, mode, language set, input option, or config identity is a
  typed error, never a silent recompute or a "close enough" read.
- **Identity allowlist drift** (a new finding-affecting field omitted from
  the hash). Mitigated by the closed, versioned contract and a CI
  enumeration test over the config/input fields.
- **Artifact format churn.** Mitigated by `schema_version` on the envelope
  with fail-closed loads; old artifacts error instead of misreading.
- **Users treating artifacts as durable evidence.** Mitigated by documenting
  the artifact as a local, disposable derivative: it may never feed a
  support-tier row, gate, badge, or proof route.

## Success criteria

1. `ripr check --diff X --write-artifact a.json` writes a schema-versioned
   artifact containing the full finding set and the identity block.
2. `ripr explain --from a.json probe:<id>` and `ripr context --from a.json
   --at probe:<id>` — with no scope flags, deriving and re-verifying the
   recorded diff source — produce output identical to the recomputed flow
   without re-running the pipeline (observable: no diff load, no RustIndex
   build).
3. Identity mismatch (different diff, mode, languages, or analysis-relevant
   config) fails closed with a typed error naming the mismatched fields.
4. A malformed, truncated, or wrong-version artifact fails closed at load.
5. `context --from` honors `--max-related-tests` beyond the JSON render cap.
6. `cargo xtask goldens check` shows no drift for existing flows; new
   fixtures pin the artifact round-trip and the identity-gate failures.
