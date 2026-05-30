# First Successful PR Workflow

Use this when a team wants to try `ripr` on one real pull request and decide
whether the recommendation is useful enough to adopt. Stable Rust gaps are the
primary path; preview Python gaps can use the same workflow when an explicit
gap ledger already supplies advisory repair records.

The success condition is intentionally small:

```text
run ripr
-> read one repairable stable Rust gap or preview Python gap
-> add one focused test or output proof outside ripr
-> verify static movement
-> keep the receipt
```

This workflow is advisory. `ripr` does not edit source, generate tests, run
mutation testing, call providers, or make merge decisions by default.

## 1. Pick One PR

Start with a normal PR where a reviewer can understand the intended behavior
change. Avoid the first run on:

- mechanical formatting-only changes;
- broad refactors with many unrelated seams;
- generated code;
- changes that require runtime mutation calibration to interpret.

The first successful PR should answer one reviewer question:

```text
Does the changed behavior have a meaningful test discriminator or checked
output proof?
```

## 2. Run The Pilot

From the PR checkout:

```bash
ripr pilot --root .
```

Read:

```text
target/ripr/pilot/pilot-summary.md
```

The pilot summary is the first screen. It should name the top actionable gap,
why it matters, the related test to inspect when available, and the command to
capture after evidence.

If the pilot reports `partial`, use the retry command it prints. Do not guess
at cache or timeout settings.

## 3. Prefer A Gap Record When Available

When a gap decision ledger already exists, use it as the repair source:

```text
target/ripr/reports/gap-decision-ledger.json
target/ripr/reports/gap-decision-ledger.md
```

Gap records are the shared vocabulary behind PR repair cards, first-action
reports, agent packets, optional gates, and repo badge targets. A useful first
PR gap record should name:

- the gap kind;
- the scope;
- the repair route;
- the anchor;
- the verification command;
- whether it is eligible for PR comments, agent packets, gates, or badges.

If you only have repo exposure evidence, derive the conservative ledger:

```bash
ripr reports gap-ledger \
  --repo-exposure target/ripr/pilot/repo-exposure.json \
  --out target/ripr/reports/gap-decision-ledger.json \
  --out-md target/ripr/reports/gap-decision-ledger.md
```

For presentation/output-text changes or Python preview repair-card findings,
derive the PR-local route from the checked JSON output:

```bash
ripr check --root . --format json > target/ripr/reports/check.json
ripr reports gap-ledger \
  --check-output target/ripr/reports/check.json \
  --out target/ripr/reports/gap-decision-ledger.json \
  --out-md target/ripr/reports/gap-decision-ledger.md
```

That path should produce `MissingOutputContract` with an `AddOutputGolden`
repair route when user-facing output changed without checked output evidence,
or a preview Python repair record with a verify command and check-output
receipt command when the Python card is actionable. It should not turn generic
`static_unknown` into an interruption.

## 4. Pick One Repairable Gap

Choose one actionable item. Prefer a gap that names a concrete repair:

- missing equality-boundary assertion;
- missing exact error variant assertion;
- missing exact return value assertion;
- missing field, object, side-effect, or mock expectation;
- missing checked output or golden fixture.

Skip report-only static limitations for the first PR unless the task is to
inspect an opaque helper, fixture, macro, or dynamic boundary.

## 5. Copy The Work Packet

For a gap-ledger-backed task, create the focused agent packet:

```bash
ripr agent packet \
  --root . \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --gap-id <gap_id> \
  --json > target/ripr/agent/gap-packet.json
```

For older seam-backed flows, use the pilot packet or start a seam workflow:

```bash
ripr agent start --root . --seam-id <seam_id> --out target/ripr/workflow
```

Give a coding agent the bounded packet, not a broad instruction. It should know
the owner, changed behavior, related test, missing discriminator or output
proof, repair route, stop conditions, and verification command.
When the packet comes from the gap ledger, `llm_guidance.copyable_packet`
contains a pasteable Markdown work order with Task, Context, Repair,
Verification, Stop Conditions, and Do Not Do sections.

## 6. Add One Focused Proof

Write the test or output fixture outside `ripr`. Keep the change narrow:

- imitate the best related test when supplied;
- exercise the missing value, branch, variant, field, object, side effect, or
  output text;
- assert the behavior that would fail if the changed code were wrong;
- add or update the output/golden fixture when the repair route is
  `AddOutputGolden`;
- avoid unrelated refactors and production changes;
- avoid smoke-only assertions when `ripr` asked for a stronger discriminator.

Run the project tests or golden checks that normally validate the PR. Static
movement is not a replacement for the test suite.

## 7. Verify Movement

