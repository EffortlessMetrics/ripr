# Configuration

This is the reference for every setting `ripr` reads today, including the
repo-root `ripr.toml` file. It pairs with:

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) for what the analysis modes mean.
- [Output schema](OUTPUT_SCHEMA.md) for what each output format produces.
- [Editor extension](EDITOR_EXTENSION.md) and [Server provisioning](SERVER_PROVISIONING.md) for how the VS Code extension launches and resolves the LSP server.

## What can be configured today

`ripr` currently reads configuration from six surfaces:

1. **CLI flags** on the `ripr` binary.
2. **Repo config** in `ripr.toml` at the workspace root.
3. **LSP `initializationOptions`** sent by an LSP client (e.g. the VS Code extension) on `initialize`.
4. **VS Code extension settings** under the `ripr.*` namespace, which the extension translates into server arguments and LSP options.
5. **Environment variables** for narrow runtime tuning of expensive local paths.
6. **Repo policy files** under `.ripr/`, including static-language allowlists,
   test intent, and suppressions.

`ripr.toml` is repo-root scoped only. `ripr` does not read global user config,
or hidden alternate config files. Environment variables are process-local tuning
knobs and are documented separately from repo policy.

Configuration is for policy and tuning, not a prerequisite for first value.
`ripr.toml` is optional, and missing config is the normal first-run state — it
uses built-in defaults. `ripr init` is **only** for teams that want to commit
those defaults to disk so they can review, version, and tune repo policy; it
does not unlock basic usefulness or enable a stronger mode. The defaults-first
adoption contract in
[RIPR-SPEC-0009](specs/RIPR-SPEC-0009-defaults-first-adoption.md) defines the
missing-config and generated-config behavior that must remain advisory.

## Defaults-first operator profile

The built-in missing-config profile and the `ripr init` generated profile are
intended to be policy-equivalent. `ripr init` writes that policy into
`ripr.toml` so a team can review, version, and tune it; it does not enable a
stronger or more useful mode than the zero-config default. Most users do not
need to run it.

| Surface | Default policy |
| --- | --- |
| Analysis | Normal/default scans use `draft` mode and include unchanged tests as static evidence. |
| Oracles | Snapshot and mock-expectation oracles are `medium`; broad error checks are `weak`. |
| Seam diagnostics | Saved-workspace LSP seam diagnostics are on, with explicit config or initialization options allowed to disable them. |
| Report caps | Context packets and collect-context commands include up to `5` related tests by default. |
| Suppressions | Badge renderers look for `.ripr/suppressions.toml`; a missing file is normal. |
| Badges | Repo badges count configured-visible unresolved seam gaps and stay advisory unless an explicit failure policy is selected. |
| Cache | Full repo seam cache stores up to `20000` seams by default, compact repo seam cache stores up to `100000` seams by default, and large repos can opt into higher process-local limits. |
| CI | Generated GitHub workflows upload advisory pilot/report/agent artifacts, keep SARIF rendering/upload optional, and use `continue-on-error` by default. |
| Calibration | Runtime data is imported only when explicitly supplied; `ripr` does not run mutation testing by default. |

Operator mode vocabulary maps to concrete analysis modes:

| Operator stance | Concrete mode | Scope |
| --- | --- | --- |
| Fastest feedback | `instant` | Changed Rust files only. |
| Normal/default | `draft` | Rust files in packages touched by the diff, including unchanged tests. |
| PR fast scan | `fast` | Same package-local scope as `draft` for now. |
| Deep static scan | `deep` | All Rust files in the workspace. |
| Ready preflight | `ready` | All Rust files in the workspace before separate mutation confirmation. |

Repo-scoped public signals intentionally filter out repository automation and
non-production trees so badges and repo seam reports describe the package
surface, not the toolchain around it. The production filter excludes paths under
`xtask/`, top-level fixture data, editor extension sources, `target/`,
`node_modules/`, test/example/bench trees, and `src/tests.rs`. Passing a fixture
workspace itself as `--root` still analyzes that fixture normally.

## CLI flags

