# Repair attempt identity

`ripr agent repair` is a two-phase transaction. The before phase prepares bounded evidence; a human or external coding agent makes one focused test edit; the after phase verifies the exact prepared transaction and writes the review receipt.

The durable object is a **repair attempt**, not a seam lookup and not the repository-global workflow directory.

## Ordinary sequence

```text
ripr agent repair --root . --seam-id <seam-id> --phase before
# make the focused test edit outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

The before phase prints the attempt manifest path and the exact `--attempt` command to run next. Preserve that command across agent sessions, process restarts, and concurrent work.

`--seam-id <id> --phase after` remains a compatibility route. It succeeds only when exactly one awaiting attempt has that seam. Zero or multiple matches fail closed; RIPR does not guess which attempt is newest or intended.

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

## After-phase authority

`--attempt <id>` resolves one manifest directly. Before producing a receipt, RIPR verifies that:

1. the manifest path is bound to the selected attempt ID and repository root;
2. the manifest is still in `awaiting_edit`;
3. the retained before snapshot, packet, and edit-cage baseline still match their recorded byte counts and digests;
4. the after phase uses that attempt's retained packet rather than a repository-global or another attempt's packet;
5. repository `HEAD` still matches the prepared head;
6. the observed edit delta is compliant with the retained packet's allowed, forbidden, and expected operational-write surfaces;
7. verify output, packet digest, delta digest, and receipt all bind to the same attempt.

A different attempt for the same seam is a different transaction. Its packet, snapshot, baseline, and terminal state cannot be substituted.

## Terminal state

The after phase records one of these states in `attempt.json`:

| State | Meaning |
| --- | --- |
| `ready_to_finish` | Current, comparable, and edit-cage compliant; receipt admission may proceed. |
| `stale` | Repository `HEAD` changed after the attempt was prepared. |
| `incomparable` | The retained and current evidence cannot support a valid comparison. |
| `failed` | The edit-cage or another terminal invariant failed. |

Only `ready_to_finish` with a current, compliant after verdict can authorize the attempt-bound receipt.

## Compatibility outputs

The composed after command still writes the established projections:

```text
target/ripr/workflow/after.repo-exposure.json
target/ripr/workflow/agent-verify.json
target/ripr/reports/agent-receipt.json
target/ripr/workflow/            # status input
```

Those paths keep existing review and cockpit integrations working. Their evidence is admitted only after the exact attempt's retained before snapshot and packet have been resolved and validated.

## Failure behavior

Repair attempts fail closed:

- malformed or unknown attempt IDs are rejected;
- missing, moved, modified, or digest-mismatched retained artifacts are rejected;
- a cross-attempt packet is rejected;
- ambiguous seam-selected after phases are rejected with an instruction to pass `--attempt`;
- stale `HEAD`, incomparable evidence, and edit-cage violations do not produce a receipt-ready state;
- unrelated repository changes outside the trusted edit surface block receipt admission.

RIPR does not select “the latest” attempt, reconstruct an attempt from mutable global files, or continue on partial evidence.

## Boundary

A repair attempt prepares and verifies evidence. RIPR does not author or apply the focused test edit, call an external model provider, run mutation testing, prove test adequacy or correctness, authorize merge, or turn static evidence into runtime proof.
