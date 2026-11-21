# RIPR-PROP-0019: Partial diff-scope analysis contract

Status: proposed

Owner: product / analysis

Created: 2026-07-21

Linked issues:

- #1918 — hard file/line limits erase all diagnostics on large diffs
- #1960 — reliability convergence program
- #1999 — implementation slice (PR B) that must follow this contract

Support-tier impact:

- None. Partial results are advisory only and may never feed a support-tier
  proof row, gate, baseline, badge, or RIPR Zero policy decision.

Policy impact:

- The two existing analysis-cost guards keep their current values, env
  overrides, and fail-closed behavior. The new partial-selection budget is an
  analysis-input budget (not a policy surface); no workflow, hook, or
  branch-protection change.

## Problem

`DIFF_INDEX_FILE_LIMIT = 800` and `DIFF_CHANGED_RUST_LINE_LIMIT = 2_000`
fail closed with `diff_scope_oversized` and zero findings. On a large PR the
LSP path shows all diagnostics vanish instead of the useful subset that fits
the budget. The current behavior is honest but useless at scale: it gives the
user nothing, when the honest thing is a bounded partial result that names
exactly what was and was not inspected.

This contract defines how RIPR may return partial analysis. It is the design
authority for #1999 (implementation) and for any future partition continuation.

## Required decisions

### 1. Selection unit

The selected unit is the **changed file, whole**. Changed lines within a
selected file are all analyzed. Files are never split: an owner's function is
the probe identity, and an owner can share a file with other owners, so
line-level or owner-level partitioning would break identity accounting and
create seams a reviewer cannot reason about. Packages are the priority tier,
not a unit: package order only decides which files are selected first.

### 2. Deterministic priority

Selection order is fully deterministic and content-independent:

1. files carrying changed lines in the supported language (Rust), then
   preview-language files carrying changed lines;
2. within a tier, package path ascending (lexicographic);
3. within a package, file path ascending;
4. ties impossible after (3).

Context-only files (no changed lines) are **not selected and not budgeted**:
they play their existing read-only context role and never consume the partial
budget, so the partition and the uninspected accounting cover changed-line
files only.

The order does not depend on diff ordering, filesystem enumeration order, mtimes,
sizes, or hashes of file content, so repeated runs over the same diff select the
same partition.

### 3. Analysis-cost versus delivery limits

Three distinct limits, never conflated:

- **Hard analysis-cost guards** (existing): `DIFF_INDEX_FILE_LIMIT` and
  `DIFF_CHANGED_RUST_LINE_LIMIT`, with their env overrides. Above these the
  run still fails closed with `diff_scope_oversized`. They protect runner
  memory; partial selection never exceeds them.
- **Partial-selection budget** (new): a smaller, separately named budget
  (`RIPR_PARTIAL_DIFF_FILE_BUDGET`, `RIPR_PARTIAL_DIFF_LINE_BUDGET`) spent in
  the priority order of decision 2. When the diff exceeds this budget, the run
  analyzes the selected partition and returns `limited_partial_scope` instead
  of erroring. Defaults are chosen inside the hard guards; operators raise them
  the same way they raise the hard guards.
- **Delivery limits** (existing): the LSP diagnostic delivery budget and
  output format bounds. These stay orthogonal; a partial result uses the same
  delivery path with its existing limits.

Budget validity and stop precedence are defined, never implicit:

- An empty, non-numeric, or overflowing budget override value is a parse
  failure and fails closed with a named `partial_budget_invalid` error,
  following the same env-parse contract as the existing guards; it never
  silently means unlimited or defaults to a hidden fallback.
- A zero or negative parsed value is likewise `partial_budget_invalid`.
- A partial budget may not exceed its corresponding hard guard; a larger
  override is clamped to the guard value and the clamp is disclosed.
- If the first selected file alone exceeds the line budget, that single file
  is analyzed anyway and the result is `limited_partial_scope` with stop
  reason `line_budget_exceeded_on_first_file` — never an empty partition.
- A later whole file that would exceed the remaining line budget is
  **excluded** (never included with overshoot); selection stops there with
  stop reason `line_budget`. The overshoot exception applies to the first
  selected file only, so an empty partition is impossible.
