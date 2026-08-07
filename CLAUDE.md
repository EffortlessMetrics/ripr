# Claude Code Instructions

This repository is the product repository for `ripr`, a static
mutation-exposure analyzer for Rust/Cargo workspaces. This file is the complete
Claude root instruction set. Do not import or route through `AGENTS.md` or
`.agents/skills/**`; Codex has its own separate file set.

Claude procedures live under `.claude/skills/**`:

```text
high-level outcome  → deliver-goal
selected issue/claim or existing PR → deliver-pr
missing or stale issue premise → prepare-issue
missing or weak oracle → prepare-proof
implementation/hardening → build-candidate
substantive exact-head inspection → review-pr
published or existing PR convergence → finish-pr
```

- `review_route:root_to_review_pr`

Use the narrowest skill that matches the current state. Enter existing work at
the earliest missing or stale judgment rather than recreating completed
ceremony.

## Product contract

`ripr` asks:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Do not turn RIPR into a full mutation engine, coverage dashboard, proof system,
second rust-analyzer, or generic test generator.

Static findings use conservative vocabulary such as:

- `exposed`;
- `weakly_exposed`;
- `reachable_unrevealed`;
- `no_static_path`;
- `infection_unknown`;
- `propagation_unknown`;
- `static_unknown`.

Do not claim runtime mutation outcomes or sufficiency from static evidence.
Real mutation testing remains a later independent authority.

## Architecture

Keep one published package:

```text
Package: ripr
Binary:  ripr
Library: ripr
Automation: xtask, unpublished
```

Internal responsibilities:

- `domain`: evidence, oracle strength, classification, repair state, relations;
- `app`: use cases and public library API;
- `analysis`: diff/syntax/probe/classification/seam/test-grip production;
- `output`: human, JSON, SARIF, GitHub, gate, packet, receipt, badge rendering;
- `cli`: command-line adapter;
- `lsp`: editor sidecar, diagnostics, hover, actions, budgets, refresh, identity;
- `agent`: bounded repair-loop command production and provenance;
- `config`: typed configuration and language detection.

Use the existing semantic owner. Do not fork a parallel validator or move a
decision into a renderer, transport, policy facade, or test helper merely
because that path is convenient.

Rust baseline:

- Rust 2024;
- MSRV 1.95;
- `unsafe_code = "forbid"`;
- Rust-first repository automation;
- non-Rust programming files only in approved policy surfaces.

## Operating law

```text
many distinct claims may be in flight
one current candidate per coherent claim
one writer mutates each candidate branch/worktree at a time
readers, researchers, reviewers, and tools may inspect it
Git or integration proof surfaces real interactions when they occur
```

Do not inspect sibling worktrees, reserve files/crates/semantic surfaces,
maintain overlap maps, or monitor sibling implementations. Check other work
only for:

- an equivalent PR implementing the same claim;
- an explicit prerequisite;
- a concrete Git conflict;
- a failed combined-tree proof;
- a material fact communicated through an issue or PR comment.

A behind-only branch needs no update. The affected lane owns its own conflict
repair and affected re-proof after a real interaction appears.

## High-level goal delivery

Preserve the user's original goal, current interpretation, constraints,
non-goals, assumptions, and acceptance predicates. Do not substitute the first
plausible issue for the actual outcome.

Evaluate goal predicates as:

```text
pass
failed
limited
not_applicable
not_established
```

“No more issues found” is not `pass`.

When a PR reaches a remote-owned state such as required CI, external review,
auto-merge, or merge queue, leave it in flight and advance another distinct
required claim when useful. Resume only after a material transition.

## Judgment passes and subagents

Research, adversarial challenge, proof design, implementation, test hardening,
simplification, review, repair, integration proof, and reconciliation are
meaningful passes. They are not mandatory identities.

The lead Claude context may perform several passes directly. Use Claude
subagents or Agent Teams only when they materially improve evidence, context,
tools, failure perspective, cost, or elapsed time.

Focused subagents are normally read-only and capability-oriented, for example:

- repository and semantic-owner mapping;
- test-oracle challenge;
- correctness or compatibility review;
- security/privacy review;
- external semantic research;
- product/editor behavior inspection.

A delegated prompt names the selected issue/PR/candidate, exact question,
governing artifacts, non-goals, write boundary, and expected evidence-backed
result. The lead context verifies every load-bearing citation and integrates one
candidate.

Do not require separate Scout, Adversary, Builder, Verifier, Reviewer, or
Cleanup Auditor identities. A different persona is not automatically
independent. Independence comes from a different oracle, source, context,
threat model, tool, or verification method where risk warrants it.

## Candidate boundary

One coherent claim normally has one branch, worktree, candidate, and PR. Do not
create rival implementations merely to consume parallel capacity. Disjoint
pieces may be delegated only through one integrating candidate owner.

Do not create repository-global active-goal, current-writer, current-stage,
liveness, lease, lock, candidate-frontier, or worker-state files. GitHub issues,
PRs, reviews, checks, and committed evidence are durable state. Claude sessions,
subagent lifetimes, and local planning notes are not.

## Engineering decisions

The existence of alternatives does not require a stop.

```text
inspect current authority
→ research material alternatives
→ choose the strongest reversible option
→ document the rationale
→ proceed
→ let proof and review correct the candidate
```

Ask for an owner decision only when materially different viable outcomes remain
after safe research and reversible engineering experiments, or when the choice
changes external commitment, destructive action, exposure, or a non-derivable
product preference.

## Evidence and actionability

