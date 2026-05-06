# Droid Review Rules — TEMPLATE

> Copy into the target repo's agent-context directory and tailor only the
> **Domain priorities** section. Everything else is part of the rollout
> contract.

These rules govern Droid PR review output. They are stricter than the model's
defaults and exist to make every comment repair-ready.

## Output shape

Each review comment must contain:

1. **Severity prefix**: `[P0]`, `[P1]`, or `[P2]`.
2. **Failure mode**: what breaks, in one sentence.
3. **Why here**: the repo invariant, policy, or edge case violated. Link to
   the doc, fixture, or spec when useful.
4. **Fix direction**: name likely files/functions when useful.
5. **Validation**: a command, report, fixture, golden, or CI check the next
   agent can run.
6. **Confidence**: `High` / `Medium` / `Low`. Justify anything below `High`.

## Evidence labels

Use one of these labels for each claim:

- **Observed** — verified directly in the diff or in repo files Droid read.
- **Reported** — surfaced by an existing report, tool, or CI artifact.
- **Not verified** — speculation or pattern-matching; treat as a hypothesis.

`Not verified` claims are allowed but must be explicitly labeled. Do not
present speculation as observation.

## What to suppress

- Duplicate findings. Collapse to one comment with multiple sites listed.
- Style nits already covered by `cargo fmt`, `cargo clippy`, or the repo's
  equivalent.
- Generic advice without a repo-specific anchor.
- Commentary directed at the PR author rather than the next repair agent.

## What never to do

- **No naked LGTM.** Even a clean review must list inspected surfaces, checks
  performed, why no comments, residual risk, and validation signal.
- **No arbitrary comment cap.** If there are 12 real findings, post 12.
- **No `@mentions`** of humans, teams, bots, or organizations in
  Droid-generated review bodies.
- **No advice to disable, bypass, or weaken** the workflow invariants in
  `review-invariants.md` (same-repo guard, trusted actor guard,
  `show_full_output: false`, pinned SHAs, BYOK literal-key handling).
- **No edits unrelated** to the PR's scope, even when fixing a finding seems
  trivial.

## Domain priorities

> **Tailor this section per repo.** The list below is `ripr`'s; each rollout
> target should rewrite this with its own top-of-mind risks.

For `ripr`-style repos, prioritize:

1. **Spec ↔ test ↔ code drift** — does the change preserve the
   `spec → test → code → output → metric` chain?
2. **Output schema stability** — does the change touch a versioned JSON or
   stable text contract without bumping/recording?
3. **Static-language compliance** — does the change introduce forbidden
   runtime mutation vocabulary?
4. **Workflow / policy drift** — does the change weaken any invariant
   enforced by `cargo xtask check-*`?
5. **Public API surface** — does the change expand or change the published
   crate's public symbols without an allowlist update?

## Validation hooks

When suggesting validation, prefer commands the repo already runs:

- `cargo test --workspace`
- `cargo xtask check-pr`
- The specific `cargo xtask check-*` gate that enforces the invariant.

For non-`ripr` repos, swap these for the target's equivalent.
