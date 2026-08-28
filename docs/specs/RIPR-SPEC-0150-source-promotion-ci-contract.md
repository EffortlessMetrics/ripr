# RIPR-SPEC-0150: Source-promotion CI contract

Status: proposed

## Problem

The exact-J verifier is useful only when the source PR invokes it against the
live PR head and makes the history-preserving merge method unambiguous. A valid
join somewhere in ancestry, a stale PR body, or a flattened equivalent tree
must not produce a promotion success.

## Behavior

The `Source Promotion Contract` workflow runs only for a pull request whose body
contains `<!-- source-promotion: true -->` and exactly one lowercase
`source-promotion-control` marker in the form
`<!-- source-promotion-control: <40 lowercase hex> -->`. It checks out
the exact PR head with full history, fetches the named control commit from the
fixed `EffortlessMetrics/ripr` source repository, and consumes only these fixed
paths from that immutable commit:

- `docs/release/source-promotion/contract-inputs.json`;
- `docs/release/source-promotion/preflight.json`; and
- `docs/release/source-promotion/resolution-manifest.json`.

The control manifest uses schema `ripr.source_promotion_ci_inputs.v2` and binds
exact `source_main`, `join_head`, fixed paths, and the SHA-256 digest of each
receipt. `join_head` must equal the live PR head and `source_main` must equal
the live PR base. The control commit must not be an ancestor or descendant of
J. On manual dispatch, `source_parent` must equal the sidecar's `source_main`
before the verifier is built. The control files are external to J's tree: the workflow
rejects candidate-provided paths and does not require a tracked input file in
the promotion PR. This avoids a circular fixed-point contract in which a file
containing J also changes `J^{tree}`.

The workflow's `validate_tracked_regular_file` helper performs the mode-100644
and canonical-path checks in the fixed source repository before any sidecar
bytes are copied into runner-temporary files. It is called for all three fixed
sidecars and must never be pointed at a candidate-checkout `INPUTS_PATH`.

The resolution schema is keyed by `kind:key`, so one workflow path may
legitimately have rows in several reviewed categories. For every workflow path
changed by the promotion PR, the authenticated immutable resolution manifest is
the sole import authority: at least one row must have that exact key and every
row with that key must have disposition `swarm_blob` or `integrated`. Missing
rows, any `source_blob`, mixed source/non-source authority, and unknown or other
dispositions fail closed. Duplicate rows for one `kind:key` remain invalid
under the resolution verifier. The workflow must not substitute a hardcoded
workflow-name exception for reviewed resolution authority.

The workflow runs `cargo xtask source-promotion verify` and emits one
normalized `ripr.source_promotion_contract.v2` receipt plus the verifier
JSON/Markdown receipts. PR and post-merge receipt directories live under the
runner-owned temporary directory rather than the candidate checkout. Every
receipt is bound to the PR head, control commit, and input digests and is
uploaded under a SHA-containing artifact name. The normalized PR receipt also
preserves the trusted `source_parent` identity emitted by input validation.

After the PR receipt upload, an always-run terminal enforcement step reads the
normalized contract and succeeds only when its schema is
`ripr.source_promotion_contract.v2`, its status is `verified`, validation is
`passed`, the verifier receipt is `present`, and the verifier exit code is zero.
Rejected evidence is therefore retained before the hosted job fails; a missing,
malformed, rejected, candidate-supplied, or non-zero-verifier receipt cannot
produce a green `Source Promotion Contract` check.

Before invoking the verifier, the workflow resolves the declared `swarm_ref`
with `git ls-remote --refs` against the fixed public
`EffortlessMetrics/ripr-swarm` remote and requires exactly one result whose SHA
equals `SWARM_PARENT`. It also reads fixed public ruleset `20661783` and
requires target `tag`, active enforcement, the singleton
`refs/tags/ripr-release-*` pattern, and both active `update` and `deletion`
rules. These checks are external provenance, not receipt-controlled
self-consistency.

The workflow summary repeats the ordered parent graph, candidate/source SHAs,
merge base, tree, receipt digests, preflight conflict/survivor/version state,
and the only supported copy-safe command:

```bash
gh pr merge <PR> --repo EffortlessMetrics/ripr \
  --merge \
  --match-head-commit <JOIN_SHA>
```

