# Neovim standard-LSP proof recipe

This is a **candidate proof surface for [#1630](https://github.com/EffortlessMetrics/ripr-swarm/issues/1630)**,
not a Neovim support claim. The claim is earned only after this journey is run against an unmodified `ripr` binary
and its receipt is reviewed.

The boundary is ripr's portable LSP baseline through an off-the-shelf client: no VS Code methods, protocol shims,
report parsing, or ripr source edits.

## Boundary

| Surface | Contract |
|---|---|
| Client | Neovim built-in `vim.lsp` |
| Server | `ripr lsp --stdio` |
| Language | Rust |
| Root priority | `ripr.toml`, then `.git`, then `Cargo.toml` |
| Configuration | Standard `workspace/configuration`; no ripr initialization options |
| Refresh | `ripr.refresh` via standard `workspace/executeCommand` |
| Authority | Saved workspace on disk |
| Enhanced actions | `riprEditor` and `riprAgent` absent |

The config starts one client per resolved root. Run it only on a Neovim build that provides
`vim.lsp.config()` / `vim.lsp.enable()`, and record the exact version; one receipt does not prove a version range.

## Configure

Open `lsp/ripr.lua`:

```vim
:execute 'edit' stdpath('config') .. '/lsp/ripr.lua'
```

Add and save with `:write ++p`:

```lua
return {
  cmd = { "ripr", "lsp", "--stdio" },
  filetypes = { "rust" },
  root_markers = { "ripr.toml", ".git", "Cargo.toml" },
  workspace_required = true,
}
```

Enable it from `init.lua`:

```lua
vim.lsp.enable("ripr")
```

Do not add `init_options`, custom handlers, command rewrites, or compatibility methods.

## Preconditions

Run from the environment that launches Neovim:

```text
ripr --version
ripr lsp --version
nvim --version
```

Confirm Neovim's executable resolution:

```vim
:lua print(vim.fn.exepath("ripr"))
```

Use a disposable Rust fixture with a known finding whose expression contains non-ASCII text and whose evidence names
a related test. Then repeat on an authorized real repository, or record the actual exclusion against #1579. The
fixture must be safe to restore, have a stable root marker, and not be open under a different ripr build.

## Journey

Record each result as `pass`, `fail`, `limited`, or `not_run`. Preserve the first failure and relevant log excerpt.

### 1. Initialize and capture the first state

Open the fixture file:

```vim
:checkhealth vim.lsp
:lua print(vim.inspect(vim.lsp.get_clients({ name = "ripr", bufnr = 0 })))
:lua local c = assert(vim.lsp.get_clients({ name = "ripr", bufnr = 0 })[1]); print(vim.inspect({ offset_encoding = c.offset_encoding, client_capabilities = c.capabilities, server_capabilities = c.server_capabilities }))
```

Confirm one initialized client, `client.config.cmd == {"ripr","lsp","--stdio"}`, and the intended `root_dir`.
Record the first honest no-snapshot, pending, diagnostic, or typed-limitation state. When Neovim does not expose a
custom status notification, record that presentation limit and use diagnostics, hover, progress, and the LSP log.

### 2. Prove Unicode range, hover, and related evidence

Place the cursor on the known seam:

```vim
:lua print(vim.inspect(vim.diagnostic.get(0)))
:lua vim.lsp.buf.hover()
```

Confirm that the range selects the expected non-ASCII expression; code and canonical identity match the expected
finding or limitation; `user_data.lsp.relatedInformation`, when present, names the expected related test and range;
and hover describes the same observation. Related evidence retained in protocol data but omitted by the default UI
is a client presentation limit, not missing server evidence.

### 3. Inspect actions and lenses

```vim
:lua vim.lsp.buf.code_action()
:lua vim.lsp.codelens.enable(true, { bufnr = 0 })
:lua print(vim.inspect(vim.lsp.codelens.get({ bufnr = 0 })))
:lua vim.lsp.buf.code_action({ filter = function(action, client_id) print(vim.inspect({ client_id = client_id, action = action })); return true end })
```

Compare every offered command with `server_capabilities.executeCommandProvider.commands`. Zero unknown client-command
IDs may be offered. VS Code-only actions may be absent or inert with a named disabled reason; they must not execute.
Record unsupported or partial client presentation as `limited`.

### 4. Prove saved refresh and unchanged delivery

Make a finding-changing edit without saving and record the observation. It is non-authoritative:
[#1625](https://github.com/EffortlessMetrics/ripr-swarm/issues/1625) owns dirty-buffer synchronization.

Save, then invoke the server-owned standard command:

```vim
:write
:lua local c = assert(vim.lsp.get_clients({ name = "ripr", bufnr = 0 })[1], "ripr is not attached"); c:exec_cmd({ title = "Refresh RIPR", command = "ripr.refresh" }, { bufnr = 0 })
```

Confirm diagnostics, hover, related information, and lenses converge on saved state without restarting. Refresh again
without another change and confirm semantic delivery is unchanged. Restore the fixture, save, refresh, and confirm
the original observation returns.

### 5. Prove root change and isolation

Open Rust files from two independent repositories:

```vim
:lua print(vim.inspect(vim.lsp.get_clients({ name = "ripr" })))
```

Confirm one correctly rooted client per repository, no cross-root buffer attachment or diagnostics, and no dependence
on Neovim's process-wide working directory. A future multi-folder session needs a separate receipt.

### 6. Observe progress, logs, and shutdown

```vim
:lua print(vim.lsp.status())
:lua print(vim.lsp.log.get_filename())
:lua vim.lsp.log.set_level("debug")
```

Record initialize, configuration pull, document, diagnostic, hover, action, lens, refresh, shutdown, and exit traffic
where present; redact local paths before publication. Restore normal logging with
`:lua vim.lsp.log.set_level("warn")`.

Stop the clients:

```vim
:lsp stop ripr
```

Or:

```vim
:lua for _, c in ipairs(vim.lsp.get_clients({ name = "ripr" })) do c:stop(1000) end
```

Confirm shutdown/exit and no orphaned `ripr lsp --stdio` process. Record the platform-specific process check.

### 7. Attempt the real repository

Repeat the journey on an authorized real repository. Otherwise record `not_run` with the actual reason and add the
attempt or exclusion to #1579. A fixture receipt is not real-repository evidence.

## Receipt

This JSON is a proof record for #1630, not a stable ripr output schema.

```json
{
  "schema": "ripr-neovim-standard-lsp-proof/v1",
  "captured_at": "YYYY-MM-DDTHH:MM:SSZ",
  "scope": "fixture|authorized_real_repository",
  "client": {
    "name": "Neovim",
    "version": "exact nvim --version output",
    "configuration": "built-in vim.lsp; no LSP plugin"
  },
  "server": {
    "ripr_version": "exact ripr --version output",
    "lsp_version": "exact ripr lsp --version output",
    "protocol": "LSP over stdio",
    "command": ["ripr", "lsp", "--stdio"],
    "ripr_source_modified": false
  },
  "platform": {"os": "name and version", "arch": "architecture", "shell": "launch environment"},
  "workspace": {
    "language": "rust",
    "fixture_or_repository": "public identifier or stable token",
    "root_marker": "ripr.toml|.git|Cargo.toml",
    "resolved_root": "stable token",
    "second_root": null,
    "configuration": {
      "repository_config": "ripr.toml|none|other",
      "base_ref": null,
      "check_mode": null,
      "diagnostic_profile": null,
      "seam_diagnostics": null
    }
  },
  "negotiated": {
    "position_encoding": "utf-8|utf-16|utf-32|unknown",
    "diagnostic_mode": "push|pull|mixed|unknown",
    "client_capabilities": {},
    "server_capabilities": {},
    "extension_private_methods_used": []
  },
  "canonical_observation": {
    "kind": "diagnostic|limitation|none",
    "code": null,
    "diagnostic_id": null,
    "canonical_gap_id": null,
    "seam_id": null,
    "finding_id": null,
    "range": null,
    "related_locations": [],
    "hover": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "position_range_parity": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "unknown_client_commands_offered": null
  },
  "journey": {
    "initialize": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "initial_state": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "diagnostics": {
      "state": "pass|fail|limited|not_run",
      "evidence": "",
      "related_information_retained": null
    },
    "code_actions": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "code_lens": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "dirty_buffer_observation": {
      "state": "pass|fail|limited|not_run",
      "evidence": "",
      "authoritative": false
    },
    "save_refresh": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "unchanged_delivery": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "root_change": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "progress_and_logs": {"state": "pass|fail|limited|not_run", "evidence": ""},
    "shutdown": {
      "state": "pass|fail|limited|not_run",
      "evidence": "",
      "orphan_processes": null
    },
    "real_repository_attempt": {
      "state": "pass|fail|limited|not_run",
      "evidence": "",
      "exclusion_reason": null
    }
  },
  "normalized_sequence": [
    "initialize", "initialized", "workspace/configuration", "textDocument/didOpen",
    "diagnostic publication or pull", "textDocument/hover", "textDocument/codeAction",
    "textDocument/codeLens", "textDocument/didChange", "textDocument/didSave",
    "workspace/executeCommand:ripr.refresh", "shutdown", "exit"
  ],
  "measured": {
    "client_binary": null,
    "server_binary": null,
    "resolved_root_path": null,
    "client_count_first_root": null,
    "client_count_two_roots": null,
    "diagnostic_count_initial": null,
    "diagnostic_count_dirty": null,
    "diagnostic_count_after_saved_refresh": null,
    "diagnostic_count_after_unchanged_refresh": null,
    "diagnostic_count_after_restore_refresh": null,
    "refresh_duration_ms": null
  },
  "first_failure": null,
  "known_limits": [
    "No Neovim version range is claimed.",
    "Dirty buffers are observational until #1625 is earned.",
    "VS Code-enhanced actions are outside this baseline."
  ]
}
```

Keep timing and absolute paths in `measured`; use stable tokens elsewhere. Do not report zero because the UI hid a
field—derive it from retained protocol data or leave it unknown.

## Acceptance boundary

A reviewed receipt may establish one exact Neovim build on the tested platform and fixture. It does not establish
dirty-buffer authority, a generic version/platform range, VS Code parity, preview-language support, proxy or MCP
compatibility, headless-agent support, or mutation execution. Those require separate evidence.
