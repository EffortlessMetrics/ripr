# #4510 prerequisite — verified shared extension plan

Implementation in progress; successor compilation/runtime NOT_RUN. The owning baseline below passed; it does not admit an installed candidate. Fresh all-state4510 body search returned[]. Reuse existing release.rs package/install authority, candidate_registry and OwnedProcess, not a new candidate selector/controller.

## Inherited seams and gaps (before the selected first PR)

release.rs363 PackageInstallResult currently contains success/artifacts/details strings. run_packaged_install369 derives target/package/ripr-version.crate and installed binary, packages/validates traversal and link-free archive, extracts outside checkout, builds a workspace control binary, installs exact extracted crate, checks archive/workspace/installed SHA256 and reported version, then external doctor. It does not return typed candidate admission/archive inventory/executable identity, and derives ambient current checkout rather than requiring an accepted immutable candidate subject. It uses fixed install/report paths and best-effort removal, so cannot represent cleanup failures per reusable row. Refactor this owner rather than scrape details strings or duplicate install.

candidate_registry.rs owns deny_unknown_fields CandidateIdentity169 (sha/tree/ref), ValidatedRegistry309, CandidateGrant315 and resolve_candidate_authority324: actual supplied bytes rawdigest -> registered release lifecycle row -> allowed operation. Current grant exposes registered_path/state/candidate_sha, so extend same validated authority internally to expose required complete candidate identity and row/manifest digests. No arbitrary newcandidate DTO may become admitted from suppliedstrings alone; package attribution needs exact sourceSHA/tree plus admitted registry bytes. No current eligiblepin => admission refuses, without choosing or freezing one.

xtask/run.rs uses ripr::process_owner::OwnedProcess. capture_output_with_timeout475 already owns timeout/termination; capture_bytes_in_dir_with_timeout550 and capture_stdout_to_file_with_timeout607 provide narrower output/cwd forms. Extend this boundary for declared cwd/env and bounded stdout/stderr/rawbyte digests plus explicit termination/reap/cleanup states if needed. Do not create a second process runner. OwnedProcess Windows assign-before-execution Job Object containment already exists. Harness telemetry reuses this owner.

## Tests-first discriminator receipt (f72, not successor proof)

At `f72fda9dd301b11a8e0bfa05d99aad901f0817cf`, owning xtask compile passed.
The actual separate Git source/controller fixture passed (1/0/0). Complete
qualification arguments failed at unknown `--controller-root`; duplicate
`--version` was incorrectly accepted: two intended behavioral RED controls,
each 0/1/0 and native 101. Unsafe-input rejection passed through the existing
unknown-flag path (preservation only). Successor source/package/install behavior
is not proved by these tests-first results.

Raw receipt: `target/ripr/reports/4510-tests-first/f72-tests-first-native/results.json`,
SHA256 `5A9B06E1A44B74EC38527CED55070423B4EE47528CAEE96263A866AA28C8D30F`.
Terminal scoped liveness found no owned PIDs/shared-target images; its receipt
SHA256 is `E4E6728F208553C25DF7167448734F624F425941486F38C70CB304BBC7E4A406`.
The shared target lease was explicitly released before source implementation.

## Selected first PR: actual admitted package subject for existing corpus

