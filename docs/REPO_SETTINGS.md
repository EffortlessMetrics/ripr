# Repository Settings

Some security and review controls live in GitHub settings instead of the git
tree. This checklist records the expected settings so local automation, CI, and
review policy do not drift apart.

## Repository Role

`EffortlessMetrics/ripr` is the public release and distribution authority.
Normal development has moved to `EffortlessMetrics/ripr-swarm`, where trusted
same-repo PRs use the routed swarm CI path. This source repository should accept
only release, security, and explicit swarm-to-source promotion PRs unless a
maintainer documents a source-only exception before opening the work.

Release, publish, signing, and marketplace secrets stay in this source
repository until a focused migration explicitly moves them. Do not copy those
secrets or release environments to `ripr-swarm` as part of routine development
cutover.

## Settings App Contract

The reviewable Settings App contract lives in `.github/settings.yml`.

Managed from git:

- repository About metadata: name, description, homepage, and topics
- repository feature toggles: issues on, projects off, wiki off, downloads on
- default branch: `main`
- merge policy: squash merge enabled, merge commits disabled, rebase merge
  disabled, auto-merge disabled, update branch enabled, and delete branch on
  merge enabled
- classic `main` branch protection fields supported by the Settings App
- required status checks for routine PR merges
- CI policy labels documented in `docs/CI.md`

Not managed from `.github/settings.yml`:

- secrets
- release environments
- Dependency Graph
- Dependabot alerts and security updates
- secret scanning and push protection
- private vulnerability reporting
- GitHub Rulesets, including the current direct-push block for `main`
- future advanced security controls unless Settings App support is verified in a
  focused PR

Post-merge receipt:

- Confirm the GitHub Repository Settings App is installed for
  `EffortlessMetrics/ripr`.
- Let the app apply `.github/settings.yml`.
- Inspect branch protection and labels through the GitHub UI or API.
- Update this document with the last verified date and any applied-state notes.

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

Dependabot version updates run weekly for Cargo, the VS Code extension npm
package, and GitHub Actions. Routine updates are grouped by ecosystem and
limited to minor/patch changes. Major dependency updates are handled as scoped
human-reviewed PRs because they may affect MSRV, release behavior, CI runtime
policy, or extension compatibility. Dependabot PRs are not auto-merged; they
must pass normal CI, security, coverage, and `xtask` checks.

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

Required checks should use the emitted check-run names, not display-style
workflow prefixes. Current required checks are:

- `rust`
- `msrv`
- `vscode`
- `cargo-deny`
- `dependency-review`

Settings App managed rules:

- block force pushes to `main`
- block branch deletion for `main`
- require conversation resolution
- require linear history
- use squash merge for PRs
- keep merge commits and rebase merges disabled unless an owner-approved
  exception is documented before changing `.github/settings.yml`
- require release workflow changes to pass security review

GitHub Rulesets should separately block direct pushes to `main` / require the
PR merge path. Keep that rule enabled until Settings App support for the same
invariant is verified and moved into `.github/settings.yml` in a focused PR.

Advisory lanes should not be required by branch protection unless they are
promoted in a focused policy PR after calibration. This includes Droid review,
coverage, future Clippy candidates, RIPR self-dogfood, SARIF upload, Test
Analytics, release packaging or publish dry-runs, PR planning, and CI budget
forecasts.

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
