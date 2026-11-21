# Security Policy

## Supported Versions

`ripr` is alpha software. Security fixes target the latest published release and
the current `main` branch.

## Scope

Security reports may cover:

- the `ripr` CLI
- the `ripr` library API
- the experimental LSP sidecar
- the VS Code extension
- release binaries, VSIX packages, manifests, and download/update paths
- repository automation that handles publish or release credentials

Static exposure findings are product output, not security claims. Report a
finding as a vulnerability only when it creates a concrete security risk in the
tool, packaging, distribution, or release process.

## Reporting A Vulnerability

Use GitHub private vulnerability reporting for this repository when available.
If that is unavailable, contact the maintainer privately before public
disclosure.

Please include:

- affected version or commit
- affected surface
- reproduction steps
- expected and observed behavior
- impact assessment
- any known workaround

Do not include live credentials, private customer data, or exploit payloads that
are not needed to understand the issue.

## Response Expectations

The maintainer will triage credible reports as soon as practical, coordinate on
scope and impact, and publish fixes or mitigations through the normal release
channels.

Please do not publicly disclose a vulnerability until a fix or mitigation is
available, or until coordinated disclosure timing has been agreed.
