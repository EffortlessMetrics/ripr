# RIPR-SPEC-0180: Bounded native npm bootstrap

Status: accepted

## Problem

A JavaScript launcher and five native npm payload packages are unnecessary
prerequisites for an honest first Linux-only prerelease. The product already
has a qualified, public Linux executable whose identity must not drift while
adding npm as a distribution channel.

## Behavior

The `@effortlessmetrics/ripr` npm alpha contains the exact pinned Linux x86-64 ELF from the
published `ripr-rs` wheel. The command remains `ripr`. It exposes `bin/ripr`
directly, declares Linux/x64/
glibc metadata, and documents glibc 2.34+ and npm 10+. It has no lifecycle
scripts, package dependencies, source compilation or install/runtime download.

The source pin binds wheel and native digests, product version and product
source. Version must match Cargo. Packaging commit/tree are separate provenance
fields. Package creation fails on an uncommitted tracked tree, wrong input
bytes, unsupported metadata, unsafe archive entries, stale RECORD, missing
notices, or tarball identity/mode/provenance drift.

Native execution and qualification retain explicit denominators. Each selected
client must independently install the actual tarball project-locally, globally,
through npm exec and the actual public npx executable, then produce nonempty
findings and a follow-up action. A missing or broken npx wrapper fails
qualification. Reinstallation requires observed package/bin-link absence before
a fresh install restores the pinned bytes, version and useful behavior.
Consumer execution has no Rust or source checkout. npm operates offline and
HTTP(S) proxies point at closed loopback; this is not OS-level network isolation. Negative
controls retain nonzero failures, unsupported-platform metadata rejection and
honest incomplete output when the Python preview is disabled.

## Required Evidence

`.github/scripts/test_npm_package.py` exercises the same byte-admission
functions used by `.github/scripts/npm_package.py`, including invalid wheel
and tar cases. `.github/scripts/npm_consumer.py` measures four installed routes
on the exact tarball. `npm-package-qualification.yml` requires both selected
Node/npm rows on attempt-bound artifacts. `docs/NPM_BOOTSTRAP.md` defines first
publication, immutable-byte readback, source separation and future migration.

## Non-Goals

No automatic registry write, account/security mutation, stable version,
multiplatform promotion, generic JavaScript API or full swarm integration.
This contract does not claim completion of the originally planned scoped npm
family or trusted-publisher setup. npm cannot enforce the glibc minor version;
older loaders may refuse execution. Forced platform installation is excluded.

## Validation

```text
python3 -m unittest discover -s .github/scripts -p test_npm_package.py
python3 .github/scripts/npm_package.py target/ripr/npm-qualified
python3 .github/scripts/npm_consumer.py target/ripr/npm-qualified target/ripr/npm-consumer
cargo xtask check-file-policy
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-spec-numbering
```

## Acceptance Examples

- A genuine pinned wheel produces one package with the original native digest,
  both licenses, SBOM and distinct packaging/product source identities.
- A changed native byte, wrong wheel hash, symlink, stale RECORD or missing
  executable is rejected before publication.
- Four installed routes each produce a real Python-preview finding and
  successfully explain its identifier; zero selected subjects is not a pass.
- Contradictory OS, CPU or libc metadata is rejected by the selected npm clients.

## Test Mapping

- `.github/scripts/test_npm_package.py`: malformed wheel/tar and identity controls.
- `.github/scripts/npm_consumer.py`: measured installs, use, disablement and cleanup.
- `xtask/src/policy/file_policy.rs`: exact non-Rust runtime exception boundary.

## Implementation Mapping

- `.github/scripts/npm_package.py`: pinned-input validation and npm tar creation.
- `packaging/npm/native-source.json`: native product-source authority and hashes.
- `packaging/npm/package.template.json`: single-platform CLI package contract.
- `.github/workflows/npm-package-qualification.yml`: exact-head, attempt-bound proof.

## Metrics

Retain four selected/executed routes for each of two selected client rows,
nonzero probes/findings, native/tar digests and named negative controls.
A missing/failed/not-run row never counts as qualified. A prepared artifact
and a publicly downloaded package remain different states.
