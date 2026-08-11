# Windows packaged qualification

`Windows Packaged Qualification` is an Actions-only rehearsal lane for the
exact-candidate acceptance in [#2769](https://github.com/EffortlessMetrics/ripr-swarm/issues/2769)
and [#2379](https://github.com/EffortlessMetrics/ripr-swarm/issues/2379). It is
separate from `Publish VSCode Extension` and the server release workflow.

## Invocation

Dispatch `.github/workflows/windows-packaged-qualification.yml` with both:

- `candidate_sha`: the full 40-character immutable candidate commit SHA;
- `candidate_ref`: the protected candidate tag/ref that must resolve to that
  SHA (for W5, use the ref recorded by the active release transaction).

The job checks out the SHA, rejects branch refs, and fails before packaging if
the ref does not resolve to the same commit. It uses Rust 1.95, Node 24, an
isolated Cargo home/target/prefix, and an extracted VSIX directory. The
installed CLI path and crate, binary, and VSIX SHA-256 digests are retained.

## Journeys and evidence

The Windows runner creates an independent two-commit fixture and runs the
isolated packaged binary through `version`, `help`, typed `doctor --json`,
`check`, `explain`, `context`, `pilot`, and `outcome`. It then packages the
candidate editor, extracts that exact VSIX, and runs the real trusted and
untrusted VS Code test hosts. The trusted suite covers default Rust, full
seam, TypeScript preview, restart, shutdown, and process cleanup; the
untrusted suite exercises the zero-work trust boundary. Any process whose
command line belongs to the lane and remains after a host exits fails the job.

The only retained transport is an Actions artifact containing the machine
receipt, command logs, artifact inventories, and SHA-256 file digests. This
workflow has `contents: read`, no secrets, and no release or marketplace API
calls.

## Claim boundary

A successful run establishes that the named candidate artifacts were built and
that these Windows packaged journeys emitted the retained evidence. It does
not qualify a release, establish product correctness or mutation/test
adequacy, authorize source integration, or authorize signing, tagging,
publication, or merge. Missing, skipped, stale, or differently headed evidence
is not a pass.
