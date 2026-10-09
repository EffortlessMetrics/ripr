# RIPR-SPEC-0223: TypeScript parse-depth budget and large-stack parse worker

Status: proposed

Owner:

Created: 2026-09-25

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #4101 (CRITICAL stack-overflow abort from deep expression nesting in any
  discovered workspace file)

Linked PRs:

Support-tier impact:

- No tier change. The TypeScript preview adapter already discloses
  `unsupported_syntax` static limits; this spec bounds when that disclosure
  is produced by a pre-parse nesting budget instead of an oxc parser error,
  and moves the oxc parse off the caller's stack. No new support claim is
  introduced.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new policy surface, gate, or allowlist entry; the budget is a
  product constant, not operator configuration, so an honest refusal cannot
  be silently raised to re-enable an abort path.

## Problem

The TypeScript preview adapter parses every discovered workspace
`.ts/.tsx/.js/.jsx` file in Phase 1 (`parse_error_reason`), not only files
in the diff. The oxc 0.130 parser is recursive-descent and bounds its
recursion only by the thread stack it runs on. On the main thread that is
1 MiB on Windows: balanced paren nesting of 350 parses cleanly and 400
aborts the process outright (`thread 'main' has overflowed its stack`,
exit 127, no report). Unary `!` / `await` chains and ternary chains abort
around n≈2000 on the same stack. Because Phase 1 parses every discovered
file, one minified/bundled/generated deeply-nested file anywhere outside the
ignore dirs kills every `ripr check` in the workspace — including Rust-only
diffs that merely have TypeScript enabled. An abort is worse than a panic:
there is no unwind, no report, and no disclosure.

Expected behavior is the existing named-limitation contract: a parse refusal
or a typed static-limit disclosure (`unsupported_syntax` vocabulary), never
a hard abort.

## Behavior

1. Pre-parse nesting budget. Before any oxc parse, a cheap single-pass byte
   scan estimates the deepest expression nesting of the source. The estimate
   sums the contributors that add recursive-descent frames in the oxc
   parser — bracket depth (`(`/`[`/`{`), open ternary chain, and
   prefix-operator runs (`!`/`~`, prefix `+`/`-`, prefix
   `await`/`typeof`/`void`/`delete`/`new`) — and reports their running
   maximum. Long binary/member chains are iterative in the parser and do not
   count. Comments, string/template literals, and regex literals are
   skipped. Sources whose estimate exceeds `PARSE_NESTING_BUDGET` (2,000)
   are refused with a typed `expression_nesting_budget` static-limit reason
   instead of being parsed; the refusal rides the existing
   `unsupported_syntax` finding and `language_scope_unsupported` limitation
   plumbing exactly like a parser error.
2. Large-stack parse worker. Sources under the budget parse on a dedicated
   worker thread with an explicit 256 MiB stack
   (`PARSE_WORKER_STACK_BYTES`), together with the AST walk that consumes
   the parse result, so legitimate deep files keep parsing and no oxc
   recursion depends on the caller's (often 1 MiB) stack. The reservation is
   address-space only; at most one worker is alive at a time because the
   adapter parses workspace files sequentially. A worker that cannot spawn
   or dies (for example an oxc panic) is disclosed as a typed
   `parse_worker_unavailable` / `parse_worker_failed` static-limit reason,
   never retried by re-parsing on the small stack.
3. Both oxc parse sites in the adapter (`parse_error_reason` and
   `extract_owners`) go through the guarded worker, so a changed file under
   the budget cannot abort during extraction either.
4. Budget placement. 2,000 sits above every historically aborting input
   (paren depth 400, chain length ≈2000 on the unguarded 1 MiB Windows
   debug stack) and far below the worker's own measured overflow depth
   (≥16,000 balanced paren levels on Windows debug, the worst frame sizes
   observed for this fix; release builds recurse further per stack byte), so
   the budget is a disclosed deterministic safety envelope, not the
   worker's limit.

## Required Evidence

