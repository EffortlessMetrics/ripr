# Editor Actionable Gap Queue

Use this guide when RIPR is already installed in VS Code and you want the
editor to answer:

```text
What is safe to work on now?
```

The queue path is deliberately local and read-only:

```text
Diagnose Setup -> Show Status -> Current Repair Queue
-> Copy Current Repair Packet or Copy Repo Gap Map
-> open related test -> verify -> receipt -> refresh -> next gap or no-action
```

The editor projects existing typed artifacts. It does not create analyzer
truth, publish PR comments, compose CI summaries, decide policy, edit source,
generate tests, call providers, run mutation testing, or claim runtime
adequacy.

## What The Queue Reads

The primary queue input is:

```text
target/ripr/reports/actionable-gaps.json
```

The editor may also project related saved-workspace state from:

```text
target/ripr/reports/actionable-gaps.md
target/ripr/reports/start-here.json
target/ripr/first-pr/start-here.json
target/ripr/reports/first-useful-action.json
target/ripr/agent/agent-receipt.json
target/ripr/reports/gap-decision-ledger.json
```

Only typed JSON fields decide action safety. Markdown explains the evidence for
humans, but the editor does not parse Markdown prose to decide whether a repair
packet is safe.

Important typed fields include:

| Field | Why It Matters |
| --- | --- |
| `canonical_gap_id` | Stable identity for the gap, receipt, and first-pr handoff. |
| `language` and `language_status` | Rust default/stable versus preview/advisory boundaries. |
| `gap_state` | Whether the item is actionable, report-only, no-action, or blocked. |
| `repair_kind` and `repair_route_source` | The bounded repair family and the typed source used to project it. |
| `target_test_type` and `related_test_or_observer` | The likely test or observer to inspect. |
| `assertion_shape` | The expected assertion or output proof shape. |
| `static_limit_kind` | Why syntax-first evidence cannot safely classify further. |
| `verify_command` | Command that checks static movement. |
| `receipt_command_or_path` | Command or path that records the movement receipt. |
| `receipt_movement` | Improved, unchanged, stale, missing, or mismatched movement state. |
| `confidence_basis` | Why the queue item is trusted enough to show or blocked. |
| top-level `root` and freshness fields | Whether the artifact belongs to the open workspace and is current. |

If a field required for safe action is missing or unsafe, the editor fails
closed.

## Show Status

Run:

```text
ripr: Show Status
```

When `actionable-gaps.json` is safe, Show Status summarizes the current queue:

| Status Field | Meaning |
| --- | --- |
| Actionable gaps | Count of repair-ready queue items. |
| Top repair | The highest-priority upstream queue item. |
| Related test | Workspace-local test or observer, when available. |
| Verify | The safe verification command. |
| Receipt | Missing, improved, unchanged, stale, or mismatched movement. |
| Report-only gaps | Evidence that is useful to read but not safe to turn into a repair packet. |
| Static-limit-only gaps | Items blocked by named static limits. |
| First-pr packet | Existing PR handoff packet state, when present. |
| Next safe action | The one editor action or regeneration step that is safe now. |

Show Status does not decide gate status, merge readiness, runtime proof,
mutation proof, policy eligibility, or coverage adequacy.

## Copy Current Repair Packet

Use:

```text
ripr: Copy Current Repair Packet
```

This action is available only for a validated actionable gap with:

- a current workspace root match;
- a `canonical_gap_id`;
- a repair route;
- a safe related test, target, or observer when required;
- a safe verify command;
- a receipt command or receipt path;
- no blocking static limit;
- fresh artifacts;
- safe workspace-local paths and command payloads.

The copied packet is a bounded work order with these sections:

```text
Task
Context
Repair
Verification
Receipt
Stop conditions
Do not do
```

It is safe to hand to a human or coding agent because it names one canonical
gap, one repair route, one verification command, one receipt command, and the
boundaries for the work. It is not permission for broad refactors or production
edits outside the packet.

## Copy Repo Gap Map

Use:

```text
ripr: Copy Repo Gap Map
```

