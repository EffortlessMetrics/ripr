---
name: prepare-issue
description: Research, correct, or create one implementation-ready issue and continue delivery. Use when a claim lacks a current premise, semantic owner, scope, acceptance contract, or dependency boundary.
---

# Result

One issue accurately describes the current problem, governing authority, coherent PR-sized claim, acceptance and negative evidence, dependencies, non-goals, and claim boundary.

# Workflow

1. Preserve the requested outcome and exact referents.
2. Read current `main` and the production consumer before trusting an audit, old issue, or agent report.
3. Search all-state issues, PRs, and relevant Git history for:
   - the same claim;
   - explicit prerequisites;
   - superseding implementation;
   - evidence that the premise is already delivered or stale.
4. Identify the semantic owner and the layer that makes the current decision.
5. Test the strongest counter-read. Keep fact, inference, recommendation, and owner ruling separate.
6. Define one coherent acceptance-and-rollback claim. Do not split by line count, directory, or provider task count.
7. Write acceptance that can fail:
   - intended behavior;
   - discriminating negative or alternate behavior;
   - production-path reachability;
   - currentness and identity where material;
   - explicit claim boundary.
8. Name dependencies and non-goals precisely.
9. Create or update the issue and continue the delivery flow. Do not stop after filing unless planning was the requested deliverable.

# Premise outcomes

- `CURRENT`
- `ALREADY_DELIVERED`
- `PARTIALLY_DELIVERED`
- `DUPLICATE`
- `SUPERSEDED`
- `MIS_SCOPED`
- `NEEDS_FRESH_EVIDENCE`

Partially delivered work stays open with landed and residual acceptance separated.

# Related-work boundary

Search related work by claim identity and explicit dependency, not by touched-file proximity. Material facts needed by another lane belong in an ordinary issue or PR comment; no lane telemetry is required.

# Subagents

Focused read-only subagents may map source authority, GitHub history, external semantics, or adversarial premise risk. The lead Claude context verifies citations and writes one integrated issue.

# Valid outcomes

- `ISSUE_READY`
- `ISSUE_UPDATED`
- `CLAIM_ALREADY_DELIVERED`
- `CLAIM_PARTIAL`
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`
