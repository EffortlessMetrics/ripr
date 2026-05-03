# Configuration

This is the reference for every setting `ripr` reads today, plus the planned
shape of the `ripr.toml` file. It pairs with:

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) for what the analysis modes mean.
- [Output schema](OUTPUT_SCHEMA.md) for what each output format produces.
- [Editor extension](EDITOR_EXTENSION.md) and [Server provisioning](SERVER_PROVISIONING.md) for how the VS Code extension launches and resolves the LSP server.

## What can be configured today

`ripr` currently reads configuration from four surfaces:

1. **CLI flags** on the `ripr` binary.
2. **LSP `initializationOptions`** sent by an LSP client (e.g. the VS Code extension) on `initialize`.
3. **VS Code extension settings** under the `ripr.*` namespace, which the extension translates into server arguments and LSP options.
4. **Repo policy files** under `.ripr/`, currently just the static-language allowlist; planned narrow files for test intent and suppressions land before any general loader.

There is **no** general `ripr.toml` loader in the current alpha. The
[`ripr.toml.example`](../ripr.toml.example) file at the repo root is a
forward-looking sketch of the planned config format; nothing in the binary
parses it yet. The "Planned: `ripr.toml`" section below documents the intended
shape so contributors can wire it up consistently when the loader lands.

If a finding's `recommended_next_step` mentions teaching `ripr` about a fixture
or builder "in `ripr.toml`", treat that as planned guidance — today the same
intent is expressed only by adding tests with stronger oracles.

## CLI flags

The CLI is the canonical, fully-supported configuration surface. All defaults
below come from [`crates/ripr/src/cli/help.rs`](../crates/ripr/src/cli/help.rs)
and [`crates/ripr/src/app.rs`](../crates/ripr/src/app.rs).

### Top-level

| Flag | Effect |
| --- | --- |
| `--help`, `-h` | Print top-level help. |
| `--version`, `-V` | Print the `ripr` version. |

### `ripr check`

Runs the static exposure analysis and renders findings.

