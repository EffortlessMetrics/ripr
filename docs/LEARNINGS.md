# Learnings

This log captures repo knowledge that should survive individual PRs and chat
sessions. It is intentionally short and actionable.

## 2026-05-01: Product Contract

`ripr` answers a narrow question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

This should remain the filter for roadmap, architecture, and output decisions.

## 2026-05-01: Static Language

Static findings should use conservative exposure language:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Do not use mutation-runtime outcome language such as `killed` or `survived`
unless explicit real mutation data is being reported in a calibration context.

## 2026-05-01: Architecture Shape

Keep one published package until there is a real external contract:

```text
Package: ripr
Binary: ripr
Library: ripr
Automation: xtask, unpublished
```

Internal modules remain the seam:

- `domain`
- `app`
- `analysis`
- `output`
- `cli`
- `lsp`

## 2026-05-01: Current Bottleneck

Distribution and product framing are in place for alpha. The next bottleneck is
analyzer truth:

```text
line-oriented scanner
-> facts
-> parser-backed syntax
-> owner symbols
-> oracle facts
-> flow facts
-> activation values
```

## 2026-05-01: Extension Path

The normal editor path must not require `cargo install ripr`. The extension
should resolve the server in this order:

```text
ripr.server.path
bundled server binary
downloaded cached server binary
verified first-run download
ripr on PATH
actionable error
```

## 2026-05-02: Runtime State Is Not Repo State

Durable repo knowledge belongs in reviewed docs, campaign manifests,
capability metadata, traceability, specs, and fixtures. Runtime/session state
belongs under generated artifact directories such as reports, receipts, or
learning output.

Do not commit local checkout notes, machine-specific paths, chat transcript
artifacts, or one-run command transcripts as repository state.

## 2026-05-03: Reasoned Policy Allowlists

Narrow exception files that bypass repo-wide checks should be structured, not
bare paths. Every entry needs an `owner` and a written `reason`, and the
checker should fail on missing or blank values, duplicate matchers, absolute
paths, backslash paths, and overly-broad globs.

The current example is `.ripr/static-language-allowlist.toml`, validated by
`parse_static_language_allowlist` in `xtask/src/main.rs`. Glob entries are
restricted to a small scoped set (`docs/*.md` and `docs/**/*.md`); broader
patterns are rejected at parse time. The legacy `.ripr/static-language-allowlist.txt`
file is rejected with a migration message if both files coexist.

Future narrow exception files should follow the same contract:

- `.ripr/test_intent.toml` (planned in `test-intent/v1`)
- `.ripr/suppressions.toml` (planned in `suppressions/v1`)

A reviewer one year from now should be able to look at any allowlist entry and
understand why it exists, who owns it, and whether the exemption could be
rewritten away. "Allowlisting because tests fail" is not a reason; "this file
defines the language boundary and must quote prohibited terms verbatim" is.

## 2026-05-04: Empty Diff Is Not Repo Baseline

A `git diff origin/main...HEAD` is empty on `main` itself. Any analysis
driven by that diff produces zero findings on `main`, regardless of
the repo's actual state. Public README badges and store-facing signals
must therefore come from a baseline that does not consult the diff.

In `ripr`: `cargo xtask badge-artifacts` is diff-scoped and feeds PR
review; `cargo xtask repo-badge-artifacts` is repo-scoped (via
`analysis::run_repo_analysis`) and is the only path safe for public
badges. Native badge JSON carries `scope: "diff"` or `scope: "repo"`
on schema 0.2 so consumers can distinguish.

Companion: a repo baseline must include test files in its index even
when probe seeding stays production-only. Otherwise the classifier's
`find_related_tests` cannot reach integration tests under `tests/`,
and `no_static_path` silently inflates.

Generalizes beyond `ripr`: any tool whose primary mode is
diff-relative needs an explicit repo-baseline mode before it can
drive public signals. Graduated from
`docs/FRICTION_LOG.md` 2026-05-03 entry "diff-scoped badge artifacts
mistaken for repo-scoped baseline."

## 2026-05-04: Live Source Beats Paraphrased Schema

When briefing a subagent or a fresh session on a schema, paste the
live JSON output (or the source-of-truth code path) into the brief.
Do not paraphrase from memory. Tests built against a paraphrased
schema pass against fixtures that match the brief, not against the
real output, and the mismatch surfaces only at integration smoke.

Same pattern applied to file paths, CLI arguments, and config keys:
the live source is the contract. A planning packet that paraphrases
the contract is a proposal, not a spec.

Graduated from `docs/FRICTION_LOG.md` 2026-05-03 entry "briefing off
in-memory schema instead of reading source."

## 2026-05-04: Step 0 Premise Check

Before editing on a long-context resume, the executor verifies the
operating brief's premises against current repo state:

- `git fetch origin` and check whether `main` has advanced
- `git status` and `git log --oneline origin/main..HEAD` to see what
  is actually on the working branch
- `cargo xtask check-campaign` and `cargo xtask goals next` to see
  the manifest's "next item"