The summary explicitly requires Create a merge commit and forbids squash and
rebase. The command is printed only; the workflow never executes it. A manual
workflow-dispatch lane reruns the verifier with `--main-head` and requires the
exact J object to remain reachable from merged source `main`.

### Resolved-tree preconstruction gate

Before an authoritative direct-J object or promotion ref exists,
`cargo xtask source-promotion validate-resolved-tree` consumes one exact source
parent, frozen W7 parent, reviewed tree, preflight file/digest, and resolution
manifest file/digest. The running checkout must equal the source parent; the
receipt records the running executable digest without claiming that a digest
alone proves build provenance. The command materializes only an unreferenced
disposable object/worktree, runs the fixed source-owned governance catalog in
order, retains byte-bounded logs, terminates timed-out process trees, and
restores the caller repository's ref/worktree state.

The canonical receipt schema is
`ripr.source_promotion_resolved_tree_validation.v1`. Every required command has
one of `passed`, `failed`, `not_run`, or `unavailable`, with an explicit reason
when it did not pass. Canonical JSON/Markdown omit observed wall-clock duration
and record the fixed timeout bound instead. Any non-pass state, exact-identity
mismatch, malformed or moved sidecar, source-checker mismatch, replacement-ref
object substitution, dirty checkout, observed ref movement, changed worktree
registry, or cleanup residue rejects construction eligibility.

The validator checks final-tree policy with source-owned commands. It does not
invent parent-comparative semantic resolutions: the reviewed manifest must bind
the separately reviewed #1557/#1572 integrated-policy receipts, and #1478 owns
the complete disposition set and reviewed tree.

### Source-owned admission, construction, and candidate-ref controller

The recovered #1609 controller extends `cargo xtask source-promotion` with four
typed subcommands:

- `write-trusted-builder-receipt` binds the exact source/workflow SHA, clean
  checkout, pinned Rust toolchain, `Cargo.lock`, isolated locked build, and the
  executable that is currently running;
- `admit-resolved-tree` requires the exact validated-tree packet, trusted
  builder packet, preflight and resolution bytes, and typed command-catalog and
  network-policy integration receipts; the integration-index bytes must match
  the caller-bound lowercase `--integration-index-sha256` before parsing and
  during the final identity snapshot;
- `construct-exact-join` consumes one admitted packet and one terminal tree
  qualification, rechecks the same caller-bound integration-index digest, and
  requires the qualification's exact bytes to match a caller-bound lowercase
  SHA-256 before the single allowed unreferenced `git commit-tree` attempt; and
- `publish-candidate-ref` requires the construction-bound target ref, exact
  old-or-absent local and remote state, and an exact expected-state
  `--force-with-lease` before publishing the candidate ref.

Integration receipts are not accepted as schema-only self-assertions. Each is
bound to `producer_source_sha == SOURCE_PARENT` and to the executable digest in
the trusted-builder packet. Admission and construction final snapshots reread
and digest every indexed packet member and every typed integration receipt;
unchanged index bytes do not make changed member bytes current. The
tree-qualification denominator is exactly this
ordered set, with every lane terminal `passed` and carrying a lowercase
64-character evidence SHA-256:

1. `editor_package_linux`;
2. `editor_package_windows`;
3. `rust_product`;
4. `source_governance`;
5. `source_survivors`;
6. `trusted_product_journeys`;
7. `untrusted_workspace_contract`; and
8. `w7_product`.

The #1609 local controller establishes exact content consistency, not producer
provenance or reviewer acceptance. Producer identity fields and digests are not
signatures. #1610 must transport those exact bytes from fixed producer
repository, immutable commit/ref, regular-file path, and digest authority;
Issue #1478 owns reviewer acceptance of integration evidence and #1507 owns the
qualification producer. No locally accepted receipt grants merge, publication,
or release authority. Construction compares the qualification receipt against
the caller-bound digest before parsing and again on its final input reread; a
digest mismatch rejects before `commit-tree`.

Missing, extra, reordered, renamed, non-passed, or evidence-free lanes reject
construction eligibility. Qualification also binds the exact admission packet
and receipt, resolved-tree validation receipt, and admitted network-policy
receipt.

