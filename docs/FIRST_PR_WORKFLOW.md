# First Successful PR Workflow

Use this when a team wants to try `ripr` on one real Rust pull request and
decide whether the recommendation is useful enough to adopt.

The success condition is intentionally small:

```text
run ripr
-> read one repairable Rust gap
-> add one focused test or output proof outside ripr
-> verify static movement
-> keep the receipt
```

This workflow is advisory. `ripr` does not edit source, generate tests, run
mutation testing, call providers, or make merge decisions by default.

## 1. Pick One PR

Start with a normal Rust PR where a reviewer can understand the intended
behavior change. Avoid the first run on:

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

For presentation or output-text changes, derive the output-contract route from
the checked JSON output:

```bash
ripr check --root . --format json > target/ripr/reports/check.json
ripr reports gap-ledger \
  --check-output target/ripr/reports/check.json \
  --out target/ripr/reports/gap-decision-ledger.json \
  --out-md target/ripr/reports/gap-decision-ledger.md
```

That path should produce `MissingOutputContract` with an `AddOutputGolden`
repair route when user-facing output changed without checked output evidence.
It should not turn generic `static_unknown` into an interruption.

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
ripr found one repairable Rust gap.
We added one focused proof for that behavior.
The static evidence improved or resolved, or the checked output proof now exists.
The result is advisory, and runtime mutation testing remains optional follow-up.
```

## Related Docs

- [Quickstart](QUICKSTART.md) covers first-hour paths for CLI, CI, editor, and
  agent users.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains the
  artifact router that can pick the next bounded action.
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) is the deeper
  before/after evidence loop.
- [PR review guidance](PR_REVIEW_GUIDANCE.md) explains repair-card comments and
  summary-only fallback.
- [Support tiers](status/SUPPORT_TIERS.md) explains current maturity,
  preview, blocked, and advisory boundaries.
