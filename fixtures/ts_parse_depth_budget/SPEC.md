# Fixture: ts_parse_depth_budget

Spec: RIPR-SPEC-0180

## Given

A TypeScript workspace whose Phase-1 discovery parses every accepted file,
not only diffed ones (issue #4101). Four source files exercise the parse
guard:

- `src/ok.ts` — a benign changed predicate (`>=` → `>`). The control that
  must keep classifying normally.
- `src/deep_under_budget.ts` — a changed file with 400 nested parens in the
  changed expression. Depth 400 aborted the unguarded Windows-debug
  main-thread parse (`thread 'main' has overflowed its stack`, exit 127,
  empty output — retained red witness on the `e_unchanged_deep` probe
  fixture). Under the 2,000-level budget it must parse on the large-stack
  worker and classify normally.
- `src/deep_over_budget.ts` — a changed file with 2,100 balanced nested
  parens. The oxc worker itself would parse it (it is balanced); only the
  pre-parse budget refuses it, so the run must carry the typed
  `expression_nesting_budget` static-limit reason — not a parser error —
  on the `unsupported_syntax` finding and the `language_scope_unsupported`
  limitation.
- `src/deep_over_budget_unchanged.ts` — 2,200 nested parens, NOT in the
  diff. The issue's exact blast-radius shape: one deep discovered file must
  not kill the run. Before the fix any such file aborted the process; after
  the fix it is refused and skipped like any other unparseable file.

## When

```bash
ripr check \
  --root fixtures/ts_parse_depth_budget/input \
  --diff fixtures/ts_parse_depth_budget/diff.patch \
  --mode fast
```

## Then

The run completes (no stack-overflow abort) and the output:

- Classifies `ok.ts` and `deep_under_budget.ts` changes normally (real
  findings; deep-but-under-budget files keep working).
- Emits an `unsupported_syntax` static-unknown finding for
  `deep_over_budget.ts` whose evidence carries
  `static_limit unsupported_syntax: static limit expression_nesting_budget:`
  with the estimated depth, the budget, and the overflow rationale.
- Emits a `language_scope_unsupported` analysis limitation for
  `deep_over_budget.ts` carrying the same typed reason.
- Produces no limitation for `deep_over_budget_unchanged.ts` (unchanged
  files follow the existing parse-failure skip policy) and — critically —
  the process survives discovering it.

## Must Not

- Abort the process with a stack overflow for any input.
- Report the budget refusal as `N parser error(s)` — the typed reason must
  name the nesting budget.
- Claim the refused file was analyzed (evidence stays
  `StaticUnknown` / `unsupported_syntax`).
