# Knowledge Library

This catalog organizes the repo's accumulated knowledge by *kind*. It does
not duplicate content — each entry points at the canonical source and notes
when to reach for it. Reading order follows the shelves below; start with the
shelf that matches your immediate question.

For the full Diataxis-organized doc index, see [Documentation](DOCUMENTATION.md).

---

## Shelf 1 — Agentic Learnings

*How to operate agents on this repo.*

Agents are first-class contributors in `ripr`. The operating model, PR
contract, and policy-checker facade exist so a non-specialist agent — or a
fresh session of the same agent — can pick up and continue work without
re-deriving context from scratch. The documents here encode the *process*
discipline: how work is routed, sized, and verified before it lands. Read
these before planning any campaign or before opening a PR.

| Document | What you'll learn / when to reach for it |
|---|---|
| [Agent Operating Model](AGENT_OPERATING_MODEL.md) | The orchestration model proven across `ripr` campaigns: unit of work, agent economics (scout → implement → verify), verify-don't-trust discipline, CI hygiene, and why constraints enable autonomy rather than blocking it. The primary reference for any agent picking up work. |
| [AGENTS.md](../AGENTS.md) | Terse rules of engagement: product contract, language rules, required gates, PR scope doctrine, commit/merge boundary, orchestration operating model, and the long-context workflow. Read once at session start. |
| [Codex Goals](CODEX_GOALS.md) | The multi-PR campaign runner model: how a goal maps to a sequence of scoped PRs, the vocabulary (goal / campaign / work item / handoff), and how repo artifacts (not chat) carry plan state. |
| [Scoped PR Contract](SCOPED_PR_CONTRACT.md) | The evidence bar for one work item: one production delta, one acceptance criterion, required fields, and the spec-test-code chain every material change must preserve. |
| [PR Automation](PR_AUTOMATION.md) | The `cargo xtask shape / fix-pr / check-pr` loop, current automation surface, and how deterministic cleanup is separated from judgment-based repair. |
| [Learnings — agentic lessons](LEARNINGS.md) | Dated discoveries from real campaigns: verify-don't-trust; plausible-but-wrong is the dominant failure mode; mirror CI with the policy-checker facade (`cargo test -p xtask policy_checker_facade_runs_current_repo_checks`); adoption breakage is invisible to tool builders; constraints produce autonomy. See the `2026-06-11` entries especially. |
| [LSP-First Repair/Receipt Workflow](LSP_AGENT_REPAIR_WORKFLOW.md) | The end-to-end programmatic agent loop via the LSP cockpit: Show Status → Copy Top Repair Packet → edit in cage → verify → receipt → Show Receipt Status → Show Route Quality → inspect guidance. Includes the limitation path (when no actionable packet is available) and the honesty bar (non-claims). Reach for this when driving a repair attempt programmatically. |

**Convention:** new agentic *process* lessons → `docs/AGENT_OPERATING_MODEL.md`
(durable operating rules) or a dated `## YYYY-MM-DD:` section in
`docs/LEARNINGS.md` (session discoveries worth cross-session survival). Cross-link
here when a new entry belongs on this shelf.

---

## Shelf 2 — Repo Domain Learnings

*What we know about the RIPR product and domain.*

`ripr` answers exactly one draft-time question:

> For the behavior changed in this diff, do the current tests appear to contain
> a discriminator that would notice if that behavior were wrong?

It is a *static* RIPR (Reach-Infect-Propagate-Observe-Discriminate) exposure
analyzer. It does not run mutants. Every product decision — output vocabulary,
evidence shape, actionability rules, use-case roadmap — flows from that
contract. The documents here define what `ripr` knows and claims, and what it
must not claim.

