# RIPR-SPEC-0150: Rust bounded value-propagation limitation

Status: accepted

Issue: #3215

## Problem

Rust analysis can identify a changed value-producing binding while remaining
unable to establish how that value reaches a later equality predicate. The
limitation must name this bounded unresolved edge without treating syntax
proximity as proof of propagation or test adequacy.

## Behavior

When a changed Rust `let` binding has a right-hand side containing `map_or`
and one of the bounded operation families `find`, `rfind`, or `len_utf8`, and
the same owner function later compares that binding in an equality predicate,
RIPR may emit:

```text
static_limit_kind: rust_value_propagation_unresolved
class: static_unknown
```

The finding must retain related tests when they are already discovered. It
must disclose the last established binding, the first unresolved edge into
the equality predicate, the analyzer route
`analysis/rust-value-propagation`, and a limitation-only non-claim.

## Required Evidence

An emitted finding records the changed binding expression, the unresolved
equality edge, the `analysis/rust-value-propagation` route, and an explicit
limitation-only non-claim. The stable enum token is
`rust_value_propagation_unresolved`; no repair or mutation evidence is implied.

## Required guards

- the changed line must parse as a simple `let` binding;
- the operation-family and `map_or` checks apply to the changed right-hand
  side, not arbitrary owner text;
- the compared identifier must be a whole identifier in the same owner body;
- a missing owner or missing related tests produces no new limitation;
- unrelated helper, loop, dynamic, and non-equality shapes remain unchanged;
- classification is never promoted and no repair packet or verify command is
  synthesized.

## Acceptance Examples

- Accept a changed `rfind(...).map_or(...)` or `len_utf8().map_or(...)`
  binding followed by an equality on the same binding in the same owner.
- Reject comments, string literals, `map_or_else`, member-name lookalikes,
  shadowed bindings, missing owners, missing related tests, and unsupported
  value shapes.

## Test Mapping

The production-path Rust adapter fixture covers the `rfind`/`map_or` binding
feeding an equality predicate with two exact related tests. Negative controls
cover a non-family `len` binding and must not receive this limitation. The
stable enum wire string and LSP accepted-kind catalog are tested separately.

## Non-Goals

This slice does not resolve value propagation, prove the equality boundary,
change probe-family inference, execute mutations, calculate rates, or create
repair/test instructions. Those are later bounded slices of #3215.

## Implementation Mapping

- `crates/ripr/src/analysis/language/rust.rs` performs the bounded,
  fail-closed detection and emits limitation evidence.
- `crates/ripr/src/domain/language.rs` owns the stable enum token and
  description; `crates/ripr/src/lsp/gap_artifacts.rs` accepts the artifact kind.
- `docs/OUTPUT_SCHEMA.md` and `docs/STATIC_LIMITS.md` document the wire and
  static-limit contracts.

## Metrics

The analyzer reports counts through the existing finding summary and preserves
the limitation token in JSON/LSP artifacts. This slice adds no coverage,
mutation, adequacy, or rate metric; those claims remain outside the contract.