Every controller receipt reports numeric `commit_tree_attempts`,
`local_ref_attempts`, `remote_push_attempts`, and `merge_command_attempts`.
Rejected admission and every preconstruction rejection report zero forbidden
attempts. A constructed receipt reports exactly one commit-tree attempt and
zero ref, push, and merge-command attempts. Publication never constructs a
commit and always reports `merge_command_attempts: 0`.

The requested control-packet output must be exclusively reserved before
`commit-tree`, local-ref, or remote-push work becomes reachable. The reservation
syncs a deterministic `control-attempt.json` before side effects; completed
packets retain it and publish `packet-index.json` last. Its reconciliation
context binds the protected refs, expected ref state, and maximum operation
counters together with the admitted or constructed identity and input packet
digests. A reserved directory
without the index is an incomplete attempt whose Git and remote state must be
reconciled before retry. A pre-existing output path or an unsafe/non-directory
output parent fails closed before those attempts. Output equal to or beneath
the worktree or common Git administration directory, a consumed packet, or an
indexed-receipt sidecar directory also fails before creating any output path;
the controller never corrupts an input or overwrites an earlier receipt to
make a later mutation look successful. The same protection applies while
emitting malformed-command rejection packets, and containment comparison
resolves filesystem aliases. The contract covers process-visible
interruption and synced file contents; it does not claim portable power-loss
durability for directory entries.

Immediately before the sole `commit-tree` attempt, construction performs a
complete live snapshot of protected refs, tree and sidecar digests, every
indexed packet member, every typed integration receipt, and terminal
qualification bytes. Any unreadable or changed value rejects with zero
commit-tree attempts.

Construction and publication require the exact protected source ref
`refs/heads/main`; a caller-selected alias cannot satisfy source-main
authority. Candidate-ref publication binds both fetch and push authority to
`https://github.com/EffortlessMetrics/ripr.git`. Before local mutation, again
after creating the local candidate ref, and after the push attempt, it rereads
the local and remote source parent, local and remote protected W7 ref, complete
indexed construction-packet bytes and inventory, exact join object, and source
remote fetch/push URLs. A stale or mismatched pre-push reread attempts to roll
the local candidate ref back to its exact old-or-absent state without pushing.
A failed or non-publishing remote attempt also attempts to roll back only that
local candidate ref behind an exact-state guard, and the receipt records
whether the rollback succeeded.
An unavailable final remote observation immediately rolls back only the local
candidate ref behind an exact-state guard, then records every mandatory
post-push authority reread before returning `publication_state_unknown`;
remote state remains unknown and is never rolled back.

Publication status follows observed state rather than process exit alone:

Publication receipts expose `push_process_succeeded` and
`target_ref_updated` separately: an exit-zero no-op sets the former true and
the latter false. An exit-zero push whose porcelain output is malformed or
otherwise cannot attribute the exact target update records true and null,
respectively, and cannot produce `published`. `atomic_push` and
`expected_state_guard_passed` record the
guarded push operation independently from the later status classification:
both are true when that operation is attributed as an actual target update,
null after an attempted push without such attribution, and false when no push
was attempted. A later divergent remote observation can therefore make the
status `rejected` without rewriting an already attributed operation to false.

- `published` means the guarded push's machine-readable status reported an
  actual update of the exact target ref, the remote target was reread at the
  exact constructed join, and all post-push authority rereads remained valid;
- `published_but_invalidated` means the remote target was observed at the exact
  join but a bound source, W7, packet, object, or URL authority invalidated
  during publication, or the post-push local candidate-ref observation was
  unavailable; and
- `publication_state_unknown` means the final remote state could not be
  observed, or it equals the join without an actual target-update attribution;
  an exit-zero up-to-date/no-op push is not publication attribution.

`rejected`, `published_but_invalidated`, and `publication_state_unknown` grant
no integration or release authority. No #1609 controller packet emits a merge
command: `merge_command` remains null and `merge_command_attempts` remains zero
for every outcome. Issue #1508 owns the later authoritative promotion candidate
and merge-command decision after accepted-tree and qualification evidence
exist.

### Trusted source-promotion admission workflow