| Document | What you'll learn / when to reach for it |
|---|---|
| [CLAUDE.md](../CLAUDE.md) | The product contract, language rules, workspace shape, and Rust baseline in one place. The `## Product Contract` section is the authoritative one-sentence scope boundary. |
| [Static Exposure Model](STATIC_EXPOSURE_MODEL.md) | The RIPR chain (Reach → Infect → Propagate → Observe → Discriminate), what a probe is, exposure classes, oracle strength, and how static analysis differs from runtime mutation. The domain reference. |
| [Output Schema](OUTPUT_SCHEMA.md) | The versioned JSON shape (`schema_version 0.2`), field stability rules, additive-change policy, and the SARIF/badge/LSP surface constraints. Consult before touching any rendered output field. |
| [Capability Matrix](CAPABILITY_MATRIX.md) | Current capability status per area (planned / alpha / usable-alpha / …), artifact proofs, and which roadmap item moves each capability. The deeper proof map behind the support-tier page. |
| [Evidence-to-Repair Use-Case Roadmap](specs/RIPR-SPEC-0065-evidence-to-repair-use-case-roadmap.md) | The use-case layer: `RIPR-SPEC-0065` defines the roadmap of evidence-to-repair use cases and is the parent spec for the SPEC-0066–0078 family. Reach for this when evaluating whether a proposed surface fits the product scope. |
| [Learnings — domain facts](LEARNINGS.md) | The product contract entry (2026-05-01), static language boundary (2026-05-01), architecture shape (2026-05-01), evidence-text-first / promote-when-second-consumer pattern (2026-05-12), and the evidence-to-repair isomorphism + honesty-as-product lessons (2026-06-11). |
| [ADRs](adr/README.md) | Point-in-time architectural decisions: ADR-0001 (one published package), ADR-0002 (static exposure language), ADR-0003 (fixtures before rewrites), ADR-0005 (scoped evidence-heavy PRs), and language-substrate ADRs (0006–0009, 0018). Reach for this before changing architecture seams. |

**Open vocabulary gap (highest-leverage refactor remaining):** the honesty
primitives `run_status`, `limitations[]`, `source_location_unresolved`,
`not_actionable_or_incomplete`, `no_limitation`, and `no_snapshot` were
invented independently per surface. They are the same "evidence-state" concept
in different costumes. Unifying them into one shared vocabulary is the
highest-leverage refactor left on the domain side. See the `2026-06-11:
Evidence-to-Repair Campaign` entry in `docs/LEARNINGS.md`.

**Actionability rule:** a finding is delegable when it is *actionable* — safe
to hand to an agent or a non-specialist human. When a finding is not
actionable, `ripr` must fail closed into a named limitation rather than
reporting false-clean. That rule drives every honest-output fix in the
evidence-to-repair wave.

**Convention:** new domain facts → the relevant canonical doc or spec
(update `docs/STATIC_EXPOSURE_MODEL.md`, `docs/OUTPUT_SCHEMA.md`, a spec file,
or an ADR as appropriate). Cross-link from here when the new fact belongs on
this shelf.

---

## Shelf 3 — Repo Learnings Over Time

*The temporal record: what changed, when, and why.*

The repo's durable conversation lives in docs, not in chat transcripts. This
shelf provides a timeline entry-point into that conversation: the chronological
discovery log, point-in-time decision records, and the current campaign
direction. When you need to know "what happened at a particular milestone" or
"why did we make that call," start here.

| Document | What you'll learn / when to reach for it |
|---|---|
| [Learnings](LEARNINGS.md) | The primary chronological log of durable cross-session knowledge. One dated section per discovery. The authoritative source for the timeline table below. |
| [Handoff Ledger](handoffs/README.md) | The index of committed handoff documents — campaign closeouts, release decision records, and high-risk boundary crossings. Each handoff is a point-in-time snapshot of architecture, PR chain, and deferred items. |
| [ADRs](adr/README.md) | Architecture decision records, indexed and cross-linked. Each ADR is a supersedeable point-in-time decision with consequences and revisit criteria. |
| [Roadmap](ROADMAP.md) | Current product direction: the end-goal loop, capability horizon, and what `ripr` must not become. |
| [Implementation Campaigns](IMPLEMENTATION_CAMPAIGNS.md) | The multi-PR campaign history: objectives, end states, and work items per campaign. |
| [Active Goal Manifest](.ripr/goals/active.toml) | Machine-readable current campaign state, drivable by `cargo xtask goals next`. The single-file operational source of truth for an in-flight campaign. |

