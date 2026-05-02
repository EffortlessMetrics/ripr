# Repository Settings

Some security and review controls live in GitHub settings instead of the git
tree. This checklist records the expected settings so local automation, CI, and
review policy do not drift apart.

## Dependency Visibility

Expected state:

- Dependency Graph
- Dependabot alerts
- Dependabot security updates

Last verified: 2026-05-02. The dependency graph SBOM endpoint returned a
document, the vulnerability alerts endpoint returned `204 No Content`,
Dependabot security updates were enabled through the GitHub API, and Dependency
Review is configured as a blocking PR check.

Why:

- Dependency Review needs Dependency Graph data to evaluate pull requests.
- Dependabot alerts create security findings in the GitHub security tab.
- Dependabot security updates create repair PRs when supported advisories apply.

Repository files:

- `.github/dependabot.yml`
- `.github/workflows/security.yml`
- `deny.toml`

## Secret Scanning

Expected state:

- Secret scanning
- Secret scanning push protection
- Secret scanning validity checks, if available
- Non-provider pattern scanning, if available

Last verified: 2026-05-02. These settings were enabled through the GitHub API
where available.

Why:

`ripr` uses release and distribution tokens for crates.io, VS Marketplace, Open
VSX, Codecov, and GitHub release assets. GitHub push protection should catch
known token formats before they enter the repository. Repo-specific hygiene
checks still live in `xtask`, including `check-local-context`.

## Vulnerability Reporting

Expected state:

- Private vulnerability reporting
- `SECURITY.md`

Last verified: 2026-05-02. The GitHub API accepted the private vulnerability
reporting enable request, and the repository has a `SECURITY.md` policy.

Why:

Security reports should have a private intake path covering the CLI, library,
LSP sidecar, VS Code extension, release binaries, and server manifest.

## Code Scanning

Expected future checks:

- CodeQL for Rust and TypeScript/JavaScript
- Gitleaks or an equivalent secret scanning workflow
- OpenSSF Scorecard on a schedule

These are review and security signals. They should not rewrite repo policy
automatically.

## Branch Protection And Rulesets

Required checks should include:

- `CI / rust`
- `CI / vscode`
- `Coverage / coverage`
- `Security / cargo-deny`
- `Security / dependency-review`

Rules:

- block direct pushes to `main`
- block force pushes to `main`
- use squash merge for normal PRs
- require release workflow changes to pass security review

## Release Environments

Use GitHub Environments for token-bearing publish jobs:

- `vscode-marketplace`
- `open-vsx`
- `github-release`
- `crates-io`, if crate publishing is automated later

Store publish tokens in the narrowest environment that needs them:

- `VSCE_PAT` in `vscode-marketplace`
- `OVSX_PAT` in `open-vsx`

Environment protection gives release approvals, scoped secrets, and audit
history without adding another repo control plane.
