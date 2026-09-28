# Repair attempt identity

`ripr agent repair` has three phases: `before`, `after`, and `verify`. The before phase prepares bounded evidence; a human or external coding agent makes one focused test edit; the after phase checks the exact prepared transaction and writes its static review receipt. For a trust-bound Python attempt, the separately authorized verify phase executes the retained packet's bounded verification route and records execution and static movement separately. The ordinary unbound before/edit/after path remains available; it does not authorize execution.

The durable object is a **repair attempt**, not a seam lookup and not the repository-global workflow directory.

## Ordinary sequence

```text
ripr agent repair --root . --seam-id <seam-id> --phase before
# make the focused test edit outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

The before phase prints the attempt manifest path and the exact `--attempt` command to run next. Preserve that command across agent sessions, process restarts, and concurrent work.

The repair packet is written to `target/ripr/workflow/agent-packet.json`. When stdout is piped or redirected, the before phase also prints that packet JSON on stdout, unchanged, for agents and scripts — the same one-JSON-document contract the after phase gives stdout. In a terminal it prints a short summary instead: the seam, the changed behavior, the missing discriminator, the one test file to edit, an assertion shape, and the packet path; the packet file remains the machine artifact either way.

`--seam-id <id> --phase after` remains a compatibility route. It succeeds only when exactly one awaiting attempt has that seam. Zero or multiple matches fail closed; RIPR does not guess which attempt is newest or intended.

## Governed Python sequence

Start with an accepted repair-trust selection manifest and its selected row ID
([RIPR-SPEC-0176](specs/RIPR-SPEC-0176-python-repair-trust-attempts.md)).
The selection row must match the current repository HEAD and the packet's exact
test-only target. This command consumes the selection; it does not create or
approve a cohort. The manifest must be inside the selected repository root;
relative paths resolve from that root. Keep its bytes unchanged throughout the
attempt. Finish verification before applying another trust-bound attempt in the
same repository: the compatibility apply record is repository-global, and a
later apply prevents verification of the earlier attempt.

```text
ripr agent repair --root . --seam-id <seam-id> --phase before --python-repair-trust-manifest <selection.json> --python-repair-trust-attempt <selection-attempt-id> --edit-authorized --edit-authority <operator-id>
# preserve the printed repair-attempt ID; make the focused test edit outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after --edit-authorized --edit-authority <operator-id>
ripr agent repair --root . --attempt <repair-attempt-id> --phase verify --verify-authorized --verify-authority <operator-id>
```

The selection-attempt ID identifies a manifest row; the repair-attempt ID is the
new durable transaction printed by `before`. They are not interchangeable.

- Pass both `--python-repair-trust-manifest` and
  `--python-repair-trust-attempt` on `before` only. Later phases consume and
  revalidate the retained binding, not a replacement manifest argument.
- Pass `--edit-authorized` and `--edit-authority` together on the trust-bound
  `before` and `after` phases. Use the same operator or agent identity.
- `verify` accepts only `--attempt`, never `--seam-id`. It requires an applied,
  trust-bound attempt and the separate `--verify-authorized` /
  `--verify-authority` pair, reaffirming the identity that authorized the edit.
  The edit flags cannot substitute for verification authorization.
- Keep HEAD, the applied patch, configuration, and retained artifacts unchanged
  between `after` and `verify`. Identity drift refuses execution; do not repair
  a stale attempt by rewriting its retained evidence.

### Verification, rollback, and receipt

The verify phase revalidates the retained identities before execution. It runs
only the packet's producer-owned typed verification route through bounded
execution controls, then reruns analysis and compares the intended native Python
behavior by identity. A missing canonical route records execution as
`unavailable`; it does not authorize a guessed command.

To request restoration of the applied test edit after the observations, append
`--verify-rollback` to the authorized verify command. This is optional and valid
only on `verify`. The rollback touches only the allowed changed edit surface,
not the workflow artifacts. It refuses a restore that cannot recover the
baseline bytes (including pre-attempt dirty content or a moved index), and
checks the pinned HEAD and remaining edit residue. Inspect the receipt's
`rollback` disposition: `proved`, `blocked`, or `not_run`. A request is not a
guarantee of restoration, and an error before the rollback step does not undo
the edit automatically.

The candidate receipt is written to
`target/ripr/workflow/python-repair-driver-verification.json`; the bounded
execution record is
`target/ripr/workflow/python-repair-driver-verification-execution.json`, and the
fresh analysis is
`target/ripr/workflow/after-verification.repo-exposure.json`. Preserve these
artifacts with the attempt's evidence. The receipt path is repository-global:
verification refuses to overwrite an existing receipt, even for another attempt.
Archive that evidence before explicitly removing the receipt for a new run.

**Execution and static movement are independent observations.** A passed command
does not imply improved static exposure, and improved exposure does not imply a
passed command. Inspect both axes and their limitations, not just the command's
exit status. The candidate receipt grants no lifecycle, acceptance, closure,
repair-correctness, support-tier, gate, badge, or promotion claim. Cohort review
and acceptance remain separate work.

## Durable location

Each before phase reserves an immutable directory:

```text
target/ripr/repair-attempts/<repair-attempt-id>/
├── attempt.json
├── before-commitment.sha256
└── artifacts/
    ├── workflow.json
    ├── commands.md
    ├── agent-brief.json
    ├── before.repo-exposure.json
    ├── agent-packet.json
    └── attempt-baseline.json
