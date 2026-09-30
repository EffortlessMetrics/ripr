# Source-owned npm Linux bootstrap

The first npm prerelease is a **single native `@effortlessmetrics/ripr` package for Linux x86-64,
glibc 2.34+**, using npm 10+. It installs `bin/ripr` directly and contains no
JavaScript launcher, optional dependencies, lifecycle scripts or runtime
Python dependency. This is an explicit bounded exception to the multi-package
family originally planned in #1784, not completion of that family or its
five-platform support matrix. Swarm launcher #4716 and native payload work
remain separate; this source packaging slice does not promote swarm features.

The installed command remains `ripr`. npm rejected the attempted unscoped
`ripr` name on 2026-09-30 under its similarity rule; no unscoped version was
published. The scoped identity is a newly qualified package, not a renamed
upload of the earlier tarball. Any later unscoped name request is separate and
does not change the identity of existing scoped versions.

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
disablement, contradictory platform metadata, reinstall and uninstall. The npx
route invokes the public executable and rejects a missing/broken wrapper; npm
and npx versions must agree. Reinstallation must first remove both the global
package and bin link, then restore the exact bytes, version and useful behavior.
No-op uninstall or reinstall operations cannot satisfy that control.

The two client rows are a client matrix, not two operating-system claims.
The observed native host remains Ubuntu 22.04. npm's `libc` metadata identifies
the libc family, not its version: glibc 2.34+ is an explicit loader prerequisite.
The package deliberately omits any guarantee for older npm, `--force`, pnpm,
Yarn, Bun, macOS, Windows, ARM64 or Alpine/musl.

Artifacts are bound to run attempt. Re-run all jobs if any row needs retry;
partial retries cannot consume a previous attempt's package. Both rows, the
staging-client transport job and the whole run must pass. The transport job uses
npm 11.15.0 against a loopback mock registry, checks exact submitted tar bytes
and lifecycle suppression, and has no npm credentials or OIDC permission. It
does not test live staging, OIDC exchange or Sigstore. PR runs are rehearsal
only. Final publication consumes a
successful source-main manual qualification and exact hashes, never a rebuild.

## Bootstrap authorization and recovery

Before the maintainer first publishes, record the exact package/version,
product and packaging SHAs/trees, npm tarball SHA-256/integrity, native digest,
qualification run/attempt and both consumer proofs. The operation is public,
one-time, with the `next` dist-tag. Stable/latest movement is separate.

The genuine first package was published by an authenticated maintainer using
the inspected tarball. Current npm staging documentation permits new packages
by creating a public `0.0.0-stage` placeholder; that is outside this contract.
The source staging controller requires an existing genuine package and cannot
bootstrap an empty name-holding package. Account login/credential creation, 2FA and future
trusted-publisher settings are operator work. No credentials are read, copied,
created or persisted by the qualification workflow.

Immediately before upload, inspect registry state. If the version exists,
compare exact public bytes. Matching bytes are already delivered; conflicting
immutable bytes require a newly authorized version. Never overwrite, blanket
skip, force an upload or silently move a tag. After publication, independently
download the public tarball, compare SHA-256, integrity and installed native
bytes, and rerun all four install routes and useful journey.

