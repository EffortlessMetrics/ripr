# Report Packet Index Workflow

Use this workflow when generated CI has uploaded a RIPR report packet and a
reviewer, maintainer, developer, or coding agent needs to know where to start.

The report packet index is a read-only map over explicit existing artifacts:

```text
report, review, receipt, workflow, agent, pilot, and CI artifact directories
-> ripr reports index
-> target/ripr/reports/index.{json,md}
-> grouped packet map, missing surfaces, and regeneration commands
```

It does not rerun hidden analysis, inspect source to infer missing fields, edit
source, generate tests, call providers, run mutation testing, publish inline PR
comments, change recommendation ranking, change gate policy, or make CI
blocking by default.

## Start In GitHub

In generated CI, start with the `Report packet index` section of the RIPR
advisory summary when it exists. That section is the packet map, not the
pass/fail authority.

The compact summary should show:

- packet status: `pass`, `warn`, `fail`, or `incomplete`;
- total entries, available entries, missing expected surfaces, warnings, and
  failures;
- the `start_here` artifact, usually `pr-review-front-panel.md`;
- the `gate_authority` artifact when a gate decision exists;
- missing expected surface labels;
- warning kinds;
- the Markdown index body when `index.md` exists.

If the section is absent, generated CI did not find any indexed RIPR artifacts
to map. Generate a PR summary, front panel, gate decision, receipt, or other
report first, then rerun the index.

## Generate Or Refresh The Index

Run the public producer against explicit directories:

```bash
ripr reports index \
  --root . \
  --reports-dir target/ripr/reports \
  --review-dir target/ripr/review \
  --receipts-dir target/ripr/receipts \
  --workflow-dir target/ripr/workflow \
  --agent-dir target/ripr/agent \
  --pilot-dir target/ripr/pilot \
  --ci-dir target/ci \
  --out target/ripr/reports/index.json \
  --out-md target/ripr/reports/index.md
```

Repo-local automation may use the wrapper:

```bash
cargo xtask reports index
```

Pass only directories and artifacts you intend to inspect. The index should
show missing expected surfaces; it should not search the workspace, rerun
analysis, or synthesize missing report content.

## Read The Groups

The index groups artifacts by reviewer use instead of filename order.

| Group | Use it for |
| --- | --- |
| `start_here` | The first artifact a reviewer should open. |
| `pr_review_story` | PR guidance, first action, front-panel, and review-comment surfaces. |
| `repair_agent_handoff` | Assistant proof, health, workflow, agent packet, and repair handoff artifacts. |
| `evidence_movement` | Baseline delta, PR ledger, RIPR Zero status, receipts, and movement reports. |
| `policy_gates` | Gate decision, waiver, acknowledgement, suppression, baseline, and configured pass/fail context. |
| `calibration` | Recommendation, mutation, and coverage/grip calibration context. |
| `validation_receipts` | Local validation, PR readiness, dogfood, and check reports. |
| `sarif_badges` | SARIF/code scanning and badge outputs. |
| `local_context` | Operator or repo-local context that helps maintainers but is not the PR decision surface. |

Repo-local automation also projects repo-ops operating packets into
`repo_ops_packets[]` in `target/ripr/reports/index.json` and the Repo-Ops
Packets table in `target/ripr/reports/index.md`. These entries cover command
mutability, PR-ready, worktree doctor, PR triage, per-PR merge readiness,
generated-clean, badge ownership, command catalog coverage, critic, receipts,
suggested fixes, and `check-pr` artifacts.
They are advisory navigation metadata, not pass/fail authority.

Treat the group order as the review path:

```text
start_here
-> PR story
-> repair or evidence movement
-> policy and calibration context
-> validation receipts
```

Open lower-level artifacts only when the index points to them, a reviewer needs
provenance, or a coding agent needs JSON.

## Reviewer Workflow

Reviewers should use the index to avoid artifact archaeology:

1. Open the `start_here` artifact.
2. Check whether a `gate_authority` artifact exists.
3. Read missing expected surfaces and warnings before asking for a test.
4. Follow the linked PR review, first useful action, assistant proof, or gate
   report for the exact next action.
5. Ask for the focused repair or missing artifact named by the packet, not a
   broad "add more tests" request.

When the index status is `warn`, inspect `missing_expected[]` and `warnings[]`
before treating the packet as complete. A warning does not mean the PR failed;
it means the packet map found missing, stale, malformed, or incomplete context.

