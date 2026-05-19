# Editor Install To First PR

Use this guide when you are trying RIPR in VS Code for the first time and want
to get from a newly opened repository to one PR-ready evidence packet.

The path is deliberately narrow:

```text
install -> open repo -> Diagnose Setup -> Show Status -> inspect one diagnostic
-> open related test or copy repair packet -> write one focused test
-> verify -> receipt -> refresh -> inspect first-pr packet -> open PR
```

The editor is an instrument panel over existing RIPR artifacts. It does not
create PR comments, compose generated CI summaries, decide gates, edit source,
generate tests, call providers, or run mutation testing.

## 1. Install And Open

Install the `EffortlessMetrics.ripr` extension from VS Code Marketplace or
Open VSX, then open the repository root in VS Code.

For normal editor use, you should not need to install the CLI separately. The
extension resolves the server in this order:

1. `ripr.server.path`
2. bundled server binary, when present
3. downloaded cached server binary
4. verified first-run download
5. `ripr` on `PATH`
6. actionable setup error

For the first Rust loop, no config file is required. Rust is the default
adapter. TypeScript, JavaScript, and Python are opt-in preview adapters and
remain syntax-first, advisory, and static-limit bounded.

## 2. Diagnose Setup

Run:

```text
ripr: Diagnose Setup
```

Check these fields first:

| Field | What It Means |
| --- | --- |
| Extension version | The installed VS Code client version. |
| Server path and version | The resolved `ripr` binary and whether it is compatible. |
| Workspace root | The root RIPR will trust for paths and artifacts. |
| Config and languages | Rust default state plus any enabled preview adapters. |
| Artifact freshness | Whether saved-workspace evidence is missing, current, or stale. |
| Receipt state | Whether repair movement has already been emitted for this gap. |
| First-pr packet state | Whether the PR handoff packet is missing, safe, stale, or unsafe. |
| Next safe action | The one local step that is safe now. |

If setup diagnosis reports an unsafe state, fix that state before copying
repair packets.

## 3. Read Show Status

Run:

```text
ripr: Show Status
```

Use this command to decide whether "nothing happened" is clean, missing,
stale, disabled, or unsafe. Safe first-use states include:

| Status | Safe Next Step |
| --- | --- |
| Actionable gap | Inspect the diagnostic and hover. |
| No actionable gap | Continue review or regenerate after a relevant change. |
| Missing artifacts | Run the command named by status, then refresh. |
| Stale evidence | Save files and refresh saved-workspace analysis. |
| Receipt missing | Verify and emit a receipt after the focused repair. |
| First-pr packet missing | Generate it after verify/receipt artifacts exist. |

Unsafe states fail closed. Repair, verify, receipt, and packet-copy actions
should stay suppressed when status reports wrong-root, malformed, unsupported,
path-unsafe, command-unsafe, receipt-mismatched, first-pr-mismatched, disabled
language, unavailable adapter, or ambiguous multi-root state.

## 4. Inspect One Diagnostic

Open one RIPR diagnostic from the Problems panel and hover it before acting.

The hover or diagnostic actions should identify:

- language and status, such as Rust stable/default or preview/advisory;
- static limits before suggested action language;
- gap identity and repair route;
- related test or repair target;
- verify command;
- receipt command;
- advisory limits and non-claims.

For the first adoption loop, prefer one Rust actionable gap. Preview-language
diagnostics can be useful, but they are not Rust-level confidence and are not
gate-eligible by default.

## 5. Repair One Gap

Use one bounded action:

- open the related test when RIPR supplies a workspace-local path;
- copy the first repair packet for a human or coding agent;
- copy the verify command;
- copy the receipt command.

Write one focused assertion, test case, or checked output proof. Do not broaden
the task. Do not edit production unless the packet explicitly scopes that work
or the focused test reveals a real issue.

## 6. Verify And Receipt

Run the copied verify command. Then run the copied receipt command.

The usual artifacts are:

```text
target/ripr/agent/agent-verify.json
target/ripr/agent/agent-receipt.json
```

Verify and receipt are static movement evidence. They do not run mutation
testing and do not prove runtime adequacy.

After the receipt, run the saved-workspace refresh action and then run
`ripr: Show Status` again. A useful first loop ends in one of these states:

- the diagnostic moved or disappeared;
- the receipt reports improved movement;
- the receipt reports unchanged movement and the focused test needs revision;
- evidence is stale and must be refreshed before acting.

## 7. Inspect The First-PR Packet

The editor can project existing first-pr packet artifacts from:

```text
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
```

When the packet is safe, use:

- `ripr: First PR - Open Packet`
- `ripr: First PR - Copy Summary`
- `ripr: First PR - Copy Repair Packet`
- `ripr: First PR - Copy Verify Command`
- `ripr: First PR - Copy Receipt Command`
- `ripr: First PR - Copy Regeneration Guidance`

The stronger copy actions require the current diagnostic identity to match the
packet's typed `canonical_gap_id` or `gap_id`. The editor does not match
Markdown prose to decide safety.

If the packet is missing, stale, wrong-root, malformed, unsupported,
path-unsafe, or command-unsafe, regenerate it from the current workspace:

```bash
ripr first-pr --root . --base origin/main --head HEAD
```

Inside this repository, the compatibility wrapper is:

```bash
cargo xtask first-pr
```

## Recovery Matrix

| State | What To Do |
| --- | --- |
| Server missing | Set `ripr.server.path`, enable automatic download, or install a compatible `ripr` binary. |
| Version mismatch | Use a server version compatible with the installed extension before copying repair actions. |
| No workspace | Open the repository root. |
| Multi-root ambiguous | Select or open one workspace root before acting. |
| Wrong-root artifact | Regenerate artifacts from the active workspace root. |
| Missing artifacts | Run the command printed by Diagnose Setup or Show Status. |
| Stale artifacts | Save, refresh, and regenerate affected packets. |
| Malformed or unsupported artifact | Regenerate with a compatible RIPR version. |
| Preview adapter unavailable | Use a binary with that adapter or remove the preview language from config. |
| First-pr packet mismatch | Regenerate after the current repair/receipt path. |
| Receipt missing | Run verify, then receipt. |
| Receipt stale or mismatched | Rerun verify and receipt from current artifacts. |

## Non-Claims

The editor install-to-first-pr path is advisory. It is not:

- merge approval;
- a gate decision;
- runtime proof;
- mutation proof;
- coverage adequacy;
- policy eligibility;
- automatic repair.

## Related Docs

- [Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md)
  covers the local setup-to-receipt loop in more detail.
- [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md)
  covers the read-only handoff from receipt to first-pr packet.
- [Editor extension](EDITOR_EXTENSION.md) covers installation, settings, and
  command reference.
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) covers the CLI and
  PR-side workflow.