- `gh pr list` for open PRs that may already do part of the work
- read cited files at cited line ranges; live source over paraphrased
  schema

When a premise is stale, **stop, surface the delta, and ask** rather
than silently adapt. Stale premises that slip past Step 0 cause
re-implementation of merged work, silent path-locks, and missed
dependencies between concurrent PRs.

The cost of pausing for Step 0 is low; the cost of acting on a stale
premise is a wasted PR or a misaligned campaign. Graduated from
`docs/FRICTION_LOG.md` 2026-05-03 entry "two-voice operating brief"
and the Campaign 4A pattern across #204, #209, #212. Codified in
[`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).

## 2026-05-04: PR Bodies Are LLM Context

PR descriptions, commit messages, and issue bodies are read by future
sessions, code-review bots (CodeRabbit, ChatGPT review passes), and
the author themselves weeks later. Densify them:

- exact schema fields, exact version strings, exact line ranges
- the load-bearing test names, not "tests added"
- explicit non-goals so reviewers do not expect them
- the *why* first, then the *what*

A short PR body that says "fixes X" forces every downstream reader to
re-derive the context. The owner skims; CodeRabbit, ChatGPT, and
future sessions consume. Codified in
[`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).

## 2026-05-04: CodeRabbit Silence Is Not Approval

CodeRabbit's review output is advisory:

- positive review = signal worth reading
- silence (rate-limit, queue depth, missed trigger) = **not** approval

CI gates are the floor. Real human review is the ceiling. CodeRabbit
sits between them and helps, but the absence of feedback should never
be read as endorsement. Surfaced repeatedly across Campaign 4A; codified
in [`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).

## 2026-05-04: Checked-in JSON Beat Pages for v1 Dogfood Hosting

Custom Shields endpoints need a stable public URL. For `ripr`'s own
v1 dogfood badge, **two committed JSON files served via
`raw.githubusercontent.com` are simpler than a GitHub Pages
deployment**:

- no Pages enablement requirement
- no deploy workflow + `policy/workflow_allowlist.txt` entry
- no implication that downstream users must enable Pages
- badge changes show up in PR diffs (useful while the count is still
  stabilizing)

The product contract that survives is `ripr emits Shields-compatible
JSON` — hosting is a replaceable layer. The v2 path is a hosted
service or org-level badge-host repo so users do not self-host at all
(see `deferred/hosted-badge-service`).

Graduated from the #209 design pivot (Pages prototype rejected for
v1; checked-in JSON adopted). See `docs/BADGE_POLICY.md` § "Why
checked-in JSON, not GitHub Pages."

## 2026-05-04: Worktree Mode Defaults

Subagent dispatch:

| Mode | When |
| --- | --- |
| Inline (current branch) | 1–2 disjoint sub-tasks, current branch state matters |
| Manual worktree | 3+ agents in parallel on disjoint files, explicit base |
| Auto worktree isolation | rarely; cuts from the wrong base for active feature-branch continuation |

When using a manual worktree, the agent prompt **must** forbid:
`git checkout`, `git switch`, `git branch -D`, `git stash`, `git
reset --hard`, `git worktree remove`. The agent stays in the
assigned directory, edits files, runs tests, and reports back. If
branch state looks wrong, it stops and reports rather than repairing.

Reason: worktree agents that reach for ordinary repo-level Git
commands disturb the main worktree's branch state in ways the owner
cannot easily diagnose. Codified in
[`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](reference/AGENT_HANDOFF_PROTOCOL.md).

## 2026-05-01: Engineering Debt to Track

The repository currently contains `unwrap`/`expect` usage in code and tests.
That conflicts with the target engineering bar. Do not normalize this pattern in
new work. Pay it down in a scoped PR with explicit fallible handling and tests.

Observed inventory during PR 0:

```text
1 production expect() call site:
  crates/ripr/src/lsp.rs

13 test unwrap() call sites:
  crates/ripr/tests/cli_smoke.rs
  crates/ripr/src/analysis/mod.rs
  crates/ripr/src/lsp.rs

4 string-pattern matches in rust_index.rs intentionally detect unwrap/expect in
analyzed user code and are not panic-family call sites.
```

## 2026-05-12: Agent-Readiness Emerges From Doctrine And Gates

Most repositories that call themselves "agent-ready" import a Python
orchestration framework, an LLM client, or a prompt-templating library
and call the job done. This repo deliberately does not. There is no
agent SDK, no orchestration runtime, no retry-with-backoff helper, no
prompt template, no LLM-coupling crate in `Cargo.toml`. The
production surface is plain Rust; the automation surface is
`cargo xtask`; the merge surface is GitHub primitives.

### What we have instead

The agent-readable layer is doctrine plus checks that fail fast:

- ~20 `cargo xtask check-*` gates: `check-architecture`,
  `check-static-language`, `check-no-panic-family`, `check-file-policy`,
  `check-public-api`, `check-workspace-shape`, `check-dependencies`,
  `check-output-contracts`, `check-traceability`, `check-network-policy`,
  `check-capabilities`, `check-fixture-contracts`, and the rest of the
  list in `AGENTS.md` § "Required Gates".
