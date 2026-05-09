# Test-Oracle Assistant Workflow

Use this workflow when a PR, editor diagnostic, or coding-agent handoff points
to a weak or missing test oracle and you want one focused test plus a receipt.

The loop is intentionally narrow:

```text
changed behavior
-> static RIPR evidence
-> PR or editor recommendation
-> bounded handoff
-> one focused test
-> after evidence
-> receipt
-> advisory PR / CI projection
```

RIPR does not edit source, generate tests, call an LLM provider, run mutation
testing, or make CI blocking by default. It gives static evidence and the
commands needed to verify whether the evidence moved after a focused test.

## Start From The PR

In GitHub, start with the RIPR advisory summary, check annotations, or
`target/ripr/review/comments.md`.

Look for one actionable recommendation:

- file and line for the changed seam;
- grip class such as `weakly_gripped` or `reachable_unrevealed`;
- missing discriminator or missing observation;
- suggested focused test shape;
- best related test when available;
- verify command or agent handoff command;
- placement status, either changed-line annotation or summary-only guidance.

If the recommendation is summary-only, keep it visible but do not force it onto
an unrelated diff line. Bad placement is worse than no inline placement.

## Start From The Editor

In VS Code, use the saved-workspace editor loop:

```text
diagnostic -> hover -> code action -> handoff packet
```

Hover the diagnostic before acting. The hover should name the seam evidence,
missing discriminator, related test, suggested assertion or test shape, packet
commands, verify command, receipt command, and static limits.

Use the editor action that matches the available evidence:

- open the best related test;
- copy the suggested assertion;
- copy a targeted test brief;
- copy the agent packet;
- copy the after-snapshot, verify, and receipt commands.

If an action is missing, the current saved-workspace evidence does not support
that handoff yet. Refresh analysis or use the PR summary instead of inventing a
broader task.

## Hand Off One Bounded Task

For a human, reviewer comment, or external coding agent, keep the handoff to
one seam:

```text
Write one focused test for this seam.
Do not edit production code unless the PR scope already requires it.
Use the related test or assertion shape when provided.
Run the after-snapshot and verify commands.
Return the receipt.
Stop.
```

A good handoff includes:

- seam ID;
- file and line;
- missing discriminator;
- suggested focused test shape;
- best related test;
- after-snapshot command;
- verify command;
- receipt command.

Example:

```text
Seam: fixtures/boundary_gap/input/src/lib.rs:2
Missing discriminator: discount_threshold (equality boundary)
Suggested test: discounted_total_boundary_discriminator
Related test: tests/pricing.rs::below_threshold_has_no_discount
```

## Write One Focused Test

Write the test outside RIPR. Target the missing discriminator or observation
named by the recommendation.

Prefer:

- one changed behavior;
- one missing discriminator;
- one related test style to imitate;
- exact assertions for the changed behavior;
- no unrelated refactors.

Do not treat a focused test as failed just because the static class does not
improve. The receipt will record `improved`, `resolved`, `unchanged`,
`regressed`, or `unknown` static movement.

## Capture After Evidence

After saving the focused test, run the after-snapshot command from the PR
guidance, editor action, or agent packet. The exact command depends on the
surface that produced the handoff, but it should write a comparable after
artifact such as:

```text
target/ripr/workflow/after.repo-exposure.json
```

Do not hand-edit artifact paths unless you intentionally changed the workspace
root or output directory.

## Verify Static Movement

Run the verify command from the handoff packet, for example:

```bash
ripr agent verify \
  --root . \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json \
  --json
```

The verify step compares static RIPR evidence. It does not run tests, run
mutation testing, or prove runtime adequacy.

Read movement this way:

| State | Meaning |
| --- | --- |
| `improved` | Static evidence got stronger for the selected seam. |
| `resolved` | The selected visible gap no longer appears under current evidence. |
| `unchanged` | Static evidence did not move; inspect the test, artifact freshness, or analyzer limits. |
| `regressed` | Static evidence got worse. Treat this as review evidence, not as a runtime mutation result. |
| `unknown` | Required before or after evidence is missing or not comparable. |

## Emit The Receipt

Run the receipt command from the handoff packet or editor action:

```bash
ripr agent receipt \
  --root . \
  --verify-json target/ripr/workflow/agent-verify.json \
  --seam-id <seam-id> \
  --json
```

The receipt is the durable review trail. It records:

- selected seam;
- before and after artifact identity;
- static movement;
- warnings and stale-looking inputs;
- next-action guidance.

When the receipt is absent, do not infer improvement from the test diff alone.

## Read PR / CI Projection

Generated CI keeps the loop advisory by default. The useful first-screen
surfaces are:

- RIPR advisory summary;
- changed-line-safe check annotations;
- `target/ripr/review/comments.md`;
- `target/ripr/reports/pr-evidence-ledger.md`;
- optional `target/ripr/reports/gate-decision.md`;
- optional `target/ripr/reports/coverage-grip-frontier.md`;
- receipt paths from `target/ripr/workflow/` or `target/ripr/reports/`.

The PR evidence ledger answers:

```text
Did this PR move behavioral test grip toward or away from RIPR 0?
```

Gate decisions, when explicitly configured, remain the pass/fail authority.
The assistant loop and ledger are evidence surfaces; they do not make CI
blocking by themselves.

## Coverage And Mutation Boundaries

Coverage and RIPR evidence answer different questions.

Coverage asks:

```text
Did this line or region execute?
```

RIPR asks:

```text
Would the current tests appear to notice this behavior changing?
```

It is valid for coverage to stay flat while RIPR evidence improves. It is also
valid for coverage to increase while a missing discriminator remains visible.

Do not use `killed`, `survived`, or runtime adequacy language unless real
mutation data was imported through a calibration artifact. Without imported
runtime calibration, this workflow is static RIPR evidence only.

## Dogfood Receipt

The current repo-local proof receipt is:

```text
docs/handoffs/2026-05-09-test-oracle-assistant-receipt.md
```

It traces seam `67fc764ba37d77bd` through PR guidance, editor/agent handoff,
before/after evidence, receipt, PR evidence ledger projection, and
coverage/grip frontier availability.

That receipt intentionally records current analyzer movement as:

```text
weakly_gripped -> weakly_gripped
state: unchanged
```

The unchanged movement is useful evidence. It proves the loop preserves the
static result honestly instead of claiming that a focused test improved RIPR
when the current classifier did not promote the seam.

## Limits

- Static RIPR evidence only.
- Advisory by default.
- No source edits by RIPR.
- No generated tests.
- No provider calls.
- No mutation execution.
- No runtime adequacy claims from static evidence.
- Optional gates remain separate from the proof loop.

## Related Docs

- [PR review guidance](PR_REVIEW_GUIDANCE.md) explains changed-line-safe PR
  recommendations and summary-only fallback.
- [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md) explains the
  saved-workspace diagnostic, hover, action, verify, and receipt loop.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains PR
  movement, waivers, baseline burn-down, repair receipts, and coverage/grip
  frontier signals.
- [LLM operator guide](LLM_OPERATOR_GUIDE.md) explains source-edit-free agent
  operation.
- [RIPR-SPEC-0019](specs/RIPR-SPEC-0019-test-oracle-assistant-loop.md) defines
  the assistant proof contract.
