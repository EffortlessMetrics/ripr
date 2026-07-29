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
- `cargo xtask check-goals` and `cargo xtask goals next` to see
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

## 2026-05-12: Evidence Text Now, Structured Field When Second Consumer Appears

### Context

A recurring situation while extending analyzers: a new evidence kind
needs to flow from an adapter to the rest of `ripr`, and a spec
already documents a structured shape for it. The textbook move is to
add the typed field today. In practice, doing that for a single
producer with no consumer balloons the diff and pulls in renderer,
fixture, and golden churn that defends nothing observable.

Surfaced concretely during Campaign 27 work on TypeScript preview
facts, where `mocked_module` static-limit detection had a choice
between adding a structured `Finding.static_limit_kind` enum field
and emitting a stable-prefix string in the existing
`Finding.evidence` array.

### The pattern

For a new evidence kind, ship as a stable-prefix string in the
existing `evidence` array first:

```text
static_limit mocked_module: `./api`
```

The prefix is grep-friendly (`starts_with("static_limit ")` is a
stable contract). The renderer, JSON shape, SARIF emitter, badge
output, and the LSP all keep working without changes. Fixture
re-bless touches the one file that actually gained evidence.

Promote to a structured field on `Finding` (or wherever the spec
places it) when a real second consumer appears. Until that trigger
exists, the text-with-prefix carries the information forward without
paying for schema ceremony that nothing reads.

### When text-with-prefix is the right call

- At the time of the scoped text-prefix ship, a single adapter is the
  only producer of the signal.
- At that same decision point, no live consumer reads the typed shape:
  no scanner aggregating by kind, no LSP code-action keyed on the
  variant, no policy aggregator counting cases.
- The scoped-PR contract is pushing for one production delta in
  this PR; promoting to a schema field would expand the diff several
  times over (constructor sites, every renderer, every TS fixture
  re-blessed to confirm field absence, serialization tests).
- The signal is straightforward enough that one stable prefix
  encodes it cleanly.

### When to promote to the structured field

- A second adapter wants to emit the same kind of signal, and the
  text prefix is starting to feel like a small parallel protocol.
- A real consumer materializes: a policy-readiness scanner that
  needs to aggregate by kind, an LSP code-action that branches on
  the variant, a metric over the typed vocabulary.
- The prefix family has grown past two or three forms and the
  string-parser at the consumer side is becoming non-trivial.
- The spec's structured vocabulary needs to be enforced — at that
  point, the typed enum carries the closure over the variant set
  and the text prefix cannot.

### Hazard

A structured field that exists on paper in a spec but is not yet
emitted by any adapter is an attractive nuisance. The next reader
sees the spec, sees the absence, and reads the text-only ship as
under-delivery rather than as a deliberate deferral.

Mitigation: file the follow-up issue at the same time as the
text-only ship. Name the second-consumer trigger explicitly in the
issue, link the spec line that documents the structured shape, and
link the analyzer site that emits the text form today. The deferral
is then recorded, not hidden.

### Concrete example

