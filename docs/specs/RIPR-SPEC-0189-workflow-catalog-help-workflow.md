# RIPR-SPEC-0189: Typed workflow catalog and bounded `help workflow` discovery

Status: proposed

Owner: product / cli

Created: 2026-10-02

Linked issues:

- #4824 (command-discovery C3)
- #1770 (parent controller under #1613; later slices remain separate)

Support-tier impact:

- None. This slice adds a help-only discovery route; it does not change what
  any command accepts or executes.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The workflow table is
  a pure static table validated in tests; `help workflow` renders bounded text
  and is held byte-identical against the repository tree by an integration
  test.

## Problem

The C2 metadata table (RIPR-SPEC-0187) tells a reader what each command is,
but not how commands compose into reviewed, bounded task sequences. The
canonical multi-command workflows currently live only in prose docs, so
workflow identity, step order, per-step cost and operation claims, result
routing, recovery routes, and limitations can drift apart from the command
metadata without any check noticing, and an unknown workflow name has no
bounded, host-independent discovery surface.

## Behavior

`crates/ripr/src/cli/workflow_catalog.rs` is the canonical typed workflow
table. Each row is keyed by a stable kebab-case workflow identity and carries:

- aliases, a closed workflow-tag identity, a one-line purpose and
  applicability statement, and prerequisites;
- ordered required steps and optional steps; each step names a catalog
  command plus a role line, and mirrors the step command's cost, operation,
  and side-effect flags from the C2 metadata table;
- result families: every family names its producing step, its outcome, and
  exactly one typed next route: a registered command, a `stop:` terminal
  state, or an explicit `limitation:`;
- artifacts read and written, recovery routes from the first command,
  stop conditions, advanced/control alternatives, and a non-empty limitations
  statement.

Authority:

1. Every step is classified Public or Compatibility, matching the C1 catalog
   class of the step's command; every advanced alternative is Advanced class.
2. The validator rejects missing, duplicate, and cyclic edges, and required
   roles unreachable from the workflow's first command. Recovery routes that
   return to the first command are retry loops, not cycles.
3. Listing and rendering order is derived from the identity keys, never from
   declaration order in the source table.
4. Every result family names exactly one next route: a command, a `stop:`
   terminal, or a `limitation:` statement; no family is left unrouted.
5. A workflow never claims more than its step commands' metadata: step cost,
   operation, and side-effect flags must equal the C2 metadata row, and every
   artifact-write claim must be a substring of the step command's declared
   output roles.
6. The `repair-gap` workflow's first required step is `ripr agent repair`,
   the ordinary public route; its advanced alternatives are all Advanced class
   control surfaces, and its rendered guidance states the non-claims (ripr
   never performs the edit, never closes an attempt because a command was
   shown) and the bounded verify claim: the verify phase runs the authorized
   test command only for a trust-bound attempt with explicit
   `--verify-authorized` and a matching authority, and refuses otherwise.
7. An unknown workflow name fails with an `unknown workflow` error whose
   suggestions come only from workflow identities (ids and aliases), never
   from command spellings; the failure family stays distinct from the
   `unknown command` family, and flag-shaped or overlong input is a usage
   error, not an unknown-workflow lookup. Aliases are checked against every
   canonical id in the table, so an alias equal to a later workflow's id is
   rejected even though lookup resolves first match.
8. Rendered output is bounded and host-independent: no absolute workspace
   paths, no host-specific separators, and a single-workflow render that stays
   within a fixed size budget.
9. `ripr help workflow` and `ripr help workflow <name>` never mutate the
   repository: an integration test snapshots the working tree before and
   after and requires byte identity.
10. Every table row and graph edge is load-bearing: removing any required
    step or family edge from the production table fails a test, and the
    recovery block cannot be emptied, so the catalog cannot silently shed
    content. Graph edges come only from declared family producers and
    recovery routes — there are no implicit adjacent-step edges — and every
    required step must produce at least one result family, so removing a
    single family fails the check even when the remaining edges still
    connect the step chain. Workflow content mirrors real producers: the
    repair handoff selects a seam ID from the pilot packet (a `probe:`
    finding ID is not a seam ID), and the PR-evidence workflow feeds
    `ripr outcome` two preserved raw `repo-exposure-json` snapshots, keeping
    the `pr-evidence` wrapper packet out of the outcome inputs.