The CLI is the canonical, fully-supported configuration surface. All defaults
below come from [`crates/ripr/src/cli/help.rs`](../crates/ripr/src/cli/help.rs)
and [`crates/ripr/src/app.rs`](../crates/ripr/src/app.rs).

### Top-level

| Flag | Effect |
| --- | --- |
| `--help`, `-h` | Print top-level help. |
| `--version`, `-V` | Print the `ripr` version. |

### `ripr init`

Optional. Writes a repo-local `ripr.toml` at the selected workspace root that
materializes the built-in defaults as repo policy so a team can review, commit,
and tune them. `ripr.toml` is not required — missing config uses the same
defaults. With `--ci github`, `ripr init` also writes a non-blocking GitHub
Actions workflow for pilot/report/agent artifacts, optional repo-local cockpit
rendering, and optional SARIF rendering/upload. It does not run mutation
testing, enable CI blocking policy, or unlock basic CLI usefulness.

```text
ripr init [--root PATH] [--ci github] [--dry-run] [--force]
```

| Flag | Default | Notes |
| --- | --- | --- |
| `--root PATH` | current directory | Workspace root where `ripr.toml` should be written. |
| `--ci github` | _(off)_ | Also write `.github/workflows/ripr.yml`. The workflow installs `ripr`, runs `ripr pilot`, uploads pilot/report/agent artifacts, writes repo badge JSON, optionally renders and uploads SARIF when `RIPR_UPLOAD_SARIF` is true, and uses `continue-on-error` so the default path is advisory. |
| `--dry-run` | _(off)_ | Print the generated config to stdout without writing. |
| `--force` | _(off)_ | Overwrite an existing `ripr.toml` or generated workflow. Without this flag, existing repo policy and workflow files are left unchanged. |

### `ripr pilot`

Runs the zero-config first-run repo evidence path and writes a pilot packet.
Missing `ripr.toml` is the normal first-run state; the command uses built-in
defaults unless repo policy or explicit flags override them.

```text
ripr pilot [--root PATH] [--out PATH] [--mode MODE] [--max-seams N]
```

| Flag | Default | Notes |
| --- | --- | --- |
| `--root PATH` | current directory | Workspace root to analyze. |
| `--out PATH` | `target/ripr/pilot` | Directory for `repo-exposure.{json,md}`, `agent-seam-packets.json`, and `pilot-summary.{json,md}`. |
| `--mode MODE` | `ripr.toml` `analysis.mode`, otherwise `draft` | One of `instant`, `draft`, `fast`, `deep`, `ready`. |
| `--max-seams N` | `5` | Maximum ranked seams shown in the pilot summary. Must be positive. |

### `ripr check`

Runs the static exposure analysis and renders findings.