When the index status is `fail`, inspect the artifact marked as authority. A
blocked gate should point to `target/ripr/reports/gate-decision.md`; the index
only reports that the packet contains a failing or blocked surface.

## Maintainer Workflow

Maintainers use the index to judge packet health and adoption state:

- confirm the front panel or first useful action is the start-here artifact;
- confirm gate decisions stay visible and separate from advisory summaries;
- confirm baseline, acknowledgement, waiver, suppression, stale, warning, and
  missing-input states are not hidden;
- use missing-surface commands to repair incomplete generated CI packets;
- keep validation receipts available for review, but do not make them the user
  front door when a PR story artifact exists.

For adoption work, the index is the map over the packet. It is not the ledger,
baseline, gate, or calibration report itself. Use the grouped links to reach the
specific authority for those decisions.

## Developer Workflow

Developers should use the index to find the smallest next repair path:

- open the start-here report for the PR-local story;
- open repair and handoff artifacts only when a focused test or receipt is
  needed;
- run any `next_command` shown for a missing required surface;
- regenerate the index after producing new reports or receipts;
- keep missing, stale, unchanged, or warning states visible.

If a missing expected surface has a command, run that command before asking
whether the test evidence moved. If no command is known, use the owning
workflow doc for that report rather than inventing a new path.

## Coding Agent Workflow

The index can route external coding agents to one bounded packet:

```text
Open index.md.
Use the start_here artifact unless the index names a missing required surface.
If a missing surface has next_command, generate that artifact first.
If a repair handoff exists, target that seam and discriminator only.
Run the verify command from the linked proof or first-action artifact.
Return the receipt and refreshed index.
Stop.
```

A good handoff includes:

- `target/ripr/reports/index.md`;
- the selected start-here artifact;
- any missing expected surface and `next_command`;
- the gate-authority artifact when configured;
- the receipt path expected after repair;
- advisory limits.

Do not ask an agent to scan the whole repository, generate tests
automatically, call a provider from RIPR, or reinterpret gate policy from the
index.

## Missing Expected Surfaces

Missing expected surfaces are part of the product. They tell the reviewer which
piece of the packet was not produced.

Common examples:

| Missing surface | Usual repair |
| --- | --- |
| PR review front panel | Generate `pr-review-front-panel.{json,md}` from explicit PR guidance and proof inputs. |
| First useful action | Generate `first-useful-action.{json,md}` from existing PR guidance, ledger, proof, gate, and receipt inputs. |
| Assistant proof | Generate `test-oracle-assistant-proof.{json,md}` after a focused test or agent loop has evidence to join. |
| Assistant loop health | Run `ripr assistant-loop health` after a proof report exists. |
| Gate decision | Configure and run `ripr gate evaluate` only when an explicit gate mode is intended. |
| Baseline delta | Run `ripr baseline diff` only when a reviewed baseline and current gate decision exist. |
| Receipt | Run the verify and receipt command from the linked agent or proof packet. |

Do not mark a missing optional artifact as success by silence. If it matters to
the review, regenerate it. If it does not apply, leave the missing reason
visible.

## CI And Gate Boundary

Generated CI surfaces the report packet index as advisory summary and artifact
content. The index does not own pass/fail behavior.

Gate authority stays with the explicit gate decision:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

If `RIPR_GATE_MODE` is unset, generated CI remains advisory. If a gate mode is
configured, use the gate report to decide whether the PR is advisory,
acknowledged, blocked, or a configuration error. Use the index to find that
report and the surrounding evidence packet.

## Related Docs

- [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) explains
  the usual start-here PR story.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains the
  one-action route that may feed the packet.
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains focused-test proof and receipts.
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) explains
  proof completeness, warning groups, and repair queues.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains PR
  movement, waivers, baseline burn-down, and coverage/grip frontier signals.
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) explains baseline
  creation, diff, and shrink-only refresh.
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md) explains
  progress toward RIPR 0.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) explains configured gate
  modes and why generated CI is advisory by default.
- [CI strategy](CI.md#generated-github-workflow) describes generated workflow
  projection into GitHub summaries and artifacts.
- [Output schema](OUTPUT_SCHEMA.md#report-packet-index) defines the JSON and
  Markdown contract.
- [RIPR-SPEC-0024](specs/RIPR-SPEC-0024-report-packet-index.md) records the
  report packet index contract and non-goals.
