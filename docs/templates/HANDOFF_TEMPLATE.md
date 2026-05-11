---
id: RIPR-HND-YYYYMMDD-<slug>
kind: handoff
title: <campaign or work item>: closeout
status: accepted
related_campaign: <campaign-id>
related_specs:
  - RIPR-SPEC-NNNN
related_proposals:
  - RIPR-PROP-NNNN
related_adrs: []
agent_read_priority: optional
---

# <Campaign or work item>: closeout

Date: YYYY-MM-DD

## What shipped

The production deltas and evidence deltas that landed during the
campaign. Link the merged PRs and the artifacts they produced (fixtures,
goldens, output contracts, capability rows, dogfood receipts).

## Validation

The commands that passed at closeout time. Keep this list short and
exact so future agents and reviewers can reproduce the closeout state.

```bash
cargo xtask check-pr
# ... add only the commands that actually ran clean
```

## Evidence

Where to look to verify the campaign end state without re-running
analysis:

- generated reports under `target/ripr/reports/`
- checked receipts under `fixtures/...` or `target/ripr/receipts/`
- capability matrix rows
- traceability behaviors
- changelog entries

## Deferred work

What was intentionally left out of scope. Include the reason and the
boundary so a later campaign can pick it up without re-litigating the
decision. Do not introduce new behavior contracts here; new contracts go
through proposal → spec → campaign.

## Next campaign

Name the next product lane (or explicitly say "no follow-up campaign
opened — choose explicitly before opening a new lane"). Reference the
proposal or campaign id if one already exists.
