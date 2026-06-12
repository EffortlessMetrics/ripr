# LSP-First Repair/Receipt Workflow for Agents

This guide documents the end-to-end loop an agent runs via the RIPR LSP/IDE
cockpit to work through one bounded repair attempt: surface a gap, acquire the
bounded packet, make a focused edit, verify, record a receipt, and inspect the
outcome and route quality before deciding to iterate or defer.

The companion human-facing tutorial is
[Editor First Run To First Receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md).
That guide walks the one-time install-and-diagnose path. This guide assumes a
working RIPR setup and describes the programmatic agent loop.

See [Agent Operating Model](AGENT_OPERATING_MODEL.md) for orchestration
economics, the scoped-PR contract, and why constraints enable autonomy. See
[Scoped PR Contract](SCOPED_PR_CONTRACT.md) for the edit-cage, verify, receipt,
and must_not_change shape a repair packet carries. See [Learnings](LEARNINGS.md)
(`2026-06-11` entries) for the evidence-to-repair isomorphism and the honesty
bar that governs all RIPR output.

---

## Non-Claims (Honesty Bar)

Read this section first. It is not a disclaimer; it is the contract that makes
the loop safe to automate.

- **RIPR does not edit code.** The agent makes every edit. RIPR hands over a
  bounded packet; the edit is the agent's responsibility, not RIPR's.
- **A receipt records the verify evidence that was run; it is not absolute
  semantic proof of correctness.** A receipt with `outcome: improved` means the
  static exposure evidence moved in the expected direction in the bounded region.
  It does not mean the behavior is semantically equivalent or runtime-correct.
- **Limitations are not repair packets.** When RIPR reports a top limitation
  instead of an actionable packet, the limitation describes a gap that cannot be
  safely delegated yet. Do not treat it as a repair; do not run a repair receipt
  command for a limitation.
- **Limited, stale, or incomplete evidence cannot establish improvement.** When
  the receipt outcome is `unknown` or `not_available`, the evidence was
  insufficient to determine movement — that is informative, not a failure to
  report. `not_available` means "not derivable yet", distinct from a real zero
  or a real empty result.
- **Counts must have real producers.** Any count reported as a number must come
  from a real LSP command output or a RIPR artifact. If a count is not directly
  producible, write `not_available` rather than zero. Zero is a real answer;
  `not_available` is honest silence. Never substitute fake-zero for a count that
  was not collected.
- **`not_available` is not the same as zero.** `not_available` means "not
  derivable yet". A count of zero means the producer ran and found nothing.

---

## Overview

```text
Show Status
  -> top packet exists?
     YES -> Copy Top Repair Packet
            -> edit ONLY allowed_edit_surface
            -> run verify_command
            -> run receipt_command
            -> Show Receipt Status
            -> Show Route Quality
            -> outcome improved? -> done for this gap
               outcome unchanged/regressed? -> inspect route-quality guidance; do NOT repeat blindly
     NO  -> inspect top limitation via Show Top Limitation
            -> repair_route describes what would unlock it
            -> do NOT run a repair receipt command for a limitation
```

---

## Step 1 — Show Status (`ripr.collectWorkspaceStatus`)

**Command palette:** `ripr: Show Status`
**LSP executeCommand:** `ripr.collectWorkspaceStatus`

Run this first. The status panel reports:

- The current workspace run status (`run_status`): whether evidence is fresh,
  stale, or incomplete.
- The top actionable packet summary, if one is available: the target seam, the
  exposure class, and the `repair_kind`.
- The top limitation, if no actionable packet is available: a named gap with a
  `repair_route` that describes what is required to unlock delegation.

Do not proceed to copy a packet if the status is stale or incomplete. Run
`ripr check --base origin/main` first to refresh evidence, then re-run Show
Status.

