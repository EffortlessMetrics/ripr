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