- If both budgets are reached by the same file, the stop reason is the file
  budget (`file_budget`), with the line count recorded alongside it.
- Precedence: the first-file line-overshoot exception wins over the
  simultaneous-hit rule. When the first selected file exceeds the line budget,
  the stop reason is always `line_budget_exceeded_on_first_file`, regardless
  of the file-budget state — the two rules can never both apply.

### 4. Representation of selected and uninspected scope

A partial result carries, in its existing limitation/output vocabulary:

```text
run state: limited_partial_scope
selection identity: see decision 7
selected files: exact paths, in selection order
selected changed-line count
uninspected files: lower-bound count (files with changed lines not selected)
uninspected changed-line lower-bound
stop reason: which budget bound was reached (file or line budget)
gate eligibility: ineligible
```

The uninspected counts are lower bounds derived from the diff, not estimates.
No field may imply the uninspected scope is clean, equivalent, or optional.

### 5. Gate, baseline, badge, and RIPR Zero eligibility

**Never.** A `limited_partial_scope` result is not a gate, baseline, badge, or
RIPR Zero input, and its result identity marks it `gate_eligibility:
ineligible` so a downstream consumer fails closed rather than treating a
partial denominator as complete. Gate surfaces must require a full-scope run
(or an explicit `diff_scope_oversized` error) and show the partial state as
blocking evidence, never as a passing denominator.

### 6. Requesting the next partition or a larger budget

For PR B (#1999), the only continuation is the explicit budget override:
`RIPR_PARTIAL_DIFF_FILE_BUDGET` / `RIPR_PARTIAL_DIFF_LINE_BUDGET`, raised like
the existing guards. The disclosure text names this route. Named partition
continuation (partition N+1 identity) is a deliberate non-goal of this
contract's first revision and must not be invented in implementation.

### 7. Run-comparable identity

A partial run is comparable to another partial run via a partition identity,
built from a canonical serialization (never a generic map serialization, whose
key order is not guaranteed):

```text
canonical form, one field per line, LF-separated, UTF-8:
  "selection_version="  "partial-diff-v1"
  "language_tier_version="  "lang-tier-v1"
  "diff_identity="      <existing diff identity string, exactly as the
                         full-scope run computes it>
  "file_budget="        <decimal>
  "line_budget="        <decimal>
  "selected="           <normalized forward-slash relative path> for every
                         selected file, sorted ascending, one per line

partition_identity = lowercase hex sha256 of the canonical form
```

Two runs with the same partition identity selected the same scope; a run with
a different budget, diff, or selection or language-tier version has a
different identity and may not be diffed, baselined, or compared against it.
`partial-diff-v1` bumps only on a contract revision of the selection
algorithm; `lang-tier-v1` bumps if language tiers are added or reordered.

## Safety posture (normative shape)

```text
oversized diff
→ deterministic bounded partition selection (decisions 1-2)
→ analyze only the selected scope (decision 3)
→ run state limited_partial_scope with exact selected paths,
  lower-bound uninspected counts, and stop reason (decision 4)
→ gate/baseline/badge/Zero ineligible (decision 5)
→ explicit budget override is the only continuation (decision 6)
→ partition identity makes runs comparable (decision 7)
```

## Non-goals

- Arbitrary "first N filesystem entries" selection.
- Owner-, hunk-, or line-level partitioning.
- Partition continuation tokens or next-partition requests.
- Any partial-derived support-tier, gate, baseline, badge, or Zero claim.
- Estimating the uninspected scope's findings.

## Implementation mapping (for #1999)

- Selection + budgets: `crates/ripr/src/analysis/language/rust.rs` beside the
  existing guards (shared helper, same env-override pattern).
- Result state and disclosure: the existing limitation/run-status vocabulary
  in `crates/ripr/src/output/` (no new free-form strings).
- Gate ineligibility marker: the gate-decision input validation path.
- Tests: deterministic order, both budget bounds, lower-bound counts, gate
  ineligibility, partition identity stability and discrimination.
