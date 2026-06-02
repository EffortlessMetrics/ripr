---
id: ripr-context-system
kind: doctrine
title: Repo context system
status: proposed
agent_read_priority: required
---

# Repo Context System

This is the doctrine doc for `ripr`'s agent-readable context graph. It is the
companion to the human-facing [Repo tracking model](../REPO_TRACKING_MODEL.md).
Both describe the same artifacts; this doc names the **typed, linked,
mechanically-checkable** shape those artifacts will take so any agent or
operator runner can produce bounded work packets from them.

It is intentionally a doctrine doc. It does not change analyzer behavior,
recommendation ranking, gate semantics, LSP/editor behavior, provider
behavior, source files, generated tests, mutation execution, branch
protection, `pull_request_target` defaults, or default CI blocking.

## Why a context graph

`AGENTS.md` already tells agents to recover long-running work from
repository artifacts rather than chat history, starting from the roadmap,
implementation plan, campaigns, active goal manifest, capability matrix,
specs, traceability, and learnings. That works, but the links between
those artifacts are still implicit prose — an agent has to *read* the
campaign to find the spec, and *read* the spec to find the code paths.

Two failure modes follow:

1. Agents read too much, because they cannot tell which docs apply to a
   given change.
2. Agents read too little, because they did not know a relevant spec or
   ADR existed.

The fix is to turn the implicit graph into stable, typed data — IDs in
frontmatter, manifests in `.ripr/context/`, and a generated context index
under `target/ripr/reports/`.

## Five layers

The repo already has these layers, but they need separation rules and
typed manifests:

| Layer | Purpose | Home |
| --- | --- | --- |
| Orientation | What the repo is, where to start, what is risky. | `AGENTS.md`, `docs/agent-context/`. |
| Product truth | What `ripr` must do (durable behavior contracts). | `docs/specs/`, `docs/OUTPUT_SCHEMA.md`, `docs/STATIC_EXPOSURE_MODEL.md`, `docs/CONFIGURATION.md`. |
| Decision | Why a load-bearing decision was made and what it constrains. | `docs/adr/`. |
| Execution | What the agent or operator should work on now. | `docs/ROADMAP.md`, `docs/IMPLEMENTATION_PLAN.md`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `.ripr/goals/active.toml`, `.ripr/goals/archive/`. |
| Evidence | What actually happened, with provenance. | `target/ripr/reports/`, `target/ripr/receipts/`, `fixtures/`, `metrics/`, `.ripr/traceability.toml`, `docs/handoffs/`, `docs/LEARNINGS.md`. |

A doc lives in exactly one layer. A spec is not a plan; a plan is not a
decision; an ADR is not a closeout. Layer separation is the discipline
that lets agents read only what is relevant.

## RIPR-native proof stack

External planning language often uses words like PRD, proof stack, source of
truth, policy ledger, or closeout. In this repo those ideas have specific
homes. Do not add a second operating model when a plan uses generic terms;
translate it into the existing RIPR graph and the accepted
[`docs/source-of-truth/`](../source-of-truth/) stack:

| Generic plan term | RIPR-native source |
| --- | --- |
| Proposal / PRD | `docs/proposals/RIPR-PROP-*` |
| Spec | `docs/specs/RIPR-SPEC-*` |
| ADR | `docs/adr/` |
| Implementation plan | `docs/IMPLEMENTATION_PLAN.md` and `docs/IMPLEMENTATION_CAMPAIGNS.md` |
| Active goal manifest | `.ripr/goals/active.toml` |
| Support tiers | `docs/status/SUPPORT_TIERS.md` |
| Policy ledgers | `policy/*.toml`, `.ripr/traceability.toml`, `docs/CAPABILITY_MATRIX.md`, and `metrics/capabilities.toml` |
| Closeout | `docs/handoffs/` |
| Durable learning | `docs/LEARNINGS.md` |

The rule is substitution, not duplication. If a handoff asks for a proof stack
or source-of-truth stack, use the existing source-of-truth docs, proposals,
specs, ADRs, campaign docs, the active manifest, traceability, capability
metadata, and handoffs. If a field is missing from that graph, add it to the
appropriate existing artifact or validator instead of creating a runner-specific
goals tree or another status ledger.

## Typed nodes

The context graph has a small node vocabulary:

```text
proposal
spec
adr
campaign
work_item
surface
module
fixture
golden
output_contract
metric
policy
validation_command
receipt
handoff
learning
```

And a small edge vocabulary:

```text
proposes
defines_behavior
decides
implements
tests
fixtures
renders_output
measures
validates
supersedes
blocks
depends_on
closes
```

An agent answers questions by walking edges:

```text
Which docs matter for this changed file?
  module --[in]--> surface --[required_docs]--> { proposals, specs, adrs }

What decisions constrain this work?
  campaign --[constrained_by]--> adr

What proves this behavior?
  spec --[tests]--> fixture
       --[validates]--> validation_command
       --[renders_output]--> output_contract
```

