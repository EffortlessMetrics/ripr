# Changelog

## 0.4.0

- Aligns the marketplace extension with the 0.4 editor-agent evidence loop:
  saved-workspace diagnostics, hover evidence, targeted-test briefs, best
  related-test navigation, and copied agent packet/brief/verify/receipt
  commands now describe the same focused-test workflow.
- Keeps normal editor installs self-provisioned through matching GitHub Release
  server manifests and archives. `cargo install ripr` remains an offline,
  pinned, or controlled-environment fallback.
- Documents that CI users should rely on the generated non-blocking artifact
  workflow, while the CLI remains the shared engine for local proof and
  automation.
- Keeps preview limits explicit: no mutation execution, no automatic edits, no
  CI blocking, no unsaved-buffer overlays by default, and no bundled
  platform-specific VSIXs yet.

## 0.3.1

- Aligns the extension package with the first defaults-first `ripr` release
  line, including saved-workspace seam diagnostics, targeted-test brief actions,
  best related-test navigation, and `draft` mode defaults.
- Keeps server self-provisioning through GitHub Release manifests, with
  `cargo install ripr` as an offline/manual fallback.

## 0.3.0

- Uses the `tower-lsp-server` sidecar from `ripr 0.3.0`.
- Adds diagnostic-targeted context actions so `ripr: Copy Finding Context`
  can use the selected finding location instead of only the active cursor.
- Shows finding-specific hover details for current `ripr` diagnostics.
- Improves diagnostic stability with workspace-root initialization, stale
  diagnostic clearing, refresh failure logging, saved-workspace refresh
  semantics, and serialized refresh generations.

## 0.2.0

- First self-provisioning preview extension.
- Starts `ripr lsp --stdio` from a configured, bundled, downloaded, cached, or
  PATH-discovered server.
- Adds first-run server download and SHA-256 verification from GitHub Release
  manifests.
- Adds commands to restart the server, show the output channel, copy a finding
  context packet, and open settings.
- Keeps `cargo install ripr` as an offline/manual fallback.

## 0.1.0

- Initial PATH-based preview extension scaffold.