```

The exact filenames follow the command-owned source artifacts. `attempt.json` identifies them by semantic role and binds each retained file by path, byte count, and SHA-256 digest.

Repository-global files under `target/ripr/workflow/` remain compatibility projections for existing cockpit and review consumers. They are not repair-attempt identity.

## Manifest contract

The manifest schema is `schemas/ripr/repair-attempt.schema.json` (`schema_version: "0.1"`). A prepared manifest records:

- the closed-form `repair_attempt_id`;
- canonical repository root and concrete Git `HEAD`;
- producer version and selected seam ID;
- creation time;
- retained before artifacts and content commitments;
- the exact next command;
- limitations and explicit non-claims.

The before commitment is derived from the prepared manifest. Terminal updates may add after-phase evidence, but they cannot silently rewrite the retained before identity or artifacts.

When an after phase refuses after it selected the attempt (for example `agent verify` finds the pair incomparable or without movement, or the receipt is refused), the manifest gains an optional `last_after_refusal` object (`reason`, `repository_head`, `recorded_unix_ms`). The `reason` is the final error followed by the cause and recovery the after phase printed (the changed analysis inputs, or the rewritten history and its reset), bounded to 4096 bytes. It is an observation, not a state: it never changes `state` or `after`, the before commitment excludes it, and the next after phase that reaches the durable finish removes it. `ripr agent status` reports it instead of repeating the refused command unannotated. Manifests without a refusal omit the field.

## After-phase authority

`--attempt <id>` resolves one manifest directly. Before producing a receipt, RIPR verifies that:

1. the manifest path is bound to the selected attempt ID and repository root;
2. the manifest is still in `awaiting_edit`;
3. the retained before snapshot, packet, and edit-cage baseline still match their recorded byte counts and digests;
4. the after phase uses that attempt's retained packet rather than a repository-global or another attempt's packet;
5. repository `HEAD` is the prepared head, or a descendant reached only by commits on top of it (see [Committing between the phases](#committing-between-the-phases));
6. the observed edit delta, including every path those commits changed, is compliant with the retained packet's allowed, forbidden, and expected operational-write surfaces;
7. verify output, packet digest, delta digest, and receipt all bind to the same attempt.

A different attempt for the same seam is a different transaction. Its packet, snapshot, baseline, and terminal state cannot be substituted.

### Build output between the phases

Before preparing a Rust attempt, Git must ignore the entire `target/` directory
(for example `/target/` in `.gitignore`, or an equivalent local/global exclude
rule). The before phase checks the effective directory rule rather than counting
existing ignored files, so a fresh repository with no build output works too.
A missing rule, or a partial rule such as `target/debug/`, refuses before workflow
preparation or durable attempt publication and names the needed rule. RIPR never
adds or rewrites ignore rules itself. Add the rule, then rerun the before phase.

Run the project tests between the phases. For a Rust repair, the retained cage policy declares Cargo's default build directory `target/` as `ignored_build_output`. The Git-ignored contents of that directory are build output from `cargo test` or `cargo build`, so they are not treated as edits. The cage still observes these paths:

- tracked and untracked-but-not-ignored paths, including paths inside `target/`;
- the command-owned `target/ripr` writes;
- every other ignored path, such as an ignored `.env` or log file.

Static analysis never reads `target/` as source. A Cargo target directory in a non-default location inside the repository (`CARGO_TARGET_DIR` or `build.target-dir`) is not declared, so writes there remain violations. Python attempts keep observing every ignored path.

If effective ignore rules change after preparation, build-output paths can still
be refused by the after phase. Recovery names the declared build directory and
the ignore rule; it does not attribute those paths to redirected RIPR output.
The failed attempt remains terminal. Restore the rule and prepare a new attempt
while the gap exists, using the recovery sequence printed by the after phase.

### Cargo.lock between the phases

A library crate often does not commit `Cargo.lock`, so the first `cargo test` between the phases creates it. That lockfile does not stop the transaction:

- The analysis input identity (`input:v4`, [RIPR-SPEC-0134](specs/RIPR-SPEC-0134-repair-artifact-provenance.md)) counts only Cargo lockfiles that Git tracks. The static seam inventory never reads lockfile content, so an untracked or ignored lockfile cannot change the evidence the before and after snapshots compare.
- For a Rust repair, the retained cage policy declares the workspace-root `Cargo.lock` as `untracked_build_lockfile`. While Git tracks it neither at the before phase nor at the after phase, creating or rewriting it is build state, not an edit.

A tracked `Cargo.lock` is an analysis input and an edit. If it changes between the phases (for example `cargo update`), or a generated one is staged or committed, the after phase refuses before it finishes the attempt. The refusal names the changed inputs and the route: restore them (for example `git checkout <before-head> -- Cargo.lock`, or `git rm --cached Cargo.lock` for a lockfile that became tracked; when a commit made after the before phase changed them, run `git reset --soft <before-head>` first) and rerun the same `--attempt` command. The same applies to Cargo manifests and `ripr.toml`. When no input file changed, the refusal says so: the analyzer build or configuration differs, and a new attempt is needed.

Why tracked lockfiles still count: a committed lockfile belongs to the reviewed change, so a dependency change between the phases must not be attributed to the focused test. Why untracked ones do not: generating the lockfile yourself in the before phase (`cargo generate-lockfile`) would write your tree, and asking every newcomer to build before the before phase was the old workaround this rule replaces.

### Committing between the phases

Committing the focused test before the after phase is accepted. The after phase admits a `HEAD` that moved only forward, by commits on top of the prepared head. The edit cage evaluates the tree diff from the prepared head together with the worktree and index, so a committed production change is refused exactly like an uncommitted one, even when the worktree was restored afterwards. The receipt records both heads (`before_head`, `after_head`), and its tracked-surface check compares the tree with the prepared head, not with the new `HEAD`.

A `HEAD` that does not descend from the prepared head (after `git commit --amend`, a rebase, a reset, or a checkout) is refused before the attempt is finished, so the attempt keeps waiting for the edit. If only your own test commit was rewritten, `git reset --soft <before-head>` restores the prepared head and keeps the edit staged; then rerun the same `--attempt` command. Otherwise, prepare a new attempt at the current head.

Trust-bound Python attempts keep the exact-head rule: their selection pins the head, and any movement records `stale`.

`ripr agent status` applies the same rule through the attempt authority (`after_phase_head_admission`): an attempt whose test was committed on top of its prepared head stays resumable with its `--attempt` command, rewritten history is reported with the reset recovery above, and a finished attempt is current while `HEAD` is the head its after phase recorded.

Why this rule rather than always refusing: a developer who commits the test has made no edit outside the cage, and the cage can check the committed range with the same rules. Rewritten history cannot be attributed that way: the prepared head is no longer part of it, so the lineage check refuses it.

## Terminal state

The after phase records one of these states in `attempt.json`:

| State | Meaning |
| --- | --- |
| `ready_to_finish` | Current, comparable, and edit-cage compliant; receipt admission may proceed. |
| `stale` | Repository `HEAD` moved after the attempt was prepared to a commit the attempt does not admit (any movement for a trust-bound attempt; a non-descendant for an ordinary one, when the move happened after the lineage check). |
| `incomparable` | The retained and current evidence cannot support a valid comparison. |
| `failed` | The edit-cage or another terminal invariant failed. |

Only `ready_to_finish` with a current, compliant after verdict can authorize the attempt-bound receipt.

A terminal attempt's after phase does not run again. Rerunning it is refused with the state (`ready_to_finish`: already finished; `stale`, `incomparable`, `failed`: ended), the receipt path, and the next step: `ripr agent status`, or a new `--phase before` while the gap is still open.

`target/ripr/reports/agent-receipt.json` holds one receipt, so the next attempt's after phase replaces the previous attempt's receipt. `ripr agent status` then reports the earlier attempt's receipt as superseded by the later attempt (`receipt.superseded_by`) rather than as never issued.

## Compatibility outputs

The composed after command still writes the established projections:

```text
target/ripr/workflow/after.repo-exposure.json
target/ripr/workflow/analysis-outcome.json
target/ripr/workflow/agent-verify.json
target/ripr/reports/agent-receipt.json
target/ripr/workflow/            # status input
```

Those paths keep existing review and cockpit integrations working. Their evidence is admitted only after the exact attempt's retained before snapshot and packet have been resolved and validated.

The after phase's stdout is exactly one JSON document, like every other agent command: the versioned `repair_after_result` envelope (`schema_version` `0.1`), which carries the agent verify 0.3 document — every existing field, including its own top-level `status` — unchanged under `verify`, with the status report embedded beside it under `agent_status`, so a caller can parse stdout once and every stdout document's shape is identifiable from its `schema_version`. Narration stays on stderr. When the after phase refuses after the verify render (for example the receipt is not receipt-ready), stdout is the bare verify 0.3 document alone — a pure verify document, still one document, honestly labeled.

### Rerunning the receipt

`ripr agent receipt` can be rerun after the after phase, with or without `--out target/ripr/reports/agent-receipt.json`, and `ripr agent status` can be run in between. Each rerun recomputes the edit-cage delta and requires it, and the verdict it yields, to equal what the after phase bound. The receipt the after phase wrote, and any other file a later `ripr` command writes under `target/ripr`, appears only after that binding. A change is left out of the recomputation only when all three of these hold:

- its path matches an expected operational write;
- the path is not the selected target, an authored edit surface, or a forbidden path;
- the bound verdict did not list the path.

Such a path cannot satisfy or violate the cage. Any other movement after the after phase still refuses the rerun with `after verdict binding is tampered or stale`. That includes a new or edited source, test, or root file, a changed kind for a bound path, and a bound change that disappeared. Leaving out every operational write would also hide the disappearance of a bound change, so that alternative was rejected. Recording a second digest in the manifest would change the published manifest schema and the Python binding's `patch_sha256`, so that alternative was rejected too.

## Failure behavior

Repair attempts fail closed:

- a packet whose selected edit target is not a test surface (a `tests` or
  `test` path component, or a `*_test.rs`, `*_tests.rs`, `test_*.py`,
  `*_test.py`, or `*_tests.py` file name) is refused before any attempt is
  created, and before the phase writes any workflow artifact or prints a
  completion line; inline `#[cfg(test)]` modules in production files are not
  valid edit targets;