The permanent `Source Promotion Admission` workflow transports the recovered
controller from committed source authority. It supports `workflow_call` and
exactly bound `workflow_dispatch` execution with `contents: read` permission.
It does not run on `pull_request_target`, accept a caller-selected runner or
command, use a privileged environment, or receive write, identity-token,
attestation, package, release, marketplace, registry, signing, or
branch-administration authority.

Every live input names a full identity rather than a success assertion:

- source repository, source parent, workflow/controller commit, and
  `trusted_checker_identity` in the closed form
  `source-owned-xtask@<workflow/controller commit>`;
- swarm repository, protected W7 ref, and peeled W7 commit;
- reviewed tree and an exact source-repository carrier commit whose tree and
  ordered parents are the reviewed tree, source parent, and W7 parent;
- producer repository, immutable commit/ref, mode-100644 path, and lowercase
  SHA-256 for the preflight, resolution manifest, resolved-tree validation
  packet, integration index, and any qualification receipt consumed by
  constructor dry-run; and
- one closed mode, `admit_only` or `constructor_dry_run`.

Mutable branch names, abbreviated SHAs, artifact names without their producer
locator, caller-supplied booleans, and candidate-checkout paths carry no
authority. The live profile rejects synthetic fixture selection. The bounded
`positive_synthetic` and `j5_negative` profiles select source-owned fixtures;
their `fixture_identity` is emitted by the production harness and cannot be
chosen as an arbitrary command or path.

The live harness fetches the exact carrier commit into its runner-owned clone
and validates the commit header before invoking the controller. Possession of
either parent alone is insufficient to materialize or admit a combined
reviewed tree.
The exact-J-free synthetic profiles use the closed `not_required` carrier
sentinel; they test admission and mutation guards without claiming that a live
carrier was transported.
The uploaded normalized packet MUST recursively index its controller receipts
and complete materialized locator closure. The finalizer MUST verify and
consume that downloaded closure, and MUST reject missing, extra, corrupt,
symlinked, or controller-summary-inconsistent members before construction.
Final constructor success MUST include a valid indexed construction receipt.
Constructor rejection MUST preserve every available partial output byte in the
indexed closure, MUST remain self-verifying when the receipt is unavailable or
malformed, and MUST invent no evidence when no output directory exists.

The checker identity must name the same full commit as `workflow_source_sha`.
It identifies committed source ownership; it is not a substitute for the
trusted-builder packet. The workflow produces that packet internally from the
exact source/workflow/checker SHA, pinned toolchain, `Cargo.lock`, isolated
external target directory, and running executable digest.

All checkouts, bare repositories, retained logs, and packet outputs live under
runner-owned temporary roots. The workflow runs `source-promotion
run-admission-workflow`, retains its producer exit status, finalizes the
immutable admission packet, and uploads every available member with
`if: always()`. It then downloads that artifact into a fresh runner-owned path,
independently verifies the indexed bytes and exact requested identities, and
enforces terminal admission before constructor dry-run becomes reachable.
Missing or malformed packet files are terminal red, not an excuse to omit
available evidence.

After admission enforcement, `source-promotion finalize-admission-workflow`
performs admit-only normalization or the optional guarded constructor phase and
produces the final normalized `workflow-disposition.json`,
`workflow-disposition.md`, and `packet-index.json`; its available final packet
also uploads with `if: always()`. `source-promotion
verify-admission-workflow` checks the packet,
and `source-promotion enforce-admission-workflow --expected-status admitted`
is the sole terminal green predicate. The normalized disposition schema is
`ripr.source_promotion_admission_workflow.v1`, the packet schema is
`ripr.source_promotion_admission_workflow_packet.v1`, and the only statuses are
`admitted` and `rejected`. `not_run`, `unavailable`, unsupported schema or
profile, non-zero producer exit, missing/malformed evidence, stale identity,
failed final reread, or disagreement between the packet and workflow conclusion
is terminal red.

The `admit_only` mode requires zero constructor, local-ref, remote-push,
merge-command, and release/publication attempts. `constructor_dry_run` is
unreachable until admission is terminal and may create at most one
unreferenced object in an isolated synthetic repository. It still requires
zero local-ref, remote-push, merge-command, and release/publication attempts.
The workflow never invokes `publish-candidate-ref` and cannot create J6, move a
promotion ref, or grant product/editor qualification, merge, or release
authority.