This action copies read-only orientation over the current queue. It may include:

- top actionable gaps;
- report-only groups;
- static-limit-only groups;
- preview-language boundaries;
- receipt states;
- first-pr packet state when already projected;
- refresh or regeneration guidance;
- hard non-claims.

The repo gap map is not a repair packet. It is useful when a reviewer or agent
operator needs to understand the repair landscape before choosing the next
bounded task.

## No-Action And Fail-Closed States

No-action is a successful state. It means the current queue does not contain a
safe local repair packet.

Unsafe states suppress repair actions and keep only refresh, Diagnose Setup, or
regeneration guidance available:

| State | Safe Behavior |
| --- | --- |
| Missing artifact | Explain which artifact is missing and show the regeneration command. |
| Malformed artifact | Explain that the JSON shape cannot be trusted. |
| Unsupported schema | Regenerate with a compatible RIPR version. |
| Wrong workspace root | Ignore the artifact and regenerate from the open workspace. |
| Stale artifact | Refresh saved-workspace evidence before copying repair packets. |
| Missing identity | Suppress repair packet actions until `canonical_gap_id` exists. |
| Missing repair route | Treat the item as blocked, not actionable. |
| Missing verify or receipt command | Suppress repair actions; a closed repair loop is not present. |
| Unsafe path | Do not open or copy paths that escape the workspace. |
| Unsafe command payload | Suppress command-copy actions. |
| Disabled language | Keep the adapter disabled unless the repo config intentionally enables it. |
| Unavailable adapter | Use a compatible server or remove the preview language from config. |
| Receipt mismatch | Rerun verify and receipt for the current gap. |
| First-pr mismatch | Regenerate first-pr artifacts after the current receipt path. |
| Static-limit-only | Use the repo map for orientation; no repair packet is safe. |

In these states, the editor should make no proof claim.

## Preview Evidence

Rust is the stable/default adapter. TypeScript, JavaScript, and Python remain
opt-in preview evidence unless promoted by a later policy.

Preview queue items are:

- syntax-first;
- advisory;
- static-limit labeled;
- not Rust-level confidence;
- not runtime adequacy;
- not mutation proof;
- not policy eligibility;
- not gate authority.

Read static limits before action language.

## Agent Handoff

For coding-agent work, prefer `Copy Current Repair Packet` over copying a broad
report. The packet should be treated as the full scope:

```text
Repair this one canonical gap.
Use this related test or observer.
Add this kind of assertion or output proof.
Run this verify command.
Emit this receipt.
Stop if identity, paths, commands, freshness, or receipt state no longer match.
Do not broaden scope.
Return proof.
```

If the packet is missing, stale, wrong-root, malformed, unsupported, static
limit-only, or preview-unavailable, do not ask the agent to repair from prose.
Regenerate or inspect manually first.

## Verify, Receipt, Refresh

After writing one focused test or output proof, run the copied verify command,
then the copied receipt command. Then refresh saved-workspace evidence and run
`ripr: Show Status` again.

Useful end states are:

- the top gap changed or disappeared;
- receipt movement is `improved`;
- receipt movement is `unchanged` and the focused repair should be inspected;
- no-action is reported;
- the artifact is stale and should be refreshed before acting again.

Receipt movement is static evidence movement. It is not runtime mutation proof
or coverage adequacy.

## Related Docs

- [Editor install to first PR](EDITOR_INSTALL_TO_FIRST_PR.md) covers first-use
  setup, repair, receipt, and first-pr handoff.
- [Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md)
  covers the local first repair loop in more detail.
- [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md) covers
  the read-only first-pr packet projection.
- [Editor extension](EDITOR_EXTENSION.md) lists commands, settings, status, and
  diagnostic behavior.
- [Static limits](STATIC_LIMITS.md) explains how to read static-limit labels.

## Non-Claims

The actionable gap queue is not:

- merge approval;
- a gate decision;
- runtime proof;
- mutation proof;
- coverage adequacy;
- policy eligibility;
- automatic repair;
- generated tests;
- provider/model execution.