- A red witness: the pre-fix binary aborts on the retained depth-400
  fixture (`thread 'main' has overflowed its stack`, exit 127, empty
  output), deterministic on Windows debug.
- The post-fix binary completes the same run and classifies an unchanged
  benign diff normally.
- A changed depth-400 file (previously aborting depth) parses and
  classifies normally with a real finding — deep-but-legitimate files keep
  working.
- A changed over-budget file emits the typed `expression_nesting_budget`
  static-limit reason through the `unsupported_syntax` finding and the
  `language_scope_unsupported` limitation, and the reason must not
  masquerade as a parser error.
- Unit tests pin the scan estimate (recursive shapes counted; string,
  comment, template, regex, and division chains ignored) and the budget
  boundary (budget+1 balanced source refused with the typed reason; budget
  balanced source parsed cleanly on the worker stack).

## Non-Goals

- No full lexer/regex-disambiguation rewrite of the scan: the
  regex-vs-division heuristic and ternary-colon tracking are lexical
  approximations and may misjudge pathological inputs; bracket terms keep
  those sources covered and the failure direction is a bounded
  estimate error, not an abort inside the budget.
- No coverage of unbounded type-position nesting (deeply nested generics
  do not count toward the estimate); catastrophic generated type nesting
  beyond the worker's own stack can still abort and remains an upstream
  oxc recursion property.
- No operator-configurable budget: the constant is part of the disclosed
  safety envelope; runtime knobs would let an operator re-enable an abort.
- No real mutation testing, no coverage dashboard, no promotion of static
  evidence to `killed`/`survived`: the refusal stays
  `StaticUnknown`/`unsupported_syntax`.

## Acceptance Examples

- Given a workspace whose discovered (unchanged) `deep.ts` nests 400
  balanced parens and a benign diff on another file, when `ripr check`
  runs, then the process completes (no abort) and the unchanged file is
  skipped from extraction exactly as any other unparseable file.
- Given a changed file at nesting depth 400, when `ripr check` runs, then
  the file parses and classifies normally (a real finding is produced).
- Given a changed file at nesting depth budget+1 (balanced — the oxc worker
  would parse it cleanly), when `ripr check` runs, then the emitted
  evidence carries `static_limit unsupported_syntax: static limit
  expression_nesting_budget: ...` and the limitations list carries the
  same typed reason with kind `language_scope_unsupported`.

## Test Mapping

- Unit (`parse_depth_tests` in `crates/ripr/src/analysis/language/
  typescript/parse.rs`): scan estimate shapes; lexical shells ignored;
  budget+1 typed refusal; budget admitted and parsed cleanly on the worker;
  depth-400 parse plus owner extraction; budget reason rendered through
  `unsupported_syntax_finding`.
- Fixture corpus (`fixtures/ts_parse_depth_budget/`): end-to-end `ripr
  check` over a workspace with a benign diff, a changed depth-400 file, and
  a changed over-budget file; `expected/check.json` pins the typed
  limitation and the surviving classification.

## Implementation Mapping

- `crates/ripr/src/analysis/language/typescript/parse.rs`:
  `PARSE_NESTING_BUDGET`, `PARSE_WORKER_STACK_BYTES`, `parse_on_worker`,
  `max_expression_nesting_estimate`, `parse_error_reason`.
- `crates/ripr/src/analysis/language/typescript/owners.rs`:
  `extract_owners` routes its parse and AST walk through `parse_on_worker`.
- `fixtures/ts_parse_depth_budget/`: fixture workspace and contract.

## Metrics

- `typescript_parse_aborts` — zero aborts: every discovered workspace file
  either parses, reports a parser error count, or carries one of the typed
  static-limit reasons (`expression_nesting_budget`,
  `parse_worker_unavailable`, `parse_worker_failed`).
- `typescript_parse_refusal_disclosure_parity` — a budget refusal is
  indistinguishable from a parser error in downstream contracts (same
  finding shape, same limitation kind) except for its named reason string.