- PR #791 (`analysis(ts): TypeScript preview facts — mocked-module
  static-limit reporting`) chose the text-with-prefix form before a
  live typed consumer existed.
- Issue #807 (`domain: emit structured static_limit_kind field on
  Finding`) records the follow-up after the consumer pressure became
  real.
- The scanner that aggregates by kind once the typed field is emitted:
  `crates/ripr/src/output/policy_readiness.rs:800-810`.
- The spec defining the structured vocabulary:
  `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`
  (`static_limit_kind`).

## 2026-05-12: Cache-TTL-Aware CI Watcher Economics

### Context

Agent loops that poll external state during a long-running task - CI
watchers, deploy waiters, queue drainers, anything that sleeps and
then checks - have a non-obvious cost dimension beyond API rate
limits: the LLM's prompt-cache TTL shapes the optimal polling
interval. Surfaced concretely during Campaign 27's PR watcher work
(PRs #794, #801, #804).

### The math

The Anthropic prompt cache TTL is roughly five minutes. Around that
window, three regions emerge:

```text
warm zone:        sleep <  ~5 min   conversation stays cached
danger zone:      sleep ~= 5 min    cache miss, no amortization
committed sleep:  sleep >> ~5 min   one cache miss across a long wait
```

- A watcher that sleeps under the TTL wakes up against a warm cache
  and reads only the new tool output.
- A watcher that sleeps exactly through the TTL pays a full re-read
  of the conversation context every cycle. This is the worst-case
  region: highest token cost per useful poll.
- A watcher that commits to a long sleep (twenty-plus minutes) pays
  one cache miss, but spreads that cost across many minutes of
  external progress.

This is a generally true coordination protocol for any agentic system
that polls external state. The specific TTL is an Anthropic prompt
cache fact today; other providers have their own cache windows, but
the three-region structure is the same.

### What works

- Active CI watch: 180-270 s backoff. Stays inside the warm zone,
  with enough headroom that one slow tool call does not push the
  cycle over the TTL.
- Genuinely idle ticks (no active PR, waiting for an unrelated
  trigger): 1200-1800 s. Commits to one cache miss per long wait
  instead of churning.
- Exit-early signals on every wake: a CI state of `CLEAN`, `UNSTABLE`,
  or `HAS_HOOKS` means ready to merge; a failure conclusion means stop
  and report; a `BEHIND` mergeable state means rebase, then re-watch.

### What doesn't

- `gh pr watch` default cadence (three-second polling). Burns the
  authenticated GitHub API rate limit fast and re-enters the agent
  loop too often to amortize cache cost meaningfully.
- Roughly 300 s polling. Lands in the danger zone: each wake pays a
  full cache miss without buying much external progress.
- Tight infinite loops with no backoff. Same failure mode as the
  default `gh pr watch`, plus the agent has no chance to terminate on
  the exit-early signals above.

### Operational signals to watch for

For GitHub PR watchers specifically, the merge-readiness signals worth
handling explicitly:

```text
CLEAN       ready
UNSTABLE    ready (non-required check failing)
HAS_HOOKS   ready (waiting on optional hook)
BEHIND      needs rebase, then re-watch
DIRTY       conflict, stop and report
```

Without GitHub merge queue or auto-merge enabled, concurrent merges on
a busy repo produce repeated `BEHIND` transitions; Campaign 27 saw
five to six rebase cycles per PR. Merge queue removes that class of
loop entirely.

### Limitations

- The five-minute number is the current Anthropic prompt-cache TTL.
  If that window changes, or if the watcher runs on a different
  provider, the warm/danger/committed boundaries shift but the
  three-region structure does not.
- The exit-early signals above are GitHub-specific. The general
  principle - wake, check, exit on a small set of terminal states,
  back off otherwise - transfers to other coordination targets.

## 2026-05-12: Agent-Readiness Emerges From Doctrine And Gates

Most repositories that call themselves agent-ready import a Python
orchestration framework, an LLM client, or a prompt-templating library
and call the job done. This repo deliberately does not. There is no
agent SDK, no orchestration runtime, no retry-with-backoff helper, no
prompt template, and no LLM-coupling crate in `Cargo.toml`. The
production surface is plain Rust, the automation surface is
`cargo xtask`, and the merge surface is GitHub primitives.

### What we have instead

The agent-readable layer is doctrine plus checks that fail fast:

- `cargo xtask check-*` gates such as `check-architecture`,
  `check-static-language`, `check-no-panic-family`, `check-file-policy`,
  `check-public-api`, `check-workspace-shape`, `check-dependencies`,
  `check-output-contracts`, `check-traceability`, `check-network-policy`,
  `check-capabilities`, and `check-fixture-contracts`.
- ADRs that supersede their own prior versions with hash references.
  `docs/adr/0009-python-parser-substrate.md`, for example, cites the
  original decision in commit `d70f1802`.
- `.ripr/traceability.toml` mapping spec to tests to code to outputs
  to metrics for every behavior, enforced by `check-traceability`.
- `.ripr/goals/active.toml` as a single-file machine-readable
  campaign state, drivable by `cargo xtask goals next`.
- Per-fixture `SPEC.md` files turning fixtures into compiled
  documentation, enforced by `check-fixture-contracts`.
- Squash-merge with descriptive commit titles, making `git log
  --oneline` a searchable campaign history.
- Symmetric durable preference stores: an agent-side cross-session
  store for habits and operator preferences, and repo-side
  `docs/LEARNINGS.md` for repo knowledge worth surviving sessions.

### Why this works

Three reasons.

First, agent-readiness is an emergent property of engineering
practice, not a dependency to import. The same artifacts that make a
human reviewer effective - specs, fixtures, ADRs, gates, and
traceability - also make an agent effective. The repo does not need a
separate agent layer because the doctrine layer is already there.

Second, the right abstraction layer is doctrine that humans and agents
both consume, enforced by checks that fail fast. Doctrine written down
but not enforced rots quietly; doctrine enforced by a gate fails
loudly the first time it is violated. The `check-*` gates turn taste
into CI signal.

Third, the investment compounds. Every campaign deposits more
traceability, more fixtures, more ADRs, and more repo learnings. The
next campaign - for the next agent, the same agent in a fresh session,
or a human contributor - starts from a richer base. Agent-specific
tooling ages out fast: the SDK gets deprecated, the prompt format
shifts, or the orchestration framework forks. Doctrine in Markdown
plus gates in Rust does not.

### How to keep this working

When tempted to add an agent-specific dependency or a hand-rolled
orchestration helper, write the doctrine artifact first:

- If the new behavior changes a public contract, add a spec under
  `docs/specs/` and an entry in `.ripr/traceability.toml`.
- If it constrains future contributors, add an ADR under `docs/adr/`.
- If it can be expressed as "this thing must never appear in output,"
  add a `cargo xtask check-*` gate and a row in the relevant `policy/*`
  or `.ripr/*` allowlist.
- If it changes how an agent should behave across sessions, add an
  entry here in `docs/LEARNINGS.md`.

Only reach for a tool, agent-specific or otherwise, when doctrine
alone cannot express the constraint and a gate alone cannot enforce
it. This ordering keeps the repo's durable conversation in the repo.

### Concrete example: Campaign 27 substrate recovery

Campaign 27 (Language Adapter Preview) initially picked
`ruff_python_parser` as the Python substrate in ADR 0009, original
version in commit `d70f1802`. That choice rested on an assumption that
the parser was published; in fact `ruff_python_parser` is a workspace
crate inside the Ruff monorepo and is marked `publish = false`. The
original ADR even named `rustpython-parser` as the documented fallback
under Revisit Criteria.

The recovery - switching the substrate from `ruff_python_parser` to
`rustpython-parser` mid-campaign, then landing the Python preview
adapter scaffold in #804 - required no agent-specific tooling. It
required:

- ADR 0008, the TypeScript parser substrate template, as the
  structural precedent for ADR 0009.
- Fixture-with-`SPEC.md` examples already in the repo as the contract
  for what the new adapter must extract.
- The `check-dependencies` gate forcing the substrate switch to be
  documented in `policy/dependency_allowlist.txt` before the new
  dependency could land.
- An ADR supersession arc visible from `git log --oneline`:
  `d70f180 adr: Python parser substrate ADR (#770) (#794)` to
  `d871d05 adr(py): switch Python parser substrate to rustpython-parser (#801)`
  to `463b0b9 analysis(py): Python preview adapter scaffold (#804)`.
- A short agent-side preference entry that "proceed" means
  act on the obvious next thing.

Eight PRs shipped in one session, from an agent that had not seen this
codebase before, with the recovery executed cleanly. The agent did not
need to remember the original substrate failure; the ADR text recorded
it, and the gate would have caught any quiet regression.

### Evidence the gates are not theater

The static-language gate (`cargo xtask check-static-language`) has, on
at least one occasion, flagged its own author for writing one of the
banned-vocabulary words in a code comment. A gate that catches its
author is doing real work. The same gate is the reason this learning
describes those terms as "the banned-vocabulary list" rather than
emitting them in plain prose: doctrine constrains the doctrine
document itself, and that is the point.

### Posture for future readers

- Resist adding an agent SDK or LLM-coupling crate to `Cargo.toml`.
- When adding a new behavior, add the gate that enforces it.
- When making a one-off architectural decision, write the ADR before
  the code.
- Treat the conversation as latency; treat the repo as the durable
  conversation.

## 2026-05-21: Repair Loop Is the Product-Critical Lane

When parallel improvement ideas compete for attention, keep the `ripr-swarm`
repair loop on the critical path:

```text
canonical actionable gap
-> repair packet
-> dry-run / attempt
-> verify command
-> receipt command
-> outcome report
-> readiness / next action
```

A repo control-plane/source-of-truth model is useful, but should be treated as
supporting infrastructure unless it directly increases repair-loop operability.
Durable product truth remains in repo-owned docs/spec/proposal/ADR/traceability
surfaces (or a future `.ripr-spec/` namespace), while tool-local goal state
remains operator state that references those durable artifacts.

Practical sequencing rule:

1. ship bounded repair-attempt evidence movement;
2. then tighten proposal/spec/ADR/closeout templates and validators as a
   separate infra lane.

## 2026-06-11: Verification Discipline And Gate/Cache Gotchas

Hard-won during a multi-PR autonomous campaign. Each item cost a real CI failure
or a near-miss.

### Verify with the policy-checker facade, not a hand-picked gate subset

Running a few `cargo xtask check-*` gates by hand before pushing missed two
required gates twice in a row (`check-generated`, then `check-static-language`).
The required CX43 "Required Rust gates" step runs the whole set. Mirror it
locally with one command:

```bash
cargo test -p xtask policy_checker_facade_runs_current_repo_checks
```

It runs the same gate set CI runs (~70s) and catches what a subset misses.
Pair it with a behavioral repro of the actual change — gate-pass is necessary,
not sufficient.

### Subagent builders emit stale diagnostics; `cargo check` is ground truth

Long builder runs leave the IDE diagnostic snapshot mid-edit, so rust-analyzer
reports E0425/E0308 that are already fixed. Do not trust them and do not trust
the builder's "all green" self-report. Run `cargo check -p ripr --all-targets`
(0 errors = clean) and a behavioral repro every time.

### The static-language gate scans all tracked prose, not just output

`check-static-language` walks every tracked `.md/.rs/.txt/.json/.toml/.yml/.yaml`
file (minus the `docs/*.md` glob exemption and path allowlist) and bans
`killed/survived/untested/proven/adequate` anywhere — including YAML comments and
policy-file reasons, where those are innocent English. In output, use the
exposure vocabulary; in prose, a plain synonym. `xtask/src/main.rs` is
self-exempt (it must name the forbidden terms).

### Default caps and the cache: a truncated hit must not read as complete

When a full-repo run is bounded by default (`repo-exposure` 10k seam cap), the
seam-cache key must include the effective limit (`seam_limit_key`) and the cache
envelope must persist the limit metadata. Otherwise a capped run and an
unbounded run share a cache file, or a cache hit serves a truncated result as
`run_status: "complete"` — silently reintroducing the dishonesty the bound was
meant to fix. `#[serde(default)]` on the new envelope field keeps old (full-run)
caches readable as "complete".

### Any hash over a path must normalize separators

Content-addressed ids hash a filesystem-walked path; `\` on Windows vs `/` on
Linux produced different ids and failed Linux CI on Windows-blessed goldens.
Normalize `\` -> `/` before hashing.

### Strict "up to date branch" serializes merges

Under branch protection requiring up-to-date branches, every merge forces all
other open PRs behind, each needing `update-branch` + a full re-run on the slow
self-hosted lane. Merge the higher-cost lane's PR first; a merge queue would
remove the thrash but is a deliberate settings change.

## 2026-06-11: The Evidence-to-Repair Campaign — Product/Process Isomorphism And Delegability

Synthesis from the wave that built RIPR's PR/LSP/agent surfaces (pr-evidence
summary, review repair cards, the four LSP cockpit commands + restrained
diagnostics, the VS Code surface). The durable, transferable lessons — kept here
because they outlive any single PR.

### The product is the process; that is why it is agent-buildable

RIPR's loop — raw signal -> canonical evidence -> actionability decision ->
bounded repair packet OR named limitation -> verify -> receipt — is the same
loop a competent agent runs to build it: scout -> spec -> build -> verify-self
-> scoped PR -> CI gate -> merge. They are isomorphic. RIPR is not "a tool for
agents"; it is an externalization of the discipline that makes work safe to
delegate. The builder and the built share an architecture, which is exactly why
a fleet could build it. When designing a new surface, design it as that same
loop.

### Legibility + bounded scope + fenced irreversibility = delegability

The repair packet (edit-cage + verify + receipt + must_not_change), the
operating model (scoped PR + facade gate + merge, with the crates.io publish as
the only hard stop), and the repo's own authorship all reduce to one shape:
bound the task, make the limits legible, fence the irreversible, verify rather
than trust. Keep new work inside that shape; it is what lets non-authors (other
agents, or a non-specialist human) own the result.

### Honesty is the product; silence that reads as "clean" is the cardinal sin

The most valuable fixes were all the same shape: a run that did not fully do its
job *looking* like it did — a bounded full-repo run reporting `complete`, a
cache hit serving a truncated result as complete, preview-language content
yielding "no findings" when it was not really analyzed. Fail-closed means the
tool must self-declare what it could not determine. The honesty primitives
(`run_status`, `limitations[]`, `source_location_unresolved`,
`not_actionable_or_incomplete`, `no_limitation`, `no_snapshot`) are one
"evidence-state" word in different costumes — **unify them into one shared
vocabulary** rather than re-inventing per surface (the highest-leverage refactor
left).

### Plausible-but-wrong is the dominant failure mode — verify the claim, not the symptom

Subagent builders report "all green" while live IDE diagnostics show compile
errors (almost always stale mid-edit snapshots — `cargo check` is ground truth).
A `doctor` shipped recommending `ripr check --diff origin/main...HEAD`, which
fails because `--diff` wants a file — the line was verified to *print*, not to
*run* (textbook weak oracle). A "not fail-closed" bug was nearly filed that was
actually a `| head` pipe masking the real exit code. The discipline: trust
`cargo check` + a behavioral repro of the *effect*, never a self-report or an
IDE diagnostic; mirror the whole CI gate set with
`cargo test -p xtask policy_checker_facade_runs_current_repo_checks` before
pushing (cherry-picking gates leaked `check-generated` and a forbidden word);
and verify the claim, not the pipeline that measured it.

### Adoption breakage is invisible to the people who built the tool

The issue tracker was full of infra/CI/spec items. The genuinely user-facing
break — the documented `check -> explain/context` drill-down dead in the default
(human) output mode — had no issue, because maintainers already knew the magic
incantation. The "easy to adopt" property cannot be assessed from inside; it can
only be *walked*. Dogfooding the actual user/agent path found more real value
than grinding the backlog. The unwalked path that matters most next: the full
agent loop end-to-end (status -> packet -> edit-in-cage -> verify -> receipt ->
re-status).

### Navigation guidance must be replayable, not merely printable

The first `check -> explain/context` route looked correct when inspected as
text, but a user replaying it could lose the analyzed scope or pass options the
follow-up parser did not accept. The reliable contract is an executable
round-trip: preserve the finding identity, analysis scope, artifact inputs,
mode, and supported configuration flags, then run the copied command against a
fresh binary. Golden output proves the guidance is present; CLI smoke tests
prove that the guidance actually works.

### Constraints produced autonomy — the intuition inverts

Conservative-language gates, the scoped-PR contract, traceability, and the
facade test are what let many PRs ship without a human reading each line: the
machine-checkable doctrine substitutes for human trust. For autonomous work,
more well-designed constraints = more delegable autonomy, not less. The gates
are a feature of the product (they are why it is buildable), not overhead — do
not erode them to move faster; they are the load-bearing wall.

## 2026-06-12: Closing the receipt → outcome → route-quality loop — the honesty-detection discipline

The "unwalked path that matters most next" from the prior entry — the full agent
loop `status -> packet -> edit-in-cage -> verify -> receipt -> outcome ->
route-quality` — was built and closed across ten PRs (#1130–#1139, then surfaced
through CLI, PR summary, LSP, VS Code, and an agent flow doc). The durable
lessons are not about the features; they are about how to add a *number* to an
honest tool without lying.

### `not_available` is better than a fake zero — and a fake zero is the cardinal sin in numeric form

A count reported as a number — *even `0`* — asserts "we inspected this condition
and here is the result." That assertion is only honest if a real production
condition can produce a non-zero value. A `verify_failed_receipts: 0` computed
over a field that no production path ever populates reads as "no receipts failed
verify" when the truth is "nothing can make this non-zero." That is strictly
worse than `not_available`, which honestly says "not derivable yet." This bit us
once for real (#1130: counted `gap-ledger receipt.state`, which the build never
sets — reverted to `not_available` with regression guards), and the rule then
caught two later attempts before they shipped.

### Trace the *producer*, not the consumer — and demand an end-to-end proof

For any `not_available -> count` flip, the question is never "does the field
exist?" but "what *writes* this field in non-test code, and can a real condition
make it non-zero?" `verify_failed_receipts` became real (#1131) only because
`verify_result` is genuinely populated by the swarm-ingest verify flow and `fail`
is a validated, supported value — proven by a fixture flowing a real failed
verify end-to-end, not a unit test that hand-sets the field. `stale_receipts`
stayed `not_available` because the honest investigation found the staleness
signal lives in `swarm-ingest.json`, which the attempt-ledger build never
consumes — so a non-zero is not producible, and declining to ship a count was
the correct result. The negative finding is a feature, not a failure.

### Verify the *claim* against the *real* repro, on a *fresh* binary

A builder's green test suite proves the cases it chose to test, which can be
exactly the cases that miss the bug. #1140's first pass disclosed preview-language
files only when the adapter was *enabled* — its tests passed, but the issue's
actual repro (`ripr check --diff ts.diff` with no config) was still silently
empty, and the builder's own "proof" had run a stale binary built from `main`.
Only a behavioral repro of the *reported* repro, against a freshly-built binary,
caught it. Stale IDE/editor diagnostics flagged phantom compile errors ~11 times
across this campaign; the committed `RUSTFLAGS="-D warnings"` build was clean
every time. The discipline that held the line: never merge on a report — merge on
the compiler and a fresh-binary repro of the real thing.

### The analysis usually already exists; the work is honestly *surfacing* it

Route-quality metrics, the attempt ledger, receipt lifecycle states — most were
already computed deep in the codebase. Each loop-close PR was a *surfacing*
exercise (a standalone report, an LSP field, a VS Code command), and the only
real risk at each step was fabricating a field whose producer wasn't actually
reachable. Cheap adversarial validation caught two such gaps on paper (a VS Code
command with no LSP producer; an agent doc describing commands that didn't exist
yet) before any code was written. Plan with cheap agents, surface with stronger
ones, and put the verification budget on the producer-reachability question.

## 2026-06-12: Release-State Boundary and crates.io Query Honesty

Two operator-grade lessons surfaced from a single stale sign-off phrase
("published 0.9.x untouched, crates.io the one hard stop") that turned out to be
wrong in two independent ways. Both are reliability lessons, not release notes.

### The release hard stop is the version bump, not an ad-hoc `cargo publish`

The published-vs-unreleased boundary, stated precisely:

```text
0.9.0 is published on crates.io.
ripr-swarm main is ahead of published 0.9.0 (all current campaign work is unreleased).
crates.io publish is AUTOMATED by the gh version-bump / GitHub release workflow.
The irreversible trigger is therefore the VERSION BUMP, not a manual `cargo publish`.
swarm CI only ever runs `cargo publish -p ripr --dry-run` (validation, never a real publish).
```

So the rule for autonomous work: normal swarm development is fine (PRs, tests,
dry-runs, docs, changelog drafts, release-candidate prep, source-sync prep), but
do **not** bump `crates/ripr/Cargo.toml`'s version or trigger the release
workflow without explicit approval — that lever auto-publishes. Publishing the
current work is an explicit next-release decision (choose 0.9.1 vs 0.10.0, audit
what is on main since 0.9.0, draft changelog, source sync, then bump).

### crates.io API errors are not release facts

A crates.io `403` can mean the request *reached* crates.io but failed the API
data-access policy — most often because no identifying `User-Agent` was sent. Do
not parse an HTTP error body as crate metadata; check the HTTP status first.

```text
Bad:  HTTP 403 body parsed as JSON -> max_version = None -> conclusion: "not published"
Good: HTTP 403 -> status: crates_io_query_failed (data-access / missing user-agent)
                -> conclusion: UNKNOWN, not "unpublished"
```

For any release-state check: use an identifying `User-Agent`
(`curl -H "User-Agent: EffortlessMetrics-ripr-release-check/0.1 (contact: ...)"`),
treat non-2xx as `unknown`/error rather than absence, and confirm published state
through a 2xx response or the crates.io UI. This is RIPR's own philosophy turned
on its own tooling:

```text
unknown is not zero
limited is not clean
failed lookup is not absence
```

The meta-lesson: the original misread blamed the local *sandbox* for what was
crates.io's *policy* (the network was wide open — 200s everywhere once a
User-Agent was sent). That is the exact proxy-for-artifact failure the tool
exists to catch, aimed at the operating environment instead of the code. Read the
error the artifact actually returned; do not round it off to the nearest
convenient cause.

## 2026-06-12: TypeScript repair-packet flip reuses the Rust validator, never a parallel path (RIPR-SPEC-0087)

The TypeScript `repair_packet_ready: false → true` flip (RIPR-SPEC-0087 §PR7)
calls the **existing shared** `validate_agent_gap_record_packet` in
`output/agent_seam_packets.rs`. It does NOT introduce a parallel TypeScript
completeness validator, mirror, or inline re-implementation.

The projection lives in `output/typescript_packet_projection.rs::typescript_gap_record_for`,
which builds a `GapRecord` from preview evidence. If the shared validator returns
`Ok(())`, the finding flips; otherwise it stays preview. This architecture means
there is exactly one source of truth for "what counts as a complete repair packet".

Key constraints for any future change:
- `analysis/**` must NOT import `crate::output` (architecture gate enforces this).
- The projection is in `output/`, not `analysis/`, to satisfy this constraint.
- G-A (category must be `incomplete_repair_packet`) is the most important precondition —
  it prevents static_limitation, strong_oracle_observed, ambiguous_related_test, and
  missing_context findings from accidentally flipping.
- The only flip condition: G-A through G-F ALL hold AND `validate_agent_gap_record_packet`
  returns `Ok(())`. There is no shortcut.
- `authority_boundary` stays `"preview_advisory_only"` even when flipped. TypeScript
  remains preview; the flipped finding is delegatable but not gate authority.

## 2026-06-13: Surface projection for a TypeScript packet goes through the shared renderer, not a parallel TS renderer (RIPR-SPEC-0088)

RIPR-SPEC-0088 (§PR8) projects the GapRecord computed in §PR7 into four surfaces:
human field-note, JSON `typescript_repair_packet` field, LSP hover section, and
LSP copy code action. The key architectural lesson is **reuse the shared helpers,
not a new renderer**.

Concretely:
- `typescript_gap_record_for(finding)` (in `output/typescript_packet_projection.rs`)
  returns `Option<GapRecord>`. Call it from each surface. `None` means "not actionable".
- `allowed_edit_surface_for_gap_route` and `gap_record_packet_do_not_do` from
  `output/agent_seam_packets.rs` provide the canonical allowed-surface and
  must-not-change lists. Use them everywhere to avoid drift.
- Forbidden-files computation was left inline in the JSON renderer (it filters the
  anchor file against the edit surface) because `forbidden_files_for_gap_record` is
  private. That is fine; the pattern is tiny.
- The LSP code action reads from `data.typescript_repair_packet` if present, with
  fallback to `data.verification_commands[0]`. This is because the JSON field is
  not yet in `diagnostic.data` — if a future PR adds it there, the action will
  prefer it.

The "not actionable" case surfaces a named limitation section in human output for ALL
TypeScript findings (not just specific ones). That drifted ~28 golden fixtures. Bless
them all: the named limitation section is the correct output for blocked findings.

When bless-count is unexpectedly large: first confirm that every drifted fixture
really is a TypeScript/JavaScript finding. If yes, bless with a reason that
cites the spec section. Do not suppress the limitation output.

Authority boundary reminder: `preview_advisory_only` stays in all four surfaces
even when the packet is actionable. No surface promotes TypeScript to gate or badge
authority.

### §PR8 follow-up: the flip must also rewrite the evidence strings, not just the boolean

The first §PR8 cut flipped `repair_packet_ready`/`gap_state`/`category` and added
the new field-note, but left the OLD incomplete-packet evidence strings
(`why_not_actionable`, `repair_route` = "...only after verify/receipt/edit
boundaries are available", `evidence_needed_to_promote`, and the analysis-layer
`recommended_next_step` "no actionable repair packet is emitted until...")
flowing straight through to the `Preview actionability` block, the preview card,
and the LSP hover. Result: the flagship actionable output simultaneously said
"category: complete_repair_packet / repair packet ready: true" AND "why not
actionable: ... / evidence needed: [the fields it already has] / project ... only
after [those fields] are available". A direct honesty contradiction.

Root cause: those strings are authored in the analysis layer for the BLOCKED
cases and are reused verbatim. The flip happens later in `output/` (via the
shared validator), so the analysis-layer strings never learn about it.

Fix pattern (one place, three consumers):
- In `output/preview_actionability.rs::preview_actionability_for`, when
  `repair_packet_ready`, replace `repair_route` with the actual repair action
  (assertion shape / missing discriminator via `actionable_repair_route`), set
  `evidence_needed_to_promote` to the empty string, and make
  `why_not_actionable` an actionable confirmation. JSON keys stay stable for
  schema compatibility; only content changes.
- In the three human/hover renderers, branch on `repair_packet_ready`: relabel
  "why not actionable"→"why actionable", "repair route"→"repair action", and
  omit the empty "evidence needed" line for the actionable case.
- The analysis-layer `recommended_next_step` is corrected at RENDER time
  (`next_step_for_finding`): strip the "; no actionable repair packet is emitted
  until ..." tail and confirm completeness. The analysis layer can't see the
  output-layer flip, so the renderer is the right seam.

General lesson: when a downstream layer flips a status, audit EVERY string that
was authored for the pre-flip status and still rides through. A boolean flip
without a message rewrite produces output that contradicts itself — the exact
proxy-for-artifact dishonesty `ripr` exists to catch, turned inward.
## 2026-06-13: Discrimination vs Coverage — `exposed` requires sink alignment

`ripr`'s value over coverage is one invariant: a strong oracle discriminates a
change only if it *observes the changed sink*. The Python classifier had drifted
into the coverage mistake — `reach + strong oracle => exposed` — crediting a
strong-but-orthogonal assertion (a wrapper's return value) as discrimination.
`classify_change` now requires the strong oracle's assertion to reference the
changed owner (by name or import alias) or a changed-sink identifier/literal from
the changed line before crediting `exposed`; otherwise it downgrades to
`weakly_exposed` with a typed reason. Protect this invariant on any future
classifier work — it is the line between a discriminator and coverage with extra
steps. See `docs/STATIC_EXPOSURE_MODEL.md` (Discrimination vs Coverage),
`RIPR-SPEC-0028` revealability, and the `strong_oracle_observes_owner` tests.

## 2026-06-13: Two error rates — and the dangerous one is silent

Trust requires tracking both: false-actionable (routed a repair for a behavior
that is discriminated; visible in emitted output) and false-`exposed` /
over-credit (called covered when no oracle discriminates; *silent* — emits
nothing). False-`exposed` is the worse failure: it is what makes a discriminator
indistinguishable from coverage, and a robustness sweep that counts emitted
findings is structurally blind to it. Measuring it needs ground-truthed
should-stay-quiet cases, not gap counts.

## 2026-06-13: External evidence finds blind spots; build the engine first

A saturated in-repo dogfood corpus (e.g. 26/26 all-pass) is necessary but says
nothing about accuracy — it is authored by the same people who wrote the
analyzer. Every real accuracy bug this campaign — `not_run` reporting a vacuous
`pass`, eval-sweep runtime measuring `cargo run` overhead, and the `exposed`
over-credit — was surfaced by running on external code, not by reasoning about
it. Build the external eval-sweep before deciding what to fix; the bugs come from
running it.

## 2026-06-13: Eval diffs must test both directions

Synthetic boundary-flip diffs only exercise the should-gap direction, so a
maximally over-eager analyzer scores perfectly on them. A trustworthy judged
panel needs deliberately-constructed should-stay-quiet cases (direct-boundary
assertions that must read `exposed`) alongside should-gap cases — otherwise the
eval itself has a weak oracle.

## 2026-06-13: A CI gate that runs but does not block over-credits itself

The `source-of-truth` job runs the policy gates — `check-support-tiers`,
`check-static-language`, `check-doc-index`, `check-campaign`, and the rest — but
branch protection on `main` requires only one status check (`Ripr Rust Small
Result`). Every policy gate is therefore *advisory at merge time*: it appears on
the PR, but a red result does not block the merge button. A PR merged with its
`source-of-truth` red (RIPR-SPEC-0088 landing without a `SUPPORT_TIERS.md`
reference), which silently broke `check-support-tiers` on `main` for every
subsequent PR until a one-line fix repaired it.

This is the product thesis turned inward. A gate that runs but does not block is
exactly a test that *reaches* the behavior but does not *discriminate* it: it
looks green, the run happened, but nothing actually caught the regression — the
gate over-credited itself the same way `reach + strong oracle` over-credits
`exposed`. The check executing is not the signal; the check *being able to stop a
bad merge* is. Verify which checks are genuinely required
(`gh api repos/<owner>/<repo>/branches/main/protection/required_status_checks`),
not which checks appear to run. Making `source-of-truth` a required check is the
fix; tracked as hardening issue #1181. This is distinct from the deliberate
`strict=false` choice recorded in the concurrency entry below — dropping the
*up-to-date* requirement is orthogonal to requiring `source-of-truth` *to pass*;
a check can be required-to-pass without being required-to-be-current. Until then,
every agent and reviewer must read the `source-of-truth` result themselves and
refuse to merge on red — the self-gate the watcher already enforces. Same
weak-oracle shape as the adjacent "all gates pass" entry, a different axis: this
one is about *merge authority* (an advisory gate cannot block), that one is about
*output honesty* (a well-formed artifact can pass every gate while lying).

## 2026-06-13: Gates and a builder's "all gates pass" are weak oracles — run + read the artifact

The TypeScript actionable wave shipped its sharpest dishonesty bug, the §PR8
honesty contradiction, *through* a fully green pipeline: `repair_packet_ready:
true` and "category: complete_repair_packet" were emitted right next to "why not
actionable: ... / evidence needed: [the very fields it already had]". That output
passed `cargo fmt`, clippy `-D warnings`, every `check-*` policy gate, and the
full test suite. Nothing flagged it — a self-contradicting string pair is still
well-formed Rust that serializes to valid JSON. It was only caught by **running
`ripr check` on a real TypeScript finding and reading the emitted block**. Tests,
gates, and a sub-builder's "all gates pass" report are proxies for the artifact,
not the artifact — exactly the proxy-for-artifact substitution `ripr` exists to
catch, here turned inward. For any output-shape change: run the binary on a
finding that reaches the new branch and read the human + JSON output with your
own eyes before declaring done. The fix lived at the render seam
(`crates/ripr/src/output/preview_actionability.rs:60-125` makes the flip atomic;
`crates/ripr/src/output/human/sections.rs:167-209` relabels "why not actionable"
to "why actionable" and drops the "evidence needed" line only when the packet is
complete).

### Builders also miss the gates outside their assigned subset

A second failure mode of "all gates pass": a builder runs the gates *it knows
about* for *its* slice and reports green, while a different `check-*` gate it
never invoked is red. This wave hit it twice — a goldens slice left
`check-generated` and `check-dependencies` failing (re-blessed goldens and
fixture `package.json` / `pnpm-lock.yaml` manifests were not reconciled into
`policy/generated_allowlist.toml` and `policy/dependency_allowlist.txt`), and the
projection slice left `check-network-policy` failing (new `curl`/`http`
references in `typescript_packet_projection.rs` needed allowlisting in
`policy/network_allowlist.txt`, even though they are comments and
absence-assertions that never touch the network). The CI definition of done is
the **full routed-rust `check-*` list**, not the builder's mental subset. Before
handoff, run every `cargo xtask check-*` gate the route runs, not just the ones
touched by the diff.

## 2026-06-13: Reuse the shared validator/renderer — a fork is caught by parity, not by review

The TypeScript `repair_packet_ready` flip does not introduce a TypeScript
validator: it projects a `GapRecord` and calls the same
`validate_agent_gap_record_packet` that owns the Rust flip
(`crates/ripr/src/output/preview_actionability.rs:66-68`,
`crates/ripr/src/output/agent_seam_packets.rs:839`). This is load-bearing and is
*enforced*, not merely a convention — the `validator_parity_*` tests in
`crates/ripr/src/output/typescript_packet_projection.rs:382-494` fail the moment
a fork drifts the TypeScript decision away from the shared authority. If you find
yourself writing a second validator or a second renderer for a new
language/surface, stop: wire it to `typescript_gap_record_for` plus the shared
validator and the shared helpers (`allowed_edit_surface_for_gap_route`,
`gap_record_packet_do_not_do`) instead. The corollary, learned from §PR8:
**reconcile derived/relabelled messaging in the layer that owns the final
decision.** The honesty contradiction happened because the flip lived in
`output/` but the contradicting strings were authored upstream and rode through
unchanged. The fix belonged at the output seam (the renderer that can see the
flip), never by teaching the upstream layer about an output-only status it cannot
observe. This ADR-anchored rule is recorded in
`docs/adr/0019-language-adapters-reuse-shared-packet-contract.md`.

## 2026-06-13: Concurrent N-wide campaigns collide on single-writer registries

Running the TypeScript wave alongside the Python eval-sweep campaign, both
4-wide, both grabbed `RIPR-SPEC-0086` from the single-writer spec registry
(`policy/doc-artifacts.toml` + `docs/specs/README.md`). The resolution that held:
**renumber-the-later-claimant** (the TypeScript specs advanced to `0087` and
`0088`) while keeping *both* registrations intact — do not delete the loser's
row, advance it. Registries with a single next-free slot (spec numbers, ADR
numbers, golden bless ledgers) are contention points that no per-PR gate detects
until merge. When launching concurrent N-wide campaigns, partition or pre-reserve
the shared-counter ranges up front; treat `check-spec-numbering` as the late net,
not the plan.

### Branch-protection `strict=true` is a merge livelock under N-wide concurrency

Under 4-wide concurrency, GitHub's "require branches to be up to date before
merging" (`strict=true`) is a livelock: every merge invalidates the other three
branches' up-to-date status, forcing a rebase + full re-run, during which another
merge lands and re-invalidates. With auto-merge already disabled (manual
`gh pr merge` after CI), `strict=true` adds no safety it does not already have —
it only serializes the queue into starvation. The fix was a **config change**
(`strict=false`), not a workflow change. The required check ("Ripr Rust Small
Result") still gates correctness; dropping strict only drops the up-to-date
*ordering* requirement that concurrency cannot satisfy.

## 2026-06-13: Escalate when the obstacle is structural, not after N grinds

The `strict=true` livelock above cost **three futile rebase-and-rerun cycles**
before it was recognized as a configuration property of branch protection rather
than a transient CI flake or a workflow bug to grind through. The signal that
should have triggered escalation immediately: the same operation succeeds in
isolation and fails *only* under concurrency, and each retry is invalidated by an
event outside the PR (another merge), not by anything in the PR. That is
structural — a property of the system's configuration — and no number of retries
fixes a structural obstacle; it is changed by editing the config or the topology.
Rule: when a retry's failure is caused by state outside the unit you control,
stop retrying after the first confirmation of the pattern and escalate to the
structural fix (config, branch protection, serialization policy). Distinguish
this from a genuinely transient tempfail (e.g. CX43 GC-age races), which *is*
fixed by an age-aware re-run — the test is whether the failure cause lives inside
or outside your PR.

## 2026-06-13: Watch for vacuous pass states

The same shape recurred four times this campaign, in four different subsystems:

```text
eval-sweep with repos_run == 0        -> a green-looking "pass" proving nothing
a CI gate that runs but isn't required -> green-looking branch protection
a strong-but-orthogonal oracle         -> exposed-looking discrimination
a README gate enforcing the old shape  -> docs-looking compliance
```

All four are one failure: **a system reports success without a discriminator for
the claim being made.** A pass needs both a denominator (something was actually
checked) and a discriminator (the check could have failed on the real condition).
Whenever a state can read "pass" with an empty denominator or a misaligned
discriminator, give it an explicit honest state instead — `not_run`, `advisory`,
`weakly_exposed`, or a typed `limitation` — never a silent green. This is the
unifying name for the eval-sweep `not_run` gate, the required-check gap, the
`exposed` sink-alignment rule, and the README gate retarget below; treat a new
"it passed" the way `ripr` treats a strong oracle: ask what it would have caught.

## 2026-06-13: Move the gate when the contract changes

A docs/artifact rewrite is only half-done if the validator still enforces the old
shape. The README could not actually become a front door while `check_readme_state`
still required `## Current Scope` / `## Current Capability Snapshot` — the repo's
own gate was pinning the stale capability-ledger model in place, and any future
edit would be dragged back to it. The gate is part of the product model: a stale
gate is a fossilized old decision that outvotes the new one. When changing a
governed contract, change the artifact, the gate that preserves it, and the docs
together in the same PR; otherwise the gate quietly wins.

## 2026-06-13: Background workflow agents mutate the shared working directory

A background planning fanout — whose agents were Explore (no Edit/Write tool) and
prompted as "read-only research" — nonetheless **authored a whole spec plus a
fixtures directory and edited four tracked files** into the working tree, because
Explore agents still have shell access (`cat >`, `git apply`, `mkdir`). The
files landed on whatever branch the main session occupied, and a `git add -A`
swept them into an unrelated PR; the contamination was caught only by reading
`git status` before pushing. Orchestration agents are not sandboxed from the
repo. So: run any workflow whose agents might write with **worktree isolation**,
not the shared tree; word planning prompts to forbid file creation; never
`git add -A` while a background workflow is live — stage explicit paths; and
diff `git status --short` against what *you* authored before every commit,
reverting foreign tracked files (`git checkout origin/main -- <path>`) and moving
untracked drafts aside rather than committing them. "Read-only by intent" is not
"read-only by capability."

## 2026-06-13: The wrong-key recurrence — count what the artifact emits

`eval_sweep::findings_have_parse_failure` reads `finding.get("class")`, but real
`ripr check --json` emits `"classification"` (`output/json/report.rs`). The
`"class"` branch is dead; parse-failure detection silently undercounts. Its unit
test uses `{ "class": "static_unknown" }` — the wrong key — so it stays green
against a fixture that does not match live output. This is the 2026-05-04 "Live
Source Beats Paraphrased Schema" learning recurring inside a metric: a parser and
its test agreed with each other and with neither the producer. When a consumer
reads another component's output by key, pin the key against a real emitted
sample (a committed golden `check.json`), not a hand-written fixture — and when a
new consumer is added (the classification-distribution counter), make it read the
*verified* key even if a sibling reads a legacy one. Tracked as a fix in #1191.

## 2026-06-13: Measured on real code — `ripr` is safe, and noisy for one reason

The Tier A sweep and Tier B judging across eight real external Python repos
(recorded on issue #1160) gave the first numbers on the two error directions, and
they are decisive about where `ripr` actually stands:

- **false-`exposed` (silent over-credit): zero**, on every repo. The conservative
  `exposed` rule — credit only a strong oracle that observes the changed sink —
  holds on code we did not author. This is the load-bearing result: `ripr` does
  not give false confidence.
- **false-actionable (over-suggestion): common**, and from a single cause. Every
  one (tenacity, anyio, structlog) was an oracle that *does* discriminate the
  change but reaches the owner through an **indirect call** the syntax-first
  analysis cannot trace: a local binding (`r.stop = stop_after_attempt(3)`), a
  function-result binding (`iterator = repeat(x, 0)`), an inline construct-call
  (`LogfmtRenderer()(...)`), or framework dispatch (a jinja template filter). The
  jinja case is genuinely opaque and a defensible limitation; the others are
  tractable.

So the honest support-tier verdict is **safe but imprecise**: trustworthy not to
over-credit, but currently low signal-to-noise on idiomatic real-world code. The
single precision lever is tracing indirect calls in **relation + oracle
extraction** (not sink-alignment). A `usable` claim must caveat this; the safety
half — the harder half — is already in hand. The error *shape* is what matters:
`ripr` errs visible-and-conservative (over-suggest), never silent-and-dangerous
(over-credit).

## 2026-06-13: Every layer of `ripr` can fail the way `ripr` exists to catch

`ripr` catches "a test that reaches the behavior but does not discriminate it."
This session that exact shape appeared at every layer of *building* `ripr`: a CI
gate that runs but is not required (reaches, does not block); a parser keyed on
`class` while the artifact emits `classification` (green, checks nothing); a
sweep over zero repos reporting a vacuous pass (no denominator); a worktree-
isolated implementation plan rated "go" that *assumed* a strong oracle running
the binary disproved (reached the files, discriminated nothing); and `ripr`'s own
indirect-call false-actionable. One failure: **a pass without both a denominator
and a discriminator** — the "vacuous pass" family already in this log. The
counter-discipline is one rule applied everywhere: **verify the artifact, not the
proxy.** Run the binary, not the code-reading. Read the required check's result,
not the merge button. Diff the golden, not "it passed." Treat an agent plan,
SHA, or diff as a lead, not a fact. Every time it was honored this session it
paid off; the two times a proxy was trusted (an "infra" failure that was a real
golden drift; a plan that assumed a non-existent oracle) it cost a cycle. The
full narrative lives in `docs/STATIC_EXPOSURE_MODEL.md`
("The discriminator test, turned inward").

## 2026-06-13: Dogfood Honesty-Audit Method — a repeatable playbook

This multi-wave method surfaced and fixed approximately ten real honesty bugs across a single campaign. Record it here so a future agent can re-run it cold.

### The audit question

For every surface and every pipeline stage, ask: can `ripr` emit a **fake-clean** (silence or under-report a real gap) or a **false-actionable** (claim `exposed`/covered when it is not)? Fail-open is the dangerous direction; fail-closed (`*_unknown` / `weakly_exposed` / named limitation) is safe. The product question under audit is the same one `ripr` answers on user diffs: "do the current tests appear to contain a discriminator that would notice if THIS changed behavior were wrong?"

### Two audit axes

**(a) Rendering surfaces.** Every surface that emits a finding — human, JSON, SARIF, GitHub annotation, LSP diagnostic, LSP hover, badge, repo-md, repo-SARIF, `explain`, `context` — must AGREE and must each carry the relevant disclosure. A fix applied on one surface (e.g. `reconcile_next_step`) must reach ALL surfaces via a shared helper plus an all-surface parity test; never fork a per-surface copy of the logic. See the "Reuse the shared validator/renderer" entry above for the parity-test pattern.

**(b) RIPR pipeline stages.** At each stage — Reach → Infect → Propagate → Observe → Discriminate — the classifier must fail-closed to `*_unknown` when it cannot prove its answer. A single fail-open default at any stage inflates the whole finding to `exposed`. Check every stage independently; an `exposed` rating that cannot be traced to a confirmed discriminator at the Discriminate stage is the silent over-credit this audit exists to catch.

### Per-gap discipline (non-negotiable sequence)

1. **Dogfood with adversarial fixtures.** Run `cargo xtask dogfood` and `cargo xtask fixtures`. If a new fix candidate is unclear, add a should-gap fixture and a should-stay-quiet fixture before writing any code.
2. **Producer-trace first.** For any classification or count change, identify the real production code path that would drive the field. A wrong flip — crediting a heuristic or a hand-set field — creates the inverse bug (a fake-zero or false-`exposed`). See the "not_available is better than a fake zero" and "Detection needs a real producer" entries.
3. **Fail-closed slice with BEFORE/AFTER fixtures.** The fix must: (a) downgrade the over-claim, AND (b) leave a correctly-discriminated case at `exposed`, proving no over-correction.
4. **Verify the artifact yourself.** Run the real command on a real finding and read the output. Gates, tests, and a builder's "all gates pass" are weak oracles — this campaign's sharpest honesty contradiction (`repair_packet_ready: true` next to "evidence needed: [the fields it already has]") passed every gate. The required discipline: `cargo check` for compilation, a fresh-binary behavioral repro of the exact reported scenario, and the full `cargo xtask check-*` gate list (not a hand-picked subset) before declaring done.

### Cross-references

This playbook generalizes several earlier entries: the "Detection needs a real producer" rule (fake-zero anti-pattern), the "any hash over a path must normalize separators" rule (content-addressed ids), the "full routed-rust gate list" rule (partial-gate leakage), the "register before launch" rule (spec-number collision under N-wide concurrency), and the "fail-open is the cardinal sin" principle throughout.

## 2026-06-13: The Single-Assertion Escape Hatch — method-level reach is not sub-expression discrimination

### The bug (first fixed in #1200, generalized in #1216)

`analysis/classify/reveal.rs` contained an escape hatch: when `assertion_count == 1`, the classifier credited the test with discriminating the changed sub-expression even when the assertion text referenced **none of the changed tokens**. A test that merely reached the owner method was being credited as observing the specific change, inflating the finding to `exposed` / confidence 1.00.

The fix for `MatchArm` (#1200) revealed the same pattern in `ReturnValue`, `FieldConstruction`, `SideEffect`, and `CallPresence` (#1216). A further variant — the type-blind-token hole — was found where a sibling-arm assertion could clear a guard via a shared enum qualifier without naming the discriminating arm's tokens.

### The durable rule

**Method-level reach must never be credited as sub-expression-level discrimination.** When the only basis for `exposed` is the single-assertion escape hatch with no token match between the assertion text and the changed sub-expression's tokens, downgrade `exposed` → `weakly_exposed`:

- Reach, Infect, and Propagate hold (the test reaches the owner and the change can infect the execution path).
- Observe and Discriminate are unconfirmed (the single assertion does not reference the specific changed token, so the oracle's discriminating power for this sub-expression is unknown).
- Emit `arm_observation_unverified` (or the analogous typed reason for the probe family) as the downgrade reason so consumers understand what evidence is missing.

### Why this matters

This is the Observe/Discriminate-stage instance of the general fail-closed rule (see "Discrimination vs Coverage" and "Two error rates" entries above). The escape hatch was intended for the case where a single comprehensive assertion covers the entire changed expression, but it over-fired whenever any assertion existed at all. Because the over-credit is silent — `ripr` emits a clean `exposed` with no caveat — it is the dangerous direction. The fix is to require at least one assertion token to match a changed token before the escape hatch fires; absent that match, `weakly_exposed` with a named reason is the honest answer.

Any future work on `reveal.rs` or the classification heuristics must apply this check per probe family and must be backed by both a should-gap fixture (where the single assertion genuinely does not observe the changed token, producing `weakly_exposed`) and a should-stay-`exposed` fixture (where a direct-token assertion keeps the finding at `exposed`, proving no over-correction).
## 2026-06-13: The local-callable "flip to exposed" goal was aimed at the wrong case

A multi-agent design pass confidently proposed a ~35-line relation fix to flip
`fixtures/python_local_callable_binding` (the tenacity
`stop = stop_after_attempt(3); self.assertTrue(stop(3))` shape) from
`weakly_exposed` to `exposed`. Reading the actual classifier disproved it twice
over, and the second disproof reframed the whole work item:

1. **A relation fix alone cannot reach `exposed`.** `classify_change` yields
   `exposed` only when `strongest_strength >= Strong` **and** `alignment.observes()`.
   `oracle_for_call` maps `assertTrue`/`assertFalse` to `OracleStrength::Smoke`.
   So even with a perfect relation, the smoke oracle falls through to
   `weakly_exposed`; and `classify_sink_alignment` only inspects `Strong`-rank
   oracles, and the oracle text `stop(3)` carries the local var `stop`, not the
   owner tokens `stop_after_attempt`/`__call__`, so it would read `orthogonal`
   anyway. Three coupled barriers, not one.
2. **`exposed` is the *wrong target*.** The golden
   `fixtures/python_broad_boolean_assertion` pins `assert is_priority(100)` — a
   *direct* call to the changed predicate owner under a broad boolean — as
   `weakly_exposed`/`smoke` **by design**. The tenacity case is that exact shape
   plus a local binding. Flipping it to `exposed` would contradict the golden and
   the discrimination-not-coverage contract: `assertTrue(predicate())` is a weak
   oracle on purpose (a single truthy check does not pin the boundary). The
   local-callable problem is therefore a **relation-diagnosis bug** (the card
   falsely implies no direct test exists), not a classification bug; the correct
   resolved state is `weakly_exposed`/`smoke`/direct-relation, *matching*
   `broad_boolean`.

The deeper correction is to the Tier B reading itself: the four measured
false-actionables split by **oracle strength**. tenacity's discriminating
assertion is `assertTrue(stop(3))` — a *smoke* oracle — so the *correct resolved*
state is `weakly_exposed`/smoke per `broad_boolean`, not a clean false-actionable.
(Be precise about cause: today `ripr` reaches `weakly_exposed` for a *different*
reason than the resolved one — the `same_stem` relation miss means it never links
the test, so it surfaces `oracle_strength: unknown`, not `smoke`. The relation fix
*surfaces* the smoke oracle and corrects the misleading "no direct test" card
without changing the class. Don't conflate "the assertion is smoke" with "`ripr`
detected smoke" — it currently detects neither the relation nor the oracle.)
jinja (`tmpl.render() == "exact"`, ExactValue), structlog
(`pytest.raises(ValueError, match=...)`, ExactErrorVariant) and anyio
(`pytest.raises(Cancelled)`) are *strong* oracles `ripr` never saw because of
relation/extraction misses (framework dispatch, cross-file, function-result
binding) — those are the **true** false-actionables, and the only ones
legitimately flippable to `exposed` (an empirical grep corroborates the gate: 14
smoke oracles classify `weakly_exposed`, 7 strong un-limited oracles classify
`exposed`). So the real precision lever is **linking the
strong oracle `ripr` is missing**, not crediting the weak smoke oracle it already
half-sees. The fixture and tracker were built around the weakest example.

**How to apply:** before "fixing" a `weakly_exposed`, check the oracle *strength*
of the discriminating test against the `broad_boolean` golden. If it is a broad
boolean / smoke assertion, `weakly_exposed` is correct and the work is a card-text
fix, not a classification change — chasing `exposed` there would drift `ripr`
back toward coverage. Reserve `exposed` flips for missed *strong, sink-aligned*
oracles. See the "verify the artifact, not the proxy" entry above — an agent panel's plausible
plan was disproved only by reading the classifier and the golden, not the plan.

## 2026-06-13: The first false-`exposed` — substring token alignment over-credits

An adversarial sweep across eight cloned Python repos found `ripr`'s first
confirmed false-`exposed` (the silent over-credit direction the whole product
exists to avoid). In anyio, changing `len(buffer) < max_buffer_size` to `<=` in
`send_nowait` read `exposed`/`changed_sink_token` even though no *strong* oracle
observes that boundary — because `classify_sink_alignment` matched changed-sink
tokens by **substring** (`text.contains(token)`), and the changed token `buffer`
is a substring of an unrelated `buffered_stream` oracle from a *different class*.
Crediting coincidental co-occurrence as discrimination is exactly the "drift back
to coverage" this log keeps warning about — and short, common tokens (`buffer`,
`len`, `key`, `_state`) are the worst offenders. Fix (#1224): match tokens only at
Python identifier boundaries (`oracle_text_observes_token`). `buffer` no longer
matches `buffered_stream`; whole words like `key` in `Invalid key` still observe.

Two durable points:

- **Verify an agent's count, not just its claims.** The sweep agents reported "6
  confirmed false-`exposed`." Reading the actual `ripr` output cut it to **one**:
  four were conservative classes (`static_unknown`/`weakly_exposed`) the agents
  mislabeled as over-credit, and one was a *correct* `exposed` they flagged in
  error. A false-`exposed` is only real when `ripr` actually emits `exposed`;
  bake that into the adjudication prompt or the agents conflate "the test doesn't
  discriminate" with "`ripr` over-credited." Re-running the sweep with the strict
  definition on the fixed binary returned **0** across the corpus, confirming the
  vector closed with no siblings.
- **The natural sweep stayed clean; this needed adversarial construction.** The
  honest claim is "0 false-`exposed` on natural single-diff sweeps; one found
  under adversarial token-coincidence probing, now closed." Both halves matter:
  the safety result is real, *and* the heuristic had a reachable hole.

**How to apply:** any token/substring match feeding `exposed` (alignment, escape
hatches, relation heuristics) must use identifier boundaries, never raw
`contains`. Guard new alignment code with a should-stay-`weakly_exposed` fixture
built from a *coincidental* substring (proven `exposed` without the guard,
`weakly_exposed` with it) — see `fixtures/python_substring_sink_alignment`.

## 2026-06-13: Cross-file inline construct-call — the precision lever that *is* contract-safe

The companion to the smoke-oracle reframing above: the Tier B false-actionables
that legitimately flip to `exposed` are missed **strong** oracles, and the
tractable one was a *relation* miss, not an alignment miss. structlog's
`LogfmtRenderer.__call__` change was discriminated by
`pytest.raises(ValueError, match='Invalid key…')` calling `LogfmtRenderer()(…)` —
an exact-error oracle — but `ripr` linked the wrong test file by name proximity
and never saw it. The fix (#1228) adds a `ConstructCall` relation that recognises
an **inline** construct-call `OwnerClass(…)(…)` on a `__call__` owner, so the
strong oracle is found and `key` aligns (post-#1224, as a whole word). structlog
flips `weakly_exposed → exposed`, correctly.

The discipline that kept it safe is the same boundary thinking: it is gated to
`__call__` owners (Guard A), requires the test to *import* the class (Guard B,
blocking same-name cross-module collisions), and an inline-only balanced-paren
check distinguishes `C()(…)` from the bound local `x = C(); x(…)` — so
`python_local_callable_binding` and `python_broad_boolean_assertion` stay
`weakly_exposed`, preserving the contract from the entry above. This is the shape
of a *good* `exposed` flip: a missed **strong, sink-aligned** oracle, linked
without widening the net. jinja (framework filter-dispatch) and anyio
(function-result binding + async non-value oracle) remain defensible limitations,
not bugs.

## 2026-06-14: Token coincidence is a false-`exposed` *family*, not one bug — and "no siblings" was premature

The substring entry above closed the `buffer ⊂ buffered_stream` vector (#1224)
and concluded the fixed re-run found "no siblings." Adversarial construction on
2026-06-14 disproved that: a second, structurally distinct false-`exposed`
exists, and it is the same disease.

The new vector (found while building the adversarial guard panel, #1244): owner
`TokenValidator.validate` changes `token` → `token.strip()`; the only related
test calls `proc.validate(...)` on an **unrelated class** `PaymentProcessor` and
asserts `== True`. `ripr` reads `exposed`. Two token-only steps compound:

- `body_calls_owner` links a `syntactic_call` via
  `contains_any_attribute_call(body, owner.name)` — a bare `.validate(` on *any*
  receiver, no type resolution.
- `classify_sink_alignment` credits `direct` /
  `strong_oracle_observes_owner_name` because the strong oracle text contains the
  owner's **bare method-name** token `validate`.

The unifying root cause is **alignment matches tokens, not entities**.
`buffer⊂buffered_stream` was a *substring* failure; this is a *whole-word* failure
where the word matches the *wrong owner*. Identifier-boundary matching (#1224)
does not help — `validate` is a whole word; it just belongs to a different class.
So the family is larger than "substring": **every alignment/relation site that
credits `exposed` on a name match without resolving identity is a latent
false-`exposed`.** The token-only sites today: `oracle_text_observes_token`
(owner-name / changed-sink), `contains_any_attribute_call` (bare `.method(`), and
same-stem relation.

Two durable points:

- **A closed-corpus "no siblings" is a statement about the corpus, not the
  analyzer.** Silent over-credit is found only by *construction* — engineering an
  input that *should* stay quiet — never by re-sampling. The first sweep's clean
  re-run was real; it just could not see a vector no diff in the corpus
  exercised. Read every "0 false-`exposed`" as conditional on the probe set.
- **Each confirmed false-`exposed` graduates into a pinned golden fixture.** #1244
  ships `fixtures/python_adversarial_buffer_token` and
  `python_adversarial_mock_call_not_value` as end-to-end goldens that fail CI if
  the coincidence ever credits `exposed` again — the unit test for
  `oracle_text_observes_token` is not enough; pin it at the *classifier output*.
  The owner-name vector
  (`fix/py-false-exposed-attribute-call-owner-name`) has its first guard pinned
  this way — `python_adversarial_same_method_other_class` is held at
  `weakly_exposed` — but this is a **partial hardening, not a closure**. The
  shipped gate requires owner-*class* identity for method owners; oppositional
  review found the same token-only disease still open on sibling vectors
  (free-function module identity, changed-sink receiver identity). The proper
  closure — resolving *receiver* identity at the relation layer — is deferred to
  `analysis/python-method-owner-receiver-binding-identity`, which supersedes the
  token-guard approach. Read this family as in-progress, not done.

**How to apply:** before crediting `exposed` from any name/token match, ask "does
this resolve to the *same entity*, or only the same *string*?" For a method
owner, the bare method name is too collision-prone to credit `direct` alone —
require the owner's class token / a receiver bound to it. When you touch one
token-matching site, audit the others; they share the disease. Prefer downgrading
token-only credit to a *visible* `weakly_exposed` over a *silent* `exposed` —
visible over-suggestion is the recoverable error.

## 2026-06-14: The fake-`exposed` is one cross-language class — now a standing gate, not whack-a-mole

The token-coincidence family above (Python identity vs string) and the oracle/seam
mismatches fixed across the fleet this run are **the same disease in different
taxonomies**: *evidence may not promote a finding to `exposed`/`strongly_gripped`/
`strong_oracle_observed` unless it **structurally matches the seam**.* The
instances:

- **Identity, not token** (Python): a strong oracle whose text merely *contains*
  the owner's bare name credits `exposed` (#1244/#1247).
- **Oracle kind, not just strength** (TS RIPR-SPEC-0104 #1248; Rust exemplar
  RIPR-SPEC-0103 #1243): an `exact_error_variant` oracle promotes a value/predicate
  seam, or a sibling `exact_value` oracle promotes an error seam — the kind must
  match the seam family.
- **Variant, not just family** (Rust RIPR-SPEC-0106 #1252, RIPR-SPEC-0107 #1254): an
  `unwrap_err` test credits only the seam whose parsed `Err(Variant)` it pins; a
  sibling variant or a generic `is_err()` stays `weakly_exposed`.
- **Stage confidence caps the headline** (Rust RIPR-SPEC-0109 #1219-D): an
  unproven infect/propagate stage caps the reported confidence; it can never read
  as certain.

**The capstone: RIPR-SPEC-0108 (`cargo xtask check-evidence-promotion-honesty`).**
Each confirmed fake-`exposed` graduates into a cross-language corpus
(`fixtures/evidence-promotion-honesty-corpus/corpus.json`) as a
`must_remain_non_promoted` case (plus `control` cases that must stay `exposed`).
The gate reads each charter fixture's **pinned golden** and asserts the class —
so it pins the *semantic* expectation **independent of the golden itself**, which
is the point: `goldens check` only asserts `binary == golden`, so a regression
that makes a charter fixture produce `exposed` **and** re-blesses the golden to
match would pass `goldens check`. **Goldens can encode dishonesty; the meta-gate
catches the dishonest re-bless.** Design rule: **share the invariant + the
adversarial corpus + the gate; do *not* unify the per-language matcher functions**
— Rust/TS/Python have legitimately different taxonomies and edge policies. A new
fix ADDS a `cases[]` entry; it never forks a matcher.

**Performance is part of honesty.** The LSP first-open ran the full-repo seam
inventory (~336s vs the CLI's ~14s diff pass), so every cockpit feature was
dead-on-arrival and agents would act on no state at all. The fix (RIPR-SPEC-0105)
defers the seam inventory off the interactive path — but it must **disclose** the
deferral (`run_status: "seams_deferred"`, a `limited`-family value) and never
present a partial/deferred run as complete. A fast path may be partial only if the
status says so.

**Verification harness can lie too.** "Verify the artifact, not the report" cuts
*both* ways: a wrong harness manufactures false **negatives**. Twice this run a
correct fix read as broken because (a) the behavioral run used a long relative
path (`../../../../../target/debug/ripr.exe`) that escaped the worktree and ran the
*main* checkout's stale binary, and (b) a local `cargo fmt --check` used a
non-pinned rustfmt and disagreed with CI's 1.95.0 / rustfmt 1.9.0. Run the
**absolute** worktree binary (`<worktree>/target/debug/ripr.exe`) and `cargo fmt
--check` under the pinned toolchain. When a fix "doesn't work" but the builder
insists it does, suspect your own harness before the builder — inject a unique
string into the output to confirm your edits are even in the binary you're running.

## 2026-06-15: Not every adversarial "false-`exposed`" is a bug — separate the runtime-equivalence floor from the static missing-discriminator

A broad red-team round (40 traps) surfaced a large residue after the token-identity
families were closed. Triaging by the **missing signal** — not the surface vector —
split them three ways, and only some are `ripr`'s to fix:

- **Tractable sink-precision (fix it).** The oracle observes the owner's *output* but
  the wrong *part* of it: a sibling dict key (`{"port": 9090}` changed,
  `cfg()["host"]` observed), a sibling list index, or an aggregate (`len(...)`). This
  is syntactic and in-contract — credit only when a strong oracle observes the
  *changed* element (changed key/index subscript, changed value, or whole-collection
  comparison). Fixed via the dict/list element gate (`field_construction_credit_ok`).
- **Runtime-equivalence floor (do NOT fix; document).** The oracle observes the
  changed output, but only *evaluation* shows old ≡ new for the test's input
  (operator identity at `0`/`1`, coincident slice/`len`, boolean short-circuit,
  ASCII `lower`/`casefold`). Detecting these = running the mutant; `ripr` is static
  and cannot, and cannot conservatively downgrade without also dropping the genuine
  discriminators (it can't tell `compute(10,3)==7` from `apply_discount(5,100)==5`
  without evaluation). This is the honest floor — see
  `docs/STATIC_EXPOSURE_MODEL.md` § The static/runtime boundary.
- **Static missing-discriminator (in scope, as a gap).** A boundary change
  (`>= → >`) is discriminated only at `total == threshold`; a far-from-boundary test
  is genuinely non-discriminating, but the gap is *nameable* and stays a valid
  repair-routing candidate — not floor, not `exposed`.

**The durable rule:** input-specific old/new equivalence is a runtime floor; a
syntactically nameable missing discriminator stays in scope. "Drive false-`exposed`
to zero" is not achievable purely statically — the honest target is *zero confirmed
in-contract false-`exposed`*, with the floor explicitly bounded. **A regression
caught the same run:** a literal-element gate that locates the brace with `find('{')`
mis-reads an f-string (`f"{value:.3f}"`) as a dict literal — require the expression
to *start with* the literal opener, and re-run the full adversarial trap set after
merge to catch downgraded positives (goldens won't cover a synthetic trap that has no
fixture).

## 2026-06-16: "Observed but not reached" is a distinct, tractable false-`exposed` family from "observed the wrong part"

Trap 45 (changed default value not exercised) is a *third* tractable sink-precision
shape, orthogonal to the dict/list/f-string "wrong part of the output" gates. Here the
oracle observes the owner's output *correctly and exactly*, but the changed code path
is **never reached**: `def render(name, verbose=True)` changes its default, yet the
only strong test calls `render("Sam", verbose=False)` — binding the parameter
explicitly, so the default is irrelevant and the assertion passes identically before
and after. This is *static and in-contract* (no evaluation needed — argument binding is
syntactic), so it is `ripr`'s to fix, unlike the runtime-equivalence floor. The
mirror image of error-path Class C (`raise` change on an untaken branch): both are
"strong oracle reaches the owner but the *specific changed behavior* is not exercised."

**Implementation rule that keeps it honest (fail open, never false-clean):** block
`exposed` only when you can *positively prove* every strong reaching call overrides the
changed default. Concretely (`changed_default_overridden_params`): (a) restrict to a
*pure* default-value change (added/removed default, rename, or method/classmethod owner
→ fail open — a method's implicit `self`/`cls` shifts positional indexing); (b) require
**each** strong related test to contain at least one *directly analyzable* `owner(...)`
call — if a strong test reaches the owner via an alias/wrapper the scanner can't
resolve, fail open (it might omit the parameter and be the real discriminator); (c)
treat `*args`/`**kwargs` unpacking or any unparseable call as fail-open. A coarse
"does any related test omit the param" gate is *not* safe — a sibling override test plus
an aliased omitting test would wrongly block. Per-candidate "must have a direct call I
can read" is what avoids the false-clean.

## 2026-06-16: Annotation-only suppression is safe at module scope, not in class bodies

The #1289 annotation-only no-probe family splits cleanly on owner scope. At **module
scope**, Python annotations are never enforced at runtime, so an annotation-only change
(identical target name and value, only the annotation text differs) has no behavior
delta and can be safely suppressed — mirror the `def`-header skeleton pattern
(`variable_annotation_skeleton` re-parses the line as an `AnnAssign` and compares the
target+value, excluding the annotation). Inside a **class body**, the same change is
behavioral: `@dataclass`, Pydantic `BaseModel`, and `attrs` drive runtime validation and
coercion from field annotations, so suppressing there would be a false-clean. The guard
is therefore `owner.is_module_owner()` first, and fails closed for every class body
until base-class tracking exists. This is the recurring rule for the whole annotation
family: the suppression's safety comes from *where* the annotation lives, not just from
*what* changed. (See `docs/DEFERRED.md` § python-annotation-only-no-probe for the two
remaining open sub-cases: class-body annotations and multiline-docstring interiors.)

## 2026-06-26: Perl mapper honesty — owner-target is not sink observation; the producer gate is the wrong harness

From the Campaign 31 Phase D mapper hotfix (PR H1, #1409). Two distinct lessons,
both load-bearing for any preview-language adapter that consumes a producer's
fact packet.

### Owner-target identity is not changed-sink observation

The Perl packet can prove `oracle.target_owner_id == changed_owner_id` — the
oracle targets the same owner the change lives in. That is **not** the same as
the oracle observing the **specific changed sink**. The production Finding
leaves `observed_sink`, `oracle_alignment`, and `alignment_reason` all `None`
because the packet carries no sink-level detail. Crediting reach-plus-a-strong
-oracle as "already discriminated" on owner-target identity alone is exactly the
recurring false-`exposed` family (cf. "Token coincidence" above): proximity
dressed up as discrimination.

The honest interim policy, until the producer contract adds
`ChangeFact.changed_observable` and `OracleFact.observed_sink`, is to **fail
closed**: a strong oracle aligned to the owner stays `WeaklyExposed` (or is
downgraded to `ReachableUnrevealed` for advisory relations), never promoted to
an "observed" claim. Do not encode the three-way matrix until sink alignment is
real; split "mapping integrity" (H1) from "classification semantics" (H2) so the
integrity fix can land without assuming the unprovable. This mirrors the
"Real producers only" rule: do not flip a field to a fabricated taxonomy before
a real production condition populates it.

### The feature gate makes the default-feature gate a false-green oracle

`lang-perl` is **not** a default feature (`crates/ripr/Cargo.toml`: `default =
["lang-rust","lang-typescript","lang-python"]`). The Perl module is
`#[cfg(feature = "lang-perl")]`. CI's `cargo clippy --workspace --all-targets`
runs on default features, so it **never compiles the Perl module** — it reports
green for code it did not see. Any validation command for a feature-gated module
must pass `--features <feature>` explicitly, or it is the wrong harness
manufacturing a false negative. (This is the "verify the artifact" rule cutting
the other way: a gate that passes because it never ran is not evidence.) The
signal that you have the right harness: the targeted test count is non-zero and
the module's symbols resolve.

### Concrete shape

PR H1 rewrote `packet_to_findings` to route through the packet-owned helpers
(`related_test_evidence_for_change`, `verify_command_for_test`,
`has_blocking_dynamic_boundary`, `canonical_gap_identity_for_change`) instead
of a parallel classifier. Before H1, every `related_test.file` was built from
the **production** source path (`PathBuf::from(&file.path)`) — the edit surface
could point at `lib/*.pm`. The cardinal regression was latent (the projection
gate returns `None` at the `gap_state:` check before production findings reach
it), not live — but it would have flipped `repair_packet_ready: true` against a
production file the moment H2 wired the evidence. The 8 adversarial tests added
were the **first** direct coverage of the mapper; it previously had zero, which
is why the bug survived three merged PRs.

Followups tracked separately: H2 classification semantics (after cross-repo
contract freeze adds sink fields), and a `perl-lsp-swarm` CI scratch-GC fix
(that repo's orphan reaper searches `/mnt/ci-scratch -maxdepth 1 -name 'ripr-*'`
but the per-run dirs nest under `/mnt/ci-scratch/perl-lsp-swarm/ripr-*` and
`/mnt/ci-scratch/tmp/ripr-*` — `ripr-swarm`'s own `scratch-gc.yml` does the
sweep correctly and is the reference pattern).

## 2026-07-12: A related test is not a repair route without producer facts

The first authorized internal-repository pilot found a real false-actionable
shape in `ub-review`: a field-construction seam was rendered `actionable` even
though producer evidence supplied no concrete missing discriminator, and the
suggested test selection resolved to a production source file. The targeted
rerun correctly rejected that selector as ambiguous, but the earlier review
projection had already made the route look safe.

The shared Rust evidence-record decision now fails closed when either invariant
is absent: no producer-owned discriminator means `static_limitation`, and a
test target equal to the production seam means `static_limitation`. Review
comments project the same decision: limitations have no repair target, verify
command, or receipt command; they carry the named investigation route instead.
Keep the real pilot row excluded until the corrected analyzer produces a
complete before/after route. A plausible test name or weak related-test match
is context, not permission to mutate.

## 2026-07-19: The gate does not gate — hardcoded seam_id breaks the blocking path

`gate/repair_route.rs:114` hardcodes `seam_id: None` for gap-ledger candidates.
`missing_route_fields` always pushes `"seam_id"`, so `gate_repair_route_is_complete`
is always false for them. Since `candidate_is_policy_eligible` requires a complete
route, `eligible` is always false, so `would_block` is always false. The gate
emits `advisory` and `gate_decision_should_fail` returns false — CI exits 0.

The tests named `*_fails_closed_*` (`tests.rs:764`, `tests.rs:1761`) assert
`advisory` status with `!gate_decision_should_fail` — they are **fail-open at
the CI level** despite their names. This contradicts `CALIBRATED_GATE_POLICY.md:74-75`
which says baseline-check/calibrated-gate "blocks for new baseline misses."

The recurring lesson: a test named `fails_closed` that asserts exit-0 is not
fail-closed. The exit code is the oracle, not the test name. When the product
contract says "blocks," the test must assert a non-zero exit — otherwise the
test encodes the bug as the expected behavior, and the bug survives indefinitely.
This is the "verify the artifact, not the report" rule applied to the gate's
own test suite.

## 2026-07-19: Static receipts are advisory — fabrication is trivial

The repair-receipt chain (`ripr agent verify`, `ripr agent receipt`,
`ripr receipt write`) performs no test re-execution, no git head binding, and no
signature verification. `ripr agent verify` reads two JSON files and computes
static movement between them. `ripr receipt write --current-head` is optional
and only format-validated (40 hex chars) — never compared to `git rev-parse HEAD`.

This is honest about ripr being a static analyzer: the receipt is a static
movement record, not a runtime proof. The output carries `status: "advisory"`,
`safe_to_merge: false`, and `provenance.runtime_mutation_execution: false`.

But downstream consumers (CI gates, dashboards, agents) who treat the receipt
as proof of testing are deceived. The lesson: a static analyzer's receipt is
only as trustworthy as the chain of custody from the analysis to the consumer.
ripr should stamp the receipt with the analyzed head SHA (resolved via git,
not caller-supplied) and the artifact content hashes, so a consumer can at least
verify the receipt corresponds to a real analysis at a known commit — even if
it cannot verify the analysis was correct.

## 2026-07-19: Every save re-runs the full pipeline — the cache exists but isn't wired in

`RustAdapter::analyze_diff` (`rust.rs:655`) calls `rust_index::build_index` →
`build_index_with_adapters` (`build.rs:80`, **uncached**). The cached variant
`build_index_from_loaded_files_with_cache` (`build.rs:19`) and `RepoFilesFactCache`
(`seam_cache.rs:769`) exist and work — but are only wired into the repo-seam
inventory path. Every `ripr check` and every LSP `did_save` re-reads and re-parses
every indexed file with `ra_ap_syntax` from scratch.

Additionally, `advance_workspace_revision` (`backend.rs:628`) bumps a counter on
every `did_save`, and the counter is part of `LspAnalysisInputIdentity`
(`input_identity.rs:17`). The dedup path at `refresh_scheduler.rs:204-221` compares
identity — but since every save produces a different revision, dedup never fires
for saves. The one cheap fast-path that exists is dead code in practice.

The lesson: a cache that isn't wired into the hot path is documentation, not
infrastructure. When you build a cache, wire it into the diff-scoped path (the
path that runs on every save), not just the repo-scoped path (which runs rarely).
And: a dedup identity that includes a monotonic counter will never dedup — use a
content hash or omit the counter from the identity comparison.

## 2026-07-19: `continue-on-error: true` on every step makes green meaningless

The generated CI workflow (`init.rs`) uses `continue-on-error: true` on ~30 of
~31 steps. Only the gate step and the diff-capture step lack it. A consumer
sees a green job and missing artifacts (SARIF, badge, reports) with no signal
that anything failed.

The lesson: advisory CI steps are fine, but they must be clearly separated from
load-bearing steps. A step that produces the gate input (`review-comments`) is
load-bearing — if it fails, the gate has no input and the "green" is a
false-clean. Reserve `continue-on-error` for genuinely advisory outputs (badge
rendering, policy reports), never for the analysis pipeline itself. Add a final
summary step that checks for expected artifacts and surfaces missing ones as
a visible warning.

## 2026-07-19: `Result<_, String>` everywhere is the single highest-leverage refactor target

2,474 `Result<_, String>` signatures across 170 files, with 1,254
`.map_err(|err| format!("...: {err}"))` call sites. Zero typed error enums in
production (only 2 defined: `DiagnosticBudgetError`, `ArtifactReadError`).

This blocks:
- Programmatic error handling for library consumers (callers cannot match on
  error variants)
- Clean `# Errors` documentation on public API functions
- The public library surface from being credible (`Ok::<(), String>(())` in
  the quick-start example is a tell)

The lesson: `String` errors are acceptable for a CLI binary but become
technical debt the moment a library surface is exposed. The fix (introduce
`thiserror`, migrate module by module) is mechanical and low-risk, but the
payoff is structural: every downstream consumer (the LSP backend, the agent
loop, external embedders) gains the ability to distinguish `Io` from `Git`
from `Parse` from `Analysis` errors.

## 2026-07-19: Cross-language consistency requires a shared vocabulary layer

`SeamKind` (7 variants) is Rust-only. Preview adapters use `ProbeFamily` (8
variants) — different variant names, different cardinality, no canonical
crosswalk. Perl defines its own `OracleKind` (12 variants) and `OracleStrength`
(5 variants) separate from the domain enum (9/6). Python and TypeScript never
emit `ReachableUnrevealed`, `InfectionUnknown`, or `PropagationUnknown`.

The lesson: a shared `LanguageAdapter` trait is necessary but not sufficient.
The trait ensures structural consistency (same method signatures), but the
*classification vocabulary* diverges because each adapter independently
decides which domain values it can produce. A shared vocabulary layer — either
a trait method that declares "this adapter can emit these ExposureClass values"
or a canonical crosswalk from `ProbeFamily` to `SeamKind` — would prevent the
silent gaps where a preview-language change whose probe is reached-but-unrevealed
falls through to `StaticUnknown` instead of `ReachableUnrevealed`.

## 2026-07-19: A file-policy gate that fails on main breaks every subsequent PR

During this session, a merged PR (#1836, authority map) added
`.allow/conformance/legacy-dialect.json` without adding a `non-rust-allowlist.toml`
entry. `check-file-policy` — a required CI gate — broke on main. Every subsequent
PR inherited the failure. The fix (#1848) was a one-line allowlist addition, but
it blocked ~15 open PRs until it landed.

The lesson: when a gate validates "every file must be in an allowlist," adding
a new file type in one PR without the allowlist entry is a main-breaking change.
The fix is either: (a) make the gate advisory with a warning instead of
required, or (b) add a pre-commit hook / xtask check that proposes the
allowlist entry when a new file type is detected. At minimum, the gate's error
message should say "add an entry to `policy/non-rust-allowlist.toml`" — which
it does, but the breakage was on main, not in the PR that added the file.

## 2026-07-19: Duplicate run-status logic drifts — extract or unify

`workspace_status_run_status` (`backend.rs:2518-2545`) and
`snapshot_run_status` (`diagnostics.rs:821-843`) are near-identical
implementations of the same five-state decision tree. The `diagnostics.rs`
version's doc comment says "This replicates the logic of
`backend::workspace_status_run_status`" — an acknowledged copy. They differ
subtly: `backend.rs` checks `gap_artifacts.iter().any(|a| a.has_static_limit())`
at line 2533, which `diagnostics.rs` omits.

The recurring lesson (cf. "Reuse the shared enforcement layer" in AGENTS.md):
when two functions implement the same decision, they will drift. The drift is
not a question of *if* but *when*. Extract the logic into one function and call
it from both sites. The cost of extraction is always lower than the cost of the
bug that drift produces — especially when the drift is in a fail-closed
posture (one copy discloses `seams_deferred`, the other doesn't).

## 2026-07-22: The required CI lane must invoke the local gate table, not enumerate a copy

The routed-rust lanes enumerated an xtask check list that had drifted from
`cargo xtask precommit` by ten gates (`check-architecture`,
`check-readme-state`, `check-workspace-shape`, `check-public-api`,
`check-doc-artifacts`, `check-doc-index`, `markdown-links`, `check-pr-shape`,
`check-proof-packs`, `check-lint-policy`). Main broke three times in one day
(#2234, #2240, #2257) when PRs passed the required lane but violated a gate
that only ran locally — and `check-architecture` is textual, so docs-only and
test-only diffs are not exempt. The fix (#2265) makes every required lane and
the docs-gate invoke `cargo xtask precommit` as the single shared table and
pins the composition with drift tests.

The lesson: never maintain two enumerated copies of a gate list — one always
drifts. A lane should invoke the same entry point developers run locally; if
a gate is too slow for the lane, narrow the documented contract explicitly
rather than letting the lane silently skip it. Command-catalog truth then
needs a semantic expansion rule (an enforced `precommit` invocation
transitively enforces its table), not just string matching.

## 2026-07-22: Advisory databases move under a green main

`cargo deny check advisories` went red on main with zero repo changes when
RUSTSEC-2026-0190 (anyhow `Error::downcast_mut` unsoundness) was published;
the locked anyhow 1.0.102 predated the patched >=1.0.103 floor. The fix was
an in-range lockfile bump (#2263), not a suppression.

The lesson: a cargo-deny failure on an unrelated PR is often a newly
published advisory, not the PR's diff. Reproduce on clean main first; the
fix is usually `cargo update -p <crate>` within the existing semver
requirement. Related verification: cargo-deny 0.18.9 does not deserialize
`until` fields on advisory ignores (verified at 0.19.0), so expiry enforcement
for long-lived suppressions must live in repo tooling (an xtask check reading
dated comments), not in deny.toml syntax (#1949).

## 2026-07-22: Canonical-input validation must compare bytes, not parsed values

The receipt verify-input validator compared two parsed `serde_json::Value`s;
hand-authored JSON with identical values but different key order or spacing
passed as "canonical" (codex P1 on #2254, reproduced by compact
re-serialization of real `agent verify` output). The fix compares the
supplied document against the canonical rendered bytes (trailing-newline
tolerant) and parses the downstream value from the canonical body, with a
regression test that re-renders real output differently and expects
rejection.

The lesson: whenever a contract says "must be the exact output of tool X",
equality of parsed values is not the contract — byte identity is. Parsed-value
equality silently accepts any producer that emits semantically equal JSON.
This generalizes beyond receipts: any "canonical" or "producer-bound" input
check should pin bytes (or a digest of bytes). On the fail-closed seams
(repair packets, receipts, provenance) that distinction is the difference
between validation and theater.

## 2026-07-22: A squash merge is invisible to branch-ancestry checks

A builder reported "PR #2259 is not merged" because the PR's branch head was
not an ancestor of main — but the PR had squash-merged hours earlier, so main
contained every change under a new commit while the branch head remained
outside the ancestry. The builder still flagged the right collision risk (its
own diff rewrote the same file), but for the wrong reason.

The lesson: in a squash-merge repo, "does my base include PR N" means "does
main contain N's squash commit" (check `git log main --grep "(#N)"` or the
PR's merged-at timestamp against your base), never "is the PR branch head an
ancestor". Sub-agent prompts should state the merge mechanism whenever base
recency matters, and builders should re-fetch main before pronouncing a PR
unmerged.

## 2026-07-23: A both-append rebase splice can silently corrupt JSON with duplicate keys

During the #2272 rebase, a "take both appended blocks" splice of
`fixtures/evidence-promotion-honesty-corpus/corpus.json` fused two case
objects into one object with duplicate `id`/`language`/`tier`/`source_fixture`
keys. `serde_json` (and Python `json`) apply last-wins to duplicate keys, so
the file still parsed, every gate stayed green — and the
`ts_hoc_wrapped_owner` case silently vanished from the parsed corpus, taking
its non-promotion pin with it. Caught only by a coderabbit review thread.

The lesson: JSON is not append-splice-safe the way TOML/Markdown lists are.
When a rebase conflicts on a JSON array, re-apply your own append on top of
the new base (or parse the result and compare case counts/ids against both
parents) instead of text-splicing. And when a corpus is authority for a
fail-closed gate, the loader should reject duplicate keys outright — the
gate hole itself is fixed in #2279 (`parse_json_rejecting_duplicate_keys`),
with a red-verified negative test. Verify the artifact, not the parse: "it
parsed" is not "it contained all the cases".

## 2026-07-23: A PR-body verification claim must be an executed experiment

While opening #2279 the draft body asserted the negative test was "verified
red before the loader swap" — but the parser and test had been written
together and only ever run green. The claim was caught before merge and made
true by actually reverting the loader swap, watching the test fail with the
exact last-wins symptom (the spliced corpus returned `Ok` and the surviving
case was the wrong one), then restoring.

The lesson: treat verification sentences in PR bodies as debt until executed.
"Red before, green after" is a two-run experiment, not a narrative device —
if you wrote fix and test together, you have only run one of the two arms.
This is the guard-disable-experiment discipline applied to prose: every
behavioral claim in a PR description should name a run that could have
contradicted it.

## 2026-07-23: Merge-then-cleanup must be `&&`-chained, and red-arm experiments must not use `git checkout --`

Two self-inflicted recovery incidents in one day, same family:

1. `gh pr merge N && gh-cleanup; git branch -D ...` — the cleanup ran on `;` even when the merge was refused (unresolved threads, GitHub recompute lag), deleting the local branch and worktree out from under an open PR. Three recoveries. Chain cleanup behind the merge's exit code with `&&`, or run cleanup only after confirming `state: MERGED`.
2. During a red-arm experiment (revert the fix, watch the test fail), restoring with `git checkout -- <file>` also reverted the *uncommitted fix itself* when the fix had not been committed yet — twice. For uncommitted work, snapshot with `git stash` (and `git stash pop`), or re-apply the edit explicitly, never `checkout --`. After any red arm, re-run the full target suite and read `git status` before believing "restored".

The lesson: a recovery command restores the LAST COMMITTED state, not the state you were just working in. Any destructive-restore step in an experiment protocol needs the uncommitted-delta question answered first: "what is the nearest committed checkpoint, and is everything I care about behind it?"

## 2026-07-23: mergeState BLOCKED is usually recompute lag, not a policy wall — and auto-merge rides it out

Branch protection here is one required check (`Ripr Rust Small Result`), `strict: false` (no up-to-date requirement). Several merges were refused with `BLOCKED` while the required check was green and `mergeable: MERGEABLE`: GitHub's mergeState computation lags a fresh check completion or a main move. The earlier habit of attributing every BLOCKED to a stale base (and rebasing reflexively) was partly superstition — strict:false means old-head checks stay valid.

The working protocol: check the required check + `mergeable` + unresolved threads. If all green and BLOCKED persists, `gh pr merge N --squash --auto --delete-branch` queues the merge and it fires when GitHub's state settles (#2287). Rebase for content reasons (real single-writer collisions, golden freshness), not to placate a lagging state machine.

## 2026-07-23: Queued-forever CI runs, local gate-list discipline, and worktree-first cleanup

Three lane-operations lessons from the #1628/#2119 wave:

- A workflow run can sit `queued` forever while newer runs on other branches get picked — the queue is not strict FIFO and a wedged queued run cannot be `gh run rerun` ("workflow file may be broken"). Diagnose by comparing `gh run list --workflow <name>` across branches: if a newer run started while yours sat queued for >10 min, cancel it and push an empty re-trigger commit (`git commit --allow-empty`); the fresh run schedules normally (#2306, run 29996424587).
- Run the FULL routed `check-*` list locally before pushing, not a hand-picked subset: `check-local-context` caught a Windows drive-letter path literal that the "usual" subset missed (#2289). The gate also scans docs — quoting such a literal in Markdown re-trips it (this entry's first draft failed CI that way); describe the class ("drive-letter path"), never the literal. HEAD-scanned gates (check-process-policy and friends) only see committed content — run them after committing, and re-run after any amend.
- `gh pr merge --squash --delete-branch` fails its local branch deletion when a worktree still holds the branch, and exits non-zero even though the merge itself succeeded — the `&&`-chained cleanup then never runs. Remove the worktree BEFORE the merge, but only after confirming `git status --short` is clean in it (or deliberately stashing/committing what matters): `git worktree remove --force` discards uncommitted work silently. Alternatively chain cleanup as separate steps and verify each. Treat a non-zero `gh pr merge` exit as ambiguous: always confirm the merge state with `gh pr view N --json state` before assuming failure.

## 2026-07-23: Verify the worktree branch a builder left behind — twice burned in one PR

A builder switched its worktree onto `main` twice (once mid-run, once at completion as a "courtesy"), and two rounds of root commits landed on local `main` instead of the feature branch — the tell was `git push` attempting `main -> main` and bouncing off branch protection. Recovery each time: force-move the feature branch to the stray commit, `git switch` to it (cherry-pick when the stray commit's parent was wrong — and expect the cherry-pick to conflict against context the branch already changed), then restore the shared `main` pointer from the main worktree with `git reset --mixed origin/main` (content-identical, so non-destructive). The cheap prevention: `git branch --show-current` is now part of the pre-commit ritual in any builder-touched worktree, and commits in builder worktrees always run with an explicit branch check before `git push origin <branch>` (never bare `git push`).

Two adjacent lessons from the same PR (#2317): adding a field to a widely-constructed struct (`AnalysisOptions.git_timeout`) cascades `field: None` into every literal site — including files outside the planned cage (`cli/commands.rs`) and into `policy/no-panic-allowlist.toml` receiver fingerprints, which embed the literal text and must be updated in the same PR. And the xtask policy facade (`tests::policy_checker_facade_runs_current_repo_checks`) runs gates a hand-picked `check-*` list misses — it caught a second `allow(dead_code)` over the per-file cap that the individual gate list never ran. Run the facade test locally before pushing policy-adjacent changes; prefer deleting a dead wrapper over bumping an allowlist cap.

## 2026-07-23: Branch-switching builders are a repeating class, and stall recovery has a pattern

Third instance of a builder leaving its worktree on `main` (agent-96, #2300) — the recovery protocol from the earlier entry worked unchanged, but the prompt rule needs to be sharper than "verify `git branch --show-current` before commit-adjacent steps": builders must treat *finding themselves on main* as a stop-and-report condition, never something to commit through. Root-side, `git push` output now gets read, not skimmed: "Everything up-to-date" after a feature commit is the tell that the commit went to the wrong branch.

Two stall recoveries from the same wave: (1) a builder that sits 60+ minutes with zero files and no process is not "reading carefully" — kill it and either re-spawn fresh with a capped-reading, start-writing-immediately directive (agent-94 → agent-95 produced the full #1972 slice within the hour) or take over in root when the design context is root-held (agent-91 → #2299 shipped root-built). (2) A resume after TaskStop preserves context and breaks model loops — a narrow "write exactly these N steps, reading is done" resume prompt finished the stalled backend.rs step on the first try.

Also: `Duration::from_mins` / `from_hours` ARE stable in the pinned 1.95.0 toolchain (core/src/time.rs:450) — review bots flag them as nonexistent roughly once per PR now; the rebuttal is one line with the toolchain evidence, and the required check compiling is the proof. And one CI-hygiene note: a `HEAD...HEAD: unknown revision` failure signature in fixture tests on CX43 was a one-off setup race under runner load (rerun green, taskset-clean locally) — if it recurs, it becomes a flake issue, not a debug-the-diff issue.

## 2026-07-25: A green check is not evidence — five ways a passing signal covered a broken surface

The #2390/#2391/#2409/#2429/#2393 wave fixed 27 deterministic Windows failures. Every one of them had been green on `main` indefinitely, and the recurring shape was not "nobody tested it" but "the passing signal did not exercise the thing it appeared to cover." Five distinct instances, all worth recognising by shape:

- **A single-platform test cannot cover a platform-specific branch.** A path-confinement guard was hardened to reject rooted anchors; the pre-existing test passed on Linux *with the old code*, because `is_absolute()` already caught the Unix form. Green CI proved nothing about the new branch. The repo's own `tests-oracle` check caught this. The rule: when platform semantics select different branches, expose the branch decision as a small pure function returning *which* reason fired, order the checks so the shared shape is attributed identically everywhere, and assert each platform-specific branch under `#[cfg(...)]` with a counterpart asserting the other platform's correct behavior. A test that looks cross-platform and proves nothing on one side is worse than two honest per-platform tests.
- **WSL settles cross-platform questions in seconds, and reasoning does not.** Three claims about Unix `std::path` behavior were wrong in one session, including asserting that a backslash-leading path yields a root component on Unix (it does not; a backslash is not a separator there) and shipping a `replace('\\', "/")` normalizer that rewrites the legal Unix filename `od\d.rs` into a two-segment path. `wsl -d Ubuntu` has `rustc`; a ~20-line probe compiled there answers definitively. For portable path output, join `Component::Normal` parts with `/` — never string-replace separators.
- **`cargo test` stops after the first failing target, so later targets are invisible.** While `--lib` was red, the `cli_smoke` binary never ran; a deterministic failure in it therefore appeared to "fail 1 of N runs" and was filed as a flake. What actually varied was whether an unrelated flake aborted the run first. Any "failed N of M runs, therefore flaky" claim is unsafe while an earlier target is also red — establish presence by running that target directly. Each fix in the chain revealed the next previously-unreachable failure, three layers deep.
- **A fixture helper that swallows a prerequisite failure blames the wrong subsystem.** `init_git_repo` used `Command::output()?`, which propagates only *spawn* failure; a git command that ran and exited non-zero was ignored, so the helper returned success having produced a repo with no commit and no refs. Seven tests then failed asserting things about default-base resolution. Fixture helpers must fail at the prerequisite boundary and name the command, status, and stderr. Likewise a fixture built by interpolating a path into a raw JSON string parsed to zero records on Windows, and the tests ran anyway — a test over a parsed artifact must assert its own input is non-empty before asserting anything downstream.
- **A policy gate can be structurally blind to the file class it governs.** `check-workflows` passed a workflow whose `run:` was truncated mid-command, because it does line-budget and text checks and never parses YAML. In a plain YAML scalar, ` #` starts a comment; the command was cut, leaving an unterminated quote. The dangerous variant is silent: with balanced quotes, truncation runs a *shorter* command and reports success. Now gated (plain-scalar `run:` containing ` #` is rejected) and every lane step uses a `|` block scalar.

Two method notes from the same wave. A mutation experiment must make behavior wrong, not code uncompilable — deleting a match arm produced a compiler error and demonstrated nothing; making the arm inert produced the vulnerable return value and the assertion failed as intended. And in PowerShell, `-match` and `Select-String` are case-insensitive by default, so a check for `FAILED` matches `0 failed` and inverts the result: an experiment reported "3/3 runs failed" for a subset that was 8/8 clean. Detect failures with an anchored case-sensitive match on the libtest shape so the measurement yields failing test *names* rather than a boolean.

Finally, two disclosure lessons. Third-party review providers post quota-exhaustion notices as ordinary comments — one as a `COMMENTED` review — so a PR page can show four reviewers "having reviewed" when none read the diff; unavailable review is evidence of missing review, never clean review (#2432). And a CI watcher must distinguish `cancelled` from `failure`: a run superseded by a newer push is expected and meaningless, and collapsing every non-success conclusion to "failed" produced two false alarms.

## 2026-07-25: False-confidence gates — the enforcement-layer cardinal sin

A full-repo audit (54 issues across every surface) revealed a recurring
defect family that the existing doctrine did not name: **gates, fields, and
commands whose stated contract is stronger than their enforcement.** This is
the policy-layer mirror of a wrong `repair_packet_ready: true` — the cardinal
sin applied to the enforcement layer rather than the finding layer.

Thirteen instances were identified in one session:

- `expires` field validated as present but never compared to today (#2344)
- traceability paths checked for file existence but not symbol resolution (#2345)
- `shell_arg` escaped backslash and quote but not `$` or backtick (#2347)
- `is_absolute()` not sufficient on Windows (#2392)
- process-allowlist `max_count` stale — bound rots silently (#2399)
- `"analyzed"` JSON field mirrored `"enabled"` — name promises more than value (#2403)
- receipt lifecycle defaulted unrecognized movement to RECEIPT_FOUND (#2404)
- `goldens bless` was a raw byte-copy with no JSON parse or count sanity (#2410)
- dogfood exit code reflected report-write success, not scenario outcomes (#2411)
- network-policy detected only 7 hardcoded substrings (#2412)
- count-based gates checked one direction only — stale bounds never flagged (#2413)
- `npm test` was a no-op — compiled but ran zero tests (#2437)

Four recurring shapes produce this family:

1. **Presence-without-value:** a field is validated as non-empty but its value
   is never checked (the `expires` shape).
2. **One-directional bounds:** a count gate flags `actual > allowed` but not
   `actual < allowed` (the `max_count` shape).
3. **Exit-code/report mismatch:** a gate's final expression is a report-write,
   not an outcome aggregation (the dogfood shape).
4. **Denylist-only detection:** a policy gate detects hardcoded substrings
   without an allowlist fallback (the network-policy shape).

The doctrine fix: **when you write or touch a gate, field, or command, bind
the enforcement to the claim.** If the schema says "burn-down ready," the gate
must compare against the current date. If the field is named `analyzed`, it
must reflect actual analysis. If the gate claims to detect network calls, it
must cover the common networking crates. A gate whose stated contract is
stronger than its code misleads every future reader who trusts it.

The existing cardinal-sin doctrine covers *findings* ("under-emit before
over-emit"). This extends it to the *enforcement layer*: a false-confidence
gate is worse than no gate, because it creates the impression of enforcement
where none exists. The evidence-promotion corpus defends goldens against
dishonest re-bless; the false-confidence doctrine defends gates against
incomplete enforcement.

Cross-references: #2346 (doctrine ask), #2463 (contract-parity meta-gate),
#2466/#2479/#2484 (implementation PRs for individual instances).

## 2026-07-29: A visible fallback must survive the warm path

Fallback disclosure is only honest if the provenance survives caching. Emitting
the lexical-fallback file list during cold computation is insufficient: a warm
file-facts or classified-seam cache can otherwise replay the result without
the limitation that explains its weaker evidence. Persist the fallback paths in
each relevant cache envelope/manifest and replay one stable, sorted disclosure
on every supported route. The same audit applies to policy candidate lists:
include the actual editor languages and pin each extension with focused tests.

The claim remains deliberately narrow. Disclosure explains where static
evidence was weakened; it does not prove runtime behavior, test adequacy, or
release readiness.
