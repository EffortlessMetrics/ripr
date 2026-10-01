# RIPR-SPEC-0190: Versioned machine discovery via `ripr help --json`

Status: proposed

Owner: product / cli

Created: 2026-10-03

Linked issues:

- #4825 (command-discovery C4)
- #1770 (parent controller under #1613; this is the final chain slice)
- #1613 (progressive discovery theme)

Support-tier impact:

- None. This slice adds a help-only discovery route; it does not change what
  any command accepts or executes.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The document is a
  pure projection of the three static catalog tables (RIPR-SPEC-0184/0187/
  0189), validated in tests; `help --json` emits one stdout line and is held
  byte-identical against the repository tree by an integration test.

## Problem

The C1 command catalog (RIPR-SPEC-0184), the C2 metadata table
(RIPR-SPEC-0187), and the C3 workflow catalog (RIPR-SPEC-0189) give a human
reader bounded, reviewed discovery surfaces. A machine consumer — an editor
plugin, an agent harness, a docs generator — has no equivalent: it must
scrape human help text, which is explicitly outside every identity surface
and free to change wording at any time. Without a versioned machine route,
each consumer invents its own parse of unstable text, and nothing enforces
that the discovery data is complete, deterministic, or honest about its
non-claims.

## Behavior

`crates/ripr/src/cli/help_json.rs` projects the three accepted typed
authorities into one strict JSON document on `ripr help --json`. The
document carries:

- `schema_version` (integer, currently `1`; bumped on any material DTO shape
  change, pinned by tests, and registered in `docs/OUTPUT_SCHEMA.md`),
  `product_version`, and `catalog_contract_version`;
- `commands`: one row per C1 catalog entry joined to its C2 metadata row on
  the catalog identity — id, path, class, discovery posture, replacement or
  retirement relation (canonical, `alias_of` with its canonical target, or
  `retired` with an optional replacement), aliases, summary, task label,
  workflow tags, operation, cost, all six side-effect flags, primary inputs,
  output roles, state target, JSON support, example, next routes, stop
  states, limitations, and the not-applicable reason;
- `workflows`: one row per C3 workflow entry — identity, aliases, command
  tag, purpose, applicability, prerequisites, first command, required and
  optional steps (each projecting its command's governed side effects from
  the C2 row), result families with a tagged `next` route (`command`,
  `stop`, or `limitation`), artifacts, recovery routes, stop conditions,
  advanced alternatives, and limitations;
- `catalog_digest`: a sha256 hex digest over the scoped catalog surface —
  the sorted command rows, the sorted workflow rows, and the catalog
  contract version — never over the product version, schema version,
  document copy, rendered whitespace, human help constants, or the binary
  path; a package release therefore remints `product_version` without
  moving the catalog digest;
- `limitations` and `non_claims`: explicit identity-surface and boundary
  statements a consumer can render without inferring them.

Authority:

1. Cross-root identity: two different equivalent workspace roots produce
   byte-identical documents; no absolute path, PID, timestamp, or
   environment observation may appear in the bytes.
2. Presentation invariance: TTY detection, color settings, terminal width,
   locale, and verbosity flags cannot move the document. The grammar is
   strict — `ripr help --json` accepts exactly one flag, so `help --json
   --quiet` and every other extra-argument shape is a usage error, not a
   silently re-shaped document. The global `--verbose`/`-v` extraction
   cannot bypass this: combined with the machine route, in either position,
   it fails closed with the same usage error instead of emitting a document
   plus a stderr diagnostic.
3. Order independence: commands sort by identity key, workflows sort by
   identity key, and every embedded string list sorts its entries, so
   reordering rows in any source table normalizes to the same bytes and the
   same digest.
4. Fail-closed identity and joins: any catalog, metadata, or workflow
   violation, a duplicate identity, a missing metadata projection, or a
   zero-row command/workflow surface fails before any document bytes exist;
   stdout can never carry a plausible partial document.
5. Workflow references resolve exactly: every step command and result-family
   producer names a registered command row, and every routed command target
   is a registered public/compatibility command.
6. Row isolation for command edits: a single metadata-field change remints
   the digest and changes exactly the one projected row, leaving every
   other command row byte-identical.
7. Row isolation for workflow edits: a single workflow transition change
   remints the digest and changes exactly the one projected workflow row.
8. Human help wording is outside the identity surface: renderer headers and
   help-screen copy never enter the document or the digest.
9. Step side effects are projected from the supplied C2 metadata row of the
   step's command for the flags a workflow does not mirror, and from the
   workflow step row itself for the three narrowable flags the C3 mirror
   validator cross-checks; flags are never silently projected as false, and
   a synthetic document can never disagree with itself about one command.
10. The document is advisory discovery data. It does not execute commands or
    workflows, does not run analysis, compilation, tests, or mutation, does
    not spawn child processes or open the network, does not read git state
    or caches, and does not write product artifacts or mutate the workspace;
    an integration test snapshots the repository tree before and after and
    requires byte identity.
