# Source-promotion preflight

`cargo xtask source-promotion preflight` creates the repeatable, read-only
receipt consumed by the source release preflight. It preserves the complete
swarm history as the selected parent; it does not create the join.

This producer emits `ripr.source_promotion_preflight.v2`. The source verifier
at `EffortlessMetrics/ripr` main `82b2d7c262d229d5244263d458d10cd0189cb966`
accepts only v1 and rejects this receipt. Do not pass v2 to that verifier or
use a v1 fallback to omit acceptance. Coordinated consumption and validation of
the acceptance field remain a source-handoff prerequisite under
[ripr#1769](https://github.com/EffortlessMetrics/ripr/issues/1769).

The disposable merge probe requires Git 2.38 or newer because it uses
`git merge-tree --write-tree --name-only -z`. The command fails closed on an
older or malformed Git version rather than falling back to localized prose.

## Command

Run it from a clean operator checkout with both repositories available locally:

```bash
cargo xtask source-promotion preflight \
  --source-parent "$SOURCE_PARENT" \
  --swarm-parent "$SWARM_PARENT" \
  --swarm-ref "$SWARM_REF" \
  --source-repo ../ripr \
  --swarm-repo ../ripr-swarm \
  --source-main origin/main \
  --swarm-main origin/main \
  --version 0.11.0 \
  --controller-root "$PACKET_ROOT" \
  --candidate-manifest live-head-selection.json \
  --selection-decision "$SELECTION_DECISION" \
  --qualification-bundle qualification-bundle.json \
  --qualification-decision "$QUALIFICATION_DECISION" \
  --out target/ripr/source-promotion
```

The parent arguments must be complete 40-character commit IDs. The source
parent must equal the declared current source main. The swarm parent must be
reachable from the declared swarm main. Repository roots and Git common
directories must be distinct, and their `origin` URLs must canonically identify
`EffortlessMetrics/ripr` and `EffortlessMetrics/ripr-swarm`; suffix matches and
URL query/path tricks are rejected. Use `--source-remote`/`--swarm-remote` only
for an explicitly reviewed mirror.
The receipt records only the stable verification result for the Git common
directory comparison; it does not serialize an operator's local checkout path.
`SWARM_REF` is required, must use the fully qualified protected candidate tag
`refs/tags/ripr-release-<version>-<SWARM_PARENT>`, and must resolve in the
supplied swarm repository to the exact `SWARM_PARENT`; a moved, missing,
legacy-local-verifier, branch, short, wrongly named, or wrong ref fails closed.
The local verifier ref (`refs/ripr/release-<version>-<SWARM_PARENT>`) is a
separate release-transaction convenience and is not accepted as `SWARM_REF`.

The command writes deterministic `source-promotion-preflight.json` and `.md`
files. It records the merge base, separately named all-reachable and
first-parent counts for each parent range, exact-parent version surfaces
(workspace, crate, Cargo.lock ripr package, extension, npm lock root, and
changelog, with missing changelog evidence represented as unknown), and
SHA-256 digests. The
all-reachable digest recipe is:

```text
git rev-list --topo-order --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

The ordered first-parent digest recipe is:

```text
git rev-list --first-parent --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

It also inventories changed paths, source-survivor candidates, a
set-differenced list of paths changed only on the swarm side, non-dispositive
swarm-authority resolution candidates, and a real
`git merge-tree --write-tree --name-only -z` dry merge with machine-readable
conflict paths. The automatic `preview_tree` is
never a final join tree. An
optional `--resolved-tree <full-tree-sha>` records a separately reviewed
resolved tree after verifying that the object exists in one supplied
repository's common object store; omission remains visibly not finalized. The
dry merge runs in a disposable repository populated by fetching both exact
commits. No branch, index, ref, working tree, version, tag, PR, or publication
state in either authoritative checkout is changed.

## Receipt boundary

The receipt is invalid when either parent, declared main, repository identity,
immutable swarm ref or its resolved SHA, merge base, ancestry count, digest,
conflict list, or resolved tree changes.
Regenerate it if `main` moves before the transaction boundary. A clean dry
merge is not proof that semantic overlap is absent; every textual and semantic
resolution still needs review.

This command does not construct or prove the two-parent join, qualify the
candidate, change versions, authorize publication, publish artifacts, or
perform the source-to-swarm back-sync. Those are separate release boundaries.

## Native selection and complete qualification admission

The v2 receipt adds `acceptance`, consumed before any merge probe and observed
again before receipt output. All five inputs are mandatory:

```text
--controller-root <packet-directory>
--candidate-manifest <controller-relative-schema-1.1-json>
--selection-decision <native-ripr-swarm-1609-issuecomment-url>
--qualification-bundle <controller-relative-complete-bundle-json>
--qualification-decision <native-ripr-swarm-2769-issuecomment-url>
```

There is no historical/CI-only fallback in this command. The manifest's raw
SHA256 is obtained from the native #1609 acceptance, not calculated from an
unreviewed local sidecar. The existing direct-manifest corpus mode remains a
preparation/execution-custody route; its digest argument cannot issue a handoff.

The trusted operator records exactly one fenced `ripr-release-acceptance`
JSON block in each native #1609 and #2769 decision. Unknown JSON fields refuse.
CRLF is normalized only in the parsing copy; retained bodies and digests use
the exact raw response bytes.
The command does not create, accept or publish those decisions. The following
is the field contract, not an accepted packet or permission to mint one:

- `Subject`: `candidate_sha`, `candidate_tree`, `candidate_ref`,
  `manifest_sha256`. These match the admitted manifest and actual Git objects.
- #1609: `schema_version: 1`, `kind: ripr_native_selection_acceptance`,
  `status: accepted`, `subject`, `selected_claims` (the complete manifest
  #2766 packet/acceptance object), `selected_claims_decision_sha256` (SHA256
  of the exact native #2766 comment body UTF-8 bytes),
  `required_execution_owners`, `proof_inputs`, `required_qualification_rows`,
  and `excluded_subjects`.
- Each required row names `id` and `owner_issue`. IDs are unique ASCII
  letters/digits/period/underscore/hyphen, 1–128 bytes. The owner set equals the
  selected applicable execution-owner roster exactly. The native owners must
  include the complete broad #2769 matrix and every applicable successor row;
  a narrow successful workflow is not a complete denominator. No fixed template
  owner list is hardcoded. In the current successor, selected #4505–#4508 and
  #4604 obligations cannot be dropped without a new native selection decision.
- #2769: `schema_version: 1`, `kind: ripr_native_qualification_acceptance`,
  `status: qualified`, `subject`, `selection_decision` (exact native #1609 URL),
  `selection_decision_sha256` (exact body bytes),
  `qualification_bundle_sha256` (complete raw bundle bytes), and the same
  `required_qualification_rows`.
- Complete bundle: `schema_version: 1`,
  `kind: ripr_complete_qualification_bundle`, `status: qualified`, `subject`,
  `selection_decision`, `selection_decision_sha256`, `excluded_subjects`, and
  `rows`. Each row has `id`, `owner_issue`, `status: passed`, `selected`,
  `executed`, `failed`, `skipped`, and `packet` (the existing
  owner/path/raw-SHA256 evidence shape). Every required row appears once and
  every retained packet is read and digest checked. Counts refer only to
  selected subjects: `selected > 0`, `executed == selected`, `failed == 0`,
  `skipped == 0`. Required pending/failed/skipped/zero-subject rows refuse even
  when a native decision contains the bundle's matching digest.
- `excluded_subjects` is an explicit list, empty when none. Each entry has
  `id`, `owner_issue`, `count > 0`, `disposition` (`excluded` or `deferred`),
  and a nonblank `reason`. IDs cannot overlap required rows. The bundle must
  exactly repeat the native #1609 list. Thus 10,968 executed/selected subjects
  can coexist with two separately accepted configured exclusions; those two
  cannot silently become skipped required subjects.

The manifest stays immutable with `required_not_run`. Qualification is a later
accepted packet, never an edit that predeclares its freeze-time results green.
The two acceptance payloads and required/excluded lists are bounded to 256 rows.
Qualification bytes have a separate 64 MiB retained aggregate budget and 16 MiB
per-file limit, using the existing unlocked snapshot reader.

### Native read and trust boundary

The adapter uses the existing bounded owned-process runner for `gh api
--hostname github.com --method GET repos/EffortlessMetrics/ripr-swarm/issues/comments/ID`.
It fetches #1609, the referenced #2766 decision, and #2769; a URL alone is
insufficient. The response must match the exact comment ID, HTML URL and API
issue URL and report a nonblank author with GitHub association `OWNER`, `MEMBER`
or `COLLABORATOR`. A public unrelated commenter (`NONE`, etc.) cannot supply
admission. Each read is limited to 30 seconds, 1 MiB stdout and 64 KiB stderr,
with strict terminal drain through `OwnedProcess`. The GitHub host is fixed;
there is no custom endpoint, local-response flag, login or credential change.

This authenticates the observed GitHub location and association through the
operator's existing trusted gh/credential context. Association is not a
cryptographic signature, current permission re-verification, proof of human
semantic review, or a guarantee against later edits/revocation. The native
owners still judge applicability and the broad matrix's completeness. Exact
native bodies, actors and digests are retained in the receipt and reread after
preflight. The observations are not an atomic GitHub/filesystem transaction.
A network failure, moved body, stale manifest, tampered packet, wrong issuer,
wrong native issue, incomplete roster or nonterminal bundle refuses output.
