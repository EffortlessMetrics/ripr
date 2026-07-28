# RIPR-SPEC-0135: Assurance State Vocabulary

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked ADRs:

- [ADR 0021](../adr/0021-static-and-executed-assurance-axes.md)

Linked issues:

- [#1978](https://github.com/EffortlessMetrics/ripr-swarm/issues/1978) - define
  the assurance vocabulary before adding command execution.
- [#1979](https://github.com/EffortlessMetrics/ripr-swarm/issues/1979) - future
  explicit command execution and provenance-bound result slice.
- [#1754](https://github.com/EffortlessMetrics/ripr-swarm/issues/1754) - typed
  command-spec domain contract and additive migration scaffold.
- [#1755](https://github.com/EffortlessMetrics/ripr-swarm/issues/1755) - populate
  typed specs for canonical verification and receipt routes.
- [#1941](https://github.com/EffortlessMetrics/ripr-swarm/issues/1941) - repair
  verification and receipt trust boundary.

Support-tier impact:

- See: [support tiers](../status/SUPPORT_TIERS.md).
- No support-tier promotion. This contract separates static movement, command
  execution, receipt issuance, and external runtime mutation confirmation.
- Existing CLI, LSP, report, and receipt surfaces remain advisory and static
  unless a later implementation slice emits the corresponding typed state.

## Problem

RIPR already compares static before/after artifacts and records caller-supplied
verification command text, but the words `verify` and `receipt` can be read as
proof that a command ran. A static movement comparison and an executed test
command answer different questions. A receipt must preserve both answers
without collapsing them into one Boolean or promoting runtime mutation claims.

## Behavior

`RepairAssuranceV1` is a schema-versioned, producer-owned envelope with
independent axes:

```text
static movement
  static_movement_evaluated
  → improved | unchanged | regressed | not_comparable

verification command
  verification_command_available
  → verification_executed_pass
  → verification_executed_fail
  → verification_not_run | verification_unavailable
  → verification_cancelled_or_timed_out

receipt
  receipt_issued | receipt_rejected | receipt_not_requested

runtime mutation
  runtime_mutation_confirmation_external
```

The axes are independent. Static improvement does not imply a command ran. A
passing command does not imply static movement. A receipt can be issued for a
validated static comparison without a command result, but it must disclose
that its assurance is static-only. A runtime mutation result is always
external to RIPR's ordinary static contract.

### Static movement

`static_movement_evaluated` is emitted only after the producer validates a
comparable before/after artifact pair under
[RIPR-SPEC-0134](RIPR-SPEC-0134-repair-artifact-provenance.md). Its result is
one of `improved`, `unchanged`, `regressed`, or `not_comparable`. The record
preserves the canonical gap/seam identity, artifact identities, repository
root, revision/currentness, and analysis input identity.

The static result is not a test execution result and must not be rendered as
`verified`, `killed`, `survived`, `adequate`, or `proven`.

### Command specification and execution

`CommandSpecV1` is a typed description, not permission to execute. It records:

- program and ordered arguments;
- root-relative working directory;
- timeout and cancellation policy;
- network and environment policy;
- expected result parser;
- cost class; and
- bounded human display text.

Only the explicit `ripr agent verify-execute` operation may turn
`verification_command_available` into an execution state. No packet, receipt
request, JSON field, or editor action automatically runs a command. The
operation requires `--authorize` and accepts only one exact producer-owned
direct `ripr agent verify` route from a schema-validated packet. Shell-required,
manual, inherited-environment, network-enabled, and write-producing routes are
rejected by policy.

`VerificationExecutionResultV1` records the declared command identity, root and
revision before/after the run, process disposition, exit status, bounded
stdout/stderr commitments, an immutable command-spec digest, and currentness.
The accepted process dispositions are `completed`, `failed_to_start`,
`cancelled`, `timed_out`, and `output_limit_exceeded`. A completed result must
carry an exit status; every other disposition must omit it. `current` and
`dirty_worktree` require the before and after HEADs to be equal.
`verification_executed_pass` means only that the declared command completed
with the producer-defined passing disposition. It does not mean that the
static gap closed or that the repository is correct. A runner must reject a
result whose command-spec digest, root, or revision does not match the declared
execution context. The typed domain validator performs those checks, while
the explicit `agent verify-execute` runner performs bounded direct execution,
reduces the child environment to a disclosed platform floor, captures stdout and
stderr separately with limits, and rechecks HEAD and worktree currentness. The
runner emits process evidence only; it does not compare static movement or issue
a receipt.

A clean environment means no ambient application or credential variable reaches
the child. It does not mean an empty environment block: the verify route invokes
`git`, so stripping `PATH` would make a passing observation unreachable on every
real repository and would report a false negative instead. The floor is fixed in
code and disclosed by name — never by value — in the emitted preflight.

The typed `command_specs.verify` array must equal exactly what the same
derivation applied to the packet's own `verification_commands` yields, and the
headline `verify_command` must resolve to that route. Exactly one reproduced
route must be executable; zero or several are refused rather than resolved by
preference.

That check establishes display/typed agreement only — the command a reviewer
reads is the command that runs. It is not provenance, because every compared
field is caller-supplied and a coherent rewrite of all of them would pass it.

Producer ownership is established by a second, independent layer. Both `--before`
and `--after` must pass the landed repo-exposure provenance contract: canonical
shape, `ripr` producer identity, the exact producer command, a repository root
equal to the selected root, a full-SHA HEAD, and a recomputed content
commitment; their base revisions must agree. The canonical verify route is then
**recomputed** over those validated artifacts, and the packet's route must equal
the recomputation. The packet selects which validated producer artifacts to
compare; it never authors the command. A coherent whole-packet rewrite naming
non-producer files is refused before any process starts.

The child also inherits `HOME`, so host global Git configuration can influence
the child's Git behavior. A clean environment here means no ambient credential
or application variables — not behavioural independence from host Git
configuration. That is a declared limitation, not a claim.

Descendant containment is a declared limitation, not a claim. The workspace
forbids `unsafe_code` and the dependency policy admits no process-group or
job-object substrate, so only the owned child is terminated. The authority
boundary is what makes this sufficient: the sole executable route is one leaf
`ripr agent verify` invocation. Widening the executable route set without first
adding a containment substrate would break this reasoning.

### Receipt issuance

`receipt_issued` requires validated static movement and a coherent assurance
record. A static-only receipt uses `verification_not_run` or
`verification_unavailable` and carries the explicit non-claim
`static_only_assurance`. `receipt_rejected` is fail-closed for malformed,
fabricated, mismatched, stale, or otherwise unverifiable inputs.

`receipt_not_requested` is an explicit absence of a receipt decision; it is
not a passing state.

### Runtime mutation boundary

`runtime_mutation_confirmation_external` is a classification of externally
provided runtime mutation evidence. RIPR may record that boundary in a future
consumer contract, but does not produce `killed` or `survived` from this
vocabulary and does not import those terms into normal static output.

## Compatibility and migration

The current `ripr agent verify` command remains a compatibility surface for
static artifact movement and must document its static-only meaning. The current
`agent receipt` output remains advisory until the later receipt implementation
binds all consumed artifacts, repository state, configuration, command
execution, and observed result.

Future output may add an assurance envelope alongside existing fields. Existing
static fields remain readable; consumers must use the typed assurance states
when present and must not infer execution from `verify_command`,
`commands_run`, `verify_result`, or receipt presence alone.

## Required fixture coverage

`fixtures/assurance_vocabulary/assurance/corpus.json` pins positive and
negative controls for:

- static improvement with no command run;
- unchanged static movement with a passing command;
- improved static movement with a failed command;
- unavailable verification;
- cancellation or timeout;
- a typed timeout result carrying its command and provenance commitments;
- malformed or unsupported `CommandSpecV1`;
- wrong repository root;
- fabricated result input; and
- externally supplied runtime mutation classification.

The corpus is contract evidence only. It does not execute commands, invoke a
provider, run mutation testing, or promote any state to a gate.

## Acceptance Examples

### Static-only improvement

An improved comparable static pair with `verification_not_run` may produce a
`receipt_issued` record only when its non-claims include static-only assurance.
It must not render `verified` or a runtime mutation outcome.

### Execution disagreement

An improved static pair with `verification_executed_fail` preserves both
states. Neither state overwrites the other, and the receipt cannot call the
repair runtime-verified.

### Invalid currentness

A wrong-root, stale, malformed, fabricated, or timed-out input is rejected or
disclosed as unavailable. It cannot become a passing current receipt.

## Required Evidence

- `schemas/ripr/repair-assurance.schema.json` closes the V1 envelope,
  `CommandSpecV1`, and `VerificationExecutionResultV1` vocabulary.
- `fixtures/assurance_vocabulary/assurance/corpus.json` contains positive and
  negative controls for every required assurance boundary.
- The regular `assurance_vocabulary` analyzer fixture proves this design-only
  corpus does not alter the existing no-finding fixture path.
- The spec, ADR, slice, indexes, traceability, and policy ledger remain
  consistent.

## Non-Goals

- executing arbitrary, shell-mediated, or caller-replaced commands;
- changing the current `agent verify` or `agent receipt` producer;
- adding signatures, remote attestation, or a policy gate;
- claiming test adequacy, correctness, merge approval, or runtime mutation;
- adding a second analyzer or provider/orchestrator authority; and
- promoting any language support tier.

## Claim boundary

The assurance vocabulary and typed verification-result validation boundary are
durable. The explicit `agent verify-execute` route makes one bounded,
producer-owned process observation available. It does not make receipt
provenance, gate blocking, runtime mutation confirmation, or proof authority
available.

## Test Mapping

- `fixtures/assurance_vocabulary/SPEC.md` describes the fixture-level
  non-claims and static-only control.
- `fixtures/assurance_vocabulary/assurance/corpus.json` is the adversarial
  assurance corpus consumed by the future schema/runner implementation.
- The existing `cargo xtask fixtures assurance_vocabulary` path proves the
  regular fixture remains a zero-finding control.

## Implementation Mapping

- `schemas/ripr/repair-assurance.schema.json` owns the design-only machine
  contract.
- `crates/ripr/src/domain/command_spec.rs` owns the additive typed
  `CommandSpec` domain shape and fail-closed field validation.
- `crates/ripr/src/agent/command_specs.rs` owns canonical verify/receipt route
  population. Legacy display strings remain unchanged; verify output
  redirection is `shell_required`, while receipt `--out` is direct argv.
- `crates/ripr/src/lsp/gap_artifacts.rs` validates projected typed specs and
  rejects malformed or role-mismatched route payloads before editor use.
- `crates/ripr/src/domain/verification_result.rs` owns
  `VerificationExecutionResultV1`, command-spec digest binding, and exact
  root/HEAD/currentness validation.
- `crates/ripr/src/app/verification_execution.rs` owns the explicit direct
  process boundary, packet/spec equality check, bounded output capture,
  currentness recheck, and atomic result write. It never issues a receipt.
- `crates/ripr/src/cli/agent.rs` and `crates/ripr/src/cli/commands/agent.rs`
  expose `agent verify-execute` as an explicit opt-in adapter.
- `.allow/spec-system/slices/command-spec-route-population.v1.toml` records the
  PR-local claim boundary and return conditions for #1755.
- `.allow/spec-system/slices/typed-command-spec.v1.toml` records the PR-local
  claim boundary and return conditions for #1754.
- `docs/OUTPUT_SCHEMA.md` owns compatibility wording for current static
  `agent verify` and `agent receipt` surfaces.
- `.allow/spec-system/slices/verification-command-execution.v1.toml` records
  the PR-local claim boundary and return conditions for #2332.

## Metrics

- `repair_assurance_vocabulary_closed`;
- `repair_assurance_static_only_disclosure`;
- `verification_execution_result_provenance_validated`.

## Proof

```text
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-doc-index
cargo xtask check-doc-artifacts
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo test -p ripr domain::verification_result --lib
cargo test -p ripr verification_execution -- --test-threads=1
cargo xtask check-command-catalog
cargo xtask check-process-policy
cargo xtask check-network-policy
git diff --check
```
