# Repository Settings

Some security and review controls live in GitHub settings instead of the git
tree. This checklist records the expected settings so local automation, CI, and
review policy do not drift apart.

This checkout is `EffortlessMetrics/ripr-swarm`, the public development landing
zone for trusted same-repo `ripr` PRs. The release-facing source repository
remains `EffortlessMetrics/ripr`.

## Settings App Contract

The reviewable Settings App contract lives in `.github/settings.yml`.

Managed from git:

- repository About metadata: name, description, homepage, and topics
- repository feature toggles: issues on, projects off, wiki off, downloads on
- default branch: `main`
- merge policy: squash merge enabled, merge commits disabled, rebase merge
  disabled, auto-merge enabled, update branch enabled, and delete branch on
  merge enabled
- branch protection requires only the routed `Ripr Rust Small Result` check
  after proof
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
  `EffortlessMetrics/ripr-swarm`.
- Let the app apply `.github/settings.yml`.
- Inspect metadata and labels through the GitHub UI or API.
- Update this document with the last verified date and any applied-state notes.

## Swarm Development Boundary

`ripr-swarm` is not the release authority for `ripr`. Keep these surfaces in
`EffortlessMetrics/ripr` until a focused promotion changes that boundary:

- crates.io publishing
- VS Marketplace publishing
- Open VSX publishing
- GitHub Release assets
- signing or release environment secrets

Self-hosted runners are limited to trusted same-repo PRs and pushes. Fork or
otherwise untrusted PRs must use GitHub-hosted runners or skip self-hosted
implementation jobs. See [Swarm development](swarm-development.md).

## Dependency Visibility

Expected state:

- Dependency Graph
- Dependabot alerts
- Dependabot security updates

Last verified: 2026-05-02. The dependency graph SBOM endpoint returned a
document, the vulnerability alerts endpoint returned `204 No Content`,
Dependabot security updates were enabled through the GitHub API, and Dependency
Review is configured as a security signal.

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
must pass the protected routed result and any owner-required security review
before merge. Security, coverage, and `xtask` lanes remain review signals unless
promoted in a focused policy PR.

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
workflow prefixes. `ripr-swarm` does not require source-repo contexts such as
`rust`, `msrv`, or `vscode`. Branch protection requires only the normalized
routed CI result job.

Required checks:

- `Ripr Rust Small Result`

Do not require conditional implementation jobs such as `Ripr Rust Small on
CX53`, `Ripr Rust Small on CX43`, or `Ripr Rust Small on GitHub Hosted`.
Do not require advisory security jobs such as `cargo-deny` or
`dependency-review` unless a focused policy PR promotes them after calibration.
CX53 and CX43 remain tracked proof obligations, but the protected branch gate is
the normalized result check.

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

These release environments and publish tokens belong to the source repository,
not `ripr-swarm`, until a dedicated release-boundary change is approved.
