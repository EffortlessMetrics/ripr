# Open VSX

Open VSX publishing is separate from crates.io and the VS Code Marketplace.

The extension identity is:

```text
EffortlessMetrics.ripr
```

The `publisher` field in `editors/vscode/package.json` defines the Open VSX
namespace. The namespace must exist before publishing.

## One-Time Setup

1. Create or confirm an Eclipse account.
2. Log in to Open VSX with GitHub.
3. Link the Eclipse account.
4. Sign the Open VSX Publisher Agreement.
5. Generate an Open VSX access token.
6. Add the repository secret:

```text
OVSX_PAT
```

7. Create the namespace once:

```bash
npx ovsx create-namespace EffortlessMetrics -p "$OVSX_PAT"
```

If this step is skipped, publishing can fail with:

```text
Unknown publisher: EffortlessMetrics
```

## Manual Publish

Use the same packaged VSIX that is used for the VS Code Marketplace:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
npx ovsx publish dist/ripr-0.2.0.vsix -p "$OVSX_PAT" --skip-duplicate
```

The GitHub release workflow uses the same `OVSX_PAT` secret for Open VSX
publishing.

## Verification

After publish:

```text
Open VSX listing exists.
Version matches editors/vscode/package.json.
Install succeeds in an Open VSX-compatible editor.
ripr starts from configured, bundled, downloaded, cached, or PATH fallback server.
```
