# Editor First-PR Bridge Workflow

Use this workflow after the editor repair loop has produced or found enough
evidence to inspect the first successful PR packet.

The bridge is intentionally read-only:

```text
Diagnose Setup -> Show Status -> inspect diagnostic -> copy repair packet
-> write one focused test -> verify -> receipt -> refresh
-> inspect first-pr packet
```

The editor consumes existing artifacts. It does not create the first-pr packet,
publish PR comments, write CI summaries, decide gates, edit source, generate
tests, call providers, or run mutation testing.

## 1. Start From Setup

Run:

```text
ripr: Diagnose Setup
```

Then run:

```text
ripr: Show Status
```

These surfaces should explain the current workspace, server, config, enabled
languages, artifact freshness, receipt state, and first-pr packet state. If
either command reports stale, missing, wrong-root, malformed, disabled,
unavailable, path-unsafe, or command-unsafe state, do not copy repair or first-pr
packet actions yet. Refresh or regenerate the relevant artifact first.

## 2. Repair One Local Gap

Use the normal editor repair loop:

1. Open one RIPR diagnostic.
2. Hover to read language status, static limits, gap state, related test or
   repair target, verify command, receipt command, and non-claims.
3. Open the related test or copy the first repair packet.
4. Write one focused test or checked output proof.
5. Run the copied verify command.
6. Run the copied receipt command.
7. Refresh saved-workspace analysis.

For Rust, this is the stable reference path. For TypeScript, JavaScript, and
Python, preview evidence remains opt-in, syntax-first, advisory, and
static-limit bounded. A preview first-pr packet is still preview evidence.

## 3. Locate The First-PR Packet

The editor validates these packet locations:

```text
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
target/ripr/first-pr/start-here.json
target/ripr/first-pr/start-here.md
```

`target/ripr/reports/start-here.{json,md}` is the reviewer-facing location
written by the first successful PR workflow. `target/ripr/first-pr/start-here.*`
is an editor-local mirror location when a workflow chooses to write one.

The JSON packet is the typed contract. The Markdown packet is the human-readable
handoff. The editor uses typed JSON fields for action safety and treats Markdown
as display content only.

## 4. Read Packet State

`ripr: Diagnose Setup` and `ripr: Show Status` can report:

| State | Meaning | Safe Next Step |
| --- | --- | --- |
| Missing | No first-pr packet was found. | Run or copy regeneration guidance for `cargo xtask first-pr` or `ripr first-pr`. |
| Top repairable gap available | A validated packet selects one advisory repairable gap. | Inspect or copy the first-pr packet after confirming the diagnostic identity. |
| No actionable gap | The packet reports no current first-pr repair. | Continue review or regenerate after a relevant change. |
| Stale | Packet or editor evidence is older than the trusted local state. | Refresh saved-workspace evidence and regenerate the packet. |
| Wrong root | Packet belongs to another workspace. | Regenerate from the open workspace root. |
| Malformed or unsupported | Packet shape is not trusted by this editor. | Regenerate with a compatible RIPR version. |
| Unsafe path | Packet points outside the workspace. | Treat it as invalid and regenerate. |
| Unsafe command | Packet command payload is outside the editor safety contract. | Do not copy command actions; regenerate or inspect manually. |

Unsafe states fail closed. The editor should explain the state and suppress
repair, verify, receipt, and packet-copy actions except safe regeneration
guidance.

## 5. Use First-PR Actions

When the packet is safe, the editor can expose:

- `ripr: First PR - Open Packet`
- `ripr: First PR - Copy Summary`
- `ripr: First PR - Copy Repair Packet`
- `ripr: First PR - Copy Verify Command`
- `ripr: First PR - Copy Receipt Command`
- `ripr: First PR - Copy Regeneration Guidance`

The stronger copy actions require the current diagnostic identity to match the
packet's typed gap identity. The editor checks `canonical_gap_id` and `gap_id`
instead of matching prose.

The copied first-pr repair packet should be narrow enough for a human or coding
agent:

```text
This gap.
This language and confidence boundary.
This static limit, if any.
This related test or repair target.
This verify command.
This receipt command.
This first-pr packet path.
Do not broaden scope.
Return proof.
Stop.
```

## 6. What The Packet Means

A first-pr packet means RIPR composed existing artifacts into one advisory
start-here surface. It can name:

- the selected gap identity;
- the language and preview/stable boundary;
- the repair route;
- the related test or repair target;
- the verify command;
- the receipt command;
- artifact paths and warnings;
- limits and non-claims.

It does not mean:

- merge approval;
- gate pass or policy eligibility;
- runtime adequacy;
- mutation proof;
- coverage adequacy;
- source edits were made;
- tests were generated;
- PR comments or CI summaries were published.

Use it as the bridge from local editor repair to PR review. Keep the receipt
and the first-pr packet together so reviewers can see what changed, what was
verified statically, and what remains advisory.

## 7. When To Regenerate

Regenerate the first-pr packet after:

- writing or changing the focused test;
- emitting a new receipt;
- refreshing saved-workspace evidence;
- changing the workspace root;
- changing language configuration;
- seeing stale, missing, wrong-root, malformed, unsafe, or no-action state that
  no longer matches the current work.

Use the command printed by the packet state when available. The common public
shape is:

```bash
ripr first-pr --root . --base origin/main --head HEAD
```

Inside this repository, the compatibility wrapper is:

```bash
cargo xtask first-pr
```

The editor may copy regeneration guidance, but it does not run regeneration for
you.

## Related Docs

- [Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md)
  covers setup through local receipt.
- [Editor gap cockpit workflow](EDITOR_GAP_COCKPIT_WORKFLOW.md) covers
  diagnostic-to-repair projection.
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) covers the PR-side
  start-here packet.
- [First successful PR demo](demo/first-successful-pr.md) shows checked
  start-here packet examples.
- [RIPR-SPEC-0052: Editor first-pr packet projection](specs/RIPR-SPEC-0052-editor-first-pr-packet-projection.md)
  defines the typed projection contract.
