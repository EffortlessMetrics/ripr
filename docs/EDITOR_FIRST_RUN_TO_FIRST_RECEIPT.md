# Editor First Run To First Receipt

Use this guide when VS Code is installed and you want one first Rust repair loop
without learning every RIPR artifact first.

The path is intentionally narrow:

```text
install/open -> diagnose setup -> read status -> inspect one diagnostic
-> open related test or copy packet -> write one focused test
-> verify -> receipt -> refresh -> inspect first-pr packet
```

Rust is the stable default. TypeScript, JavaScript, and Python are opt-in
preview surfaces; preview findings stay advisory, syntax-first, and
static-limit bounded.

## 1. Install And Open

Install the `EffortlessMetrics.ripr` extension from VS Code Marketplace or
Open VSX, then open the repository root in VS Code.

The normal editor install path should not require `cargo install ripr`. The
extension resolves the server from, in order, `ripr.server.path`, a bundled
server, a downloaded cached server, a verified first-run download, `ripr` on
`PATH`, or an actionable error.

For a first Rust loop, no `ripr.toml` is required. Missing config is a normal
first-run state; Rust remains enabled by default.

## 2. Diagnose Setup

Run this command from the command palette:

```text
ripr: Diagnose Setup
```

Read it as an instrument panel:

| Field | What To Check |
| --- | --- |
| RIPR server | Found or missing, plus the resolved binary/source when available. |
| Workspace | The root must be the repository you are editing. |
| Config | Missing config is normal for Rust default use. |
| Languages | Rust should be enabled and available. Preview languages must be explicitly enabled. |
| Artifacts | Missing artifacts explain why no diagnostics are shown yet. |
| Freshness | Stale evidence means refresh before acting. |
| Next safe action | The one command or local step that is safe now. |

If setup diagnosis says server missing, workspace unresolved, language
disabled, adapter unavailable, artifact missing, or stale, fix that state first.
Do not copy repair packets from a fail-closed state.

## 3. Read Status

Run:

```text
ripr: Show Status
```

Use it to distinguish "nothing happened" states:

| Status | Meaning | Safe Next Step |
| --- | --- | --- |
| Actionable gap | A current saved-workspace artifact supports a local repair action. | Inspect the diagnostic and hover. |
| No actionable gap | RIPR has no focused local repair for the current saved workspace. | Continue review or refresh after a relevant change. |
| Already observed | The current test evidence already checks the selected gap. | No repair packet is needed for that diagnostic. |
| Stale evidence | Files or artifacts changed after the trusted analysis. | Save and refresh before acting. |
| Language disabled | Config excludes the language. | Enable it intentionally or stay Rust-only. |
| Adapter unavailable | This binary lacks an enabled preview adapter. | Use a binary with that adapter or remove the language from config. |
| Wrong-root artifact | The artifact belongs to another workspace root. | Regenerate artifacts from the open workspace. |
| Malformed artifact | The editor cannot trust the artifact shape. | Regenerate with a compatible RIPR version. |
| Receipt missing | No existing receipt is linked to this gap. | Verify and emit a receipt after the test. |
| Receipt stale or mismatch | Existing receipt evidence does not match the current gap state. | Re-run verify and receipt from current artifacts. |
| Receipt improved | Existing receipt records improved static movement. | Refresh and confirm the diagnostic state moved as expected. |
| Receipt unchanged | Existing receipt records no static movement. | Revisit the focused test or repair target. |
| First-pr packet missing | No `start-here` packet exists for this workspace. | Run or copy first-pr regeneration guidance after verify/receipt artifacts exist. |
| First-pr packet ready | A validated packet selects a top repairable gap or no-action state. | Inspect the packet before opening the PR. |
| First-pr packet stale, wrong-root, malformed, or unsafe | The editor cannot trust the first-pr packet. | Regenerate from the open workspace before copying packet actions. |

Status is advisory. It does not decide policy or gate eligibility.

## 4. Inspect One Diagnostic

Open the Problems panel and choose one RIPR diagnostic. Hover before acting.
The hover should show:

- language and stable/preview status;
- static limits, if any;
- gap state;
- why the missing observation matters;
- related test or repair target;
- verify command;
- receipt command;
- limits and non-claims.

