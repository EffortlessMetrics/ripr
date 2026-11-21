# LLM work loop status report

Date: 2026-05-07

Branch / PR: `agent-loop-status-report` / pending

Work item:

`agent/loop-status-report`

This PR opens Campaign 11 (`llm-work-loop`) and adds the first read-only control
plane surface for LLM agents:

```bash
ripr agent status --root . --json
```

The command reads existing artifacts only. It does not run analysis, generate
tests, edit source, run mutation testing, refresh LSP state, or change the
brief, packet, verify, or receipt schemas.

It reports:

- before snapshot, after snapshot, agent brief, agent packet, agent verify, and
  agent receipt presence
- recovered `seam_id` from receipt, verify, packet, or brief JSON when
  available
- one missing-input command per absent artifact
- `next_command` as the first missing workflow step
- stale-looking timestamp warnings when verify is older than snapshots or
  receipt is older than verify

Validation target for this PR:

```bash
cargo test -p ripr agent_status
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

Next ready work item:

`agent/centralize-loop-command-templates`

That follow-up should remove command-string drift across agent status, brief
next commands, LSP copy actions, cockpit missing-input commands, CI artifacts,
docs examples, fixtures, and release-readiness proof.
