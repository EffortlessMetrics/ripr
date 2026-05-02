# Marketplace Release

This document covers the editor extension release surfaces:

```text
VS Code Marketplace:
  EffortlessMetrics.ripr

Open VSX:
  EffortlessMetrics.ripr
```

The Rust crate release is documented separately in [RELEASE.md](RELEASE.md).

## Versioning

Keep versions aligned:

```text
ripr crate:      0.2.x
VS extension:    0.2.x
Open VSX:        0.2.x
```

For `0.2.x`, the universal extension can download the matching `ripr` server
from GitHub Releases. `cargo install ripr` remains a manual fallback.

## Required Files

Before publishing, confirm `editors/vscode` contains:

```text
README.md
CHANGELOG.md
LICENSE
icon.png
package.json
package-lock.json
```

Use `icon.png`, not SVG.

The extension icon should be regenerated from the canonical brand asset at
`assets/logo/ripr-icon-dark.svg`. The committed marketplace PNG derivative is
kept at `assets/logo/ripr-icon-dark.png` and copied to `editors/vscode/icon.png`.

## Local Package Gates

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.2.0.vsix --force
```

Also run the Rust gates from the repository root:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Before publishing a self-provisioning extension, confirm the matching server
binary assets and manifest exist on the GitHub Release. See
[RELEASE_BINARIES.md](RELEASE_BINARIES.md).

## Secrets

Repository secrets:

```text
VSCE_PAT
OVSX_PAT
```

The Open VSX namespace must exist before publish:

```bash
npx ovsx create-namespace EffortlessMetrics -p "$OVSX_PAT"
```

## Manual Registry Publish

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
npx @vscode/vsce publish --packagePath dist/ripr-0.2.0.vsix --pat "$VSCE_PAT"
npx ovsx publish dist/ripr-0.2.0.vsix -p "$OVSX_PAT" --skip-duplicate
```

## CI Publish

Use:

```text
.github/workflows/publish-extension.yml
```

The workflow packages one VSIX, uploads it as an artifact, publishes that same
VSIX to both registries, and attaches it to the GitHub Release when run from a
tag.

To publish only Open VSX from a manual workflow run:

```bash
gh workflow run publish-extension.yml \
  --field version=0.2.0 \
  --field publish_vs_marketplace=false \
  --field publish_open_vsx=true
```

For the first self-provisioning extension release, publish the server binary
assets before publishing the marketplace VSIX.

## Post-Publish Verification

```text
VS Code Marketplace listing is live.
Open VSX listing is live.
Both listings show the same version.
VS Marketplace manual install badge count was refreshed.
Open VSX badges render.
GitHub Release has the VSIX attached.
Installing from each registry works.
The extension starts `ripr lsp`.
Missing `ripr` executable shows the install/settings message.
```

## Marketplace Badge Maintenance

VS Marketplace install-count badges are manually maintained.

Do not use live VS Marketplace Shields routes for install, download, or version
counts. They are intentionally not treated as a reliable source of truth for
this repo.

Use static VS Marketplace badges instead:

```text
https://img.shields.io/badge/VS%20Marketplace-<count>%20installs-0078D4
```

Open VSX badges may use live Shields routes:

```text
https://img.shields.io/open-vsx/v/EffortlessMetrics/ripr
https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr
```

After each extension release:

1. Open the VS Marketplace publisher metrics page.
2. Record the current install count.
3. Update the manual badge count in `README.md` and
   `editors/vscode/README.md`.
4. Update the hidden `Last checked: YYYY-MM-DD` comment near each manual badge.
5. Leave Open VSX badges as live Shields badges.
