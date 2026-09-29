# PyPI and npm distribution plan

Status: implementation proposal, not an installation or release announcement.
The live acceptance graph is [swarm #4487](https://github.com/EffortlessMetrics/ripr-swarm/issues/4487).
For account and namespace setup, use the
[registry claim guide](how-to/claim-package-registries.md).

## Outcome

Install the native ripr CLI through Python and JavaScript tooling without
requiring a Rust toolchain, a source checkout, lifecycle scripts or a second
binary download. The installed tool must produce a useful result and a working
next step, not merely pass `--version`.

| Surface | Intended identity |
| --- | --- |
| Product, executable and Cargo package | `ripr` |
| PyPI distribution | `ripr-rs` |
| npm launcher | `@effortlessmetrics/ripr` |

These names are design inputs, not verified reservations. Python/npm packages
are distribution adapters, not new Rust crates or language-specific analyzers.
No Python SDK or `import ripr` API is proposed.

Development and credential-free rehearsal belong in `ripr-swarm`. Public
publication belongs only in `EffortlessMetrics/ripr`, following the existing
[release authority](RELEASE.md) and [release transaction](RELEASE_TRANSACTION.md).
This work selects no release version and adds no automatic 0.11 release blocker.
A merged planning document does not authorize settings, staging, publication,
credentials, tags or support-tier promotion.

## Work to implement

| Work | Issue | Dependency |
| --- | --- | --- |
| Package identity, versions, targets and policy integration | [#4488](https://github.com/EffortlessMetrics/ripr-swarm/issues/4488) | First implementation slice |
| Shared final payloads and compatibility evidence | [#4489](https://github.com/EffortlessMetrics/ripr-swarm/issues/4489) | Identity contract |
| Native Python wheels | [#4490](https://github.com/EffortlessMetrics/ripr-swarm/issues/4490) | Contract and selected target payload |
| npm launcher and native packages | [#4491](https://github.com/EffortlessMetrics/ripr-swarm/issues/4491) | Contract and selected target payload |
| Installed journeys and truthful user documentation | [#4492](https://github.com/EffortlessMetrics/ripr-swarm/issues/4492) | Actual adapter for the selected channel |
| No-publish consumer qualification and CI routing | [#4493](https://github.com/EffortlessMetrics/ripr-swarm/issues/4493) | Actual packages and journey contract |
| Human account/scope/publisher setup | [ripr #1780](https://github.com/EffortlessMetrics/ripr/issues/1780) | Account preparation can start now |
| Source-owned publishing and recovery | [ripr #1781](https://github.com/EffortlessMetrics/ripr/issues/1781) | Qualified channel inputs and operator setup |
| First functional PyPI prerelease | [ripr #1782](https://github.com/EffortlessMetrics/ripr/issues/1782) | Selected Python proof and explicit publication authority |
| First functional npm prerelease | [ripr #1784](https://github.com/EffortlessMetrics/ripr/issues/1784) | Selected npm proof and explicit publication authority |

The wheel and npm adapters may proceed in parallel after the identity contract.
A bounded PyPI prerelease need not wait for npm or every target. It still needs
real functionality, correct compatibility claims, source-owned artifacts and
independent public-install verification. Unadvertised targets and the broader
parent acceptance remain open.

A bounded npm prerelease need not wait for PyPI. It must publish genuine native
payload packages before the launcher that references them, use one exact version
throughout the selected package family, keep prereleases off `latest`, and
independently install the public bytes. A selected target subset is acceptable
only when package metadata and public documentation cannot imply absent targets
are supported.

## Repository surfaces

The implementation should extend existing owners rather than establish another
release framework. At the inspected swarm commit
[`69feaf4bd9834194f93a499fe1dd21925ceb7ed9`](https://github.com/EffortlessMetrics/ripr-swarm/commit/69feaf4bd9834194f93a499fe1dd21925ceb7ed9),
`crates/ripr/Cargo.toml` owns the native binary,
`editors/vscode/package.json` is the editor extension, and
`.github/workflows/server-archive-qualification.yml` supplies exact-candidate
archive qualification. See [binary packaging](RELEASE_BINARIES.md).

Proposed new authoring surfaces:

```text
packaging/python/pyproject.toml
packaging/python/README.md
packaging/npm/package.json
packaging/npm/bin/ripr.cjs
packaging/npm/README.md
.github/workflows/package-distribution-qualification.yml  (swarm rehearsal)
.github/workflows/publish-pypi.yml                       (source only)
.github/workflows/publish-npm.yml                        (source only)
```

The package authoring and qualification paths above remain implementation work.
Source PRs
[`ripr#1783`](https://github.com/EffortlessMetrics/ripr/pull/1783) and
[`ripr#1785`](https://github.com/EffortlessMetrics/ripr/pull/1785) establish the
exact proposed workflow filenames and environments as deliberately non-publishing
contracts. They do not build, retrieve, stage or upload packages and do not grant
registry-write authority.

Use the existing Rust-first `xtask` release/validation owners for target metadata,
package generation, receipts and tests. Extend version-bump/readiness checks so
one product version cannot leave stale adapter metadata. Register new surfaces
with workspace/file/non-Rust policy, generated-file checks and CI routing. The
npm launcher needs a narrow justified non-Rust exception, not a blanket escape
from `policy/non-rust-allowlist.toml`. Do not publish the editor dependency tree
as the CLI or commit generated binaries/build residue.

## Payload and compatibility contract

One selected source/version/feature/toolchain contract produces a final payload
for each target. Record the executable and any required bundled native libraries,
relative layout and digests after stripping, repair or signing. Container hashes
for wheels, npm tarballs and archives are separate. Compare extracted installed
payloads for the same target; cross-platform byte equality is not required.

[Maturin](https://www.maturin.rs/bindings.html) supports binary wheels. Its
[build and repair configuration](https://www.maturin.rs/config.html) must be
accounted for: it is not merely a wrapper around any arbitrary prebuilt binary.
Establish the final staging boundary and verify reuse rather than asserting
that pre-repair and installed bytes match.

| Intended target | npm payload suffix | Required compatibility evidence |
| --- | --- | --- |
| Linux x64, glibc | `ripr-linux-x64-gnu` | Selected manylinux/native libc floor and shared-library audit |
| Linux ARM64, glibc | `ripr-linux-arm64-gnu` | Native execution at the selected baseline |
| macOS x64 | `ripr-darwin-x64` | Declared deployment target and native execution |
| macOS ARM64 | `ripr-darwin-arm64` | Declared deployment target and native execution |
| Windows x64 MSVC | `ripr-win32-x64-msvc` | Native clean-host/runtime and process proof |

All npm payload names are scoped under `@effortlessmetrics`. These are targets
to qualify, not a statement that packages already exist. Do not relabel a newer
Ubuntu binary as older-manylinux compatible. musl/Alpine, Windows ARM64 and
untested long-path behavior are not implied. Python
[compatibility tags](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/)
are promises about the installed payload.

Map supported native SemVer prereleases deterministically to
[PEP 440](https://packaging.python.org/en/latest/specifications/version-specifiers/),
for example `X.Y.Z-rc.1` to `X.Y.Zrc1`. Reject ambiguous/lossy forms. Do not rename
an RC artifact to manufacture a stable release. Keep publisher-tool minimums,
consumer runtime minimums and Rust source-build requirements separate.

## Adapter behavior

**Python:** select the existing binary explicitly, build with locked inputs and
emit platform-specific executable wheels. Validate inherited workspace metadata,
licenses, RECORD entries, permissions and actual installer behavior. Initially
omit an sdist; an extracted source-build test is a separate requirement. No
runtime downloader, implicit compiler or fictitious Python import surface.

**npm:** expose `bin.ripr` through a small launcher with exact-version optional
native dependencies and an explicit package contents allowlist. Resolve from the
launcher package, check identity/version and spawn an absolute native path with
argument-array/no-shell execution. Preserve CWD, environment, exit behavior and
stdio. A missing optional dependency must produce actionable failure, not a
GitHub download, stale PATH fallback or another version.

Consumer installs must work with npm lifecycle scripts disabled. The wrapper
must not emit protocol stdout chatter or silently enable languages. Public npm
metadata and optional dependency semantics are described in the
[package.json reference](https://docs.npmjs.com/cli/v11/configuring-npm/package-json/).

## Installed qualification

The acceptance unit is an installed journey:

```text
exact package -> clean install -> useful analysis -> supported next step
              -> correct verification environment -> clean teardown
```

Use a local wheelhouse and isolated npm test registry seeded with the actual
artifacts. The wheelhouse aggregate and no-publish workflow live in
[Python wheelhouse qualification](PYTHON_WHEELHOUSE_QUALIFICATION.md). Remove
source checkout, Rust and ambient ripr from consumer reach;
retain the package-manager runtime and required Git. Exercise offline execution
after installation, native platform baselines and scripts-disabled npm.

Assert expected nonzero subjects/findings on Python and TypeScript fixtures,
including no-config and explicit-config behavior. Installing through npm must
not imply that TypeScript is automatically enabled. Preserve current
[support tiers](status/SUPPORT_TIERS.md) and existing activation authority.

Run the emitted next command from a fresh shell/foreign CWD. Prefer persistent
uv tool installation and exact project-local npm scripts for multi-step loops;
one-shot invocation must not imply bare `ripr` remains on PATH. A tool virtual
environment must not silently replace the project's test environment. Any
advertised repair or stdio route must execute with the real installed package;
a typed preview limitation is not a successful repair receipt.

Include missing/wrong-version payloads, planted PATH executables, corrupted
packages, incompatible targets, cross-platform lockfiles, quoting, exit codes,
Ctrl+C, LSP shutdown, upgrade/reinstall/uninstall and cleanup. Mark alternative
package managers supported only after exact-client proof. Record download size,
cold/warm install time and launcher overhead without mixing their conditions.

A receipt binds source and artifact identities, runtime/tool versions, selected
and executed counts, failures/skips, actual commands and cleanup. Missing or
zero-subject required rows are not pass. Reuse existing Windows/root/command
owners linked from the epic rather than creating a second execution framework.

## Publication and completion

Keep qualification and authorization separate. Source publication consumes the
qualified exact files; rebuilding after integration requires new qualification.
A publisher job must not quietly rebuild approved packages. No registry secrets
or OIDC publication permissions belong in swarm.

PyPI publication checks the complete selected wheel set, handles partial uploads
by exact filename/hash readback and then verifies a fresh public install. The
[first PyPI publication issue](https://github.com/EffortlessMetrics/ripr/issues/1782)
owns that separately authorized registry transaction.

npm publishes and verifies all exact native packages before the matching
launcher. The
[first npm publication issue](https://github.com/EffortlessMetrics/ripr/issues/1784)
owns the genuine package bootstrap, payload-first/launcher-last ordering,
independent public installs and post-bootstrap per-package publisher setup.

The [source publication issue](https://github.com/EffortlessMetrics/ripr/issues/1781)
owns reusable workflow admission, authorization, staging, provenance and
partial-failure/fix-forward controls. One channel's readiness or authority does
not imply the other's.

Current [npm staged publishing](https://docs.npmjs.com/staged-publishing/) needs
an existing package. The proposed steady-state path is OIDC staging followed by
human review/2FA approval; the first genuine package publication is a separate
bootstrap. A staged upload, a green rehearsal or a configured account is not
public delivery.

The implementation is complete only when advertised channels/targets are
independently installed and verified, their next steps work and documentation
matches the resulting evidence. Planning, implementation, qualification,
publication and public verification remain distinct states.