The scoped `0.11.0-alpha.1` is public. Independent readback on 2026-09-30 matched
tar SHA-256 `10ea5fef2002a911e65aae4516e5b3de1562aaaf4a97372c3f00f9433855ab6d`
from [main qualification 36745513617](https://github.com/EffortlessMetrics/ripr/actions/runs/36745513617).
The requested tag was `next`; npm also retained `latest` on the initial package,
and authenticated removal returned E400. Both resolve to the alpha, so ordinary
unqualified installs currently select it too. This is not a stable-quality
claim or a successful next-only transition. See the matching
[npm CLI report](https://github.com/npm/cli/issues/8490). No replacement version
or unpublish workaround was used.

## Stage-only source workflow

`publish-npm.yml` is manual and defaults to `admit_only`. It takes the exact
qualification run ID **and attempt**, current source SHA/tree, prerelease
version and tarball SHA-256. Its admission job has read-only GitHub access and
no npm credentials or OIDC permission. It independently verifies:

- the full `refs/heads/main` authority, publisher snapshot and qualified source;
- a successful manual-main run of the exact npm qualification workflow, all
  four completed jobs, and four unique unexpired attempt-bound artifacts;
- every downloaded ZIP's API digest/size and bounded regular-file inventory;
- the committed native pin, original public wheel, exact native/SBOM/notices,
  source harness and fixture, tar manifest and provenance;
- both named Node/npm/npx client rows, four distinct executed routes each,
  retained nonempty analysis/explanation, fresh reinstall and negative controls;
- the real CLI loopback receipt bound to those same qualified tar bytes.

The source and latest-attempt API checks repeat after admission. These checks
establish current main **at admission time**, not after an arbitrary later
environment-approval delay. The admitted source/digest stay pinned throughout
that delay; moving main never silently repacks or retargets them.

`admit_only` copies the exact tar and records `admitted_not_staged`, with
`stage_eligible: false`. It can inspect a candidate whose version is already
public, but that does not permit overwriting it. Explicit `stage` rejects any
existing public version and requires this exact confirmation text:

```text
stage @effortlessmetrics/ripr VERSION next SHA256
```

Before scheduling its credentialed job, stage admission requires the existing
GitHub `npm` environment, the sole deployment policy `main` of type `branch`
(no tag rules), and required reviewer `EffortlessSteven`, with self-review
allowed for the sole-maintainer workflow. It never creates the environment.
When the environment API exposes `can_admins_bypass`, true is rejected; an
omitted field is explicitly not evidence that bypass is disabled. The operator
must verify that setting in GitHub's UI before enabling the trust grant.

The stage job receives only the admitted `.tgz`. It has `id-token: write`, no
checkout and no package/native execution. Pinned Node 24.19.0/npm 11.15.0 uses
fresh empty npm configuration, rechecks the sole file's SHA-256, and runs
`npm stage publish` on that **file** with fixed registry, public access, `next`,
ignored lifecycle scripts and provenance enabled. It never calls `npm publish`,
`npm stage approve`, registry administration or a build command. No npm token
fallback is configured.

A separate read-only job validates the returned stage ID, identity and
integrity. Its receipt says `staging_response_received`; independent staged-byte
inspection, maintainer approval and public delivery are explicitly false.
The in-tar `automatic_npm_oidc_provenance: false` remains the preparation-time
claim. npm may attach an external OIDC provenance statement at staging; it must
be inspected separately and never requires rewriting the qualified tar.

### Operator setup and later promotion

The exact proposed trust grant is:

```text
package: @effortlessmetrics/ripr
provider: GitHub Actions (GitHub-hosted runners)
owner: EffortlessMetrics
repository: ripr
workflow filename: publish-npm.yml
environment: npm
allowed: npm stage publish
direct npm publish: disabled
```

Create the `npm` GitHub environment with the protection rules above and disable
administrator bypass, then configure this package-level grant only under the
separate operator approval. A repository merge does not create either setup.
The environment was absent at the 2026-09-30 setup inspection; this source change
does not claim it now exists. Do not restrict or revoke unrelated credentials
as part of this preparation.

OIDC tokens can stage but cannot list, view, download, approve or reject stages.
An authorized maintainer session must inspect the recorded stage using
`npm stage view` and `npm stage download`, compare the downloaded tar digest,
package/version/tag and any external provenance, and rerun selected consumers
before separately approving with 2FA. Approval changes public registry state.
Afterward, independently download public bytes and verify named installations
and actual dist-tags. A staged version is never public-delivery proof.

Staging reserves the version. If a request times out, returns incomplete JSON
or loses its stage ID, record the uncertain result and inspect the maintainer's
stage list before any retry. Do not blindly repeat, reject, change tags, publish
directly or invent a replacement version. The workflow serializes attempts and
does not cancel a running staging operation. Staging uses `--fetch-retries=0`
so a failed transport is reconciled before another write attempt. Rejection is a separate destructive
operator action. Existing matching public bytes remain delivered; a conflict
requires a newly authorized version.

Live OIDC exchange, staging, environment enforcement, staged download and 2FA
promotion remain unverified until an actual future qualified version exercises
them. The already-public alpha must not be staged again merely to test setup.
References: [trusted publishing](https://docs.npmjs.com/trusted-publishers/),
[staged publishing](https://docs.npmjs.com/staged-publishing/),
[stage CLI and permissions](https://docs.npmjs.com/cli/v11/commands/npm-stage/).

## Future multi-platform transition

A later immutable version can keep package `@effortlessmetrics/ripr` and command
`ripr` while replacing the
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
