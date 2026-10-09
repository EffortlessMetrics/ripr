# RIPR-SPEC-0180: Bounded native npm bootstrap

Status: accepted

## Problem

A JavaScript launcher and five native npm payload packages are unnecessary
prerequisites for an honest first Linux-only prerelease. The product already
has a qualified, public Linux executable whose identity must not drift while
adding npm as a distribution channel.

## Behavior

The `@effortlessmetrics/ripr` npm prerelease contains the exact Linux x86-64 ELF
from a fresh wheel built and qualified in the same run and attempt. The npm
qualifier reuses the existing Python build/pip/uv workflow; the standalone PyPI
publisher keeps its independent manual-main workflow authority. The command remains `ripr`. It exposes `bin/ripr`
directly, declares Linux/x64/
glibc metadata, and documents glibc 2.34+ and npm 10+. It has no lifecycle
scripts, package dependencies, source compilation or install/runtime download.

A runtime native pin is derived from the immutable three-file wheel artifact,
its API ID/ZIP digest, qualification and wheel receipts, and trusted current
Cargo version/lockfile/features/toolchain. All must agree on source/run/attempt.
The original alpha.1 public-wheel pin is historical evidence only; it is never
relabelled or used as a fallback. Version authority remains Cargo. Packaging commit/tree are separate provenance
fields. Package creation fails on an uncommitted tracked tree, wrong input
bytes, unsupported metadata, unsafe archive entries, stale RECORD, missing
notices, or tarball identity/mode/provenance drift.
ZIP member names must retain their literal wire identity: backslashes and NUL
suffixes cannot be normalized into an otherwise expected artifact or wheel member.
This refusal applies on both Unix and Windows readers before member bytes are read.

Native execution and qualification retain explicit denominators. Each selected
route, including fresh reinstall, retains an explanation naming its selected
follow-up finding from the retained check output. Unrelated nonempty explanation
bytes cannot satisfy that journey.
Each selected client must independently install the actual tarball project-locally, globally,
through npm exec and the actual public npx executable, then produce nonempty
findings and a follow-up action. A missing or broken npx wrapper fails
qualification. Reinstallation requires observed package/bin-link absence before
a fresh install restores the pinned bytes, version and useful behavior.
Consumer execution has no Rust or source checkout. npm operates offline and
HTTP(S) proxies point at closed loopback; this is not OS-level network isolation. Negative
controls retain nonzero failures, unsupported-platform metadata rejection and
honest incomplete output when the Python preview is disabled.

Source-owned staging is a separate manual operation. Read-only admission binds
the exact current main SHA/tree, latest complete manual qualification attempt,
seven executed jobs and five selected immutable artifact ZIP digests. The
closed job set includes native build, pip and uv success; the same-run native
artifact is independently revalidated before either npm receipt is admitted. It reuses the
native/wheel/tar authority above and requires both retained installed-use rows,
all four distinct routes, fresh reinstall, and real npm staging-client transport
of those bytes against a loopback mock registry. No artifact code runs during
admission.

Admission-only cannot request OIDC or write to npm. Explicit staging requires
the exact version/digest confirmation, an existing non-placeholder package,
an absent public version, and an existing main-only GitHub npm environment with
the selected maintainer reviewer. The isolated OIDC job receives one admitted
tarball, rechecks its digest, and invokes pinned npm stage publish with scripts
disabled and fixed public/next metadata. It performs no checkout, package
execution, build, direct publication, stage approval or settings mutation.
After environment approval and immediately before the stage request, trusted
inline code repeats the anonymous existing-package/version-absence check. A
public version that appeared during the wait fails before the attempted write.
The final short read/write race is owned by the registry's immutable version
constraint; the preflight does not claim atomicity.

The returned stage ID and identity are inspected in a separate read-only job.
A staging response does not establish staged-byte readback, external provenance
verification, maintainer 2FA approval, or public delivery. Missing or uncertain
stage results require operator reconciliation before retry. Existing published
versions are never repacked, restaged or overwritten to test this path.

