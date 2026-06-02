# Agent Handoff Protocol

This is the operating contract between the human owner, the planning
partner, and the executor agent that build `ripr` together. It exists
so a fresh session can pick up the work without re-deriving the
method from transcript memory.

The protocol is **descriptive of the working method that produced
Campaign 4A**, not a speculative future ideal. If a section here
disagrees with how the next campaign actually runs, update this doc.

## Roles

### Steven (owner)

Product, risk, and architecture judgment. The owner:

- decides what the product is and is not
- accepts or rejects design pivots
- approves scope, blast radius, and merge timing on high-risk PRs
- chooses between options when the executor surfaces a real decision
- owns the public surface (README, store-facing docs, network policy)

The owner is **not** expected to read every PR diff. PR bodies and
docs exist so the owner can spot-check the load-bearing claims; if a
PR can't be reviewed by spot-check, the executor has not done its job.

### ChatGPT (slow-loop research, planning, review strategy)

Long-context architecture and product reasoning. ChatGPT:

- frames campaigns and breaks them into PR sequences
- runs adversarial review passes on PR bodies and docs
- catches stale or contradictory directives across long packets
- proposes design pivots before implementation

ChatGPT does not commit code directly to this repo. It produces
operating packets that the executor follows.

### Claude (fast-loop executor, integrator, local reality-checker)

Implementation, verification, and integration. Claude:

- runs Step 0 premise checks before editing
- implements scoped PRs end-to-end (code, tests, docs, gates)
- catches stale plans by reading current repo state
- handles mechanical repairs and gate failures inline
- writes dense PR bodies for downstream LLM context
- merges scoped, in-lane PRs when they are current, clean, and review-ready

Ordinary review repair, validation, PR updates, merge, and post-merge proof are
part of executor ownership. The executor should ask the owner only when the next
step needs product, architecture, credential, release, public-contract, or
out-of-scope judgment that is not already covered by the active lane.

Claude's primary value is **not throughput**. It is verifying the
plan against the current repo before executing. A faster executor that
runs against stale plans wastes the owner's review time more than it
saves.

### CodeRabbit (and other code-review bots)

Advisory review. CodeRabbit's approval is a positive signal but its
silence (rate limits, queue depth) is **not approval**. CI gates are
the floor; CodeRabbit is a ceiling-helper.

## Durable surfaces

| Surface | Purpose | Persistence |
| --- | --- | --- |
| `git` history | source of truth for code and policy | permanent |
| Issues, PRs, PR bodies | reviewable trace of decisions and design pivots | permanent (GitHub-side) |
| `docs/` | settled product/architecture/protocol decisions | permanent (in-repo) |
| `.ripr/goals/active.toml` | current campaign manifest | rolling (one campaign at a time) |
| `target/ripr/reports/` | build/CI artifacts; ephemeral handoff drafts | local-only / per-CI-run |
| this conversation transcript | fast-loop coordination | session-only |

The transcript is for *coordination*. Anything that needs to survive
the session belongs in a durable surface above. If the executor finds
itself relying on transcript memory across sessions, that is a signal
to commit a doc, an issue, a PR body, or a deferred-decision register
entry instead.

## Step 0 — premise check (mandatory)

Before any editing, the executor verifies the operating brief's
premises against current repo state:

1. `git fetch origin` and check whether `main` has advanced.
2. `git status` and `git log --oneline origin/main..HEAD` to see what
   is actually on the working branch.
3. `cargo xtask check-goals` and `cargo xtask goals next` to see
   the manifest's current "next item" (which may differ from the
   brief).
4. `gh pr list` to see open PRs that may already do part of the
   requested work.
5. Read the cited files at the cited line ranges; live source beats
   paraphrased schema in any planning packet.

When a premise is stale, the executor **stops, surfaces the delta,
and asks for direction** rather than silently adapting the plan into
something else. Stale premises that slip past Step 0 cause:

- re-implementation of work that already shipped
- silent path-locks where the executor picks a direction the owner
  didn't choose
- missed dependencies between concurrent PRs

The cost of pausing for Step 0 is low. The cost of silently adapting
a stale plan is a wasted PR or a misaligned campaign.

## "If authorized, proceed without re-asking"

Once the owner has authorized a scope, the executor proceeds without
re-asking for confirmation on routine sub-steps. Authorized work
includes:

- mechanical repairs (formatting, sort fixes, gate-pass edits)
- following an explicit PR sequence the owner laid out
- merging PRs the owner has staged when CI is green
- closing stale branches the owner has scheduled for cleanup
- updating ephemeral plan-forward / handoff scratch artifacts

Re-asking on every step turns the executor into a queue. The owner is
not the executor's queue — the executor is the owner's leverage.

## Ask-back triggers

The executor stops and asks back when **any** of:

| Trigger | Example |
| --- | --- |
| Behavior contract change | renaming a public API, changing exit codes, changing a stable JSON schema |
| Schema/output contract change | bumping `schema_version`, adding/removing a counted field, changing reason vocabulary |
| Architecture split | introducing a new crate, splitting a module across the workspace, adding an external service |
| Risk/blast radius | force-push, network policy edit, README badge URL change, store-facing copy, secret handling |
| Scope expansion beyond the authorized brief | the obvious next step would expand the PR; bundling unrelated mechanical fixes |
| Hard-to-reverse choice with multiple reasonable options | hosting model, schema bump shape, CI gate hardness |
| Evidence disproves the plan | dogfood number invalidates the brief's framing; cited file no longer exists |

