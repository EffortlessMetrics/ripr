# `ripr lsp` in Helix, Neovim, Zed, and other LSP clients

`ripr lsp --stdio` is a standard LSP server. VS Code has a packaged extension
([Editor extension](../EDITOR_EXTENSION.md)); every other client starts the
server itself. This page is a setup guide, not a support claim: no editor other
than VS Code has a reviewed journey receipt yet. The Neovim proof recipe for
that receipt is [neovim-lsp.md](neovim-lsp.md) (#1630).

The protocol behavior below was observed on 2026-09-28 by driving
`ripr lsp --stdio` from a scripted stdio client. Editor-specific notes say
whether they were read from the editor's source or docs, or run in the editor.

## What every client needs to know

**One repository per server.** ripr analyzes one workspace root per server
process. The root is the single entry in `initialize.workspaceFolders`, or
`rootUri` when the client sends no folders (`rootPath` is not read). Two or
more folders stop analysis with `workspace_ambiguous`; no folder and no
`rootUri` stops it with `root_unavailable`. The server says so with a
`window/showMessage` warning at startup, on a later folder change that stops
analysis, and in hover (#4454). A client that adds a folder to a running
server keeps its first root; the added folder is named as not analyzed.
Configure your editor to start one ripr server per repository.

**Saved files are the input.** Opening or saving a Rust file schedules an
analysis of the saved workspace against the base ref. Typing without saving
does not re-analyze: the edited file's diagnostics are withdrawn, and hover
says its evidence is paused until the file is saved. A file that no refresh
has analyzed yet says so, and its evidence appears after the next refresh. Save to get fresh
evidence, or run the `ripr.refresh` command. The server advertises full
document sync and relies on `textDocument/didSave`.

**Few diagnostics is normal.** The default `actionable` profile publishes only
`weakly_exposed`, `reachable_unrevealed`, and `no_static_path` findings that
carry a repair route. A diff with other findings shows no diagnostics, and
hover on the line explains why. Set `diagnosticProfile` to `full` to see every
finding.

**Settings.** ripr reads three layers; a later layer overrides an earlier one
key by key:

1. `ripr.toml` at the repository root, `[lsp]` table: `diagnostic_profile`,
   `seam_diagnostics` ([Configuration](../CONFIGURATION.md)). Editor-neutral
   defaults.
2. `initializationOptions` with the keys `baseRef`, `checkMode`,
   `includeUnchangedTests`, `seamDiagnostics`, `diagnosticProfile`,
   `gitTimeoutMs`, `refreshDeadlineMs` at the top level.
3. `workspace/configuration`: when the client advertises
   `workspace.configuration`, ripr asks for section `ripr` with the same keys.
   A key the editor returns wins over the other two layers, so an editor
   setting overrides `ripr.toml`.

**Diagnostics transport.** Push (`textDocument/publishDiagnostics`) unless the
client advertises pull diagnostics (`textDocument.diagnostic`).

**Commands.** `executeCommandProvider` lists `ripr.refresh`,
`ripr.collectContext`, `ripr.collectEvidenceContext`,
`ripr.collectWorkspaceStatus`, `ripr.collectRepairPacket`,
`ripr.collectTopLimitation`, and `ripr.collectReceiptStatus`. The server runs
all of them. Clipboard and navigation actions (`ripr.copy*`,
`ripr.openRelatedTest`) are offered only to clients that advertise them in the
`experimental.riprEditor` capability, which only the VS Code extension does.

**Custom notification.** The server sends `ripr/analysisStatus` after startup
and each refresh. A client without a handler can ignore it. `ripr.collectWorkspaceStatus` returns a
`workspace_status` object on request, with the latest status payload under
`analysis_status`.

## Helix

Read from Helix master source on 2026-09-28; not run in a Helix binary.

Add to `~/.config/helix/languages.toml` (or `.helix/languages.toml` in the
repository):

```toml
[language-server.ripr]
command = "ripr"
args = ["lsp", "--stdio"]

# Answers ripr's workspace/configuration pull for section "ripr".
[language-server.ripr.config.ripr]
diagnosticProfile = "full"

[[language]]
name = "rust"
language-servers = ["rust-analyzer", "ripr"]
```

- Refresh after saving: `:lsp-workspace-command ripr.refresh`. Restart after a
  root change: `:lsp-restart ripr`.
- Helix shows `window/showMessage` in the status line and writes
  `window/logMessage` to its log (`:log-open`).
- **One repository per Helix session is analyzed.** When a file from a second
  repository opens, Helix reuses a server that supports workspace folders and
  adds the second root to it. ripr keeps analyzing the first repository, says
  in the status line that the second is not analyzed, and hover on the second
  repository's files says they are outside the analyzed root (#4459). Open the
  second repository in its own Helix session to analyze it.

## Neovim

Follow [neovim-lsp.md](neovim-lsp.md) for the configuration and the proof
journey. For everyday use, pass settings through `settings`, which Neovim uses
to answer `workspace/configuration`:

```lua
vim.lsp.config("ripr", {
  cmd = { "ripr", "lsp", "--stdio" },
  filetypes = { "rust" },
  root_markers = { "ripr.toml", ".git", "Cargo.toml" },
  workspace_required = true,
  settings = { ripr = { diagnosticProfile = "full" } },
})
vim.lsp.enable("ripr")
```

`workspace_required = true` keeps Neovim from starting ripr on a file outside
any root, which would stop with `root_unavailable`. Refresh with the
`exec_cmd` call in the recipe's step 4.

Neovim v0.12.5 does not redraw refreshed pull diagnostics in an open buffer on
its own; the recipe's
[measured compatibility limit](neovim-lsp.md#measured-compatibility-limit)
records the run and the upstream fix. On that release, re-open the buffer or
request document diagnostics after a refresh.

## Zed

Zed launches only language servers that Zed or an extension registers, so
ripr needs the extension in [`editors/zed`](../../editors/zed/README.md)
(#4460). It is not in the Zed extension registry: install it with
`zed: install dev extension` from a checkout, with `ripr` on your `PATH`.
Settings go under `lsp.ripr.settings` in Zed's `settings.json`, written flat:

```json
{ "lsp": { "ripr": { "settings": { "diagnosticProfile": "full" } } } }
```

Zed starts one ripr server per worktree, so each folder of a project is
analyzed as its own repository. Read from Zed's source on 2026-09-29; not run
in a Zed binary.

## Any other client, or a script

A minimal session:

```text
→ initialize        { rootUri or one entry in workspaceFolders, capabilities }
← result            { capabilities, serverInfo: { name: "ripr" } }
→ initialized
← ripr/analysisStatus, and a showMessage warning if the root blocks analysis
→ textDocument/didOpen (a .rs file under the root)
← textDocument/publishDiagnostics (push clients)
→ textDocument/didSave after edits, or workspace/executeCommand ripr.refresh
→ shutdown
→ exit
```

Observed error behavior:

| Situation | Response |
| --- | --- |
| Any request before `initialize` | `-32002` server not initialized (unknown methods: `-32601`) |
| Second `initialize` | `-32600` invalid request |
| Request after `shutdown` | `-32600` invalid request |
| Unknown method | `-32601` method not found |
| Invalid params | `-32602` with the field named |
| Request whose method starts with `$/` | No response; the LSP spec requires `-32601` (#4456) |
| Malformed frame or JSON | `-32700` with a null id, then the server exits with status 0 |
| `exit` without `shutdown` | Exits with status 0 (the spec suggests 1) |
| Client stops reading output | The server stops after two minutes without write progress |

`initialize.processId` is accepted but not watched: close the server's stdin
when the editor exits.
