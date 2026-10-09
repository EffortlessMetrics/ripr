# Inactive complete-execution resource experiment

Issue #1627 owns this source experiment. It is not complete-history qualification.

The ordinary installed `ripr pr-evidence` and `cargo xtask ripr-pr` paths keep
their existing configuration/discovery, changed-line and partial-scope guards.
No limit override, narrow base, partitioned analyzer run, or inherited proof
is introduced.

## Experimental invocation and containment

`cargo xtask ripr-pr --experimental-complete-execution --base <literal-base>
--head <literal-head>` resolves the pinned RIPR binary using the existing build
authority. The new experiment rejects RIPR_BIN overrides; the ordinary
compatibility path retains its existing override behavior. Subject authority and any prior experimental receipt are revoked
before resource discovery, build, limiter setup or spawn.

On Linux, the launcher invokes `/usr/bin/prlimit` with equal finite soft/hard
RLIMIT_AS and RLIMIT_FSIZE ceilings. The address-space maximum is 2 GiB;
the file-size maximum is 256 MiB. Each effective ceiling is the minimum of
that experimental maximum and both finite inherited ceilings. No inherited
ceiling is raised. Core files are disabled for this owned subprocess.

The private installed worker entry verifies bounded `/proc/self/limits`
before repository/config/Git reads, analysis, JSON conversion or serialization.
The real installed producer then runs in-process, with its existing full
check conversion, bounded canonical index/projection and packet serialization
inside the limited worker. Existing Git deadlines remain in effect.

The launcher reuses the existing owned subprocess group/deadline/capture
authority. Stdout and stderr are each capped at 64 KiB; incomplete or
over-limit byte capture refuses. Producer progress stdout is diagnostic only.
One designated receipt file is read with a 16 KiB limit and exact-document,
typed parsing; unknown/duplicate fields and extra documents refuse.
The parent stream-verifies the fixed six artifact digests with 64 KiB scratch,
individual file bounds, a finite total bound and a cooperative regular-file
I/O deadline. Symlinks and special files are rejected before open under the
single-writer artifact scope; blocking filesystem reads are not preempted.
A hard parent-I/O bound remains an activation prerequisite. The checks refuse
post-worker body mutation, duplicate artifact keys and stale identities.
The normal configured producer timeout remains in effect.

RLIMIT_AS is an address-space ceiling per process, not RSS or a process-tree
aggregate. Children inherit limits. A fixed, bounded helper inventory and
the aggregate parent/descendant/disk envelope still need qualification.
RLIMIT_FSIZE bounds individual files; it is not a total-disk quota.
No runner permission, security policy, toolchain or protected-check change is
part of this experiment.

## Publication and failure semantics

Every successfully published experimental packet, subject and review input carries
`experimental_complete_execution`. The shared production rejection authority
refuses any presence of that field, including null or a claimed complete
generation. Installed saved-check, xtask saved-check and both review subject
and review-input admission use that authority. Production activation is
hard disabled. There is no admissible unmarked legacy subject from this route.

The compact receipt binds this invocation's nonce, resource profile, exact
base/head/head tree, check/diff/configuration identities, index counts/bytes,
and a fixed set of artifact digests. It carries `coverage: not_established`
and `production_admission: false`. It does not embed the full canonical index.
A successful worker exit plus receipt records only a completed under-cap
ordinary producer experiment. The public experimental launcher deliberately
returns failure so it cannot satisfy a gate.

Worker setup/analysis/serialization failure revokes authority. Existing error
packets remain unmarked status=error artifacts, rejected by the same authority,
and never gain an experimental completion receipt. A failed or
absent native status, timeout, incomplete capture, missing/malformed/stale
receipt also revokes authority in the launcher. Allocation abort or signal
may prevent a child error packet; non-success plus absent completion evidence
is sufficient for refusal. Only the worker may emit experimental completion;
the parent cannot synthesize success.

## Required proof before a complete route

Native controls exercise under-cap real production, configuration/context
semantics, experimental refusal by saved/check/review consumers, missing
limiter revocation and ordinary recovery. A separate owned native child must
start under a verified finite limit and observe an allocation refusal; setup
failure alone is not memory-exhaustion proof.

This first inactive slice does not implement exhaustive raw obligations,
immutable whole-head context, a resumable extractor/classifier or another
accepted complete generation. Those contracts must be reconciled with the
single-read/raw-parser owner in #1832/#1835 before activation. No new input
partition or alternate parser is permitted.

Activation additionally needs failure counterexamples, frozen-context
equivalence, above-5,400 and single-7,038-owner controls, and actual literal
full-carrier producer/check/review/check qualification with measured resources
and V1 4,096-entry/2 MiB fit. Report measured failure before broad paging.
Child or under-cap controls are not full-carrier proof.
