# Agent Dispatch Workflow

This document describes the practical loop for closing test-grip gaps
that `ripr` surfaces. It is a playbook, not a manifesto: one developer
or one coding agent should be able to read it once and use it.

`ripr` is a **static test-evidence audit for Rust behavior**. It does
not run mutants, claim coverage adequacy, or generate tests. What it
does is name the missing discriminator: the value or oracle a test
must produce to grip a behavioral seam.

For the operating contract between the owner, planner, and executor
that builds `ripr` itself, see
[`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).
This document is one level lower: how a developer or coding agent
*uses* `ripr` to close a specific seam gap.

## What `ripr` produces and does not produce

| Produces | Does not produce |
|---|---|
| Inventory of behavioral seams | Mutation-test pass/fail outcomes |
| Per-seam test grip evidence (reach / activate / propagate / observe / discriminate) | A "this is covered" claim |
| `SeamGripClass` (e.g. `weakly_gripped`, `ungripped`) | Auto-generated tests |
| Missing discriminator hypotheses | Patches or edits to your code |
| Suggested assertion templates with placeholders | Filled-in expected values |
| Repo exposure report (JSON + Markdown) | Whole-program semantic proof |
| Agent-ready seam packets (`write_targeted_test` work orders) | A score, grade, or "quality number" |
| LSP diagnostics + hover with evidence path | Adequacy / proof claims |

The vocabulary boundary matters. Static output uses *behavioral seam*,
*test grip*, *missing discriminator*, *static evidence*, *runtime
confirmation*. It does not use *coverage gap*, *untested*, *adequate*,
*proven*, *killed*, or *survived* — those belong to runtime mutation
testing, which is a separate calibration step.

## The loop

```text
1. Run ripr.
2. Inspect the repo exposure report or editor diagnostic.
3. Read the seam evidence hover.
4. Copy the targeted test brief or seam packet for the gap you want to close.
5. Hand the packet to a coding agent (or to yourself).
6. Agent writes the targeted test.
7. Rerun ripr and generate a targeted-test outcome receipt.
8. Optionally align SARIF, badge, and cargo-mutants calibration artifacts.
```

Each step has a concrete `ripr` command or LSP capability behind it.

### 1. Run `ripr`

Two entry points:

- **Diff scope** (default for PRs):
  ```bash
  cargo run -p ripr -- check --diff path/to/diff.patch
  ```
  Surfaces seams touched by the diff and their grip evidence. Fast
  enough for draft-PR feedback.
- **Repo scope** (full inventory):
  ```bash
  cargo xtask repo-exposure-report
  cargo xtask agent-seam-packets .
  ```
  Walks every production Rust file, classifies every behavioral seam,
  and writes `target/ripr/reports/repo-exposure.{json,md}` plus
  `target/ripr/reports/agent-seam-packets.json`.

The repo-scope report is multi-second on large workspaces today.
`cache/repo-seam-facts-v1` will make it cheap enough to run on every
keystroke; until then, treat it as a checkpoint pass, not a
hot-path command.

If you want the before/after receipt, preserve the before JSON before
editing tests:

```bash
mkdir -p target/ripr/workflow
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json
```

### 2. Inspect the report

Open `target/ripr/reports/repo-exposure.md`. The summary table shows
counts per `SeamGripClass`. The Top gaps section lists the
headline-eligible seams sorted by file and line, capped at 50 entries.
For the full set use `repo-exposure.json`.

For diff-scope output, the same evidence is in the JSON output of
`ripr check`.

### 3. Read the seam evidence hover

When `seamDiagnostics: true` is set in LSP initialization options, the
editor publishes a `Diagnostic` for every actionable seam with a
stable `ripr-seam-{class}` code. Hovering over the diagnostic renders
the full RIPR evidence path:

```text
Behavioral seam
  amount >= discount_threshold

Grip
  weakly_gripped

Evidence
  reach yes: Related tests appear to reach `pricing::discounted_total`
  activation yes: Observed 2 concrete activation values for seam
  propagation yes: Static propagation to `return_value` sink is yes
  observation yes: Observation evidence is `yes`
  discrimination weak: Strongest oracle for seam kind `predicate_boundary`
                       is `strong` (kind-match true)

Observed values
- `50`
- `10000`

Missing discriminator
- `discount_threshold (equality boundary)` — observed values do not
  include the equality-boundary case for this predicate

Related tests
- `below_threshold_has_no_discount` — exact_value / strong

Next step
Add an exact-value assertion for the equality boundary.
```

The hover never parses the diagnostic message — it looks up the
`ClassifiedSeam` by `data.seam_id`.

### 4. Copy the seam packet

Two ways to get the packet for a specific seam:

- From the bulk artifact:
  `target/ripr/reports/agent-seam-packets.json` contains one packet
  per actionable seam. Find the one matching your `seam_id`.
- From the editor: `Copy seam packet` copies the server-owned JSON packet,
  while `Copy targeted test brief` copies a plain-language work order for
  the seam under the cursor. `Copy suggested assertion` and `Open best
  related test` appear when the analysis snapshot has those fields.

For the end-to-end human path, see
[Targeted test workflow](TARGETED_TEST_WORKFLOW.md).

A packet looks like this:

```json
{
  "task": "write_targeted_test",
  "seam_id": "f3c9e4d21a0b7c88",
  "owner": "src/pricing.rs::discounted_total",
  "seam_kind": "predicate_boundary",
  "file": "src/pricing.rs",
  "line": 88,
  "changed_expression": "amount >= discount_threshold",
  "current_grip": "weakly_gripped",
  "headline_eligible": true,
  "evidence": {
    "reach": "yes",
    "activate": "yes",
    "propagate": "yes",
    "observe": "yes",
    "discriminate": "weak"
  },
  "observed_values": ["50", "10000"],
  "missing_discriminators": [
    {
      "value": "discount_threshold (equality boundary)",
      "reason": "observed values do not include the equality-boundary case for this predicate"
    }
  ],
  "missing_oracle_shape": "exact returned value assertion at the equality boundary",
  "related_existing_tests": [
    {
      "name": "below_threshold_has_no_discount",
      "file": "tests/pricing.rs",
      "line": 12,
      "oracle_kind": "exact_value",
      "oracle_strength": "strong",
      "evidence_summary": "exact value assertion"
    }
  ],
  "suggested_assertions": [
    "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
  ],
  "runtime_confirmation": "optional cargo-mutants confirmation; ripr reports static evidence only"
}
```

The packet is the agent's work order. It names the seam, the missing
input, the oracle shape, and a template assertion — the placeholders
are intentional, since `ripr` does not invent expected values.

### 5. Hand the packet to a coding agent

Both `task: "write_targeted_test"` (headline-eligible classes) and
`task: "inspect_static_limitation"` (`opaque`) are valid. The agent
should:

- read `current_grip`, `evidence`, `missing_discriminators` to
  understand what the existing tests do *not* observe;
- read `related_existing_tests` to imitate the project's oracle style
  rather than inventing one;
- fill the `/* expected */` placeholders in `suggested_assertions`
  with values derived from the production code or its spec;
- avoid duplicating an assertion that the related tests already make.

For `inspect_static_limitation` packets, the agent's job is not to
write a test — it is to surface the helper, macro, or fixture
boundary that hides the seam from static analysis, so a human can
decide whether to refactor the indirection or leave the seam opaque.

### 6. Agent writes the targeted test

The agent commits the new test in the repo's normal test layout. No
auto-edits, no CodeLens; just a pull request that adds the
test.

### 7. Rerun `ripr` and write the receipt

After the test lands locally:

```bash
cargo run -p ripr -- check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json
cargo xtask targeted-test-outcome \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json
```

The receipt at `target/ripr/reports/targeted-test-outcome.md` shows whether
the matched seam moved, stayed unchanged, regressed, appeared, or disappeared.
The cleanest result is a matched seam moving toward `strongly_gripped` with
evidence deltas such as a missing discriminator no longer reported or a
stronger related oracle becoming visible.

### 8. Optional: align SARIF, badges, and cargo-mutants calibration

`ripr` makes no mutation-runtime claim. Use SARIF, badge, and calibration
reports only when those surfaces matter for the review:

```bash
cargo run -p ripr -- check --root . --mode ready --format repo-sarif > target/ripr/workflow/after.repo-sarif.json
cargo xtask sarif-policy --current target/ripr/workflow/after.repo-sarif.json --mode advisory
cargo xtask repo-badge-artifacts
cargo xtask mutation-calibration . \
  --mutants-json target/ripr/workflow/cargo-mutants.json \
  --repo-exposure-json target/ripr/workflow/after.repo-exposure.json
```

Runtime mutation vocabulary stays in the calibration report. The normal
targeted-test receipt remains a static evidence movement receipt.

## Examples by seam kind

### Predicate boundary

```text
seam: amount >= discount_threshold
grip: weakly_gripped
observed: amount = 50, amount = 10000
missing: discount_threshold (equality boundary)
agent action:
  add `assert_eq!(discounted_total(100, 100), <expected>)` to exercise
  the equality case
```

### Error variant

```text
seam: Err(AuthError::RevokedToken)
grip: weakly_gripped
related test: parse_rejects_empty — broad_error / weak (only is_err())
missing oracle shape:
  exact error-variant assertion (matches! / assert_matches!)
agent action:
  replace `assert!(parse("").is_err())` with
  `assert!(matches!(parse(""), Err(AuthError::RevokedToken)))`
```

### Return value

```text
seam: tail expression `amount + fee`
grip: weakly_gripped
related test: total_runs — smoke_only / smoke (.unwrap() only)
missing oracle shape: exact-value assertion
agent action:
  add `assert_eq!(total(100, 5), 105)` rather than `let _ = total(100, 5);`
```

### Field construction

```text
seam: Quote { amount: amount, fee: fee }
grip: weakly_gripped
related test: builds_quote — calls but does not assert on .amount
missing oracle shape: field equality or whole-object assertion
agent action:
  add `assert_eq!(build_quote(10, 2).amount, 10);` or use
  `assert_eq!(build_quote(10, 2), Quote { amount: 10, fee: 2 });`
```

### Side effect

```text
seam: service.publish(event)
grip: weakly_gripped
related test: publish_runs_without_panic — no observer
missing oracle shape:
  mock expectation, event/state observer, or persistence assertion
agent action:
  arrange a mock service or event spy and assert
  `service.publish` was called with the expected event
```

### Opaque evidence

```text
seam: helper-derived predicate
grip: opaque
diagnostic task: inspect_static_limitation
agent action:
  do not add a test. Identify the helper or fixture that hides the
  activation values. Decide whether to inline / refactor for
  visibility, or accept the opacity and document the intent.
```

### Declared intent

When a test is deliberately a smoke test (e.g. an integration probe),
declaring intent prevents `ripr` from emitting an actionable packet:

- record the intent in `.ripr/intents.toml` (declared-intent file path
  is documented in `docs/specs/RIPR-SPEC-0005-repo-seam-inventory.md`);
- the seam classifies as `intentional` and is visible in the repo
  exposure report but produces no agent packet.

`Intentional` is governance, not a mistake. Use it when the smoke
shape is the deliberate test design.

### Reasoned suppression

When a finding is accepted technical debt:

- add a `.ripr/suppressions.toml` entry with kind, owner, reason, and
  optional expiry;
- the seam classifies as `suppressed` and is visible but not
  headline-eligible.

`Suppressed` is also governance. It is not the absence of a problem —
it is the recorded acceptance of one.

### Runtime confirmation

After enough seams reach `strongly_gripped`, run `cargo-mutants`
against the affected modules. Use the `calibration/cargo-mutants-v1`
output (when it lands) to compare static `SeamGripClass` to runtime
mutant outcomes. Calibration tightens the per-repo oracle priors;
static reports do not adopt mutation-runtime vocabulary.

## What to push back on

- **"Add more tests."** That is not what `ripr` says. The packet
  names a specific missing discriminator. A duplicate test that
  doesn't exercise the missing input does not change the grip class.
- **"Coverage is fine."** Coverage and grip are different
  questions. `ripr` answers "do tests appear to produce evidence
  that would notice this behavior changing?" — coverage answers
  "did the line execute?".
- **"This is proven."** `ripr` is preflight static evidence. Real
  proof requires runtime mutation testing or formal methods, neither
  of which `ripr` claims to perform.

## Surfaces this document expects to exist

- `cargo xtask repo-exposure-report` (PR #239)
- `cargo xtask agent-seam-packets` (PR #240)
- LSP `seamDiagnostics: true` opt-in (PR #241)
- LSP seam evidence hover (PR #242)
- `inventory_classified_seams_at` plus `ClassifiedSeam` API (PR #237)

The only piece of the practical loop that does not yet exist is the
**copy seam packet** code action in the editor, and a
`cargo-mutants` calibration import. Both are deferred to follow-up
work items (`cache/repo-seam-facts-v1`, `calibration/cargo-mutants-v1`).
