# RIPR-SPEC-0127: LSP Diagnostic Code Catalog

Status: accepted

Owner: product / swarm

Created: 2026-07-18

Linked issues:

- #1662

## Problem

RIPR LSP diagnostics already carry a stable `source = "ripr"` and a `code`
value, but the code contract is not yet a governed interface. The finding,
seam, and gap diagnostic families each build their `code` string with an
inline `format!` at the emission site in `crates/ripr/src/lsp/diagnostics.rs`.
There is no single registry that lists every emitted code, so nothing
mechanically checks that every emitted code is a known code or that an unknown
code cannot be emitted.

This is slice A of #1662. It establishes the typed registry and current-code
inventory only. It does not add per-code documentation, `codeDescription` URIs,
recovery metadata, or a renderer migration — those are later slices of #1662.

## Behavior

A single governed catalog (`crates/ripr/src/lsp/diagnostic_catalog.rs`) owns
the closed set of stable `code` values a RIPR diagnostic may carry. The message
text is presentation; the catalog owns the stable identity of the code.

Three families of code are inventoried and each is built by one catalog
constructor that is the single source of truth for that family:

- finding codes: the verbatim `ExposureClass` label (`exposed`,
  `weakly_exposed`, `reachable_unrevealed`, `no_static_path`,
  `infection_unknown`, `propagation_unknown`, `static_unknown`);
- seam codes: `ripr-seam-{class}` for every `SeamGripClass`, snake to kebab;
- gap codes: `ripr-gap-{kind}` for every known `GapRecord.kind`.

Each catalog entry carries the code identity used today: the stable `code` and
the deprecated `aliases` that still resolve to it. Compatibility aliases are
added only where current output already requires them; none are required today.
Per-code human title, summary, category, and documentation are the metadata
that later slices add as they consume it.

`resolve(code)` maps a code — or a deprecated alias — to exactly one entry, and
returns nothing for an unknown code.

The finding and seam constructors are infallible because `ExposureClass` and
`SeamGripClass` are closed enums whose every label is a governed code. The gap
constructor takes an open `GapRecord.kind` string that may originate from an
external ledger, so it **fails closed**: it returns a code only when that code
is a governed entry, and the emission site skips a gap whose kind is not
registered rather than surfacing an unregistered code.

For every known kind the emitted diagnostic wire shape is unchanged: the
constructors produce the same code strings the previous inline `format!` calls
produced, so no golden output changes in this slice.

## Required Evidence

- Every finding, seam, and gap code the emission path can produce resolves to
  exactly one catalog entry.
- An unknown or malformed code does not resolve.
- The gap constructor returns nothing for a kind that is not a governed entry,
  and the gap emission site does not emit a diagnostic in that case.
- No two entries share a code, and no alias collides with a code or alias.
- The emitted diagnostic code strings for known kinds are byte-identical to the
  previous inline construction, so existing LSP diagnostic fixtures are
  unchanged.

## Non-Goals

- No per-code title, summary, or category metadata (#1662 PR B).
- No `codeDescription.href` projection or generated documentation pages (#1662
  PR B).
- No typed recovery, actionability, or non-claim machine metadata (#1662 PR C).
- No migration of renderer-local code strings outside the LSP diagnostics path
  (#1662 PR D).
- No analyzer classification, severity policy, or gate policy change.

## Acceptance Examples

1. `resolve("exposed")` returns the exposure entry; `resolve("ripr-bogus")`
   returns nothing.
2. Building the seam code for every `SeamGripClass` variant yields a code that
   resolves, including the default-suppressed `strongly_gripped`,
   `intentional`, and `suppressed` classes.
3. Building the gap code for every known `GapRecord.kind`, including the
   externally-sourced `MissingArtifact`, yields a code that resolves.
4. Building the gap code for an unregistered kind returns nothing, and no
   `ripr-gap-*` diagnostic is emitted for it.

## Test Mapping

- `diagnostic_catalog.rs` unit tests cover non-empty codes, global code/alias
  uniqueness, single-entry resolution, every emitted finding/seam/gap code
  resolving, the gap constructor failing closed for an unknown kind, and
  unknown codes not resolving.
- `diagnostics.rs::tests::gap_record_diagnostic_fails_closed_for_unregistered_kind`
  covers the emission site itself: a gap record whose kind is not a governed
  code produces no diagnostic, while a registered kind still emits one.
- The existing LSP diagnostic unit tests continue to pass unchanged, pinning
  the byte-identical emitted code strings for known kinds.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/lsp/diagnostic_catalog.rs` | the governed registry, code constructors, resolution, and validation tests |
| `crates/ripr/src/lsp/diagnostics.rs` | finding, seam, and gap diagnostics build their `code` through the catalog constructors; gap emission fails closed on an unregistered kind |
| `crates/ripr/src/lsp.rs` | registers the `diagnostic_catalog` module |

## Metrics

- `lsp_diagnostic_catalog_entries`
- `lsp_diagnostic_catalog_unresolved_gap_kinds`

## Claim boundary

This contract establishes only that every RIPR LSP diagnostic code is a governed
catalog entry, that unknown codes do not resolve, and that an unregistered gap
kind is not emitted. It does not improve analyzer evidence, decide severity or
gate policy, project documentation URIs, or claim that any suggested repair is
correct.
