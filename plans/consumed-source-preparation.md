# Consumed Rust bytes at refresh preparation

Owner: [#1765](https://github.com/EffortlessMetrics/ripr-swarm/issues/1765).
This is a prerequisite for #1602/#4807/#3089, not their completed snapshot or
all-language input-identity contract. The design and observed regression are
recorded in [the implementation packet](https://github.com/EffortlessMetrics/ripr-swarm/issues/1765#issuecomment-5892466081).

## Observed defect

The actual saved-worktree producer completed two findings from A. With its
refresh generation unchanged, disk and buffer became B before preparation.
Preparation reread B, attributed the A findings to B, and cleared quarantine.
The focused test at `4c0962ead5d9280c1a0b1848c7e0500d6de6d580` compiled and
failed on that explicit attribution condition: one failed, zero passed or
ignored. Later signature and refusal checks did not execute on that RED.
Earlier setup, diagnostic compilation and Git admission failures are separate
retained evidence; they do not establish the product defect.

## Private connection

Record workspace-relative paths and SHA-256 of the raw buffers loaded by the
Rust diff/repo adapters before cache lookup. Identical observations are
idempotent; missing or conflicting observations cannot supply a digest.
Cached normalized source and diagnostic geometry cannot mint commitments.
Carry the observations through internal results, a private app wrapper and the
completed workspace snapshot. Preparation uses them for `.rs` documents;
absent, out-of-root or conflicting commitments quarantine instead of borrowing
disk or didSave state. Existing commit and cancellation ownership remain.

For an LSP refresh with a clean diff, snapshot admitted open `.rs` paths before
analysis. The Rust adapter intersects them with discovered, non-generated,
Git-tracked analyzable files and adds them to its index-only load set before
the existing index budget. The adapter reads those saved bytes and records
their commitments before cache lookup. Open paths do not seed changed-file
probes or findings. Foreign, symlink-escaped, untracked and unavailable paths
are not added solely because they are open. Existing mode-selected inputs can
still acquire commitments when the adapter actually loads them. An oversized
combined index retains the named
scope limit instead of silently omitting open files. The same cap bounds the
admitted candidate count before Git tracking probes, and returned Git paths
must match an admitted candidate.

Public `CheckOutput`, source-range DTOs and existing origin-wrapper signatures
remain unchanged. `AnalysisResult` already has private fields. Other extensions
retain inherited fallback, including non-`.rs` Rust-language buffers. Reads
outside the captured vectors do not establish complete include/dependency
identity. #1602/#4807 retain that broader work. Preserve the #4830
identity-law and #4844 RepairAttempt ownership.

Admitted file URIs are projected to relative keys by the existing LSP URI
containment owner. Open-file indexing and commitment lookup share that
projection: physical containment is checked first, admitted lexical keys are
preserved, and canonical-relative keys support a symlinked workspace root.
Roots or candidate paths containing parent segments use the canonical-relative
key, because lexical collapse of `symlink/..` can name a different location.
The carrier accepts only normalized relative keys and performs no filesystem
canonicalization. Outside-root and symlink-escaped documents cannot borrow a
captured commitment.

Snapshot diagnostic lookup keeps exact URI and existing wire-equivalence
precedence. Its additional root-scoped fallback accepts only one stored entry
with the same admitted relative key; ambiguity is refused. Distinct lexical
aliases inside the workspace are not collapsed. Global URI/action identity and
per-document quarantine are unchanged.
The immutable snapshot prepares the stored URI projection once before
publication, with a commit guard for unprepared or changed roots/keys. Request
lookup projects only the requested URI; it never recanonicalizes every stored
diagnostic on a miss. Root/key mismatches refuse the cached fallback until it
is rebuilt, including same-cardinality key replacements. Missing indexes do
not silently restore the filesystem-scanning path.

## Proof

The real Backend A-to-B regression must retain A attribution and quarantine B,
then a fresh real B analysis must carry B, clear quarantine and serve the
committed diagnostics. Actual saved producers cover cold misses/stores and warm
hits with plain, BOM, CRLF and invalid-UTF-8 raw inputs. Test-only telemetry is
populated from actual cache stats. Unit controls cover unavailable/conflicting
paths, quarantine recovery and unchanged other-extension behavior.
The framed clean-open saved-workspace route must remain non-quarantined, then
withdraw dirty content and recover after save. A clean open-file producer
control checks cold and warm capture without adding findings, rejects
untracked/foreign paths, and keeps captured A when disk and buffer become B.

Unix root-alias controls cover the combined containment/commitment boundary,
missing and conflicting input, outside and escaped paths, exact-URI precedence,
ambiguous diagnostic lookup, and distinct in-workspace lexical keys. A real
producer journey opens a canonical URI under a symlinked root, captures clean
saved A without seeding findings, refreshes actual B, serves B through the
canonical pull URI, then withholds it after a later unsaved edit. Restoring the
old lexical attribution or removing only the root-scoped lookup must fail the
corresponding currentness or delivery assertion.

Run the focused library namespaces `lsp::tests::consumed_source_tests::` and
`analysis::consumed_source::tests::` against the admitted freshly compiled
library executable. Candidate compilation/runtime and owning guards are not
established by source review or the historical RED. Exact receipts accompany
the candidate before publication.

## Boundary and rollback

No public schema, cache generation, portable identity, snapshot mint, scheduler
engine, geometry, discovery, ranking, MCP analysis service or release change.
Revert the private connection normally if necessary; retain the discriminating
tests and proof. Parent issues remain open.