For everything else, the executor decides locally and records the
decision in the PR body.

## Issue-per-PR discriminator

Use a design-plan issue **before** opening a PR when the work is:

- multi-session
- planning-iterated by ChatGPT
- campaign-level
- public-contract changing
- schema/output changing
- workflow/publishing/security sensitive
- likely to be reviewed by ChatGPT or CodeRabbit later

A PR body alone is enough for trivial, single-session, low-risk
changes (typo fixes, mechanical sort fixes, friction-log entries).

The issue makes the design discussion reviewable separately from the
diff. The diff implements one decision; the issue records the
options considered.

## PR bodies are downstream LLM context

PR bodies are read by future Claude sessions, by ChatGPT review
passes, by CodeRabbit, and by the author themselves weeks later.
**Densify them**:

- exact schema fields, exact version strings, exact line ranges
- the load-bearing test names, not "tests added"
- explicit non-goals (so reviewers don't expect them)
- the *why* first, then the *what*

A short PR body that says "fixes X" forces every downstream reader to
re-derive the context. The owner will skim; CodeRabbit, ChatGPT, and
future Claude sessions will *consume*.

## Verification depth by surface area

Effort scales with blast radius:

| Surface | Verification floor |
| --- | --- |
| Mechanical (formatting, sort fixes) | gates green |
| Internal logic (no public surface change) | gates + targeted tests |
| Public schema / API | gates + targeted tests + load-bearing regression test + dogfood snapshot in PR body |
| CI / network / publishing surface | all of the above + workflow lint + post-merge live verification (curl, etc.) |
| Architecture (crate split, new module seam) | all of the above + ADR or design-plan issue |

Routine PR work targets the lowest applicable row. The executor does
not invent ceremony for changes that don't warrant it; equally, the
executor does not skip the floor for the row the change actually sits
in.

## Worktree rules

| Mode | When to use |
| --- | --- |
| Inline (current branch) | 1–2 disjoint sub-tasks; current branch state matters |
| Manual worktree (`git worktree add`) | 3+ agents working in parallel on disjoint files; explicitly created with the right base |
| Auto worktree isolation (agent tool) | rarely; risks cutting from the wrong base for active feature-branch continuation |

When dispatching an agent into a manual worktree, the prompt **must**
include these guardrails:

- Do not run `git checkout`, `git switch`, `git branch -D`, `git
  stash`, `git reset --hard`, or `git worktree remove`.
- Stay in the assigned working directory; do not `cd` into the main
  worktree to "fix" branch state.
- Edit files, run tests, and report back. If branch state looks
  wrong, stop and report rather than repairing it.

These guardrails exist because worktree agents that reach for
ordinary repo-level Git commands can disturb the main worktree's
branch state in ways the owner can't easily diagnose.

## Decision-making cadence

| Decision shape | Owner | Default response time |
| --- | --- | --- |
| Mechanical fix-up | Claude | inline, no ping |
| Routine PR within an authorized campaign | Claude | inline, no ping |
| New PR within an authorized campaign | Claude | open and ping |
| Design pivot inside an authorized campaign | Steven | surface the option set; wait |
| Cross-campaign scope or new campaign | Steven | surface the option set; wait |
| Risk/blast-radius decision | Steven | surface the risk; wait |

Speed up the inline rows. Slow down the wait rows. The asymmetry is
the protocol.

## Failure modes this protocol guards against

- **Silent stale-plan adaptation** — caught by Step 0.
- **Re-implementation of already-merged work** — caught by Step 0
  (`git log --oneline origin/main..HEAD` and `gh pr list`).
- **Owner-bottlenecking on routine cleanup** — addressed by
  "if authorized, proceed without re-asking."
- **Owner not seeing real decisions** — addressed by ask-back
  triggers; the protocol biases toward asking on architecture and
  risk, not on routine.
- **PR bodies too thin to review** — addressed by "PR bodies are LLM
  context."
- **Worktree agents wrecking the main worktree** — addressed by the
  manual-worktree guardrails.
- **Transcript-only memory** — addressed by the durable-surfaces
  table at the top of this doc.

## Living document

If a session encounters a situation this protocol does not cover, the
preferred fix is to:

1. Make the decision in the moment (the owner makes the judgment
   call; the executor surfaces the option set).
2. Record the resolution in `docs/FRICTION_LOG.md` while it is still
   raw.
3. Graduate it into this protocol or `docs/LEARNINGS.md` once the
   pattern repeats.

This document is descriptive, not aspirational. Keep it that way.

## See also

- [`docs/handoffs/README.md`](../handoffs/README.md) — handoff ledger
  convention (when committed handoffs are appropriate).
- [`docs/SCOPED_PR_CONTRACT.md`](../SCOPED_PR_CONTRACT.md) — the
  scoped-PR contract this protocol composes with.
- [`docs/FRICTION_LOG.md`](../FRICTION_LOG.md) — raw same-day
  observations.
- [`docs/LEARNINGS.md`](../LEARNINGS.md) — settled principles.
- [`docs/DEFERRED.md`](../DEFERRED.md) — deferred-decision register.
