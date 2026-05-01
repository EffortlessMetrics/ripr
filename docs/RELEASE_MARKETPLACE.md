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
ripr crate:      0.1.x
VS extension:    0.1.x
Open VSX:        0.1.x
```

For `0.1.x`, the extension requires `ripr 0.1.0` or newer installed separately:

```bash
cargo install ripr
```

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

## Local Package Gates

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.1.0.vsix --force
```

Also run the Rust gates from the repository root:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Secrets

Repository secrets:

```text
VS_MARKETPLACE_TOKEN
OPEN_VSX_TOKEN
```

The Open VSX namespace must exist before publish:

```bash
npx ovsx create-namespace EffortlessMetrics -p "$OPEN_VSX_TOKEN"
```

## Manual Marketplace Publish

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
npx @vscode/vsce publish --packagePath dist/ripr-0.1.0.vsix --pat "$VS_MARKETPLACE_TOKEN"
npx ovsx publish dist/ripr-0.1.0.vsix -p "$OPEN_VSX_TOKEN"
```

## CI Publish

Use:

```text
.github/workflows/publish-extension.yml
```

The workflow packages one VSIX, uploads it as an artifact, publishes that same
VSIX to both registries, and attaches it to the GitHub Release when run from a
tag.

For the first extension release after the `v0.1.0` crate tag already exists,
prefer manual `workflow_dispatch` or local publish using the packaged VSIX.

## Post-Publish Verification

```text
VS Code Marketplace listing is live.
Open VSX listing is live.
Both listings show the same version.
GitHub Release has the VSIX attached.
Installing from each registry works.
The extension starts `ripr lsp`.
Missing `ripr` executable shows the install/settings message.
```