## Required Evidence

`.github/scripts/test_npm_package.py` exercises the same byte-admission
functions used by `.github/scripts/npm_package.py`, including invalid wheel
and tar cases. `.github/scripts/npm_consumer.py` measures four installed routes
on the exact tarball. `npm-package-qualification.yml` requires both selected
Node/npm rows and a separately executed real npm CLI loopback transport control
on attempt-bound artifacts. The latter job also runs pinned actionlint to reject
invalid workflow syntax or expression contexts. Release admission tests challenge wrong source/run,
attempt, artifact, native pin, route denominators, environment, public version
and staging-result identities. Tests exercise the retained admission entrypoint
and actual workflow shell guards. `docs/NPM_BOOTSTRAP.md` defines first
publication, immutable-byte readback, source separation, stage-only setup and
future migration.

## Non-Goals

No automatic public release, account/security mutation, stable version,
multiplatform promotion, generic JavaScript API or full swarm integration.
This contract does not claim completion of the originally planned scoped npm
family or trusted-publisher setup. npm cannot enforce the glibc minor version;
older loaders may refuse execution. Forced platform installation is excluded.
The loopback test does not establish OIDC, live staging, Sigstore, environment
enforcement, or 2FA promotion. GitHub main is checked at admission, not after
an arbitrary later environment-approval delay. Environment admin-bypass state
requires operator UI verification when GitHub's GET response omits it.

## Validation

```text
python3 -m unittest discover -s .github/scripts -p test_npm_package.py
python3 .github/scripts/npm_package.py target/ripr/npm-qualified # inside the qualified workflow run
python3 .github/scripts/npm_consumer.py target/ripr/npm-qualified target/ripr/npm-consumer
cargo xtask check-file-policy
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-spec-numbering
```

## Acceptance Examples

- A fresh same-run qualified wheel produces one package with its exact native
  digest, both licenses, SBOM and separately recorded packaging/product identities.
- A changed native byte, wrong wheel hash, symlink, stale RECORD or missing
  executable is rejected before publication.
- Four installed routes each produce a real Python-preview finding and
  successfully explain its identifier; zero selected subjects is not a pass.
- Contradictory OS, CPU or libc metadata is rejected by the selected npm clients.
- A PR qualifier, stale attempt, failed/skipped job, altered archive or empty
  installed-use row cannot reach the credentialed stage job.
- Admission-only can inspect a candidate for an already-public version but returns
  stage_eligible false; explicit staging rejects that immutable version.
- A successful loopback stage sends exactly the qualified tar, no lifecycle
  marker appears, and the receipt retains two selected/observed requests.
- Wrong/missing stage IDs or returned integrity fail without claiming release.

## Test Mapping

- `.github/scripts/test_npm_package.py`: malformed wheel/tar and identity controls.
- `.github/scripts/npm_consumer.py`: measured installs, use, disablement and cleanup.
- `xtask/src/policy/file_policy.rs`: exact non-Rust runtime exception boundary.

## Implementation Mapping

- `.github/scripts/npm_package.py`: same-run native-input validation and npm tar creation.
- `.github/workflows/python-wheel-qualification.yml`: canonical native build and pip/uv qualification.
- `packaging/npm/bootstrap-alpha1-native-source.json`: immutable historical bootstrap record.
- `packaging/npm/package.template.json`: single-platform CLI package contract.
- `.github/workflows/npm-package-qualification.yml`: exact-head, attempt-bound proof.
- `.github/workflows/publish-npm.yml`: default read-only admission and isolated
  explicitly requested stage-only OIDC, followed by a read-only response record.

## Metrics

Retain four selected/executed routes for each of two selected client rows,
nonzero probes/findings, native/tar digests and named negative controls.
A missing/failed/not-run row never counts as qualified. A prepared artifact
and a publicly downloaded package remain different states.
Retain stage_eligible, selected_requests and observed_requests. A local transport
pass, admitted artifact, staging response and public delivery are distinct.