`workflow_catalog()` lookup is static data. It does not run analysis, spawn a
process, open a network, mutate a workspace, or write a product artifact.

## Required Evidence

- Production table integrity is empty of violations across all five initial
  workflow rows (inspect-change, guided-adoption, repair-gap,
  compose-pr-evidence, adopt-ci).
- Contradiction fixtures reject, one at a time: a step classified outside
  Public/Compatibility, an artifact-write claim absent from the step command's
  declared outputs, a step cost contradicting the metadata row, a result
  family routing to an unregistered command, a missing workflow tag, a
  repair-route fixture violating the repair-gap law, a cyclic edge fixture,
  an edge-removal fixture against the production table, a single-family
  removal that leaves its producing step without outcomes, and an alias that
  collides with a later workflow's canonical id.
- Rendered `help workflow` lists the five identities in sorted order with the
  bounded-render footer; rendered `help workflow repair-gap` carries the
  Purpose, Commands, Result families, and Limitations sections.
- An unknown workflow at the built binary exits nonzero with an `unknown
  workflow` error that never mentions the `unknown command` family.
- An isolated-repository integration run proves byte-identical tree state
  across the workflow help routes and bounds the rendered output.

## Non-Goals

- No `help --json` schema or new machine surface (#4825). The seam for that
  follow-up is `workflow_catalog()` joined with `metadata()` and `catalog()`;
  no consumer is added in this slice.
- No new workflows beyond the initial five; adding a workflow needs explicit
  command/task authority in its own claim.
- No change to the default help screen, `help --all`, or any golden output.
- No command rename, removal, execution, analyzer, durable-attempt, release,
  publication, credential, or repository-setting change.

## Acceptance Examples

1. `workflow_catalog()` returns five rows; `workflow_for` resolves an id or
   alias to its canonical row.
2. A synthetic step whose cost contradicts the metadata row is reported as a
   violation naming the workflow, the step, and the contradiction.
3. `ripr help workflow` renders the five identities and the
   `Run \`ripr help workflow <name>\`` footer; `ripr help workflow adoption`
   renders the `guided-adoption` workflow.
4. `ripr help workflow repar-gap` exits nonzero with an `unknown workflow`
   error; `ripr help workflow repair-gap extra` exits nonzero with the usage
   line.

## Test Mapping

- `crates/ripr/src/cli/workflow_catalog.rs` unit tests cover production-table
  integrity, the five-denominator presence, listing sort/bound/host
  independence, render determinism, alias resolution, the unknown-workflow
  suggestion family, the repair-gap law, and the missing/duplicate/cycle/
  classification/artifact-claim/tag/repair-route/edge-removal/
  single-family-removal/forward-alias-collision contradiction fixtures.
- `crates/ripr/src/cli/command.rs` unit tests pin the `help workflow [name]`
  parser grammar, including the usage-error family for flags and extra
  arguments.
- `crates/ripr/tests/cli_help_hierarchy.rs` pins the rendered listing, the
  single-workflow sections, and the unknown-workflow failure family against
  the built binary.
- `crates/ripr/tests/cli_smoke.rs` proves the workflow help routes are
  byte-identical against an isolated repository tree and bounded in output.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/cli/workflow_catalog.rs` | workflow types, governed table, fail-closed validators, graph checks, bounded renderers, tests |
| `crates/ripr/src/cli/command.rs` | `CliCommand::HelpWorkflow` variant and the `help workflow [name]` parser route |
| `crates/ripr/src/cli/execute.rs` | dispatch of `HelpWorkflow` to the help renderer |
| `crates/ripr/src/cli/help.rs` | `print_workflow` entry point shared by the dispatch |
| `crates/ripr/src/cli/command_metadata.rs` | unchanged C2 table consumed by the cross-check (RIPR-SPEC-0187) |

## Metrics

- `workflow_catalog_entries`
- `workflow_catalog_violations`
