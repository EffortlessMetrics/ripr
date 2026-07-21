# Repository Settings

Some security and review controls live in GitHub settings instead of the git
tree. This checklist records the expected settings so local automation, CI, and
review policy do not drift apart.

This checkout is `EffortlessMetrics/ripr`, the release, publishing, signing,
marketplace, badge, and distribution authority. Reviewed development lands in
`EffortlessMetrics/ripr-swarm` and is promoted here with preserved history.

## Settings App Contract

The reviewable Settings App contract lives in `.github/settings.yml`.

Managed from git:

- repository About metadata: name, description, homepage, and topics
- repository feature toggles: issues on, projects off, wiki off, downloads on
- default branch: `main`
- merge policy: squash and merge commits enabled, rebase merge and auto-merge
  disabled, update branch enabled, and delete branch on merge enabled
- branch protection for `main` requires `rust`, `msrv`, `vscode`, `cargo-deny`,
  and `dependency-review`
- the classic protection setting does not require an approving review; the
  active `main` ruleset separately requires conversation resolution
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
- Inspect metadata and labels through the GitHub UI or API.
- Update this document with the last verified date and any applied-state notes.

## Source / Swarm Boundary

`ripr` is the release authority. Keep these surfaces here unless a focused,
reviewed boundary change explicitly moves them:

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
must pass the protected source checks and any owner-required security review
before merge. Additional security, coverage, and `xtask` lanes remain review
signals unless promoted in a focused policy PR.

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

Required checks use emitted check-run names, not display-style workflow
prefixes. Source `main` requires:

- `rust`
- `msrv`
- `vscode`
- `cargo-deny`
- `dependency-review`

The routed `Ripr Rust Small Result` remains useful development-trunk evidence,
but it does not replace the source repository's release-oriented protected
checks.

Settings App managed rules:

- block force pushes to `main`;
- block branch deletion for `main`;
- leave conversation resolution disabled unless a focused policy change
  promotes it;
- leave linear history disabled so history-preserving source promotions are
  possible;
- permit squash merges for ordinary PRs and merge commits for audited source
  promotions;
- keep rebase merge and auto-merge disabled; and
- require release workflow changes to pass security review.

GitHub Rulesets should separately block direct pushes to `main` and require the
PR merge path. A source promotion must use **Create a merge commit** so the
reviewed two-parent join remains reachable; it must never be squashed or
rebased. See [Source Promotion](SOURCE_PROMOTION.md).

Advisory lanes should not become protected requirements without a focused
policy change after calibration.

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