The candidate PR can prove this static workflow and production-harness
contract only. Trusted hosted behavior requires a later `workflow_dispatch`
from the committed default-branch workflow with the exact merged source SHA,
workflow blob SHA, controller/schema versions, fixture identities, and dispatch
inputs. Candidate-branch execution is not a substitute for that post-merge
control.

## Required Evidence

- unrelated PRs skip without a success claim;
- a stale or substituted `join_head`, control commit, source parent, receipt digest, body
  command SHA, or fixed receipt path fails closed;
- missing, abbreviated, uppercase, unreachable, wrong-repository, symlinked,
  directory, and placeholder control inputs fail closed;
- a dispatch whose trusted `source_parent` differs from the immutable sidecar's
  `source_main` fails before the verifier is built;
- one or more `swarm_blob`/`integrated` rows for distinct resolution kinds are
  accepted when every row for the workflow key authorizes non-source movement;
- missing, source-only, or mixed allowed/source dispositions for one workflow
  key are rejected;
- duplicate rows for the same `kind:key` remain rejected by the resolution
  verifier;
- a valid two-parent join passes the verifier;
- a single-parent or equivalent-tree flattened history fails post-merge;
- uploaded receipts retain exact heads, ordered parents, input digests, checks,
  failure reasons, and claim boundaries;
- the retained verified `ripr.source_promotion_verification.v2` shape passes the
  balanced normalizer predicate while malformed shapes fail closed;
- a rejected normalized contract is uploaded and then fails the hosted job,
  while a verified, passed, present, zero-exit contract passes terminal
  enforcement;
- candidate-checkout files cannot substitute for runner-owned verifier or
  normalized-contract receipts.
- The uploaded post-merge contract receipt uses schema
  `ripr.source_promotion_post_merge_contract.v1` and retains the exact
  `control_commit` alongside J, source, trusted-parent, and merged-main SHAs.
- the verifier is built from the trusted source-parent SHA in an isolated target
  and invoked against the candidate checkout; the candidate cannot supply it;
- malformed, abbreviated, uppercase, duplicate, missing, moved, symlinked, or
  digest-mismatched resolved-tree inputs fail closed;
- a J5-shaped Git-tracked tree with the source-only network ledger fails with
  three missing live rows and one orphan, while the semantic reconciliation
  passes and duplicate, under-counted, raw-union orphan, and removal controls
  fail;
- rejected and validated resolved-tree JSON/Markdown fixtures are byte-stable,
  every command state is explicit, and observed elapsed time cannot perturb the
  canonical bytes;
- replacement refs, source-checker mismatch, wrong object kinds, ref movement,
  and worktree-residue path aliases are exercised by production-helper tests.
- all four controller subcommands reject malformed, missing, stale, moved, or
  producer-mismatched evidence before the forbidden attempt counter advances;
- the exact eight-lane qualification denominator accepts only the ordered
  complete set and rejects missing, extra, renamed, reordered, failed, or
  evidence-free lanes;
- output-path collisions reject before commit-tree, local-ref, or remote-push
  attempts, while injected finalization failures retain a deterministic
  incomplete attempt journal and no complete packet index;
- target-ref mismatch and wrong expected local or remote state reject before
  publication, while a remote non-publication or unavailable final target
  observation restores the exact prior local candidate-ref state;
- before/after controls cover local and remote source, local and remote W7,
  complete indexed construction-packet bytes, the exact join object, and source
  fetch/push URLs;
- source-main aliases reject, and publication receipts distinguish `published`,
  `published_but_invalidated`, `publication_state_unknown`, and `rejected` from
  guarded-push attribution plus observed remote and authority state; every
  controller outcome retains a null merge command with zero merge-command
  attempts.
- the admission workflow exposes only `workflow_call` and `workflow_dispatch`,
  uses read-only permissions, and cannot select a privileged event, arbitrary
  command, runner, ref writer, publication path, or caller-controlled success;
- source, W7, reviewed-tree, checker, and sidecar identities are exact and
  movement of any bound repository, commit/ref, mode-100644 path, digest,
  controller, or workflow identity invalidates the packet;
