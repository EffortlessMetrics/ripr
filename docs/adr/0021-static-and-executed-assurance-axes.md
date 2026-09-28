# ADR 0021: Keep Static and Executed Assurance on Separate Axes

Status: accepted

Date: 2026-07-22

Artifact ID: RIPR-ADR-0021

## Context

RIPR's repair loop compares static before/after evidence and can display a
verification command or receipt route. Those artifacts are useful, but their
names can cause downstream consumers to mistake a static comparison for an
executed test or a receipt for runtime proof. The gate and receipt trust work
needs a stable vocabulary before command execution is added.

## Decision

Represent assurance as independent static-movement, command-verification,
receipt, and external-runtime-mutation axes. Define the vocabulary and typed
`CommandSpecV1` shape in `RIPR-SPEC-0135` and
`schemas/ripr/repair-assurance.schema.json`.

Static movement is producer-owned evidence from comparable RIPR artifacts.
Command availability is only a typed description until an explicitly governed
runner executes it. Receipt issuance is a validation decision over the inputs,
not proof that a command ran. Runtime mutation confirmation remains external.

No output consumer may infer a stronger state from a neighboring field. In
particular, `verify_command`, `commands_run`, `verify_result`, a successful
process exit, or receipt presence cannot independently mean `verified`,
`killed`, `survived`, `adequate`, or `proven`.

## Consequences

Positive:

- static movement and executed-result disagreement remains visible;
- static-only receipts can remain useful without overstating assurance;
- later command execution can be added without renaming or overloading the
  analyzer's static evidence model; and
- consumers have closed, versioned states for unavailable, stale, malformed,
  cancelled, and externally supplied evidence.

Costs and limits:

- existing output fields need compatibility documentation during migration;
- the command runner and final receipt binding require separate implementation
  slices; and
- this decision does not itself add gate authority or runtime mutation proof.
