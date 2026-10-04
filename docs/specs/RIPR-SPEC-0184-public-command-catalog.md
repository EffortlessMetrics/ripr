# RIPR-SPEC-0184: Typed public command catalog and parser/alias parity

Status: proposed

Owner: product / swarm

Created: 2026-09-29

Linked issues:

- #4822 (command-discovery C1)
- #1770 (parent controller under #1613; later slices remain separate)

Support-tier impact:

- None. This inventory classifies existing parser-accepted command paths. It
  does not rename, remove, execute, or support-promote any command.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. Catalog lookup is a
  pure static table.

## Problem

Public RIPR command identity lived in several lists: the top-level parser
match, `KNOWN_COMMANDS` for typo suggestions, nested-parser expected-subcommand
strings, and help-path tables. Those lists could drift independently, so a
parser-accepted command could miss typo coverage or a catalog-only spelling
could appear without a parser owner. Later discovery slices need one typed
inventory they can consume without parsing parser source or help prose.

## Behavior

`crates/ripr/src/cli/command_catalog.rs` is the canonical inventory of public
RIPR command paths and aliases. Each row carries:

- a stable catalog identity;
- a canonical command path (`check`, `agent repair`);
- aliases and compatibility spellings;
- classification `public | compatibility | advanced | internal`;
- parser owner, dispatch owner, and help owner;
- public discovery posture;
- a replacement or retirement relation when applicable.

Authority:

1. Every parser-accepted public command path appears exactly once in the
   catalog, or has an explicit compatibility/internal disposition.
2. Every catalog canonical path and alias is accepted by the parser at its
   documented strength.
3. Typo suggestions derive from the catalog. There is no second known-command
   list.
4. Duplicate identities, duplicate paths, duplicate aliases, alias cycles, and
   alias/path collisions fail closed.
5. Compatibility, advanced, internal, and retired rows stay out of the ordinary
   public inventory. Internal and retired rows stay out of typo suggestions.
6. Nested paths remain distinct identities from top-level spellings of the same
   last token (`receipt` vs `agent receipt`).
7. Catalog lookup is static data. It does not run analysis, spawn a process,
   open a network, mutate a workspace, or write a product artifact.
8. Ordering of normalized catalog text is deterministic and independent of
   source-row order.

The top-level parser retains help/version flag dispatch and `help <command>`
rewriting, then resolves remaining top-level spellings through the catalog.
Nested parsers remain hand-written; two-way tests bind them to catalog
children.

## Required Evidence

- Focused tests reject a parser-only path, a catalog-only path, a duplicate
  alias, and an internal/retired leak into the public or typo surfaces.
- Production catalog integrity is empty of violations.
- Nested family unknown-subcommand expected lists equal catalog children, and
  each catalog child accepts `--help`.
- Existing bounded `ripr --help`, `help <command>`, and `help --all` tests
  continue to pass.

## Non-Goals

- No rich cost, side-effect, input, or output metadata (#4823).
- No human-help rewrite and no change to detailed option bodies (#4823).
- No `help workflow` (#4824).
- No `help --json` (#4825).
- No command rename, removal, execution, analyzer, durable-attempt, workflow,
  release, publication, credential, or repository-setting change.

## Acceptance Examples

1. `resolve_top_level("start-here")` is the compatibility alias of `first-pr`
   and parses to the same dispatch as `first-pr`. `start-here` is absent from
   the ordinary public inventory and present in typo suggestions.
2. A synthetic extra parser path is reported as `parser-only`; a synthetic extra
   catalog path is reported as `catalog-only`.
3. `agent receipt` and `receipt` keep different catalog identities.
4. Reordering catalog source rows does not change `normalized_catalog_text`.

## Test Mapping

- `crates/ripr/src/cli/command_catalog.rs` unit tests cover integrity, purity,
  normalized stability, public-inventory exclusion, alias dispatch, top-level
  two-way parity, mutation controls, and nested family two-way parity.
- `crates/ripr/src/cli/command.rs` known-command and typo tests consume the
  catalog-derived spelling list.
- `crates/ripr/tests/cli_help_hierarchy.rs` continues to pin rendered help
  behavior unchanged.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/cli/command_catalog.rs` | typed catalog, resolution, integrity, two-way helpers, discriminating tests |
| `crates/ripr/src/cli/command.rs` | help/version special cases; remaining top-level spellings resolve through the catalog; typo suggestions use catalog spellings |
| `crates/ripr/src/cli/help.rs` | exhaustive-help completeness still tracks catalog-derived known commands |

## Metrics

- `public_command_catalog_entries`
- `public_command_catalog_two_way_violations`
