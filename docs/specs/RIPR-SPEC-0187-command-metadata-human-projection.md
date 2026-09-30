# RIPR-SPEC-0187: Rich command metadata projected into human help and inventories

Status: proposed

Owner: product / cli

Created: 2026-10-02

Linked issues:

- #4823 (command-discovery C2)
- #1770 (parent controller under #1613; later slices remain separate)

Support-tier impact:

- None. This slice describes existing command behavior; it does not change
  what any command accepts or executes.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The metadata table is
  a pure static table validated in tests; human help gains only line-end class
  markers.

## Problem

The C1 catalog (RIPR-SPEC-0184) gives every public command one typed identity,
but not what running it means: which task it serves, what it may cost, what it
reads, writes, or changes, and where it routes next. Those facts currently live
only in prose help and docs, so a reader (human or agent) cannot compare two
commands, an automated surface cannot be built on top of help text, and the
concise help, exhaustive help, and hierarchy documentation can drift apart
without any check noticing.

## Behavior

`crates/ripr/src/cli/command_metadata.rs` is the canonical rich-metadata table.
Each row is keyed by the C1 catalog identity and carries:

- a one-line summary and the canonical task label from the hierarchy
  vocabulary;
- membership tags drawn from a closed workflow set (promoted to the typed
  workflow catalog by #4824);
- an operation class `ReadOnly | WritesArtifacts | StateChanging`;
- a cost class `Small | Analysis | Workspace` shared with the `check --mode`
  help and #1572's cost vocabulary;
- explicit side-effect flags (analysis, compile, test run, mutation, network,
  child process) in the #1572 vocabulary plus child processes;
- primary inputs, output roles (default and optional), and the durable state a
  state-changing command mutates;
- JSON support, an example synopsis, next routes (catalog paths only),
  explicit stop states, advisory limitations, and an explicit
  not-applicable reason when a row intentionally carries no content.

Authority:

1. Every public-facing catalog row is described or carries an explicit
   not-applicable reason; internal rows stay out of scope.
2. A row cannot declare a change class the command does not perform:
   read-only rows declare no product writes and no state target; writing rows
   declare at least one output role; state-changing rows name their state
   target.
3. Cost and side-effect facts stay inside the canonical contract: no row may
   claim mutation execution, projection-cost rows claim no analysis, compile,
   or test work, and a test run implies its compile.
4. Summaries and limitations never claim runtime-mutation outcomes; the five
   prohibited tokens fail closed in the validator.
5. Human surfaces validate against the typed tables rather than scraping each
   other: the default screen routes only to catalog commands; the `help --all`
   grouped listing documents each ordinary or advanced command by its own line
   or a child line (family umbrellas such as `cache` dispatch to help and are
   documented through their children); advanced rows carry an `[advanced]`
   line marker, the compatibility alias carries `[compatibility]`, and public
   rows carry no marker; hidden, retired, and internal rows appear nowhere in
   `help --all`.
6. `docs/COMMAND_HIERARCHY.md` command spans resolve to catalog identities
   with metadata rows, and the eight pinned task rows keep their documented
   task labels in the metadata table.

Metadata lookup is static data. It does not run analysis, spawn a process,
open a network, mutate a workspace, or write a product artifact.

## Required Evidence

- Production table integrity is empty of violations across all 80 catalog
  rows.
- Contradiction fixtures reject, one at a time: a read-only row declaring a
  product write, a write claim without an output role, a state change without
  a state target, a mutation claim, projection cost with analysis work, a
  test run without its compile, a next route outside the catalog, and a
  prohibited runtime-outcome token in a summary.
- The rendered default screen and `help --all` agree with the typed tables
  (identity, class markers, uniqueness, no hidden/internal leaks).
- The hierarchy documentation resolves to catalog rows and the pinned task
  labels match.
- Rendered `help --all` output carries the visible `[advanced]` and
  `[compatibility]` markers, asserted at the integration level against the
  built binary.

## Non-Goals

- No `help workflow` route or workflow state model (#4824). Workflow tags stay
  an opaque closed set here.
- No `help --json` schema or new machine surface (#4825). The seam for both
  follow-ups is `metadata()` joined with `catalog()`; no consumer is added in
  this slice.
- No rewrite of help prose, per-command option bodies, or the default screen
  layout; the only rendered change is the line-end class markers in
  `help --all`.
- No command rename, removal, execution, analyzer, durable-attempt, release,
  publication, credential, or repository-setting change.

## Acceptance Examples

1. `metadata()` returns one row per catalog identity; `metadata_for` joins a
   catalog entry to its row by identity alone.
2. A synthetic read-only row that names an output file is reported as a
   violation naming the row and the contradiction.
3. `ripr help --all` renders `ripr agent start ... --out target/ripr/workflow]
   [advanced]` and `ripr start-here [same options as first-pr]
   [compatibility]`, while `ripr check` carries no marker.
4. Removing the advanced marker from one `help --all` row fails the projection
   agreement test naming the row.

## Test Mapping

- `crates/ripr/src/cli/command_metadata.rs` unit tests cover production-table
  integrity, full public-row coverage, the eight contradiction fixtures, the
  rendered-surface agreement check (including the negative fixture that an
  identical duplicated `help --all` listing line is reported), and the
  hierarchy-doc resolution and pinned-label checks.
- `crates/ripr/tests/cli_help_hierarchy.rs` pins the rendered `[advanced]` and
  `[compatibility]` markers against the built binary.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/cli/command_metadata.rs` | metadata types, governed table, fail-closed validators, projection-agreement check, tests |
| `crates/ripr/src/cli/command_catalog.rs` | unchanged C1 identity table consumed by the join (RIPR-SPEC-0184) |
| `crates/ripr/src/cli/help/overview.rs` | `help --all` grouped listing carries the class markers |
| `crates/ripr/src/cli/help.rs` | test-only `discovery_surfaces()` accessor for the agreement check |

## Metrics

- `command_metadata_entries`
- `command_metadata_violations`