## Frontmatter

Every typed doc carries a frontmatter block. The block is the anchor; the
prose below it is the explanation.

YAML fenced with `---`:

```yaml
---
id: RIPR-SPEC-NNNN
kind: spec
title: <human title>
status: proposed | accepted | superseded | deprecated
surfaces:
  - analysis
  - output
related_proposals:
  - RIPR-PROP-NNNN
related_adrs:
  - ADR-NNNN
related_campaigns:
  - <campaign-id>
agent_read_priority: required | recommended | optional
---
```

ID schemes:

- `RIPR-PROP-NNNN` for proposals.
- `RIPR-SPEC-NNNN` for behavior specs.
- `ADR-NNNN` for architectural decisions.
- `RIPR-HND-YYYYMMDD-<slug>` for handoffs (date-anchored to mirror existing
  filenames).
- kebab-case `scope/name` for campaign work items.
- kebab-case for surface ids and route ids.

Frontmatter migration is an explicit later PR (see "Rollout" below). This
doc defines the shape; existing files do not change in this PR.

## Manifests in `.ripr/context/`

Four small manifests will carry the typed graph data agents need without
forcing them to parse Markdown:

| Manifest | Role |
| --- | --- |
| `.ripr/context/surfaces.toml` | Maps surface ids to code paths, required docs, validation commands, and risk class. |
| `.ripr/context/doc-types.toml` | Declares required sections per document kind (proposal, ADR, handoff, learning, …). |
| `.ripr/context/validation.toml` | Maps changed paths to the validation packet a PR needs to produce. |
| `.ripr/context/context-routes.toml` | Tells agents which docs to read first, which to require, and which checks to run for a named task family. |

No manifest files ship in this doctrine PR. They land with the xtask
checks and generators that consume them so dead data does not sit in the
repo without a verifier. See "Rollout" below.

## Generated artifacts

Future xtask work (PRs 2–5 of this lane) generates:

```text
target/ripr/reports/context-index.json
target/ripr/reports/context-index.md
target/ripr/reports/work-item-packet.json
target/ripr/reports/work-item-packet.md
```

The index lists all typed docs with their frontmatter; the packet is the
bounded "read this first" packet for a chosen work item. Both are
advisory and read-only — they do not change analyzer or gate behavior.

## Lifecycle

```text
proposal (docs/proposals/RIPR-PROP-NNNN-*.md)
  -> specs (docs/specs/RIPR-SPEC-NNNN-*.md)
  -> ADRs when a durable decision is needed (docs/adr/)
  -> campaign entry (docs/IMPLEMENTATION_CAMPAIGNS.md)
  -> active manifest (.ripr/goals/active.toml)
  -> scoped PRs (one production delta + evidence package)
  -> receipts and reports (target/ripr/)
  -> closeout handoff (docs/handoffs/YYYY-MM-DD-<campaign>-closeout.md)
  -> learnings worth surviving sessions (docs/LEARNINGS.md)
  -> archive (.ripr/goals/archive/YYYY-MM-DD-<campaign>.toml)
```

A change does not touch every layer. Most behavior PRs touch a spec, a
campaign work item, fixtures, code, and a closeout. Only repo-shape or
multi-spec changes need a proposal. Only load-bearing architectural
choices need an ADR.

## Layer separation rules

To keep individual docs from being overloaded:

- A **spec** defines behavior. It must not carry a work queue or a
  campaign decision.
- A **proposal** explains design intent. It must not duplicate a spec's
  behavior contract or define output schemas.
- An **ADR** records a durable decision. It must not double as a spec,
  proposal, or work plan. It must name what it constrains and the
  conditions that would justify revisiting it.
- A **campaign ledger entry** sequences PRs. It must not redefine specs
  or duplicate proposal reasoning.
- The **active manifest** names the current execution campaign. It may stay on
  a closed campaign only when the manifest also declares
  `successor = "<campaign-id>"` or `no_current_goal = true`; closed manifests
  also move to the archive.
- A **scoped PR** is the smallest reviewable unit. It must not bundle
  unrelated contracts. See [`SCOPED_PR_CONTRACT.md`](../SCOPED_PR_CONTRACT.md).
- A **closeout handoff** records what happened. It must not invent new
  contracts; new contracts go through proposal → spec → campaign.

When in doubt, ask which question the reader is asking when they reach
for the doc. A reader asking "why does this exist?" wants the proposal.
"What must ripr do?" wants the spec. "What constrains this change?" wants
the ADR. "What is the agent doing right now, or what campaign just closed
with a successor or explicit idle marker?" wants the active manifest.
"What shipped last week?" wants the handoff.

## PR alignment cadence

