# Source-owned npm Linux bootstrap

The first npm prerelease is a **single native `ripr` package for Linux x86-64,
glibc 2.34+**, using npm 10+. It installs `bin/ripr` directly and contains no
JavaScript launcher, optional dependencies, lifecycle scripts or runtime
Python dependency. This is an explicit bounded exception to the multi-package
family originally planned in #1784, not completion of that family or its
five-platform support matrix. Swarm launcher #4716 and native payload work
remain separate; this source packaging slice does not promote swarm features.

## Native versus packaging source

`packaging/npm/native-source.json` pins the genuine published PyPI wheel by its
public URL, filename and SHA-256, plus its native executable SHA-256, source
commit/tree, lockfile and successful qualification run. That exact audited ELF
is extracted unchanged. The npm package's version derives from this native pin
and must agree with the Cargo workspace version. Version drift fails closed;
advancing to stable requires a freshly qualified stable native input.

The package records **product source** and **packaging source** independently
in `provenance.json`. It never claims its executable was rebuilt from the later
npm packaging commit. It retains both original licenses and the native SBOM.
Manual bootstrap has no automatic npm OIDC provenance claim.

## Read-only qualification

`npm-package-qualification.yml` checks out the exact candidate, validates the
pinned wheel's hash, metadata, RECORD, inventory, ELF architecture, payload hash
and notices, then lets pinned npm create the tarball. The actual tarball is
inspected for exact files, bytes, executable mode, version, platform metadata,
public access and `next` tag. Negative unit controls reject missing/mutated
payloads, metadata drift, stale RECORD, traversal, links and provenance drift.

Separate checkout-free Node 20/npm 10 and Node 24/npm 11 jobs install the same
artifact with lifecycle scripts disabled. They verify project-local, global,
explicit npm exec and npx routes, each with a nonzero-subject Python-preview
analysis and a real follow-up explanation. Rust and source checkouts are absent from native execution; npm runs offline
and HTTP(S) proxy variables point to a closed loopback port. This is a client
network control, not an operating-system network sandbox. The proof also checks planted PATH
and project Python, nonzero exit, clean LSP stdin/stdout, explicit preview
disablement, contradictory platform metadata, reinstall and uninstall.

The two client rows are a client matrix, not two operating-system claims.
The observed native host remains Ubuntu 22.04. npm's `libc` metadata identifies
the libc family, not its version: glibc 2.34+ is an explicit loader prerequisite.
The package deliberately omits any guarantee for older npm, `--force`, pnpm,
Yarn, Bun, macOS, Windows, ARM64 or Alpine/musl.

Artifacts are bound to run attempt. Re-run all jobs if any row needs retry;
partial retries cannot consume a previous attempt's package. Both rows and the
whole run must pass. PR runs are rehearsal only. Final publication consumes a
successful source-main manual qualification and exact hashes, never a rebuild.

## Bootstrap authorization and recovery

Before the maintainer first publishes, record the exact package/version,
product and packaging SHAs/trees, npm tarball SHA-256/integrity, native digest,
qualification run/attempt and both consumer proofs. The operation is public,
one-time, with the `next` dist-tag. Stable/latest movement is separate.

npm cannot stage a brand-new package. First publication therefore requires an
authenticated maintainer CLI operation on the genuine tarball. Do not create an
empty name-holding package. Account login/credential creation, 2FA and future
trusted-publisher settings are operator work. No credentials are read, copied,
created or persisted by the qualification workflow.

Immediately before upload, inspect registry state. If the version exists,
compare exact public bytes. Matching bytes are already delivered; conflicting
immutable bytes require a newly authorized version. Never overwrite, blanket
skip, force an upload or silently move a tag. After publication, independently
download the public tarball, compare SHA-256, integrity and installed native
bytes, and rerun all four install routes and useful journey.

The existing `publish-npm.yml` identity remains read-only. Configure a per-
package trusted publisher only after the real package exists, under separate
approval. Prefer stage-only permission for future versions. Do not claim
staging or trusted publishing is operational until tested with an actual
staged package and maintainer 2FA approval.

## Future multi-platform transition

A later immutable version can keep package/command `ripr` while replacing the
single-platform bin with a verified launcher plus exact-version platform
payload packages. Qualify and publish every selected payload before that
launcher. Remove the Linux-only root restrictions only when each added target
passes real native install/use qualification. This alpha version remains
unchanged and continues to identify its exact native source.

## Spec, tests, implementation, proof

- Contract: `RIPR-SPEC-0180` and source #1784/#1781
- Native and tar admission: `.github/scripts/npm_package.py`
- Discriminating controls: `.github/scripts/test_npm_package.py`
- Installed consumer: `.github/scripts/npm_consumer.py`
- Exact source/client proof: `.github/workflows/npm-package-qualification.yml`

Rollback before publication is an ordinary source revert. Registry publication
is immutable and must be handled by the explicit fix-forward rule above.