| Flag | Default | Notes |
| --- | --- | --- |
| `--root PATH` | current directory | Workspace root used for diff and source discovery. |
| `--base REV` | `origin/main` | Git revision used as the diff base when `--diff` is not given. |
| `--diff PATH` | _(unset)_ | Path to a unified diff file. Overrides `--base`. |
| `--mode MODE` | `ripr.toml` `analysis.mode`, otherwise `draft` | One of `instant`, `draft`, `fast`, `deep`, `ready`. See the [mode reference](#analysis-modes). |
| `--format FORMAT` | `human` | One of `human` (alias `text`), `json`, `github`. |
| `--json` | _(off)_ | Shortcut for `--format json`. |
| `--no-unchanged-tests` | `ripr.toml` `analysis.include_unchanged_tests`, otherwise tests included | Limits the source index to changed Rust files. By default unchanged tests are part of the index so `Reach` evidence can find them. |

### Environment Variables

Environment variables are process-local tuning knobs. They do not change
repo-policy defaults in `ripr.toml`, and they should be set only for the command
that needs the tuning.

| Variable | Default | Effect |
| --- | --- | --- |
| `RIPR_REPO_SEAM_CACHE_LIMIT` | `20000` | Maximum classified seam count per shard in the full repo seam cache for a completed repo-exposure run. Larger cache entries are written as bounded shard files under `target/ripr/cache`. Raise this to reduce shard count only when the machine has enough disk and time budget for larger shard writes. Must be a positive integer. Invalid values fail with a diagnostic naming the variable. |
| `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` | `100000` | Maximum seam count per shard in the compact repo seam cache. Larger compact cache entries are written as bounded shard files under `target/ripr/cache`. Raise this for large repos when the machine has enough disk and time budget for larger shard writes. Must be a positive integer. Invalid values fail with a diagnostic naming the variable. |

Repo seam cache entries larger than the active limit are stored as a manifest
plus shard files, with cache-store trace status such as
`sharded_ok_seams_135812_shards_7_limit_20000`. Warm loads stitch the shards
only when the manifest and every shard match the current cache key; missing or
corrupt shards are ignored as cache corruption and the run recomputes instead
of using partial evidence. `cargo xtask cache report` summarizes sharded
families, largest shard sets, and orphan or incomplete shard sets. The
`cargo xtask cache gc --dry-run` command still sees sharded entries because
every shard lives under `target/ripr/cache`.

To reduce full-cache shard counts for a repo on a machine with enough headroom:

```bash
RIPR_REPO_SEAM_CACHE_LIMIT=150000 ripr check --root . --format repo-exposure-json
```

To reduce compact-cache shard counts for that repo:

```bash
RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS=200000 ripr check --root . --format repo-badge-json
```

Lane 1 audit reports may still emit limited run states when repo-exposure input
is sampled, timed out, incomplete, stale, runner-failed, or skipped by a
large-cache preflight. Sharding cache files does not turn those limited inputs
into full repo truth.

### `ripr explain`

Renders a single finding in human format.

```text
ripr explain [--root PATH] [--base REV | --diff PATH] <finding-id | file:line>
```

The trailing positional argument selects the finding. Either form works:

- A finding id, e.g. `probe:src_lib.rs:88:predicate`.
- A `file:line` location, where the file matches the finding's path by exact
  match or path-suffix match.

### `ripr context`

Emits a compact JSON context packet for one finding.

```text
ripr context [--root PATH] [--base REV | --diff PATH]
             --at <finding-id | file:line>
             [--max-related-tests N] [--json]
```

| Flag | Default | Notes |
| --- | --- | --- |
| `--at SELECTOR` | _(required)_ | Same selector grammar as `explain`. |
| `--max-related-tests N` | `ripr.toml` `reports.max_related_tests`, otherwise `5` | Caps the number of related tests embedded in the packet. |
| `--json` | _(off)_ | Forces JSON; `context` already returns JSON-shaped output, this flag is for parity with `check`. |

### `ripr doctor`

```text
ripr doctor [--root PATH]
```

Reports local tooling and workspace shape. Takes no analysis-shaping flags.

`doctor` also reports the repository config status for the selected root:

```text
Config: loaded ripr.toml
Config path: ./ripr.toml
Analysis mode default: deep
LSP seam diagnostics default: true
Suppressions path: .ripr/suppressions.toml
```

When no repo config exists, that is healthy and explicit:

```text
Config: not found; using built-in defaults
```

Malformed config makes `doctor` fail with the config path and validation
problem, but `doctor` does not print the config source text.

### `ripr lsp`

```text
ripr lsp [--stdio] [--version]
```

| Flag | Default | Notes |
| --- | --- | --- |
| `--stdio` | implicit | Run the language server over stdio LSP framing. This is the only supported transport today. |
| `--version` | _(off)_ | Print the language server version and exit. |

LSP runtime behavior is not configured by CLI flags; clients pass options via
`initializationOptions` (next section).

## LSP `initializationOptions`

When an LSP client starts `ripr lsp --stdio`, it can shape analysis by sending
an `initializationOptions` object on the `initialize` request. The server
reads four keys; everything else is ignored. The schema lives in
[`crates/ripr/src/lsp/config.rs`](../crates/ripr/src/lsp/config.rs).

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `baseRef` | string | `"origin/main"` | Git base ref for editor-triggered diffs. Empty string disables base-ref diffing. |
| `checkMode` | string | `ripr.toml` `analysis.mode`, otherwise `"draft"` | One of `instant`, `draft`, `fast`, `deep`, `ready`. Unknown values fall back to the repo config/default. |
| `includeUnchangedTests` | boolean | `ripr.toml` `analysis.include_unchanged_tests`, otherwise `true` | Mirror of the CLI's `--no-unchanged-tests` (inverted). |
| `seamDiagnostics` | boolean | `ripr.toml` `lsp.seam_diagnostics`, otherwise `true` | Enables repo seam evidence diagnostics in addition to diff-derived Finding diagnostics. |

Initialization options are treated as explicit LSP settings and override
`ripr.toml`. Defaults match `CheckInput::default()` when no repo config is
present, except that LSP diagnostics render JSON-shaped data internally.

## VS Code extension settings

The bundled VS Code extension exposes the settings below under the `ripr.*`
namespace. The full schema lives in
[`editors/vscode/package.json`](../editors/vscode/package.json) under
`contributes.configuration`. The extension is responsible for turning these
into LSP `initializationOptions` and server-launch arguments.

### Server resolution

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `ripr.enabled` | boolean | `true` | Enables the VS Code saved-workspace server, diagnostics, hovers, status, and code actions. Set to `false` for an explicit disabled editor state without starting the language server. |
| `ripr.server.path` | string | `""` | Absolute path to a `ripr` executable. Wins over bundled, downloaded, and `PATH` resolution. |
| `ripr.server.args` | string array | `["lsp", "--stdio"]` | Arguments used to start the language server. |
| `ripr.server.autoDownload` | boolean | `true` | Auto-download a matching server binary when no configured, bundled, or cached one is available. |
| `ripr.server.version` | string | `""` | Pin a specific server version. Empty means match the extension version. |
| `ripr.server.downloadBaseUrl` | string | `""` | Override the server manifest base URL (e.g. internal mirror). Empty uses GitHub Releases. |

For the full resolution order (configured → bundled → cached → first-run
download → `PATH`), see
[Server provisioning](SERVER_PROVISIONING.md).

### Analysis

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `ripr.check.mode` | enum: `instant` \| `draft` \| `fast` \| `deep` \| `ready` | `draft` | Editor-side analysis mode. Forwarded as `initializationOptions.checkMode`. |
| `ripr.baseRef` | string | `"origin/main"` | Git base ref used by editor diagnostics and the context commands. Forwarded as `initializationOptions.baseRef`. |

The editor default matches the CLI and direct LSP missing-config default:
`draft`.

### Diagnostics

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `ripr.trace.server` | enum: `off` \| `messages` \| `verbose` | `off` | LSP message tracing in the `ripr` output channel. |

### Commands

The extension contributes:

- `ripr.restartServer`
- `ripr.showOutput`
- `ripr.copyContext`
- `ripr.copySuggestedAssertion`
- `ripr.copyTargetedTestBrief`
- `ripr.copyAgentPacketCommand`
- `ripr.copyAgentBriefCommand`
- `ripr.copyAfterSnapshotCommand`
- `ripr.copyAgentVerifyCommand`
- `ripr.copyAgentReceiptCommand`
- `ripr.openRelatedTest`
- `ripr.openSettings`

These are not configured directly. They are surfaced through the command
palette and from LSP code actions when diagnostics carry the required
finding or seam data.

## Repo policy files

Narrow, durable policy files live under `.ripr/`. They are not suppression
mechanisms in the runtime sense — they are reasoned exceptions to repo-wide
checks, and `cargo xtask check-pr` enforces that every entry has a named
owner and a written reason.

### `.ripr/static-language-allowlist.toml`

Files allowed to mention prohibited mutation-runtime vocabulary because they
define the language boundary, document calibration plans, or describe agent
rules. Validated by `cargo xtask check-static-language`. Source of truth for
the prohibited terms themselves is `forbidden_static_terms` in
[`xtask/src/main.rs`](../xtask/src/main.rs).

Schema (parsed by `parse_static_language_allowlist` in
[`xtask/src/main.rs`](../xtask/src/main.rs)):

```toml
schema_version = 1

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "Agent instructions define the static-language boundary and must quote the prohibited terms verbatim."

[[allow]]
glob = "docs/**/*.md"
owner = "docs"
reason = "Nested documentation specs and ADRs may describe static-language policy and future calibration vocabulary."
```

Validation rules (all enforced; violations fail `check-static-language`):

| Rule | Behavior |
| --- | --- |
| `schema_version = 1` required | Missing or other values fail. |
| Exactly one of `path` or `glob` per `[[allow]]` | Both or neither fail. |
| `owner` required, non-blank | Missing or whitespace-only fails. |
| `reason` required, non-blank | Missing or whitespace-only fails. |
| Duplicate matchers | Two entries with the same `path` or `glob` fail. |
| Absolute paths | Entries starting with `/` or matching `<letter>:` fail. |
| Backslash paths | Entries containing `\` fail; use `/` separators. |
| Glob entries scoped | Currently only `docs/*.md` and `docs/**/*.md` are accepted; broader globs like `*.md` or `**/*.md` fail. |
| Exact paths must exist | `path = "..."` entries that don't exist on disk fail at load time. |

A legacy `.ripr/static-language-allowlist.txt` file is explicitly rejected;
the loader fails with a clear migration message if both files are present.

### `.ripr/test_intent.toml`

Positive declarations for intentionally smoke, duplicative, opaque, or
otherwise-special tests. Each declaration carries an `owner`, a written
`reason`, and an `intent` from a closed set. The original `class`
emitted by the test-efficiency report is preserved — intent is additive
metadata, never a replacement.

Validated by `cargo xtask test-efficiency-report` via
`parse_test_intent_manifest` in [`xtask/src/main.rs`](../xtask/src/main.rs).

```toml
schema_version = 1

[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
reason = "CLI startup and help text smoke test."
owner = "devtools"

[[test_intent]]
test = "escapes_json"
path = "crates/ripr/src/output/json/formatter.rs"
intent = "business_case_duplicate"
reason = "These duplicate-looking tests document distinct escaping cases."
owner = "output"
```

Supported `intent` values:

| Intent | Typical use |
| --- | --- |
| `smoke` | Intentional smoke-only test (CLI startup, help text). |
| `business_case_duplicate` | Structurally similar tests that document distinct business cases. |
| `opaque_external_oracle` | Test with an opaque oracle ripr cannot statically inspect. |
| `integration_contract` | End-to-end contract test whose static class varies. |
| `performance_guard` | Test exists to guard a performance characteristic. |
| `documentation_example` | Test exists primarily as a documentation example. |

Validation rules (all enforced; violations fail `test-efficiency-report`):

| Rule | Behavior |
| --- | --- |
| `schema_version = 1` required | Missing or other values fail. |
| `test`, `intent`, `owner`, `reason` required | Missing or whitespace-only values fail. |
| `intent` must be one of the supported values | Unknown intents fail with the supported-list message. |
| `path` optional, repo-relative, slash-separated | Absolute paths and backslash paths fail at parse time. |
| `path` exists on disk | Missing files fail at load time. |
| Duplicate `(test, path)` selectors rejected | First-declared line cited in the violation. |
| Unknown `[[test_intent]]` fields rejected | Catches typos and prevents silent shape drift. |
| Unmatched declarations rejected | A declared `test`/`path` selector that matches no test fails the report. |
| Ambiguous name-only selectors rejected | If `test = "..."` matches multiple entries and no `path` is given, fail and list the candidates. |

Future `ripr+` will use the `declared_intent` metadata to exclude
declared intentional test-efficiency findings from its count. See
[Badge policy](BADGE_POLICY.md).

### `.ripr/suppressions.toml`

Accepted exceptions for exposure-gap or test-efficiency findings the team
has agreed to carry as known debt. Suppressed findings remain visible in
detailed reports — they only move from `unsuppressed_*` into `suppressed_*`
in the badge counts.

Validated by `ripr check --format badge-*` (loaded relative to `--root`).
Schema:

```toml
schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/pricing.rs:88:predicate"
reason = "Covered by integration test in tests/billing/integration.rs that ripr cannot statically inspect yet."
owner = "billing"
expires = "2026-09-01"
scope = "seam:pricing::threshold"
created_at = "2026-01-01"
last_seen = "2026-05-01"
review_by = "2026-12-01"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "rust"

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
path = "tests/cli.rs"
reason = "The CLI help smoke test is intentionally broad and covered by CLI contract tests."
owner = "devtools"
```

Supported `kind` values (closed set): `exposure_gap`, `test_efficiency`.

| Rule | Behavior |
| --- | --- |
| `schema_version = 1` required | Missing or other values fail. |
| `kind`, `owner`, `reason` required and non-blank | Missing or whitespace-only values fail. |
| `kind = "exposure_gap"` requires `finding_id` | And rejects `test`. |
| `kind = "test_efficiency"` requires `test` | And rejects `finding_id`; `path` is optional for disambiguation. |
| `path` repo-relative, slash-separated | Absolute paths and backslash paths fail at parse time. |
| `expires` ISO `YYYY-MM-DD` if present | Other formats fail at parse time. |
| `created_at`, `last_seen`, `review_by` ISO `YYYY-MM-DD` if present | Other formats fail at parse time. |
| Unknown fields rejected | Catches typos. |
| Duplicate selectors rejected | Same `finding_id` (or `(test, path)`) twice fails. |
| Unmatched selectors surface as warnings | Selector that matches no current finding is reported but does not fail the badge. |
| Expired suppressions do **not** apply | They surface as warnings on the badge so silent green-forever debt is impossible. |

Policy-health metadata:

| Field | Meaning |
| --- | --- |
| `scope` | Reviewed scope for the durable exception. Avoid broad values such as `repo`, `workspace`, `global`, `all`, or `*`. |
| `created_at` | Date the durable exception was created. |
| `last_seen` | Date the suppressed finding was last reviewed or observed. |
| `review_by` | Date by which the exception should be reviewed again. |
| `expected_visibility` | Expected reporting treatment, usually `suppressed_visible`. |
| `static_class` | Static evidence class covered by the suppression, such as `weakly_exposed` or `reachable_unrevealed`. |
| `language` | Optional evidence language, such as `rust`, `typescript`, or `python`. |
| `language_status` | Required as `preview` for preview-language suppressions until an explicit policy promotes them. |

`ripr policy suppression-health` reads the same manifest and writes
`target/ripr/reports/suppression-health.json` plus Markdown. It flags missing
owner, missing reason, stale review windows, overbroad scope, unknown
selectors, missing policy-health metadata, and preview-language suppressions
without `language_status = "preview"`. The report is advisory and read-only;
it does not create, delete, apply, or gate suppressions.

Suppressions and `declared_intent` are distinct concerns: intent is a
positive declaration about test purpose; a suppression is an accepted
exception or accepted debt. They can coexist on the same test (suppression
wins for the badge count); the test-efficiency JSON shows both fields
independently.

When suppressions affect the badge, the native JSON `warnings` array
surfaces expired/unmatched selectors. The Shields projection always
remains exactly four fields and never leaks warning text.

## Analysis modes

Modes change how much of the workspace is loaded into the syntax index before
classification. They do **not** change the meaning of any
[exposure class](STATIC_EXPOSURE_MODEL.md#exposure-classes).

The default operator stance is `draft`: enough package-local context for a
useful first scan without whole-workspace cost. `instant` is the cheapest
fast-feedback mode, `fast` currently shares `draft`'s package-local scope, and
`deep` / `ready` are whole-workspace static scans.

| Mode | Index scope | Intended use |
| --- | --- | --- |
| `instant` | Changed Rust files only. | Editor-safe, cheapest feedback. |
| `draft` | Rust files in packages touched by the diff. | Default local CLI scan. |
| `fast` | Same package-local scope as `draft` for now. | Draft PR scan; future bounded graph work lands here. |
| `deep` | All Rust files in the workspace. | Manual or CI scan when wider static evidence is acceptable. |
| `ready` | All Rust files in the workspace. | Static preflight before real mutation confirmation. |

`ready` does not run mutants. It remains static exposure analysis until a
calibration or mutation adapter is explicitly invoked.

## Repo discovery defaults

Repo-mode commands discover Rust files from the selected `--root`, then apply
the analysis mode scope above. Discovery skips directories that are normally
generated, policy-only, fixture-only, or editor/package-manager state:

- `.git`
- `target`
- `.ripr`
- `.direnv`
- `fixtures`
- `node_modules`

When repo seam inventory decides whether a Rust file is production code, it also
excludes tests, examples, benches, `target`, top-level fixtures, editor
extension code, package-manager directories, and `xtask` automation. Test files
can still be indexed as evidence when `include_unchanged_tests = true`; they are
not treated as production seams.

## Output formats

| Format | Selector | When to use |
| --- | --- | --- |
| `human` | default, or `--format human` / `--format text` | Local terminal review. |
| `json` | `--json` or `--format json` | Tools, editors, CI, agents. Versioned via `schema_version`. See [Output schema](OUTPUT_SCHEMA.md). |
| `github` | `--format github` | GitHub Actions annotations. |

The `context` command always returns JSON-shaped output regardless of
`--format`.

## `ripr.toml`

`ripr` discovers `ripr.toml` at the workspace root passed by `--root` or by the
LSP initialization root. Missing config is normal and preserves current
built-in defaults. Unknown keys are errors so policy typos do not silently
change analysis intent.

The example file ([`ripr.toml.example`](../ripr.toml.example)) is kept in sync
with the supported v1 shape.

`ripr init` generates a profile aligned with
[RIPR-SPEC-0009](specs/RIPR-SPEC-0009-defaults-first-adoption.md). That
generated profile is a materialized policy file, not an activation step: it
records the same built-in defaults so a team can review, commit, and tune
them. It does not enable runtime mutation execution or CI blocking policy.

### `[analysis]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `mode` | enum: `instant` \| `draft` \| `fast` \| `deep` \| `ready` | `draft` | Default analysis mode when not set by a CLI flag or LSP initialization option. |
| `include_unchanged_tests` | boolean | `true` | Whether unchanged tests may be indexed as static evidence. |

### `[oracles]`

These settings adjust repo policy for oracle shapes whose strength can vary by
team convention. Exact value and exact error-variant assertions remain strong;
smoke-only assertions remain smoke.

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `snapshot_strength` | enum: `strong` \| `medium` \| `weak` \| `smoke` \| `none` \| `unknown` | `medium` | Strength assigned to snapshot-style oracles. |
| `mock_expectation_strength` | same enum | `medium` | Strength assigned to mock/side-effect expectation oracles. |
| `broad_error_strength` | same enum | `weak` | Strength assigned to broad error checks such as `is_err()`. |

### `[severity.findings]`

Finding severities affect human output, JSON `severity`, GitHub annotations,
and LSP Finding diagnostics. Valid values are `info`, `warning`, and `note`.
Use suppressions for accepted debt; Finding severities cannot be `off`.

| Key | Default |
| --- | --- |
| `exposed` | `info` |
| `weakly_exposed` | `warning` |
| `reachable_unrevealed` | `warning` |
| `no_static_path` | `warning` |
| `infection_unknown` | `warning` |
| `propagation_unknown` | `note` |
| `static_unknown` | `note` |

### `[severity.seams]`

Seam severities affect LSP seam diagnostics. Valid values are `off`, `info`,
`warning`, and `note`. Classes set to `off` do not publish diagnostics.

| Key | Default |
| --- | --- |
| `strongly_gripped` | `off` |
| `weakly_gripped` | `warning` |
| `ungripped` | `warning` |
| `reachable_unrevealed` | `warning` |
| `activation_unknown` | `info` |
| `propagation_unknown` | `info` |
| `observation_unknown` | `info` |
| `discrimination_unknown` | `info` |
| `opaque` | `info` |
| `intentional` | `off` |
| `suppressed` | `off` |

### `[lsp]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `seam_diagnostics` | boolean | `true` | Default for bounded saved-workspace repo seam diagnostics. LSP `initializationOptions.seamDiagnostics` still wins. |

### `[reports]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `max_related_tests` | integer | `5` | Default cap for context packets and server-side collect-context commands. |

### `[suppressions]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `path` | repo-relative path | `.ripr/suppressions.toml` | Badge renderers load suppressions from this path. Absolute paths, `..`, and backslashes are rejected. |

### `[languages]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enabled` | array of strings | `["rust"]` | Language adapters the analysis pipeline will dispatch to. Valid values: `rust`, `typescript`, `python`. Unknown values and duplicate entries are rejected. TypeScript covers `.ts`, `.tsx`, `.js`, and `.jsx`; Python covers `.py`. TypeScript and Python are opt-in preview adapters; Rust remains the reference adapter and the only adapter that may be `stable` per [RIPR-SPEC-0026](specs/RIPR-SPEC-0026-language-adapter-contract.md). |

`[languages]` controls runtime routing for the selected repository. The `ripr`
binary must also be built with the corresponding adapter feature. The default
published build includes the preview adapter features, while a Rust-only build
can omit them:

```bash
cargo build -p ripr --no-default-features --features lang-rust
```

If repo config enables a language that is not available in the current binary,
configuration fails closed with a message naming the missing Cargo feature,
for example `lang-python`. The editor reports that configuration problem
instead of publishing phantom preview diagnostics.

To evaluate preview languages, keep Rust enabled and add only the preview
adapters the repo wants to inspect:

```toml
[languages]
enabled = ["rust", "typescript", "python"]
```

### `[profiles.bun_ub]`

The Bun UB profile is an opt-in advisory profile for Bun stable-byte review.
It records where TypeScript-family integration tests and Bun bridge hints live
so operators can run the calibrated Blob / ArrayBuffer cross-language preview
without changing Rust defaults.

```toml
[languages]
enabled = ["rust", "typescript"]

[profiles.bun_ub]
test_roots = [
  "test/js/**/*.test.ts",
  "test/js/**/*.test.js",
]
bridge_hints = "ripr.bun.bridge.toml"
```

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `test_roots` | non-empty array of repo-relative glob strings | none; required when the profile is present | Names the TypeScript-family Bun integration tests to inspect. JavaScript test files are covered by the `typescript` adapter. |
| `bridge_hints` | repo-relative path | none; required when the profile is present | Points to the opt-in Bun TS-to-Rust bridge-hint file. |

This profile is read-only configuration. It does not enable TypeScript by
itself, run `tsc`, start `tsserver`, execute Bun/Jest/Vitest, generate tests,
edit source, contribute to gates, badges, baselines, or RIPR Zero, or promote
TypeScript/JavaScript out of preview. `ripr doctor` reports whether the profile
is configured and repeats that authority boundary.

See [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md) for how to
read preview labels, static limits, generated-CI grouping, editor projection,
and rollback. See the
[Bun UB TypeScript preview runbook](BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md) for
the calibrated stable-byte review loop and the exact advisory actions for
missing discriminators, token-only evidence, and unknown bridges.

### Worked example

```toml
[analysis]
mode = "draft"
include_unchanged_tests = true