- malformed or unknown attempt IDs are rejected;
- missing, moved, modified, or digest-mismatched retained artifacts are rejected;
- a cross-attempt packet is rejected;
- ambiguous seam-selected after phases are rejected with an instruction to pass `--attempt`;
- stale `HEAD`, incomparable evidence, and edit-cage violations do not produce a receipt-ready state;
- tracked differences from the prepared head outside the trusted edit surface block receipt admission, committed or not, and so do untracked paths the attempt wrote outside it; an untracked file that already existed at the before phase and is byte-identical afterwards was not written by the attempt and does not block admission;
- only a receipt whose `status` is `advisory` recommends including it in review. For an `incomplete` or `invalid` receipt, the receipt's own `summary.next_action.recommended_action` and `summary.next_recommendation` state the status and reason, say the receipt is not review evidence, and name the recovery; the after phase prints that same field as its `next:` line.

Two refusals happen before the attempt is finished, so the attempt stays `awaiting_edit` and the printed rerun works: changed analysis inputs ([Cargo.lock between the phases](#cargolock-between-the-phases)) and a `HEAD` that no longer descends from the prepared head ([Committing between the phases](#committing-between-the-phases)).

A failed, incomparable, or stale attempt is terminal: re-running its after phase or `ripr agent receipt` refuses. The after phase lists each refused path and the recovery route. If you committed the test edit or a refused change, uncommit it first (for example `git reset --soft HEAD~1` when it is the last commit; the changes stay in the worktree). Undo the refused changes and set the test edit aside, for example with `git stash`. While the gap still exists, run `ripr agent repair --root . --seam-id <seam-id> --phase before` to prepare a new attempt. Restore the test edit, then run the new `--attempt` command.

RIPR does not select “the latest” attempt, reconstruct an attempt from mutable global files, or continue on partial evidence.

## Boundary

A repair attempt prepares and verifies evidence. RIPR does not author or apply the focused test edit, call an external model provider, run mutation testing, prove test adequacy or correctness, authorize merge, or turn static evidence into runtime proof.
