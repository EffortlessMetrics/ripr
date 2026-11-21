# RIPR-SPEC-0157: Changed-binding → predicate-operand relation

Status: proposed

Issue: #3294 (parent #3215; follows #3271)

## Problem

Slice #3271 made the `find`/`rfind`/`len_utf8` through `map_or`
boundary honest by naming `rust_value_propagation_unresolved` on the
generic static-unknown finding — but it did not make the changed
behavior satisfiable. When a diff changes a `let` initializer whose
binding later feeds a same-function predicate (`let end =
input.rfind(delim).map_or(0, |idx| idx); … if end == start {`), the
probe stays the catch-all static-unknown "changed syntax is not mapped
to a high-confidence probe family" finding even though the exact
behavioral predicate is lexically present. The user can already have
exact equality-boundary tests while infection reads as unknown for a
generic syntax reason.

## Behavior

- One internal typed relation (`analysis/probes/binding_predicate.rs`,
  `ChangedBindingPredicateUse`) carries the binding identity, the
  initializer, the predicate expression, the use line, the operand
  side, and the value-resolution status. The link is never encoded in
  evidence prose alone.
- The relation resolves within the owning function only (sibling
  functions never relate) and only when the binding reaches the use
  directly: no re-binding (including a destructuring re-bind) and no
  reassignment between the changed declaration and the use.
- Supported use positions, under **strict direct-operand semantics**:
  the operand position must hold exactly the binding, optionally behind
  `&`/`*`. Supported positions are a comparison operand
  (`==`, `!=`, `<`, `<=`, `>`, `>=`) anywhere at statement level, a
  whole `&&`/`||` term of an `if`/`while` condition, and a bare `match`
  scrutinee. Comment and string text is masked. A method receiver
  (`end.is_empty()`), a call or index interior (`map.get(&end)`,
  `f(a == end, b)`), a composite expression operand (`end + 1`), a
  grouped or negated comparison (`(end == start)`, `!(end == start)`),
  a field path (`self.end`), and a longer identifier (`endpoint`) never
  relate, and a `<`/`>` that is half of a `<<`/`>>` shift is not a
  comparison — `sink(flags << end)` never relates.
- Region tracking keeps the scan in the owning scope: braces opened by
  a closure or a nested item are no-use regions (separate binding
  scopes or unmodeled capture), a nested item's multi-line signature is
  skipped to its body brace, a line inside unclosed foreign
  parentheses (a multi-line initializer or call continuation) is a
  continuation, not a use site, and leaving the changed declaration's
  block depth ends the live span (a same-named outer binding's later
  uses belong to that outer binding). A single `|` between operands is
  a bitwise OR, not a closure pipe; only argument-position pipes count.
- On resolution, the probe retargets to the predicate: family
  `predicate`, location at the use line, expression the predicate line,
  and the old/new initializers as `before`/`after` causal evidence.
  Identity stays content-addressed over the predicate expression, so
  multiple uses stay separately identifiable and deterministic.
- The retarget only fires on a complete single-line declaration: an
  added `let` line that does not end in `;` or leaves
  parentheses/braces unbalanced is the first fragment of a multi-line
  initializer and fails closed to the generic per-line probes.
- The finding keeps its normal predicate-shaped classification and
  gains evidence: `binding_predicate_relation` names the binding, both
  causal initializers, and the use line; when the initializer contains
  an operation, `binding_predicate_value_unresolved` (a neutral
  disclosure prefix, deliberately outside the structured `limitation_*`
  contract, since the finding carries no `static_limit_kind`) names the
  earliest (leftmost) one — a std-operation token (`.find(`, `.rfind(`,
  `.len_utf8(`, `.chars(`, `.map_or(`), a shift (`<<`/`>>`), the
  earliest call prefix, or the earliest binary operator (arithmetic,
  bitwise, comparison). A bare literal/identifier copy resolves to
  text with no value-unresolved line.
- The generic `changed syntax is not mapped` limitation is absent for
  supported direct binding-use cases.
- Fail-closed blockers (recorded as the relation's explicit scope
  decision, never related): shadowing, reassignment, closure capture,
  macro invocation. Blocked and control shapes keep the pre-#3294
  static-unknown path; the #3271 `rust_value_propagation_unresolved`
  limitation remains the fallback for them.

## Required Evidence

- The retarget reproduction (the #3215 equality-boundary shape reported
  generic static-unknown on main) and the removal experiment: with the
  relation disabled, the positive shape regresses to the old generic
  limitation.
- Golden blast radius measured on the slice head:
  `cargo xtask goldens check` (zero drift on pre-existing goldens; the
  four new fixtures blessed) and `cargo xtask dogfood` green.
- The corpus fixtures: the equality-boundary positive, direct-position
  positives (boolean test, `match` scrutinee, numeric comparison),
  two-uses determinism, and the scope-control set (sibling function,
  inner-scope shadowing, reassignment, comment/string-only mention,
  destructured binding) that must not receive the relation.
- In-crate recurrence tests: `probes/binding_predicate.rs`,
  `probes/diff.rs` retarget tests, and the diff-analysis tests pinning
  the retarget, the earliest-operation naming, and the #3271 macro
  fallback.

## Required guards

- The relation never evaluates an initializer; the operand value stays
  unresolved until a later slice evaluates bounded operations.
- The evidence attach never changes a class, adds a stop reason, or
  flips repair readiness.
- A predicate line the diff itself changes keeps its own direct probe;
  the retarget never duplicates it.
- Removed-only changed bindings do not retarget (base-side evidence per
  RIPR-SPEC-0151).

## Acceptance Examples

- Accept: changed `let end = input.rfind(delim).map_or(0, |idx| idx);`
  with a later `if end == start` — one predicate probe at the use line,
  initializer before/after, `binding_predicate_relation` and
  earliest-operation evidence, no generic changed-syntax limitation.
- Accept: two direct uses — two probes, scan order, distinct ids.
- Reject: the relation through a shadowed, reassigned, closure-captured,
  macro-guarded, sibling-function, or comment/string-only use.

## Test Mapping

`analysis/probes/binding_predicate.rs` `tests`;
`analysis/probes/diff.rs` `changed_binding_initializer_retargets_to_predicate_use`,
`shadowed_binding_initializer_keeps_static_unknown`,
`retarget_skips_predicate_lines_that_are_changed`;
`analysis/language/rust.rs`
`diff_analysis_retargets_changed_binding_to_predicate_use`,
`diff_analysis_keeps_value_propagation_limitation_for_macro_guarded_use`;
fixtures `binding_predicate_{equality_boundary,positions,two_uses,scope_controls}`.

## Non-Goals

- No evaluation of `find`/`rfind`, `chars`, `map_or`, slicing, or
  arithmetic values (later #3215 slices).
- No cross-function helper propagation, closure modeling, or macro
  expansion.
- No relation through a foreign `let` initializer or a call argument
  interior: `foreign` means any comparison whose operand position is
  not the direct identifier — a comparison elsewhere in the statement
  (a control condition or a tail expression) still relates when the
  operand is the bare binding. Composite, grouped, negated, and
  receiver operands fail closed.

## Implementation Mapping

- `analysis/probes/binding_predicate.rs` — the typed relation.
- `analysis/probes/diff.rs` — retarget in `probes_for_file_with_relations`.
- `analysis/language/rust.rs` — evidence attach; #3271 stays fallback.

## Metrics

No new metric; retargeted probes move from the static-unknown to the
predicate family in existing counts.