[oracles]
snapshot_strength = "medium"
mock_expectation_strength = "medium"
broad_error_strength = "weak"

[severity.findings]
weakly_exposed = "warning"
static_unknown = "note"

[severity.seams]
weakly_gripped = "warning"
opaque = "info"

[lsp]
seam_diagnostics = true

[reports]
max_related_tests = 5

[suppressions]
path = ".ripr/suppressions.toml"

[languages]
enabled = ["rust"]
```

## Precedence

For CLI commands:

```
CLI flag  >  ripr.toml  >  CheckInput::default()
```

For LSP:

```
LSP initializationOptions  >  ripr.toml  >  CheckInput::default()
```

## See also

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — what each mode and
  exposure class actually means.
- [Output schema](OUTPUT_SCHEMA.md) — the stable JSON contracts for `--json`,
  badges, SARIF, and the `context` command.
- [Editor extension](EDITOR_EXTENSION.md) and
  [Server provisioning](SERVER_PROVISIONING.md) — how VS Code launches and
  resolves the server.
- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md) — how to
  enable and interpret opt-in TypeScript, JavaScript, and Python evidence.
- [Roadmap](ROADMAP.md) and
  [Implementation plan](IMPLEMENTATION_PLAN.md) — when the `ripr.toml`
  loader and the bounded-graph keys are expected to land.
