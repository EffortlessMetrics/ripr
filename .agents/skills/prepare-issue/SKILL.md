---
name: prepare-issue
description: Research, correct, or create one implementation-ready issue without stopping the delivery loop. Use when a selected claim lacks a current premise, scope, acceptance contract, or owner.
---

# Useful result

One issue states the current problem, governing authority, coherent PR-sized claim, acceptance, negative evidence, dependencies, non-goals, and claim boundary. The delivery flow continues unless a real blocker remains.

# Procedure

1. Restate the requested outcome and preserve its referents.
2. Read current `main` and the actual production consumer before trusting an audit, old issue, or agent report.
3. Search issues, all-state PRs, and relevant Git history for:
   - the same claim;
   - an explicit prerequisite;
   - a superseding implementation;
   - evidence that the premise is already delivered or stale.
4. Identify the semantic owner and the layer that currently makes the decision.
5. Test the strongest counter-read. Separate fact, inference, recommendation, and owner ruling.
6. Define one coherent acceptance-and-rollback claim. Do not split by line count, directory, or provider task count.
7. Write acceptance that can be disproved:
   - positive behavior;
   - discriminating negative or alternate behavior;
   - production-path reachability;
   - currentness and artifact identity where material;
   - no overclaim beyond the implemented authority.
8. Name exact dependencies and non-goals.
9. Create or update the issue, then return to the delivery flow. Filing the issue is not the goal unless the user explicitly requested planning only.

# Premise outcomes

- `CURRENT`
- `ALREADY_DELIVERED`
- `PARTIALLY_DELIVERED`
- `DUPLICATE`
- `SUPERSEDED`
- `MIS_SCOPED`
- `NEEDS_FRESH_EVIDENCE`

Do not close partially delivered work. Record what landed and what remains.

# Coordination boundary

Search related work by claim identity and explicit dependency. Do not use touched-file proximity, crate overlap, or nearby symbols as ownership evidence. When another lane materially needs a fact, use an ordinary issue or PR comment.

# Useful fan-out

Use focused read-only agents for independent source mapping, external authority, issue/PR history, or adversarial premise review. The root integrates one issue and verifies every load-bearing citation.

# Valid exits

- `ISSUE_READY`
- `ISSUE_UPDATED`
- `CLAIM_ALREADY_DELIVERED`
- `CLAIM_PARTIAL`
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`