For preview-language findings, read the static limit before the suggested
action. Preview evidence is useful for local repair hints, but it is not
Rust-level confidence and it is not a runtime adequacy claim.

For the first successful loop, prefer a Rust diagnostic with an actionable gap.

## 5. Open The Test Or Copy The Packet

Use one of the bounded editor actions:

- `Open related test` when the path is workspace-local and same-language.
- `Copy first repair packet` when the gap identity, repair route, verify
  command, and receipt command are all present.
- `Copy verify command` and `Copy receipt command` when you want the command
  chain separately.
- `First PR - Open Packet` and first-pr copy actions when a validated
  `start-here` packet matches the current workspace and diagnostic identity.
- `Refresh Analysis - Saved Workspace Check` when evidence is stale or missing.

The first repair packet is safe to hand to a human or coding agent because it
names one gap, one related test or repair target, one verify command, one
receipt command, and the evidence limits. It is not permission for broad
refactors.

## 6. Write One Focused Test

Add the smallest test assertion that would notice the changed behavior named by
the diagnostic.

Keep the scope tight:

- edit the related test when one is supplied;
- add one assertion or one focused test case;
- avoid production-code edits unless the packet explicitly scopes them;
- avoid generated tests and broad rewrites.

Save the file so the editor can refresh from the saved workspace.

## 7. Verify

Run the copied verify command. The typical editor packet uses a saved before
artifact and a new after snapshot, then writes:

```text
target/ripr/agent/agent-verify.json
```

Verify compares static before/after evidence. It does not run mutation testing
and does not prove runtime adequacy.

## 8. Emit The Receipt

Run the copied receipt command. The typical editor packet writes:

```text
target/ripr/agent/agent-receipt.json
```

The receipt records the selected gap, the verify input, static movement, and
remaining limits. Useful receipt states include:

- receipt found;
- receipt missing;
- receipt stale;
- receipt gap mismatch;
- receipt movement improved;
- receipt movement unchanged.

If the receipt is missing, stale, wrong-root, malformed, or gap-mismatched,
treat repair actions as unsafe until you rerun the current verify and receipt
chain.

## 9. Refresh

Run:

```text
Refresh Analysis - Saved Workspace Check
```

Then check `ripr: Show Status` again. A good first loop ends with one of these
clear states:

- the diagnostic moved or disappeared;
- the receipt records improved static movement;
- the receipt records unchanged movement and the next action is to revise the
  focused test;
- the evidence is stale and must be refreshed again before acting.

## 10. Inspect The First-PR Packet

After the receipt and refresh, run `ripr: Diagnose Setup` or
`ripr: Show Status` again and read the first-pr packet state. The editor checks:

```text
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
```

When the packet is safe, use the first-pr actions to open the Markdown packet,
copy the summary, copy the repair packet, or copy the verify/receipt commands.
The stronger copy actions require the current diagnostic identity to match the
packet's typed `canonical_gap_id` or `gap_id`.

If the packet is missing, stale, wrong-root, malformed, path-unsafe, or
command-unsafe, treat it as a fail-closed state. Use the regeneration guidance
instead of copying repair claims. The common public command is:

```bash
ripr first-pr --root . --base origin/main --head HEAD
```

Inside this repository, the compatibility wrapper is:

```bash
cargo xtask first-pr
```

The first-pr packet is the PR handoff surface, not a merge approval. It tells a
reviewer what one gap was selected, what evidence moved, which receipt to
inspect, and what remains advisory. See
[Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md) for the
full handoff.

## Non-Claims

The editor first-run loop is read-only projection over existing RIPR artifacts.
It does not:

- edit source;
- generate tests;
- call model or provider services;
- run mutation testing;
- publish PR comments;
- decide gates or default blocking;
- claim runtime adequacy;
- make preview evidence equivalent to stable Rust evidence.

For the full editor repair model, see
[Editor gap cockpit workflow](EDITOR_GAP_COCKPIT_WORKFLOW.md). For the PR
handoff after receipt, see
[Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md). For
command and server details, see [Editor extension](EDITOR_EXTENSION.md). For
preview language boundaries, see
[Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md) and
[Static limits](STATIC_LIMITS.md).
