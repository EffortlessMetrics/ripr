# Public command hierarchy

Choose the command for the task. This guide describes the development checkout;
see [installation](QUICKSTART.md#installation) for published and source builds.

| Task | Command | Result |
| --- | --- | --- |
| Inspect one change | `ripr check` | Static findings, or an explicit no-action or limited result. |
| Inspect a finding | The `ripr explain` command printed by `check` | Evidence for that finding, using the same root, diff, mode, and ID. |
| Hand off a finding | The `ripr context` command printed by `check` | Context for a human or coding agent. |
| Explore the repository | `ripr pilot --root .` | Broader analysis, pilot reports, and a supported next action. |
| Repair a selected Rust gap | The `ripr agent repair` command printed by pilot | A prepared before/edit/after attempt; you or your agent edit the test. |
| Resume a repair | `ripr agent status --root .` | The continuation command or a recovery step. |
| Compose PR evidence | `ripr first-pr` with the inputs described in [First PR workflow](FIRST_PR_WORKFLOW.md) | A summary of existing artifacts, not a new analysis or repair. |
| Add advisory CI | `ripr init --ci github` | A non-blocking GitHub workflow to review and commit. |
| Diagnose setup | `ripr doctor` | Tooling and configuration checks with recovery guidance. Not required before every run. |
| Check configuration | `ripr config validate` | Validation of `ripr.toml` without analysis. |
| Start the LSP sidecar | `ripr lsp --stdio` | Saved-workspace feedback for an LSP client. |
| Serve MCP status | `ripr mcp --stdio` | [Read-only workspace status](interop/mcp.md), not analysis or execution. |
| Read detailed help | `ripr help <command>` or `ripr help --all` | Options for one command or the full reference. |

## Repair transaction

Start with `ripr pilot --root .`. When it supplies a supported repair, copy its
before command. `check` prints probe IDs; `agent repair` accepts repository-scoped
seam IDs. They are not interchangeable.

The sequence below is a reference, not a copy-ready command: replace `SEAM_ID`
with the ID from pilot and `ATTEMPT_ID` with the ID printed by the before phase.
Prefer the complete commands printed by ripr.

```bash
ripr agent repair --root . --seam-id SEAM_ID --phase before
# Read the packet, then edit one allowed test and run its authorized test command.
ripr agent repair --root . --attempt ATTEMPT_ID --phase after
```

The before phase records the initial evidence and prints the continuation
command. The after phase records static evidence after the edit and emits the
receipt. Test execution and static movement are separate observations.

Keep the `--attempt` command when changing sessions. The seam ID selects the
gap; the attempt ID selects its prepared transaction. To recover, run
`ripr agent status --root .` and follow [Repair attempt identity](REPAIR_ATTEMPT.md).

The compatibility form `--seam-id ... --phase after` requires exactly one
waiting attempt for that seam. Zero or multiple matches are rejected rather
than guessed.

### Trust-bound Python repair

Python uses a third, separately authorized verification phase. The following is
an argument reference; replace every placeholder with the value from the
accepted selection and prepared attempt.

```text
ripr agent repair --root . --seam-id <seam-id> --phase before --python-repair-trust-manifest <selection.json> --python-repair-trust-attempt <selection-attempt-id> --edit-authorized --edit-authority <operator-id>
# Edit one allowed test; retain the printed repair-attempt ID.
ripr agent repair --root . --attempt <repair-attempt-id> --phase after --edit-authorized --edit-authority <operator-id>
ripr agent repair --root . --attempt <repair-attempt-id> --phase verify --verify-authorized --verify-authority <operator-id>
```

`verify` accepts only `--attempt`, not a seam selector. The verification authority
must match the edit authority. Trust-selection flags belong on `before` only.
Optional `--verify-rollback` requests restoration after observation; inspect the
reported rollback result rather than assuming restoration.

Follow [the governed Python sequence](REPAIR_ATTEMPT.md#governed-python-sequence)
for selection, authorization, and recovery. Execution success does not itself
establish static improvement or acceptance.

## Advanced commands

`agent start`, `brief`, `packet`, `verify`, `verify-execute`, `receipt`, and
`review-summary` remain available for explicit control, compatibility, and
debugging. Use their help and [Agent workflows](AGENT_WORKFLOWS.md) rather than
assembling them as mandatory first-run steps.

## Drift rule

README, Quickstart, editor onboarding, and CLI help should agree on each
command's job. Keep detailed options in command help and the relevant reference;
do not copy them into every introduction. The typed discovery catalog is tracked
in [#1613](https://github.com/EffortlessMetrics/ripr-swarm/issues/1613).

## Non-claims

This guide describes existing commands. It does not authorize code edits or
verification, change support tiers, or turn static findings into runtime proof.