### Learning Milestones Timeline

Major durable discoveries in `docs/LEARNINGS.md`, newest first:

| Date | One-line lesson | Section in LEARNINGS.md |
|---|---|---|
| 2026-06-11 | RIPR product/process isomorphism: delegability = bound + legible + fenced; honesty is the product; constraints produce autonomy | [Evidence-to-Repair Campaign](LEARNINGS.md#2026-06-11-the-evidence-to-repair-campaign--productprocess-isomorphism-and-delegability) |
| 2026-06-11 | Policy-checker facade, stale builder diagnostics, static-language gate scope, cache-key honesty, path separator normalization, merge serialization | [Verification Discipline And Gate/Cache Gotchas](LEARNINGS.md#2026-06-11-verification-discipline-and-gatecache-gotchas) |
| 2026-05-21 | Repair loop is the product-critical lane; source-of-truth is supporting infrastructure | [Repair Loop Is the Product-Critical Lane](LEARNINGS.md#2026-05-21-repair-loop-is-the-product-critical-lane) |
| 2026-05-12 | Agent-readiness emerges from doctrine and gates, not from agent SDKs | [Agent-Readiness Emerges From Doctrine And Gates](LEARNINGS.md#2026-05-12-agent-readiness-emerges-from-doctrine-and-gates) |
| 2026-05-12 | Cache-TTL-aware CI watcher economics (warm / danger / committed zones) | [Cache-TTL-Aware CI Watcher Economics](LEARNINGS.md#2026-05-12-cache-ttl-aware-ci-watcher-economics) |
| 2026-05-12 | Evidence text first; promote to structured field when second consumer appears | [Evidence Text Now, Structured Field When Second Consumer Appears](LEARNINGS.md#2026-05-12-evidence-text-now-structured-field-when-second-consumer-appears) |
| 2026-05-04 | Step 0 premise check before acting on a long-context resume | [Step 0 Premise Check](LEARNINGS.md#2026-05-04-step-0-premise-check) |
| 2026-05-04 | Live source beats paraphrased schema; briefing from memory produces wrong fixtures | [Live Source Beats Paraphrased Schema](LEARNINGS.md#2026-05-04-live-source-beats-paraphrased-schema) |
| 2026-05-04 | PR bodies are LLM context; densify with exact schema fields and non-goals | [PR Bodies Are LLM Context](LEARNINGS.md#2026-05-04-pr-bodies-are-llm-context) |
| 2026-05-04 | Worktree mode defaults; subagents must be forbidden from branch-management Git commands | [Worktree Mode Defaults](LEARNINGS.md#2026-05-04-worktree-mode-defaults) |
| 2026-05-04 | Empty diff on `main` is not a repo baseline; diff-scoped vs repo-scoped analysis are different modes | [Empty Diff Is Not Repo Baseline](LEARNINGS.md#2026-05-04-empty-diff-is-not-repo-baseline) |
| 2026-05-01 | Product contract, static language, architecture shape | [Product Contract](LEARNINGS.md#2026-05-01-product-contract), [Static Language](LEARNINGS.md#2026-05-01-static-language), [Architecture Shape](LEARNINGS.md#2026-05-01-architecture-shape) |

**Convention:** every durable cross-session discovery gets a dated
`## YYYY-MM-DD: <title>` section appended to `docs/LEARNINGS.md`. The timeline
table above references it by anchor. When adding a new entry, also add a row to
the table (newest first) so the front-door timeline stays current.