| Flag | Default | Notes |
| --- | --- | --- |
| `--root PATH` | current directory | Workspace root used for diff and source discovery. |
| `--base REV` | `origin/main` | Git revision used as the diff base when `--diff` is not given. |
| `--diff PATH` | _(unset)_ | Path to a unified diff file. Overrides `--base`. |
| `--mode MODE` | `draft` | One of `instant`, `draft`, `fast`, `deep`, `ready`. See the [mode reference](#analysis-modes). |
| `--format FORMAT` | `human` | One of `human` (alias `text`), `json`, `github`. |
| `--json` | _(off)_ | Shortcut for `--format json`. |
| `--no-unchanged-tests` | _(off; tests included)_ | Limits the source index to changed Rust files. By default unchanged tests are part of the index so `Reach` evidence can find them. |

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
| `--max-related-tests N` | implementation default | Caps the number of related tests embedded in the packet. |
| `--json` | _(off)_ | Forces JSON; `context` already returns JSON-shaped output, this flag is for parity with `check`. |

### `ripr doctor`

```text
ripr doctor [--root PATH]
```

Reports local tooling and workspace shape. Takes no analysis-shaping flags.

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
reads three keys; everything else is ignored. The schema lives in
[`crates/ripr/src/lsp/config.rs`](../crates/ripr/src/lsp/config.rs).

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `baseRef` | string | `"origin/main"` | Git base ref for editor-triggered diffs. Empty string disables base-ref diffing. |
| `checkMode` | string | `"draft"` | One of `instant`, `draft`, `fast`, `deep`, `ready`. Unknown values fall back to the default. |
| `includeUnchangedTests` | boolean | `true` | Mirror of the CLI's `--no-unchanged-tests` (inverted). |

Defaults match `CheckInput::default()` so omitting `initializationOptions`
yields the same behavior as `ripr check` with no flags except `--format json`.

## VS Code extension settings

The bundled VS Code extension exposes the settings below under the `ripr.*`
namespace. The full schema lives in
[`editors/vscode/package.json`](../editors/vscode/package.json) under
`contributes.configuration`. The extension is responsible for turning these
into LSP `initializationOptions` and server-launch arguments.

### Server resolution

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
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
| `ripr.check.mode` | enum: `instant` \| `fast` \| `deep` | `instant` | Editor-side analysis mode. The extension exposes a narrower set than the CLI; the server still accepts all five modes via `initializationOptions.checkMode`. |
| `ripr.baseRef` | string | `"origin/main"` | Git base ref used by editor diagnostics and the context commands. Forwarded as `initializationOptions.baseRef`. |

Note that the editor default is `instant`, not `draft`. CLI invocations and
direct LSP clients still default to `draft`.

### Diagnostics

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `ripr.trace.server` | enum: `off` \| `messages` \| `verbose` | `off` | LSP message tracing in the `ripr` output channel. |

### Commands

The extension contributes:

- `ripr.restartServer`
- `ripr.showOutput`
- `ripr.copyContext`
- `ripr.openSettings`

These are not configured directly; they are surfaced through the command
palette.

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

### Planned policy files

The remaining narrow file before any general `ripr.toml`:

- `.ripr/suppressions.toml` — exceptions for exposure gaps and remaining
  test-efficiency findings that aren't covered by intent. Reason and owner
  required; expiry encouraged.

## Analysis modes

Modes change how much of the workspace is loaded into the syntax index before
classification. They do **not** change the meaning of any
[exposure class](STATIC_EXPOSURE_MODEL.md#exposure-classes).

| Mode | Index scope | Intended use |
| --- | --- | --- |
| `instant` | Changed Rust files only. | Editor-safe, cheapest feedback. |
| `draft` | Rust files in packages touched by the diff. | Default local CLI scan. |
| `fast` | Same package-local scope as `draft` for now. | Draft PR scan; future bounded graph work lands here. |
| `deep` | All Rust files in the workspace. | Manual or CI scan when wider static evidence is acceptable. |
| `ready` | All Rust files in the workspace. | Static preflight before real mutation confirmation. |

`ready` does not run mutants. It remains static exposure analysis until a
calibration or mutation adapter is explicitly invoked.

## Output formats

| Format | Selector | When to use |
| --- | --- | --- |
| `human` | default, or `--format human` / `--format text` | Local terminal review. |
| `json` | `--json` or `--format json` | Tools, editors, CI, agents. Versioned via `schema_version`. See [Output schema](OUTPUT_SCHEMA.md). |
| `github` | `--format github` | GitHub Actions annotations. |

The `context` command always returns JSON-shaped output regardless of
`--format`.

## Planned: `ripr.toml`

> **Status: planned, not implemented.** No code in the current binary parses
> `ripr.toml`. The shape below describes the intended config surface so
> contributors can wire it up consistently when the loader lands. Until then,
> the file at the repo root is example-only.

The intended discovery rule is: walk up from `--root` until a `ripr.toml` is
found, falling back to no project-level config. Workspace-level keys would
default to whatever `CheckInput::default()` produces today.

The example file ([`ripr.toml.example`](../ripr.toml.example)) sketches three
sections.

### `[analysis]`

| Key | Planned type | Planned default | Intended effect |
| --- | --- | --- | --- |
| `default_mode` | string | `"draft"` | Mode used when neither CLI nor LSP options set one. |
| `max_call_depth` | integer | `5` | Forward call-graph depth used by future propagation analysis. |
| `max_reverse_call_depth` | integer | `4` | Reverse call-graph depth used by future reachability analysis. |
| `max_paths_per_probe` | integer | `20` | Cap on enumerated paths per probe, to keep static analysis bounded. |
| `unknowns_are_warnings` | boolean | `false` | When true, opaque/unknown findings would surface as warnings rather than informational entries. |

These keys map to bounded-graph work that is on the roadmap (see
[`docs/IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md)). Until that work
lands, propagation analysis is shallow and these values have nothing to clamp.

### `[oracles.snapshots]`

| Key | Planned type | Planned default | Intended effect |
| --- | --- | --- | --- |
| `default` | enum: `low` \| `medium` \| `high` | `medium` | Default strength assigned to snapshot-style oracles when no per-pattern entry matches. |

### `[oracles.mocks]`

| Key | Planned type | Intended effect |
| --- | --- | --- |
| `patterns` | array of `{ path, strength, sink }` | Teach `ripr` that a specific mock-method call counts as a strong oracle for a named sink. `path` is a glob over qualified call paths; `strength` is `low`/`medium`/`high`; `sink` is a logical sink name (e.g. `published_event`, `persisted_state`). |

### `[external_boundaries]`

| Key | Planned type | Intended effect |
| --- | --- | --- |
| `patterns` | array of `{ path, kind }` | Teach `ripr` that a call (matched by glob) crosses a known external boundary, so propagation can assign it a sink kind. `kind` is one of the recognized sink labels (`persisted_state`, `published_event`, `metric`, …). |

### Worked example

The current example, with planned-only annotations:

```toml
# All keys below are planned; the current binary does not read this file.

[analysis]
default_mode = "draft"           # planned
max_call_depth = 5               # planned
max_reverse_call_depth = 4       # planned
max_paths_per_probe = 20         # planned
unknowns_are_warnings = false    # planned

[oracles.snapshots]
default = "medium"               # planned

[oracles.mocks]
patterns = [                     # planned
  { path = "MockPublisher::expect_publish", strength = "high", sink = "published_event" },
  { path = "MockRepo::expect_save",         strength = "high", sink = "persisted_state" },
]

[external_boundaries]
patterns = [                     # planned
  { path = "*.save",       kind = "persisted_state" },
  { path = "*.publish",    kind = "published_event" },
  { path = "metrics.*",    kind = "metric" },
]
```

## Precedence (planned)

When `ripr.toml` is wired up, the precedence rule will be:

```
CLI flag  >  LSP initializationOptions  >  ripr.toml [analysis]  >  CheckInput::default()
```

Until then, only the first and last apply on the CLI path, and the first two
apply on the LSP path.

## See also

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — what each mode and
  exposure class actually means.
- [Output schema](OUTPUT_SCHEMA.md) — the stable JSON contract for `--json`
  and the `context` command.
- [Editor extension](EDITOR_EXTENSION.md) and
  [Server provisioning](SERVER_PROVISIONING.md) — how VS Code launches and
  resolves the server.
- [Roadmap](ROADMAP.md) and
  [Implementation plan](IMPLEMENTATION_PLAN.md) — when the `ripr.toml`
  loader and the bounded-graph keys are expected to land.