Capture the after snapshot with the command from the pilot, first-action report,
or agent packet. The common shape is:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json
```

Then compare before and after:

```bash
ripr outcome \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json
```

Read the result conservatively:

| Movement | Meaning |
| --- | --- |
| `improved` | Static evidence got stronger for the selected behavior. |
| `resolved` | The visible gap no longer appears under current evidence. |
| `unchanged` | The test may be misplaced, too broad, stale, or beyond current static limits. |
| `regressed` | Static evidence got weaker; inspect before continuing. |
| `unknown` | Required before or after evidence is missing or not comparable. |

For output-contract repairs, run the verification command from the gap record,
usually:

```bash
cargo xtask goldens check
```

## 8. Keep A Receipt

For a human-only pilot, attach the `ripr outcome` Markdown to the PR or upload
it with the CI artifact packet.

For an agent or repeatable workflow, produce the focused receipt:

```bash
ripr agent verify \
  --root . \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json \
  --json > target/ripr/agent/agent-verify.json

ripr agent receipt \
  --root . \
  --verify-json target/ripr/agent/agent-verify.json \
  --seam-id <seam_id> \
  --json \
  --out target/ripr/agent/agent-receipt.json
```

The receipt is the review trail. Without a receipt, do not infer improvement
from the test diff alone.

If you are working from VS Code, run `ripr: Show Status` after the receipt and
refresh. The editor can point to the validated first-pr packet, open the
Markdown packet, and copy the bounded summary or repair packet when the packet
matches the current workspace and diagnostic identity.

## 9. Add Advisory CI After One Manual Win

After one PR has a useful before/after receipt, add generated advisory CI:

```bash
ripr init --ci github
```

The generated workflow is advisory by default. It uploads pilot, agent, report,
workflow, and review artifacts; writes a PR summary; and keeps gate authority
separate. Do not make it blocking until the repository has reviewed its first
advisory baseline and explicitly opted into policy gates.

## What Success Looks Like

A successful first PR leaves this trail:

```text
pilot-summary.md
gap-decision-ledger.md, when available
one focused test or output fixture
after.repo-exposure.json
ripr outcome Markdown
optional agent-verify.json
optional agent-receipt.json
```

The reviewer should be able to say:

```text
ripr found one repairable stable Rust gap or preview Python gap.
We added one focused proof for that behavior.
The static evidence improved or resolved, or the checked output proof now exists.
The result is advisory, and runtime mutation testing remains optional follow-up.
```

## Surface Ownership

The first-run loop composes existing surfaces. Do not add a new artifact when
one of these already owns the job:

| Surface | Opens with | Owns | Does not own |
| --- | --- | --- | --- |
| First-run packet | `target/ripr/reports/start-here.md` | Top repairable stable Rust gap, preview Python gap, or no-action state, repair route, verify command, artifact links, advisory boundary. | Analyzer truth, gate authority, PR comments, source edits, generated tests. |
| First successful PR workflow | This document | Manual adoption path from one PR to one repair receipt. | Output schema contracts or editor behavior. |
| Quickstart | [Quickstart](QUICKSTART.md) | First-hour path selection across CLI, PR, editor, and agent use. | Full report topology. |
| Generated CI | [CI strategy](CI.md) | Advisory PR summary, artifact upload, start-here projection, optional gate artifact links. | Pass/fail authority unless an explicit gate-decision artifact owns it. |
| PR repair comments | [PR review guidance](PR_REVIEW_GUIDANCE.md) and [PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md) | Bounded repair cards from existing review-comment artifacts when explicitly configured. | Free-form review, branch protection, default blocking. |
| Editor handoff | [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md) | Read-only projection of existing first-pr packets into saved-workspace editor status and actions. | Producing first-pr packets or PR/CI artifacts. |
| Agent packet | [LLM operator guide](LLM_OPERATOR_GUIDE.md) | Bounded work order with task, context, repair, verification, stop conditions, and non-goals. | Provider calls, generated tests, or source edits. |
| Badge endpoints | [Badge policy](BADGE_POLICY.md) and [verification contracts](verification/) | Repo-scoped public trust markers. | PR-local test adequacy or runtime mutation proof. |
| Gate decision | [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) and [blocking readiness](BLOCKING_READINESS.md) | Explicit pass/fail authority when a repository opts into a gate mode. | Start-here summaries, comments, or badges. |

Implementation and cleanup follow-up lives in
[`plans/adoption-integration-cleanup/`](../plans/adoption-integration-cleanup/).

## Related Docs

- [Quickstart](QUICKSTART.md) covers first-hour paths for CLI, CI, editor, and
  agent users.
- [First successful PR demo](demo/first-successful-pr.md) shows the checked
  boundary-gap, output-contract, preview Python, no-action, and blocked
  fixture cases.
- [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md)
  explains the VS Code handoff from local receipt to `start-here` packet.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains the
  artifact router that can pick the next bounded action.
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) is the deeper
  before/after evidence loop.
- [PR review guidance](PR_REVIEW_GUIDANCE.md) explains repair-card comments and
  summary-only fallback.
- [Support tiers](status/SUPPORT_TIERS.md) explains current maturity,
  preview, blocked, and advisory boundaries.