Treat subagent and automated-review findings as leads until verified against
current source and executable evidence.

- Verify the retained production path, not only helper output.
- Assert fixture construction and intended parsed subjects before downstream
  claims.
- Use positive and discriminating negative or alternate proof.
- Keep missing, skipped, unobserved, stale, opaque, unsupported, and
  incomparable evidence explicit.
- Separate source failure, test/oracle failure, instrument failure,
  infrastructure failure, and not-established state.
- Use an independent corpus or invariant when a golden could encode an
  incorrect promotion.
- Platform-specific branches require platform-capable proof.
- Process success, static movement, semantic correctness, mutation adequacy,
  receipt issuance, gate result, and merge readiness remain separate axes.

The actionability flip is load-bearing. A wrong actionable repair signal is
worse than several missed advisories. Keep it fail-closed and let the shared
owning validator be the only authority.

A gate, field, or command whose stated contract exceeds its real enforcement is
a false-confidence surface. Bind every control to a negative experiment and the
actual required decision path.

## Review and currentness

Keep separate:

```text
PR head             implementation and review subject
integration basis   current base or queued predecessors
squash result       combined-tree interaction subject
```

Review and proof currentness are dimensional:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

Refresh only the dimensions affected by the latest change. Unrelated movement
on `main` invalidates nothing by itself.

Automated findings are hypotheses. Repair valid findings through the same
candidate. Reply to incorrect findings with source-backed evidence. Resolve only
after a repair or reply exists.

Quota, unavailable, skipped, failed, or stale review-provider output means
review is missing; it is not a clean review.

Use `.claude/skills/review-pr/SKILL.md` for the substantive current-head pass.
Reading threads and checks is remote triage, not review completion. Before merge
convergence, inspect the complete diff, semantic owner and consumers, test
stimulus and oracle grip, rendered/public behavior, runtime/schema/docs/output
parity, platform-relevant branches, and exact-head job/artifact evidence. A
clean self-review records what was inspected and uses a `COMMENT` disposition on
the author's PR; GitHub's inability to request changes from the author is not
approval.

Review binds only to committed Git objects. A pre-publication candidate review
may establish source and oracle findings but must retain absent hosted checks,
artifacts, and external review as `REVIEW_INCOMPLETE`. After the PR exists and
remote evidence is current, re-run `review-pr` on the exact published head. Only
that published-head pass may emit `REVIEW_READY` for merge convergence.

## Local validation

Use focused proof during implementation. Before publication, run:

```bash
cargo xtask precommit
```

`precommit` is the authoritative local shift-left command. It preserves
repository policy checks and selects Rust linting from the actual local change
set. Full-workspace and release qualification remain separate fixed-candidate
steps.

Do not run broad workspace Clippy or tests after every edit. Hooks, if used,
remain thin conveniences around canonical repository commands and do not own
policy.

For Rust, on-diff Clippy means compiling the complete impacted package and
relevant targets, not scanning changed lines without crate context.

Run one Cargo command at a time per candidate worktree. Do not kill unrelated
Cargo processes. Report lock, timeout, runner, or capacity failures as
infrastructure state rather than source failure.

Focused and fixed-candidate commands include, as appropriate:

```bash
cargo fmt --all -- --check
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-generated-clean
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo xtask check-command-catalog
```

Do not claim a gate passed when it did not run, timed out, or consumed zero
subjects.

## PR convergence

Use `.claude/skills/finish-pr/SKILL.md` for publication, review repair, CI,
integration, merge, and reconciliation. A committed candidate with
`REVIEW_INCOMPLETE` may be published so remote evidence can run. `finish-pr` may
arm merge only after the exact published head has a `REVIEW_READY` disposition
from `review-pr`; a later material head change refreshes affected review
dimensions.

Useful remote-owned outcomes:

```text
PR_IN_FLIGHT
AUTO_MERGE_ARMED
WAITING_REQUIRED_CHECKS
WAITING_EXTERNAL_REVIEW
WAITING_INTEGRATION_PROOF
```

Do not poll unchanged remote state. Resume after a substantive finding,
required failure, candidate-head change, concrete conflict, changed prerequisite,
merge, or closure.

After merge:

```text
verify current main
→ update delivered and remaining issue acceptance
→ update parent goal/campaign
→ refresh generated evidence where required
→ close only acceptance-complete issues
→ remove completed worktree and stale branch
```

Deferred, partial, blocked, or superseded work remains visible with an accurate
disposition.

## Durable repository sources

Resume from artifacts, not chat history:

- GitHub issues and PRs for live claim/candidate/review/merge state;
- `docs/ROADMAP.md` for product direction;
- `docs/IMPLEMENTATION_PLAN.md` for current implementation direction;
- `docs/IMPLEMENTATION_CAMPAIGNS.md` for history and multi-PR context, not a
  global selector;
- `.allow/spec-system/slices/` for PR-sized claim boundaries;
- `docs/specs/` and `.ripr/traceability.toml` for spec-test-code relationships;
- `docs/LEARNINGS.md` for durable failure modes and hidden invariants;
- `.claude/skills/**` for Claude procedures.

## Explicit exclusions

- no Codex skill imports or routing;
- no Kiro skill, overlay, lifecycle route, handoff, or provider entry;
- no executor DAG or permanent role roster;
- no candidate tournament for one claim;
- no sibling-lane telemetry or reservation system;
- no mandatory approval pause for ordinary engineering decisions;
- no lifecycle state encoded in issue comments or labels;
- no release, publication, tag, signing, marketplace, or secret operation
  without the existing explicit repository authority.