- every available admission receipt and bounded log uploads before terminal
  enforcement, while missing, malformed, unsupported, rejected, `not_run`,
  `unavailable`, non-zero, stale, or contradictory evidence remains red;
- `positive_synthetic` reaches terminal admission through the production
  harness, while `j5_negative` retains a complete rejected packet with all
  constructor/ref/push/merge/publication attempt counters at zero;
- `constructor_dry_run` is unreachable before admission and creates at most one
  unreferenced object in the isolated fixture repository without publishing or
  moving a ref; and
- pre-merge exact-head proof establishes the workflow contract only; the next
  post-merge control owns trusted default-branch dispatch evidence.

## Non-Goals

## Acceptance Examples

- A marked promotion PR with one exact source-control marker, canonical
  mode-100644 sidecar files, matching digests, and a numeric,
  repository-bound `gh pr merge` command passes the PR-head verifier lane.
- A changed workflow with one reviewed `swarm_blob` row is eligible for import.
  If the same workflow also appears in another reviewed category, that second
  row may be `integrated` or `swarm_blob`; any `source_blob` row makes the
  workflow authority mixed and rejects the promotion.
- Missing, duplicate, abbreviated, uppercase, unreachable, wrong-repository,
  symlinked, directory, or placeholder sidecar inputs, candidate-tree input
  paths, and flattened ancestry are rejected.
- Duplicate or mixed merge strategies are rejected, and the sole command must use `--merge`.
- An unrelated PR skips the promotion lane honestly, while manual `--main-head` verification proves the post-merge ancestry contract.

No automatic merge, publication, release, tag, signing, secret use, settings,
branch-protection mutation, ref mutation, or product-correctness claim.

## Test Mapping

Executable proof lives in
`xtask/src/command.rs::tests::source_promotion_workflow_is_exact_head_and_read_only`,
`source_promotion_workflow_rejects_symlink_and_path_escape_inputs`,
`source_promotion_workflow_rejects_placeholder_and_wrong_repo_commands`,
`source_promotion_workflow_disables_checkout_credentials_before_code`,
`source_promotion_workflow_binds_trusted_source_parent`,
`source_promotion_workflow_refutes_crlf_rewrite_thread`, and the integration
contract tests in `xtask/tests/source_promotion_workflow_contract.rs`, including
missing/source-only, mixed allowed/source, one-or-more unanimously allowed
workflow-disposition cases, the balanced verifier-receipt predicate, and
runner-owned upload-before-enforcement ordering. The exact-J graph, `kind:key`
completeness, duplicate
`kind:key`, and flattened-history controls remain covered by the verifier tests
mapped in `.ripr/traceability.toml`.

The permanent admission transport is covered by the same workflow-contract
integration target and by production-harness tests in
`xtask/src/reports/source_promotion_admission_workflow.rs`. Those tests pin the
closed profile/mode surface, exact locator and digest validation, deterministic
positive and J5-shaped packets across absolute roots, upload-before-enforcement
ordering, normalized conclusion agreement, constructor reachability, and zero
publication authority.

## Implementation Mapping

- Workflow: `.github/workflows/source-promotion-contract.yml`
- Trusted admission workflow:
  `.github/workflows/source-promotion-admission.yml`
- Admission workflow harness:
  `xtask/src/reports/source_promotion_admission_workflow.rs`
- Operator contract: `docs/SOURCE_PROMOTION.md`
- Exact verifier: `xtask/src/reports/source_promotion_verify.rs`
- Resolved-tree validator: `xtask/src/reports/source_promotion_validate_resolved_tree.rs`
- Admission/construction/publication controller:
  `xtask/src/reports/source_promotion_control.rs`
- Controller hostile and isolated Git fixtures:
  `xtask/src/reports/source_promotion_control/tests.rs` and
  `xtask/tests/fixtures/source_promotion_control/`
- J5 final-tree corpus: `xtask/tests/source_promotion_resolved_tree.rs`
- Byte-stable receipts: `fixtures/source_promotion_resolved_tree/expected/`
- External control sidecar: the fixed source-repository commit named by the
  PR-body `source-promotion-control` marker; its files never enter J.

## Metrics

The workflow receipt denominator is the number of promotion-specific runs; a
skipped unrelated PR is not a passing promotion result. GitHub check state and
receipt status remain distinct evidence axes.