11. Completeness is enforced: a document with an empty `commands` or
    `workflows` section fails closed, so a partial catalog cannot present
    itself as the complete discovery surface.
12. Schema, documentation, and producer agree: the schema version constant,
    the `docs/OUTPUT_SCHEMA.md` registry row, and the emitted document carry
    the same version bytes, and the version is pinned by unit tests.

`help --json` serializes existing catalog authority only. It never parses
human help, executes commands, inspects a repository, or strengthens any
command or workflow claim.

## Required Evidence

- Production document integrity is empty of violations and the document is
  valid JSON with `schema_version` 1, non-empty `commands` and `workflows`,
  and a 64-hex-character sha256 digest.
- Determinism fixtures: reversing the source table order normalizes to the
  same digest; the rendered bytes are stable across builds of the same
  catalog.
- Contradiction fixtures reject, one at a time: a duplicate command
  identity, a zero-command surface, a zero-workflow surface, and a metadata
  row loss.
- Row-isolation fixtures: a single metadata-field edit changes exactly one
  command row and the digest; a single workflow transition edit changes
  exactly one workflow row and the digest.
- Digest-scope fixture: a product-version or document-copy-only change
  remints neither the catalog nor its digest.
- Relation fixture: the compatibility spelling `start-here` projects its
  `alias_of` relation naming `first-pr` and its compatibility discovery
  posture.
- Supplied-table fixture: an unmirrored side-effect flag changed in a
  supplied metadata table projects identically onto the command row and
  every workflow step of that command.
- Human-wording fixture: known human help headers and copy strings are
  absent from the document.
- Classification fixture: compatibility and advanced classes both project,
  and every source alias list projects exactly in sorted order.
- An isolated-repository integration run proves: byte-identical documents
  across two distinct roots, byte-identical documents across
  color/width/locale environment cases, empty stderr, a host-path-free
  document, a usage error for `help --json --quiet`, the versioned shape,
  and byte-identical tree state across all runs.

## Non-Goals

- No command execution API, shell plan, or query engine over the catalog.
- No change to the default help screen, `help --all`, `help workflow`, or
  any golden output.
- No new command, workflow, analyzer, durable-attempt, release, publication,
  credential, or repository-setting change.
- No consumption surface is added beyond this document; agent/editor
  consumers land in their own claims.

## Acceptance Examples

1. `ripr help --json` prints one JSON line carrying `schema_version` 1,
   every catalog command row, every workflow row, and the catalog digest;
   stderr is empty and the workspace is untouched.
2. `ripr help --json extra`, `ripr help --json --quiet`, and `ripr help
   --json --all` exit nonzero with the same usage line.
3. The same binary run in two different fresh git repositories produces
   byte-identical stdout.
4. Removing every workflow row from the source table fails the help --json
   zero-row gate before any bytes reach stdout; removing one row fails the
   C3 governed-denominator check (RIPR-SPEC-0189), which owns the workflow
   identity surface — this slice adds no second identity authority.

## Test Mapping

- `crates/ripr/src/cli/help_json.rs` unit tests cover production document
  validity, byte stability and explicit ordering, source-reorder
  normalization, duplicate-identity and zero-row fail-closed behavior,
  workflow reference resolution, command row isolation, workflow row
  isolation, human-wording exclusion, classification/alias projection,
  compatibility relation projection, digest scope, and supplied-table step
  effect consistency.
- `crates/ripr/src/cli/command.rs` unit tests pin the strict `help --json`
  parser grammar, including the usage-error family for extra arguments and
  the precedence of the `--json` intercept over the `help <command>`
  rewrite.
- `crates/ripr/tests/cli_smoke.rs` proves cross-root determinism,
  environment invariance, the versioned shape, the strict-grammar rejection
  (including the global verbosity flag in both positions), host-path
  freedom, and byte-identical repository state against the built binary.
- `crates/ripr/src/cli/mod.rs` rejects the global verbosity flag combined
  with the machine route before the verbose extraction can bypass the
  strict grammar.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/cli/help_json.rs` | versioned DTO, catalog projection, digest, fail-closed document validation, render/print entry points, tests |
| `crates/ripr/src/cli/command.rs` | `CliCommand::HelpJson` variant and the strict `help --json` parser route |
| `crates/ripr/src/cli/execute.rs` | dispatch of `HelpJson` to the document printer |
| `crates/ripr/src/cli/mod.rs` | rejection of the global verbosity flag combined with the machine route |
| `crates/ripr/src/cli/command_catalog.rs` | `CATALOG_CONTRACT_VERSION` constant naming the catalog contract generation |
| `crates/ripr/src/cli/command_metadata.rs` | `as_str()` projections of the cost/operation enums consumed by the DTO |
| `crates/ripr/src/cli/workflow_catalog.rs` | unchanged C3 table consumed by the projection (RIPR-SPEC-0189) |

## Metrics

- `help_json_schema_version`
- `help_json_command_rows`
- `help_json_workflow_rows`
- `help_json_document_violations`
