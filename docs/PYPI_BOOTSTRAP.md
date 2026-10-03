# Source-owned PyPI bootstrap

This lane prepares one real Linux x86-64 CLI wheel for `ripr-rs`. It does not
authorize upload, the wider 0.11 release, npm publication, or other targets.
The proposed first version is Cargo `0.11.0-alpha.1`, Python `0.11.0a1`.
The version is selected in Cargo source before building, never by renaming
an existing binary or changing wheel metadata after the fact.

## Qualification

`python-wheel-qualification.yml` builds the exact source candidate with Rust
1.95 and locked dependencies. Maturin selects the existing binary and retains
its native version. Auditwheel repairs and checks its actual ELF dependency
closure for `manylinux_2_34_x86_64`. The package claims only Linux x86-64 with
glibc 2.34 or newer; the observed native consumer host is Ubuntu 22.04.
No macOS, Windows, ARM64, musl or full-matrix qualification is implied.

The wheel metadata, licenses, file inventory, RECORD hashes, executable
permissions and payload digest are inspected. Independent missing-executable
and stale-RECORD controls must fail. Separate pip and uv jobs install the exact
wheel without Rust or a source checkout, reject a planted PATH replacement,
run useful no-config Python preview analysis and a fresh-shell explanation, exercise
explicit disablement, reinstall, and uninstall without removing the project.

A build artifact alone is not qualification. The complete run, including both
consumer jobs, must pass. The attempt-specific artifact retains the exact wheel
and `qualification.json` for five days. PR qualification is rehearsal; publication
admission consumes a successful main-branch manual qualification run.

Retries must use **Re-run all jobs**, never **Re-run failed jobs**. Both consumers
require the build's attempt number to equal the current run attempt before
artifact download. A partial retry fails with explicit recovery guidance;
a full retry builds a new attempt-bound wheel and reruns both consumers.
No previous-attempt wheel is silently reused or admitted.

Manual qualification accepts only `refs/heads/main`, not a tag named `main`.
The trusted publisher independently verifies that the selected source is the
current `refs/heads/main` commit at admission through the GitHub API and matches
the publisher dispatch snapshot. If main advances before admission, qualify the
new main source and dispatch publication from that source again. Later environment
approval still uploads only the immutable admitted bytes.

The repository's own `ripr.toml` enables Python preview alongside Rust and
TypeScript so the new admission controller receives analysis in the required
Rust job's PR-evidence and review-guidance gates. This does not change the
installed CLI's language defaults or promote Python beyond preview. The
mixed-language producer test verifies real Python findings and rejects the
previous Python-disabled configuration.

## Publication

The source `publish-pypi.yml` workflow admits existing artifacts without
rebuilding. Its default operation is read-only admission. Actual publication
requires an explicit exact-version request and protected `pypi` environment,
with the exact pending-publisher tuple configured at PyPI. Account security and
new publisher grants remain operator actions.

Review the source SHA/tree, native/Python version, wheel name/digest, successful
qualification run and selected target before separately authorizing upload.
A pending publisher does not reserve the name; only a genuine accepted upload
creates the project. After upload, independently download the public bytes,
compare hashes, and repeat the installed journey before claiming delivery.

## Boundaries and recovery

This is a source packaging port from the adapter reviewed in swarm PR #4711,
not a claim that all later swarm product development was integrated. It uses
the existing source product and introduces no product-feature promotion.
The one-wheel initial scope avoids multi-file partial-upload ambiguity.
If the version already exists, inspect its public filename and digest; never
overwrite it or use a blanket skip-existing option to ignore a conflict.
Retain a failed or partial state and use a newly authorized version for changed
bytes. Reverting this PR before upload changes no registry state.

## Advancing to the stable product version

Use `cargo xtask bump-version 0.11.0` to advance the native workspace and editor
development metadata together. A producer-version change also requires a narrow
refresh of the `unchanged-after-attempt` before/after snapshots through
`normalize_unchanged_repo_exposure_producer_fixture`, followed by fresh
`ripr agent verify` and `ripr agent receipt` dependent bindings. Preserve the
version, input identity and content commitments; do not hide their drift or
blanket-rebless unrelated evidence. Rerun the focused corpus test.

The initial publisher deliberately admits prereleases only. A subsequent stable
PyPI release must explicitly extend that admission contract, obtain stable
publication authority, and build/qualify fresh exact stable bytes. An alpha wheel
must never be renamed into a stable release.

## Reuse by npm qualification

The Python qualifier also exposes a read-only reusable workflow for npm's
same-source native input. The caller gets build/pip/uv proof and a bounded
wheel evidence artifact; it receives no registry credentials or OIDC access.
The PyPI publisher continues to require a standalone manual-main run of
`python-wheel-qualification.yml`. An npm caller run is not PyPI publication
authority, even when it contains the same qualified wheel bytes.