The first slice is one private source-to-archive-to-installed-subject custody
spine consumed by the existing negative-corpus authentic baseline. It does not
complete the later reusable-row harness or pronounce #2769 qualified. Preserve
#3924's historical accepted lifecycle-integration defer; successor #2769's
[execution graph](https://github.com/EffortlessMetrics/ripr-swarm/issues/2769#issuecomment-5882045830)
requires explicit candidate admission before this harness.

Extend existing `release-negative-corpus --version` with an all-or-none input
group: `--controller-root`, `--candidate-source-root`, `--candidate-artifact`.
The exact controller-relative artifact bytes pass the existing registry
resolver; neither a filename nor a newest-file heuristic grants authority.
Reject partial, blank, duplicate and unsafe inputs before cleanup or spawning.
No group preserves explicitly unqualified legacy smoke. An admission error in
qualification mode never falls back to legacy. Do not gate the fifteen ambient
`release-readiness` checks or create a new command/readiness aggregate.

Controller root owns actual release policy, registry and registered artifact
bytes. Candidate source root owns actual Git HEAD/tree/ref, manifests/lock and
package inputs. The source commit precedes the control packet; controller HEAD
need not equal the pin; candidate identity is checked independently. Separate
worktrees of one repository are valid; canonical physical roots must neither
be equal nor contain one another. Retain
and independently revalidate both domains. Source-promotion's different-repo
common-directory condition is not this contract. No production pin/ref or
checkout selection is created by this implementation.

Extend `candidate_registry`'s existing validated grant with complete identity
and raw-byte custody, keeping constructors private. A private release admission
module uses the existing bounded explicit-CWD process owner for actual Git.
Refactor the existing package owner only as needed to return a private installed
subject, retaining its legacy wrapper. All source builds, archive/install paths
and product execution use explicit owned roots. Do not reconstruct trusted
handles from supplied digest strings, details text, matching versions or unequal
workspace/executable hashes. No new public crate or dependency.

`release_negative::resolve_candidate` passes the admitted absolute installed
binary to existing `build_baseline` and `produce_authentic_chain_in_fixture`.
That genuine consumer creates concrete before/after commits and runs installed
RIPR analysis, verify and receipt. Retain source/archive/executable custody and
mode in the same corpus report. The existing external installed doctor is
another actual observation. Later cases consume the same subject; no synthetic
passed row or second qualification verdict.

### Archive attribution

Cargo's [package documentation](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
explicitly says VCS metadata does not verify provenance. Check parsed
`.cargo_vcs_info.json` SHA, `dirty=false` and package-relative `path_in_vcs`, then
map every ordinary packaged entry to exact committed source bytes. Retain paths,
sizes, SHA256 and committed blob identities. Reject duplicates, traversal,
links, unsupported types and unexpected or ignored/untracked included files,
even when tracked Git status is clean. Initially refuse source symlink
flattening rather than admit content from outside the selected source.

Generated files have explicit rules: `Cargo.toml.orig` matches committed package
manifest; VCS metadata is checked above; normalized `Cargo.toml` and packaged
`Cargo.lock` are retained outputs of the successful bounded Cargo invocation in
fresh exclusively owned output. Parse/check package/version/bin and lock
identities. Their attribution is producer custody and retained actual bytes,
not a claim to independently reimplement every Cargo normalization rule. Unknown
generated/copied entries or unsupported normalization fail visibly. The archive
handle is constructed only by successful production plus validation, never from
an arbitrary caller archive/hash. Revalidate archive bytes/inventory before
extraction/install and executable bytes before product consumers.

Boundary checks remain unlocked: this is observed local producer custody, not
atomic checkout exclusion, hostile-host protection or authenticated build
attestation. Retain that limitation rather than strengthening hashes into
provenance claims.

### Tests first and acceptance

- Parser controls cover legacy, complete qualification group and malformed,
  partial, duplicate, blank and unsafe inputs.
- A real temporary source repository commits the candidate first and creates a
  fully qualified ref. A separate controller fixture then registers its actual
  SHA/tree/ref through the real evaluator/resolver. Controller HEAD differs.
  Independently mutate both root domains and require refusal/revalidation; the
  restored unchanged positive remains usable.
- Decoy source/foreign package/install paths remain byte-identical on admission
  refusal, with no producer invocation. Ignored included foreign input rejects.
- A dependency-free real Cargo fixture named `ripr` exercises actual package,
  extraction, install and absolute execution with a unique source token. Wrong
  same-version source/archive/executable, metadata/inventory and post-production
  substitutions reject. This proves machinery only; never fabricate doctor or
  baseline success from that fixture.
- Actual RIPR selected through a legal fixture registry must be genuinely
  packaged/installed, run external doctor and produce the existing authentic
  before/after/verify/receipt baseline with nonzero observations before delivery
  is claimed. The full corpus remains a separately reported actual denominator.

Observe behavioral RED before production; compilation/host failure is not RED.
After an explicit native lease run focused registry/package/corpus controls,
actual tiny producer, real RIPR baseline and relevant characterizations, then
fmt, committed check-fast/precommit and independent selector parity. Preserve
raw native exits, deadlines, owned process cleanup and failures. No eligible
live pin is an honest refusal, not candidate selection or a release verdict.

Rollback the coherent invocation/custody slice to restore legacy entry behavior;
preserve receipts before owned fixture cleanup. Later #4510 rows, budgets,
cleanup taxonomy, downstream packets, blind acceptance and #2769 aggregation
remain out of scope. The source candidate and controls are committed; successor
compilation and actual execution of the new controls remain NOT_RUN.

The qualified authentic baseline also takes its fixture bytes from the admitted
source's retained committed blobs through the opaque installed candidate. It
does not read the launch directory's fixture. Qualified fixture Git operations
use the existing bounded process owner, including movement and restoration in
negative cases; the legacy wrappers retain their previous process mode.
Package and install pass explicit higher-precedence owned Cargo temporary
configuration so a source workspace's forced TEMP cannot redirect builds.
The external authentic fixture owns its existing Cargo target and temporary
configuration. These corrections and the legal wrong-tree registry control are
source changes; compilation and actual end-to-end execution remain NOT_RUN.

At ce023f419b6694d4771ae06942a93155e694a5c8, owning test compilation
passed and the three argument controls plus actual separate Git registry
fixture each passed with one test executed and none ignored. The source
custody control then failed with native status 101: valid `[package]` document
bytes were rejected by the TOML value parser. This is an implementation defect,
not a timeout or fixture instrumentation failure. Topology and package/install
controls were NOT_RUN. Raw evidence remains under
`target/ripr/reports/4510-tests-first/ce023-controls-native`.
The correction uses document deserialization for the root and package
manifests, normalized archive manifest and packaged lock, preserving UTF-8 and
malformed-document error contexts. Successor execution remains NOT_RUN.

At 1046f9adcf048647720b4bea40ebaf66d8db30ec, owning compilation
and the source-custody and physical-root controls passed. The tiny Cargo
package control then failed with native status 101 before producing an archive:
its temporary source inherited the surrounding RIPR workspace. This is a
fixture/setup defect, not an observed archive or install custody failure.
The tiny source now explicitly declares its standalone workspace, following
the existing nested-fixture convention. Production source attribution and
negative oracles are unchanged; the updated fixture controls remain NOT_RUN.
Direct-test temporary fixtures and the real machinery controller are prepared
under exclusive owned sibling proof directories outside Cargo workspace
ancestors. Compilation retains the pinned own checkout Cargo TEMP configuration.
The legal controller outside an ancestor Cargo workspace is the intended proof
class. A controller inside another Rust workspace is NOT_ESTABLISHED: Cargo
may reject the extracted install input through ancestor workspace discovery.
Keep that real topology as an explicit follow-up/refusal control before broader
qualification; do not transform attributed production manifests or claim that
the external positive proves all controller topologies.

## Later shared-harness scope (not the first PR)

Private release submodule xtask/src/reports/release/candidate_harness.rs plus tests.rs, called from release.rs existing install path; candidate_registry existing grant accessor only as needed; run.rs narrow captured typed observation extension only if absent; release-server sha256_file and existing path/container helpers reused. One private CandidateProofPacket DTO/render projection exported through existing reports module for4505-4508. No new release-readiness command/qualification aggregate. No public crate/deps.

AdmittedSubject can only be constructed by existing validated registry + exact candidate source/repository/ref SHA/tree/currentness and accepted manifest/raw digests. InstalledCandidate can only be constructed by successful package/install checks retaining archive path,size,digest,validated inventory, source subject, installed absolute path,size,digest/version and workspace-control exclusion. Opaque strings remain inputs until these constructors validate them. Child corpus receives an installed handle, owned fixture/launch context and generic typed row API; cannot override admission or process cleanup.

Rows: exact argv/cwd/envpolicy/input/output/artifactdigests; generic states from4510; selected/executed/skipped/failed counts; pre/post filesystem expectedwrite inventory; cleanup retained resources. Render JSON/Markdown from same evaluated DTO, semanticdigest excludes expresslytelemetry PID/time/temp-root spelling only. Recheck registry bytes/ref/source/install digest before each row. Generic harness does not invent child semantic verdict.

## Proof-first matrix

Accept unchanged registry-authorized subject+package+absoluteinstalledbinary, nonzero rows, bounded child and successfulcleanup. Reject independentwrongSHA/tree/movedref/manifest/package/binary(sameversion),PATHdecoy, workspacebinary, foreignCWDdecoywrites, staleartifactinput, zero subjects/execution, alteredinventory/output, invalidfutureDTO. Timeout/cancel and cleanupfailure remain nongreen and preserve resources. Equivalentportable roots same semanticidentity; changedcandidate/inputdifferent. JSON/Markdown parity. Tests invoke owner API/validatedregistry rather than preconstruct admittedhandles. Existing install tests/extractionnegative controls characterize owner before refactor. Native proof uses the recorded baseline below, then focused candidate_harness/install/registry/run controls and required guards; new implementation proof remains NOT_RUN.

## #4603 reviewer authority and meaningful independent subclaim

Existing rust_judged_panel/release_judgments.rs binds a CLOSED fixed metrics/rust-judged-behavior-panel/release-judgments.json to frozen release-selection digest, roles/evidence and #3806 authority. It proves structural judgment packet consistency and checked-in source provenance; it is not a generic accepted reviewer registry and cannot be repurposed as blind prompt acceptance. eval_sweep_report accepted receipt/current pointer similarly owns only its named Python sweep. No observed reusable blind semantic reviewer acceptance authority.

A meaningful4603preliminary subclaim is closed public-input/event/intervention decoding, digest/order/visibility/privacy validation and deterministic invalid/not_run/honestlimitation projections. It may never emit passed_blind_journey until shared4510 admission AND independently accepted exactprompt/answerkey judgment exist. This is partial4603 acceptance, notfullcompletion. Separate future reviewedprompt verdict should be repository-carried retained exactbytes artifact, with reviewed change provenance; validator validates bindings/closedstatus and does not claim cryptographic reviewer authenticity. Rootmustchoose acceptance gate/location beforepositive production path. No forgeableapprovedBoolean or testonlytrust context inproduction.

## Owning baseline
Immutable source basis: 3911480a34c6ce2224932f8494576636c73e1732, tree 34485dd11f02b27ac91b8ff0a85a463f5d6e4108. The plan-only baseline executed at d1d22bd6ed7448b910bbebcf28ce51f288da65f7, tree cec1a3d1ada9dcb28f75aeabc4763da80fb9d51a, against retained selector base 53b7059f0cb2608d5a85a52fd26302b751fece24.

Pass: owning xtask all-target checking and test compilation; all six characterization controls, each with 1 passed, 0 failed and 0 ignored; all 14 check-fast gates with the complete 670-file selector and no skips. Doctor exited 0 with a warning to preserve or clean retained report artifacts at closeout. Precommit was NOT_RUN by this baseline driver. The timeout and pipe-inheriting descendant controls execute actual owned children; package/version and registry fixtures retain the narrower meanings described below. No package installation, source admission, current release pin, or installed qualification was established.

Retained receipt: target/ripr/reports/4510-d1d-baseline-native-v2/native-results.json, SHA256 AF38E6E94A6465342AB5A9157F94F29E15B4BF727D898E9A486AD1498C85AF37. Raw phase logs, exact command/commit contexts and bounded process receipts are alongside it. At 2026-09-29T09:24:47Z, all ten owned phase PIDs and the scoped shared-target executable observations were empty. Source remained clean. This evidence belongs to the baseline object, not a future implementation head.

## Baseline versus implementation oracles

The six selected existing tests characterize inherited behavior only. The package test supplies digest strings, version output and doctor JSON to private validators; it does not install a package or attribute an executable to a source producer. The archive test covers traversal components. Registry controls cover required identity fields and template refusal through the existing resolver, rather than live ref or package attribution. The two timeout controls execute real children, including the platform-specific descendant cleanup path. None constructs an accepted installed-candidate handle.

Before implementation acceptance, add controls through the real admission and row APIs:

- Bind a registry-authorized immutable source SHA/tree and retained manifest bytes to the actual package producer, validated archive inventory and actual installed executable. A matching version or unequal workspace/executable digest is insufficient.
- Independently reject changed or foreign manifest/package/executable bytes, moved source ref, wrong tree and ambient-PATH or workspace binary substitution before invoking a semantic row. Revalidate the subject before each row.
- Exercise selected-root, foreign-CWD and decoy-root writes, stale artifact consumption and portable input identity through owned fixture roots and before/after inventories.
- Observe bounded stdout/stderr, timeout, cancellation and cleanup failure through the existing process owner. Retained resources and missing termination/reap evidence remain non-green.
- Require nonzero selected/executed required subjects, explicit skipped/failed counts and deterministic JSON/Markdown parity. A packet with altered output identities or strengthened human wording must reject.

Tests must call validated constructors and the actual retained producer/consumer path. They must not create trusted handles from supplied digest strings, pre-mark rows passed, or use test-only acceptance as production authority. Observe intended behavioral rejection only after test compilation succeeds; compile or instrument failure is NOT_ESTABLISHED. The characterization baseline is recorded above; new admission controls remain NOT_RUN.

### Attributed installation below a controller workspace

The 530ce required hosted run failed in the real package/install custody control:
Cargo discovered the controller's ancestor workspace from the extracted package
and refused installation. This is an observed defect, not a completed topology
qualification. The test-first correction places a valid package plus workspace
manifest and library under the owned fixture ancestor; selected source remains
standalone and all archive, installed-executable, foreign-CWD, and mutation
custody controls remain intact.

The proposed byte-preserving repair places extraction beneath the owned
`target/package` boundary used by Cargo 1.95's workspace ancestor traversal.
Cargo 1.95 matches this repository's MSRV. Actual installation must discriminate
this layout from the current refusal before the layout is accepted. Do not
rewrite attributed package manifests, mutate the controller workspace, or add
global exclusions. Existing full inventory byte revalidation before/after
installation remains the equality owner. Native topology proof and current
report-control GREEN are not run yet; historical failures remain retained.

Primary evidence: [530ce failed Rust gates](https://github.com/EffortlessMetrics/ripr-swarm/actions/runs/36576232132/job/109432584947).
The layout rationale is Cargo 1.95's
[`find_root_iter` ancestor traversal](https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/workspace.rs#L2265-L2284),
which stops at `target/package`; this source rationale is not runtime proof.


## Direct live-head manifest reconciliation, 2026-10-02 (local review draft)

Basis: existing PR4915 head `b7b4b6da6e593045dadb9139c92c4c07339cef83`.
Native ownership is recorded on that PR; no parallel carrier, remote push,
candidate pin or qualification run was created. This local draft is not yet
rebased/merged with current main. Main's merged blind contract is SPEC-0200,
with 35 synthetic scenarios; neither this patch nor a selected agent run claims
all 35 actual journeys or a novice-human observation.

The source/package/install/process spine is retained. A complete qualification
argument group plus explicit `--candidate-manifest-sha256` selects direct
schema-1.1 #1609 admission; without it historical registry mode is unchanged.
No refusal falls back. The expected raw digest comes from the trusted release
operator/controller's independently reviewed #1609 handoff. It is not read
from the candidate document, calculated as implicit approval, or discovered
from an adjacent sidecar. It does not re-prove human audit judgments.

SPEC-0144, the existing template/projection and runbook propose one versioned
manifest with actual candidate tree/package/lock, accepted #2766/#2768/#3807 bytes,
exact pin readback/protection and required_not_run proof consumers. A minimal
owner-status envelope records accepted status, exact candidate SHA/tree,
reviewed packet digest and that owner's native decision reference. The root
must review this new envelope; the release operator verifies its native source
before accepting the manifest digest. Missing/unknown status remains
not_established even on a matching hash. No new audit engine is required.
SOURCE_PARENT stays null until the later source #1769 transaction.

Proof at this drafting boundary: Rust 1.99 rustfmt parse/format and git diff
checks only. The repository pin remains 1.95. No Cargo compile/test, package,
installation, policy aggregate, actual pin or blind execution ran. The
new pure-data and real temporary-Git controls are authored but NOT_RUN;
there is no claimed executable RED/GREEN pair. Current-main additive contracts,
applicable policies, native pinned CI and independent review remain required.
The separate thin4604 canonical stamp/assess packet consumer is still pending
and is not mixed into this authority patch before review.

### 2026-10-02 review repair before execution

The review's four findings are repaired locally: handle-based limit+1 reads
with observed regular-file snapshots; 64 proof inputs / 16 MiB each / 64 MiB
aggregate retained bytes; actual source Git range recomputation through the
existing source-promotion helper; and exact supported origin spellings. Root
approved the schema-1.1 serialization alignment: topo-order/reverse and
first-parent/reverse full SHA+LF bytes. Historical JSON hashes are different.
Record-set adjudication remains the reviewed #2768 packet's claim.

Additional authored controls cover actual read limits, aggregate/cardinality
refusals, regular-file requirements, a real merge topology with distinct range
counts, incorrect counts/order/digests, and supported/unsupported origin forms.
The retained boundary commit fixture hashes to the actual 45b56c object; its
synthetic descendants do not claim release acceptance. Local execution and
current-main reconciliation are still pending at this checkpoint.

### 2026-10-02 integrated execution handoff

Published #4915 history is retained. Local merge `5eec1e56d` integrated native
main `b5b75658`; its sole run.rs conflict preserved the stdin writer together
with main's shared deadline API. Subsequent review found and repaired a real
composition defect: an observed deadline kill was masked by the stdin writer's
BrokenPipe. The control failed with that exact classification error before
`9e1cc0d19`; afterward timeout evidence survives, while early-exit BrokenPipe
and unknown writer/drain failures still refuse.

Actual all-target xtask metadata passed on `5eec1e56d` and `a0e5a6a74` using the
task-local Rust 1.99 toolchain (82 s and 74 s, native repository pin unchanged).
The exact-source harness executed 40 passing controls on `9e1cc0d19`, including
large stdin/timeout/early-exit/cancellation and existing pipe-drain controls.
`a0e5a6a74` only strengthened the Git-range test: the changed real-Git control
passed again and production source bytes are unchanged. Harness source hashes,
commands, native exits, first setup failures, and the intended behavioral red
are retained with the local execution handoff. No setup failure is claimed as
a product behavioral red.

The harness includes real source/live-head custody, registry authority, shared
range recipe and owned-process dependencies. Archive/package orchestration is
not part of this bounded execution claim. The actual runbook jq filter passed
a synthetic shape check; 55 Bash blocks passed syntax checks and retained
registry hashes matched. Full independent review, applicable owning policies,
native pinned CI and final-candidate package/install/doctor/corpus proof remain
open. Selected controls do not establish the 35-scenario blind campaign or a
human journey. The separate #4604 consumer remains pending.


### 2026-10-02 source-owner review repair (execution pending)

Native COMMENT review 5395118632 identified replacement-object inconsistency
and missing source/output bounds on the preserved published #4915 head; both
also applied to this local adapter. Source metadata and cat-file batches now
share one no-replacement Git invocation. Metadata includes ordinary blob sizes
before capture, with independent source count/file/aggregate limits. The
existing owned process helper has optional stdout/stderr byte caps, and byte
drain expiry refuses in that budgeted mode. Existing uncapped callers keep
their timeout/reporting behavior; byte overflow can surface after the existing
child deadline rather than cancelling the child immediately. Checkout reads reuse the manifest's observed snapshot
reader with exact admitted-size caps; manifest and source budgets stay separate.

New authored controls exercise active replacement-object parity and concealed
same-size checkout substitution, exact/over-budget Git output, per-file/count/
aggregate source limits, wrong batch sizes, stdout/stderr overflow with cleanup,
and missing terminal drain output. This checkpoint records source/static work
only; execution awaits the shared compiler handback.

### 2026-10-02 source-owner execution handoff

The source repair is local at `9cf1dfefd0e218f52fef416adf021e7650c039c4`.
It addresses native #4915 discussions 4168448472 and 4168448479. The initial
45-control run had 44 passes and one stale expected-error string in the new
same-size substitution test; that oracle failure is retained as setup/proof
repair, not a behavioral red. The corrected exact-source batch then passed
45/45 controls on `add1682d5`.

Three isolated wrong-implementation variants failed for their intended
reasons: allowing replacement objects only for blob batches broke parity;
removing the aggregate retained-source check admitted an oversized aggregate;
and removing the actual output read cap consumed beyond limit+1. After all
variants were restored, the complete bounded batch passed. Mutations were
confined to the disposable proof harness, never the candidate worktree.

The actual engineering checkout at `9cf1dfefd` was then read through the same
bounded inventory and checkout verifier: 5,414 ordinary blobs / 58,586,484 bytes
passed in 1.31 seconds. This is source-budget workload proof, not an accepted
release candidate or package/install qualification. Its source capture mode is
explicitly `byte_budgeted_strict_terminal_drain`; both metadata and body reads
use `--no-replace-objects` and no truncated placeholder can be admitted.

Actual `cargo check -p xtask --all-targets --offline` passed on `e8db60425`
(78 s). Its single unused legacy stdin-wrapper warning was removed by scoping
that test-only wrapper to tests; the affected production check passed without
warnings on `9cf1dfefd` (46 s). The final exact-source workload compile also
covered that wrapper in test mode. Native repository pin remains 1.95; these
local checks used the authorized task-local 1.99 toolchain. Rustfmt/diff checks
passed and registered artifact bytes stayed intact. Compiler slot was returned
with the shared target/dependencies preserved.

Read-only caller audit retained existing Python replay `not_run`, Rust replay
`spawn_error`, and other uncapped consumers' established timeout reporting.
Only budgeted source capture opts into strict terminal-drain refusal. Byte
overflow drops the reader pipe immediately but can surface after the existing
child deadline/cleanup; it does not promise immediate cancellation. Independent
full review, owning policy/hosted checks and actual installed-candidate proof
remain separate publication/qualification gates.

### 2026-10-02 publication review and noncompiling inventories

The full carrier was reviewed through manifest/registry admission, actual Git
source custody, Cargo archive attribution, isolated installation, installed
execution, authentic fixture/corpus routing, report projections, shared process
ownership, tests, traceability and release-control documentation. Native #4915
remained draft at the preserved `b7b4b6d` head. Its two new source review findings
are addressed by the source-owner repair above; the older report/retention/
workspace-extraction findings remain addressed in that published history.

Sixteen affected noncompiling inventories ran against the current candidate
worktree using retained xtask controller `5b452d9f` (executable SHA256
`a93b4212f811d82760d5c5e63bcd807836e12ca988511919555a9ac43b8176d0`).
Static language, no-panic, allow-attributes, local context, covered-by, executable
files, spec format/numbering, traceability, doc artifacts/index, release targets,
network, output contracts and support tiers passed. Process policy initially
found the new test-only cancellation `Command::new` without a count entry. The
single owned test entry was added and the actual process inventory passed.
This controller/data run does not claim an exact-current-source xtask build.
The current file-policy gate invokes Cargo test enumeration, so it was not run
during this noncompiling batch. Native pinned CI remains required.

Current native main was read back as `66b67c62`; read-only merge-tree composition
was conflict-free. Published `b7b4b6d` and integrated `b5b75658` remain ancestors;
behind-only main movement does not justify rewriting or restacking this carrier.
The source/code/tests are unchanged from the selected proof above; this final
correction changes only the process-policy declaration and this record.

Candidate-review disposition is `REVIEW_INCOMPLETE`: no remaining blocking
source finding in the inspected scoped carrier, but current published-head CI,
full native package/install/doctor/corpus qualification and the actual final
accepted #1609 candidate remain unestablished. Archive/executable memory and
uncapped legacy output are not covered by the source-specific resource budget.
Owner acceptance envelopes bind independently reviewed packets; they do not
authenticate GitHub decisions or re-prove human audit judgments. The direct
adapter does not complete #1609, #4510's entire reusable packet contract, the
#4604 consumer, or any real 35-scenario/human blind campaign. Publication keeps
this same PR draft while those native review/proof dimensions are obtained.

### 2026-10-02 native review follow-up

Published engineering head `77fbba0e` has the exact local `3ad5bfdbf` tree;
Git-data transport preserved every published/integrated ancestor and mapped
only unpublished commits. Native routed run 37053575722, job 110992742436,
passed its policy inventories (including file policy) and then refused four
redundant unit expressions in the new process tests under strict Clippy. Those
expressions are removed without changing their assertions. A strict pass of
the bounded exact-source harness additionally found a byte-slice spelling in
the reused range helper; replacing `[b'\n']` with `b"\n"` preserves serialized
bytes and addresses that newer lint.

Review discussion 4169093089 correctly identified that qualified failure hints
still named the ambient legacy report path. The shared report writer now
returns the exact path it wrote, and both failure exits render that path. A
focused actual-file control covers a controller path with spaces/non-ASCII
and the legacy hint. The existing admission/report failure control also checks
the exact hint. Original write-failure causes remain retained.

Discussion 4169093099 raised the repeated full-custody cost. The existing
per-command pre/post boundary is retained; SPEC-0134 now discloses the linear
byte/file work multiplied by the command count. The one engineering census
is not promoted to a full-corpus benchmark, and reducing the boundary would
require separately proved invalidation behavior.

The exact-source harness now copies the repository Rust/Clippy lint tables and
has no blanket allow. Its first strict pass exposed 47 individual unused-item
spans from intentionally omitted driver/report/archive consumers. Every span
was source-audited and narrowly annotated only in the generated harness with
an explained `expect(dead_code)`; all owning bodies remain compiled. No new
lint suppression was added to the product. With those explicit harness-only
omissions, strict Clippy passed. Path-recovery red/green and the selected
runtime refresh are retained separately when terminal.

The follow-up selected runtime batch passed 46/46 controls. Replacing the hint
with the old legacy path failed the intended actual-report oracle; restoring
the exact helper passed. Strict harness Clippy was terminal with the exact
repository lint table and enumerated harness-only unused-item expectations.
A final review normalized the existing full admission test's expected
controller path too, so Windows canonical prefixes and macOS temporary-directory
aliases do not create a false mismatch. That last change affects only the
existing full integration test oracle; hosted execution of that test remains
pending. Both production failure exits consume the writer-returned path.

### 2026-10-02 native ruleset-oracle reconciliation

Published `a8c069e6` passed native repository preflight and strict workspace
Clippy. Routed run 37056622070 / job 111002939420 then reported 10,496 tests
passed, one failed, 444 not run after fail-fast, and two skipped. The failing
`release_pin_ruleset_requires_fully_qualified_tag_ref` still required the old
jq spelling and schema-1.0 `pin_recipe` template fields. Its positive synthetic
ruleset also omitted the now-required empty exclusions/bypass arrays.

The owning control now keeps its exact documented jq linkage and executes the
same fragment against full, short, mixed, branch and mismatched refs, plus
exclusion/bypass and missing-guard variants. The fixture names empty guards.
The schema-1.1 template is tested through actual admission with its correct
raw digest; it must still refuse before any candidate/source execution. The
contract body takes explicit fixture roots so it can run in the existing
bounded harness without compiling the monolithic test target. No production
admission rule or release protection was weakened.

All nine actual jq fragment cases passed locally. Removing the two guard
conditions caused the four guard variants to be accepted, the intended
behavioral counterexample. Native logs separately confirm current source,
manifest, process, qualified report and real tiny package/install controls
passed before the unrelated stale-oracle failure. Those passes are retained;
actual full RIPR/final-candidate and the skipped native tail remain gaps.