Every PR should leave enough repo state for the next agent or maintainer to
continue without chat history. This is the self-aligning repo invariant: one
bounded capability moves, its proof and claim boundary are visible, and the next
slice can be chosen from repo artifacts rather than memory. A PR should answer:

```text
1. What product or repo capability moves?
2. What exact behavior or invariant changes?
3. What proof command validates it?
4. What user-facing claim changes, if any?
5. What support tier changes, if any?
6. What policy ledger or traceability entry changes, if any?
7. What should the next PR do?
```

Most PRs should have one movement type:

| PR type | Purpose |
| --- | --- |
| Capability PR | Improves analyzer or product behavior. |
| Surface PR | Makes existing evidence easier to use. |
| Proof PR | Adds fixtures, goldens, receipts, or dogfood. |
| Control-plane PR | Makes repo alignment stricter. |
| Claim PR | Updates support tier or public claim boundary after proof. |

Avoid mixing movement types unless the second type is the evidence required to
review the first. The expected closeout shape is:

```text
What landed:
What proved it:
What did not change:
Support-tier impact:
Next recommended PR:
```

## Agent neutrality

External agents (Codex `/goal`, Kiro specs/tasks, Claude Code's task
tools, Cursor rules, GitHub Copilot agents, generic operators) have
their own task systems. They consume this graph; they do not replace
it. Repo tracking stays in the repo. The two coexist:

```text
agent-side task state (Codex /goal, Kiro tasks, Claude Code TaskCreate, ...)
  |
  |  reads / writes
  v
repo-side typed context graph
  proposals -> specs -> ADRs -> campaigns -> active.toml -> work items
                     -> traceability -> context manifests -> reports
                     -> handoffs / learnings
```

Codex `/goal` specifically is documented in [Codex Goals](../CODEX_GOALS.md);
that doc points back to this one for the agent-neutral model.

## Work packet shape

When the context packet command exists, every packet should include:

```text
- objective                       (one sentence)
- current state                   (what is already true)
- target state                    (what must become true)
- read first                      (ordered list of docs)
- relevant specs                  (ids + paths)
- relevant ADRs                   (ids + the constraint each names)
- relevant code surfaces          (paths + why they matter)
- evidence to inspect             (fixtures, goldens, reports, receipts)
- required production delta       (what code/behavior may change)
- required evidence delta         (specs, fixtures, tests, docs, metrics)
- non-goals                       (explicit exclusions)
- stop conditions                 (when to write a blocked report)
- validation commands             (smallest sufficient set + full gate)
- PR summary seed                 (pre-filled production/evidence delta,
                                   acceptance, non-goals)
```

This mirrors the scoped PR contract: one narrow production delta, one
acceptance criterion, and the evidence package needed to review it.

## Future context-quality checks

Each future xtask check is its own PR:

- `cargo xtask check-doc-context`
  - frontmatter parses
  - IDs match filenames
  - linked docs and code paths exist
  - required sections per kind exist
  - active campaign has a proposal/spec link
  - work items have acceptance and commands
  - ADRs have consequences and revisit criteria
  - specs have test and implementation mappings
- `cargo xtask context index`
  - writes `target/ripr/reports/context-index.{json,md}` from the typed
    graph
- `cargo xtask context query --surface <id>`
  - returns surface-scoped docs, fixtures, and checks
- `cargo xtask context route --changed-path <path>`
  - returns matching surfaces, required docs, and validation commands
- `cargo xtask context packet --work-item <id>`
  - emits the bounded work packet described above
- `cargo xtask context stale`
  - flags accepted specs without traceability, retired campaigns still
    active, ADRs referencing missing specs, or work items pointing at
    nonexistent branches

None of those are added in this PR. This PR establishes the contract; the
xtask code follows.

## Rollout

Planned PR sequence for this lane:

1. **This PR.** Doctrine doc + handoff template + Diataxis registration.
   No manifests, no xtask code, no check enforcement, no frontmatter
   migration.
2. `context: add machine-readable context manifests`. Add `surfaces.toml`,
   `doc-types.toml`, `validation.toml`, and `context-routes.toml` paired
   with the xtask check that validates them.
3. `xtask: check doc context`. Validate frontmatter, IDs, linked paths,
   required sections, and missing references.
4. `xtask: generate repo context index`. The first generated artifact.
5. `xtask: generate work-item context packet`. The agent startup command.
6. `docs(context): migrate existing docs to typed frontmatter`. Done
   incrementally, doc kind by doc kind.

This lane is intentionally lower priority than Campaign 27's
implementation work items. Manifests and tooling land only when they
unblock real PRs; the doctrine PR is enough to align future work and
review.

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

## Validation

This doctrine PR runs:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-campaign
cargo xtask check-file-policy
cargo xtask check-pr
git diff --check
```

No analyzer, fixture, or golden behavior changes.