If the status reports a limitation at the top, go to the
[Limitation Path](#limitation-path) section below.

---

## Step 2 — Copy Top Repair Packet (`ripr.collectRepairPacket`)

**Command palette:** `ripr: Copy Top Repair Packet`
**LSP executeCommand:** `ripr.collectRepairPacket`

Only invoke this step when Show Status reported a complete, actionable packet.

The packet shape (defined in [Scoped PR Contract](SCOPED_PR_CONTRACT.md))
carries:

| Field | What it means |
| --- | --- |
| `allowed_edit_surface` | The bounded region the agent is permitted to edit (the edit cage). Edits outside this region violate the packet contract. |
| `verify_command` | The command to run after the edit to capture static exposure evidence. |
| `receipt_command` | The canonical command to record the verify evidence as a receipt. |
| `must_not_change` | Fields the edit must leave unchanged to satisfy the packet contract. |
| `repair_kind` | The category of repair (e.g. `add_assertion`, `narrow_coverage`, `split_case`). Used by route-quality reporting. |
| `exposure_class` | The static exposure classification for this gap (`exposed`, `weakly_exposed`, etc.). |

If the packet field is absent or the status reported `not_available`, the packet
is not ready. Do not fabricate a packet or attempt an edit without one.

---

## Step 3 — Edit Only the `allowed_edit_surface`

Make the narrowest focused edit that addresses the gap named in the packet.

Rules:
- Edit **only** files and symbols within `allowed_edit_surface`. Edits outside
  that surface violate the edit cage and invalidate the receipt.
- Do not change behavior named in `must_not_change`.
- Do not add production logic; the gap is a test-coverage gap, not a missing
  production feature.
- Do not run a receipt command before the edit is complete.

If the edit surface is ambiguous or the packet seems to describe a production
gap rather than a test gap, stop and re-read the limitation path. A packet that
requires a production change is not an actionable test-coverage packet.

---

## Step 4 — Run the `verify_command`

Run the `verify_command` from the packet exactly as provided.

The verify command captures a static exposure snapshot after the edit. Its
output feeds the receipt in step 5. Do not modify the command or substitute a
different comparison; the packet's verify command is the canonical measure for
this gap.

If the verify command fails (non-zero exit), inspect the output before
proceeding. A compile error or test failure means the edit introduced a defect;
fix it before recording a receipt.

---

## Step 5 — Run the `receipt_command`

Run the `receipt_command` from the packet to record the verify evidence.

The receipt command is the canonical `ripr receipt write` / `ripr agent receipt`
invocation. It writes a structured receipt artifact that records:

- which seam was targeted
- the before/after exposure evidence
- the `outcome` field: `improved`, `unchanged`, `regressed`, or `unknown`

A receipt outcome of `improved` means the static exposure evidence moved in the
expected direction. It does not mean the behavior is semantically verified or
runtime-correct.

A receipt outcome of `unknown` or `not_available` means the evidence was
insufficient to determine movement. Do not re-run the receipt to force a
different outcome; inspect the route-quality guidance in step 7 instead.

---

## Step 6 — Show Receipt Status (`ripr.collectReceiptStatus`)

**Command palette:** `RIPR: Show Receipt Status`
**LSP executeCommand:** `ripr.collectReceiptStatus`

Run after the receipt command completes.

The receipt status panel reports:

- The `latest_attempt_outcome`: `improved`, `unchanged`, `regressed`, or
  `unknown`.
- The `receipt_backed` flag: whether the outcome is supported by receipt
  evidence or is provisional.
- Any honesty caveats: if the evidence was incomplete or the snapshot was stale,
  the panel says so explicitly.

If `latest_attempt_outcome` is `improved` and `receipt_backed` is true, the
evidence for this gap moved in the expected direction. The packet loop is
complete for this gap.

If `latest_attempt_outcome` is `unchanged`, `regressed`, or `unknown`, do not
repeat the edit blindly. Proceed to step 7.

---

## Step 7 — Show Route Quality (`ripr.showRouteQuality`)

**Command palette:** `RIPR: Show Route Quality`
**LSP command:** `ripr.showRouteQuality`

Run when the receipt outcome is `unchanged`, `regressed`, or `unknown`, or any
time you want to understand whether this `repair_kind` tends to move evidence.

Route quality reports:

- Whether this `repair_kind` has a track record of moving exposure evidence in
  the `improved` direction.
- Whether the gap structure (exposure class, propagation path, oracle strength)
  suggests that a test edit is the right repair route at all.
- Advisory guidance on what a more effective next step might be (e.g. a
  different `repair_kind`, a production fix first, or a limitation acknowledgment).

If route quality reports that this repair kind rarely moves evidence for this
exposure class, do not iterate. Record the outcome as `unknown` or `unchanged`,
read the guidance, and consider whether the gap should be escalated to a named
limitation.

---

## Limitation Path

When Show Status reports a limitation at the top instead of an actionable
packet, the loop takes a different branch:

**Command palette:** `ripr: Show Top Limitation`
**LSP executeCommand:** `ripr.collectTopLimitation`

The limitation panel reports:

- The named gap that blocks delegation.
- The `repair_route`: what must happen before an actionable packet can be
  produced. Common routes include: production behavior needs a test-facing
  discriminator, exposure class is `no_static_path` (no reachable probe), or
  evidence is `infection_unknown` (analysis could not determine propagation).
- Whether the route is something the agent can address (e.g. by adding a
  specific integration test), or whether it requires a human decision (e.g. a
  design change that would make the behavior observable).

**Do not treat a limitation as a repair packet.** There is no `allowed_edit_surface`
for a limitation because the repair is not bounded. Do not run a repair receipt
command for a limitation; such a receipt would record an unbounded edit as
evidence, which violates the honesty bar.

Instead: read the `repair_route`, record it in the relevant issue or PR body,
and either address the prerequisite condition or defer the gap with a named
explanation.

---

## Supporting Commands

| Command palette title | LSP or VS Code command | When to use |
| --- | --- | --- |
| `ripr: Show Status` | `ripr.collectWorkspaceStatus` | Start of every loop iteration |
| `ripr: Copy Top Repair Packet` | `ripr.collectRepairPacket` | When an actionable packet is available |
| `ripr: Copy Verify Command` | `ripr.copyTopVerifyCommand` | Copy the verify command to the clipboard |
| `ripr: Copy Receipt Command` | `ripr.copyTopReceiptCommand` | Copy the receipt command to the clipboard |
| `RIPR: Show Receipt Status` | `ripr.collectReceiptStatus` | After the receipt command completes |
| `RIPR: Show Route Quality` | `ripr.showRouteQuality` | When outcome is unchanged, regressed, or unknown |
| `ripr: Show Top Limitation` | `ripr.collectTopLimitation` | When no actionable packet is available |
| `RIPR: Open Attempt Ledger` | `ripr.openAttemptLedger` | Inspect the full attempt history for this gap |
| `ripr: Open Report` | `ripr.openReport` | Open the full RIPR report for this workspace |

---

## Loop Termination Conditions

Stop iterating when one of the following is true:

| Condition | Action |
| --- | --- |
| Receipt outcome `improved`, `receipt_backed: true` | Gap is validated by receipt evidence for this attempt. Record and move to the next gap. |
| Route quality advises against this repair kind | Record the outcome, read the guidance, escalate or defer. |
| Two consecutive `unchanged` or `regressed` outcomes | Do not iterate a third time without re-reading route quality and the packet. |
| Status reports stale or incomplete evidence | Refresh evidence first (`ripr check --base origin/main`), then re-evaluate. |
| Limitation at top with no actionable packet | Follow the [Limitation Path](#limitation-path); do not attempt a bounded edit. |

A receipt with outcome `improved` states that evidence moved; it does not close
the gap forever. Future diffs can re-expose the same behavior. The receipt is a
point-in-time record, not a permanent guarantee.

---

## Connection to Other Docs

- [Scoped PR Contract](SCOPED_PR_CONTRACT.md) — defines the packet shape:
  edit-cage, verify, receipt, must_not_change.
- [Agent Operating Model](AGENT_OPERATING_MODEL.md) — orchestration economics,
  verify-don't-trust discipline, and why constraints enable autonomy.
- [Editor First Run To First Receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md) —
  the human-facing install-to-receipt tutorial for VS Code; cross-links here for
  the programmatic agent continuation.
- [Learnings](LEARNINGS.md) (`2026-06-11: The Evidence-to-Repair Campaign`) —
  the product/process isomorphism lesson: the builder and the built share an
  architecture; the repair loop IS the operating model, externalised.
- [Static Exposure Model](STATIC_EXPOSURE_MODEL.md) — the RIPR chain and
  exposure vocabulary: `exposed`, `weakly_exposed`, `reachable_unrevealed`,
  `no_static_path`, `infection_unknown`, `propagation_unknown`, `static_unknown`.
- [Output Schema](OUTPUT_SCHEMA.md) — the versioned JSON shape that backs every
  packet, receipt, and limitation field referenced in this guide.