- ADRs that supersede their own prior versions with hash references —
  see `docs/adr/0009-python-parser-substrate.md`, whose status line
  cites the original decision in commit `d70f1802`. A reviewer (or a
  future agent) can reconstruct the entire decision arc from
  `git log --oneline` plus the ADR text.
- `.ripr/traceability.toml` mapping spec → tests → code → outputs →
  metrics for every behavior, enforced by `check-traceability`.
- `.ripr/goals/active.toml` as a single-file machine-readable
  campaign state, drivable by `cargo xtask goals next`.
- Per-fixture `SPEC.md` files turning fixtures into compiled
  documentation, enforced by `check-fixture-contracts`.
- Squash-merge with descriptive commit titles, making
  `git log --oneline` a searchable campaign history.
- Symmetric durable preference stores: an agent-side cross-session
  memory store for habits and operator preferences, repo-side
  `docs/LEARNINGS.md` for repo knowledge worth surviving sessions.

### Why this works

Three reasons.

First, agent-readiness is an emergent property of engineering
practice, not a dependency to import. The same artifacts that make a
human reviewer effective (specs, fixtures, ADRs, gates, traceability)
also make an agent effective. The repo does not need a separate
"agent layer" because the doctrine layer is already there.

Second, the right abstraction layer is doctrine that humans and
agents both consume, enforced by checks that fail fast. Doctrine
written down but not enforced rots quietly; doctrine enforced by a
gate fails loudly the first time it is violated. The `check-*` gates
turn taste into CI signal.

Third, the investment compounds. Every campaign deposits more
traceability, more fixtures, more ADRs, more memory entries into the
repo. The next campaign — for the next agent, or the same agent in a
fresh session, or a human contributor — starts from a richer base.
Agent-specific tooling, by contrast, ages out fast: the SDK gets
deprecated, the prompt format shifts, the orchestration framework
forks. Doctrine in Markdown plus gates in Rust does not.

### How to keep this working

When tempted to add an agent-specific dependency or a hand-rolled
orchestration helper, write the doctrine artifact first instead:

- If the new behavior changes a public contract, add a spec under
  `docs/specs/` and an entry in `.ripr/traceability.toml`.
- If it constrains future contributors, add an ADR under `docs/adr/`.
- If it can be expressed as "this thing must never appear in
  output," add a `cargo xtask check-*` gate and a row in the
  relevant `policy/*` or `.ripr/*` allowlist.
- If it changes how an agent should behave across sessions, add an
  entry here in `docs/LEARNINGS.md`.

Only reach for a tool — agent-specific or otherwise — when doctrine
alone cannot express the constraint and a gate alone cannot enforce
it. This ordering keeps the repo's durable conversation in the repo.

### Concrete example: Campaign 27 substrate recovery

Campaign 27 (Language Adapter Preview) initially picked
`ruff_python_parser` as the Python substrate (ADR 0009, original
version in commit `d70f1802`). That choice rested on an assumption
that the parser was published; in fact `ruff_python_parser` is a
workspace crate inside the ruff monorepo and is marked
`publish = false`. The original ADR even named `rustpython-parser`
as the documented fallback under Revisit Criteria.

The recovery — switching the substrate from `ruff_python_parser` to
`rustpython-parser` mid-campaign, then landing the Python preview
adapter scaffold in #804 — required no agent-specific tooling. It
required:

- ADR 0008 (TypeScript parser substrate template) as the structural
  precedent for ADR 0009.
- Fixture-with-`SPEC.md` examples already in the repo as the
  contract for what the new adapter must extract.
- The `check-dependencies` gate forcing the substrate switch to be
  documented in `policy/dependency_allowlist.txt` before the new
  dependency could land.
- An ADR supersession arc visible from `git log --oneline`:
  `d70f180 adr: Python parser substrate ADR (#770) (#794)` →
  `d871d05 adr(py): switch Python parser substrate to rustpython-parser (#801)` →
  `463b0b9 analysis(py): Python preview adapter scaffold (#804)`.
- A short memory entry on the agent side that "proceed" means
  act-on-the-obvious-next-thing.

Eight PRs shipped in one session, from an agent that had not seen
this codebase before, with the recovery executed cleanly. The agent
did not need to remember the original substrate failure; the ADR
text recorded it, and the gate would have caught any quiet
regression.

### Evidence the gates are not theater

The static-language gate (`cargo xtask check-static-language`) has,
on at least one occasion, flagged its own author for writing one of
the banned-vocabulary words in a code comment. A gate that catches
its author is doing real work. The same gate is the reason this
learning describes the banned words as "the banned-vocabulary list"
rather than emitting them in plain prose: the doctrine constrains
the doctrine document itself, and that is the point.

### Posture for future readers

- Resist adding an agent SDK or LLM-coupling crate to `Cargo.toml`.
- When adding a new behavior, add the gate that enforces it.
- When making a one-off architectural decision, write the ADR
  before the code.
- Treat the conversation as latency; treat the repo as the durable
  conversation.
