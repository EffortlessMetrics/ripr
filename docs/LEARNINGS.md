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

## 2026-05-12: Cache-TTL-Aware CI Watcher Economics

### Context

Agent loops that poll external state during a long-running task — CI
watchers, deploy waiters, queue drainers, anything that sleeps and
then checks — have a non-obvious cost dimension beyond API rate
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

This is a generally-true coordination protocol for any agentic system
that polls external state. The specific TTL is an Anthropic-prompt-
cache fact today; other providers have their own cache windows, but
the three-region structure is the same.

### What works

- Active CI watch: 180–270 s backoff. Stays inside the warm zone,
  with enough headroom that one slow tool call does not push the
  cycle over the TTL.
- Genuinely idle ticks (no active PR, waiting for an unrelated
  trigger): 1200–1800 s. Commits to one cache miss per long wait
  instead of churning.
- Exit-early signals on every wake: a CI state of `CLEAN`,
  `UNSTABLE`, or `HAS_HOOKS` means ready to merge; a failure
  conclusion means stop and report; a `BEHIND` mergeable state
  means rebase, then re-watch.

### What doesn't

- `gh pr watch` default cadence (three-second polling). Burns the
  authenticated GitHub API rate limit fast, and re-enters the agent
  loop too often to amortize cache cost meaningfully.
- ~300 s polling. Lands in the danger zone — each wake pays a full
  cache miss without buying much external progress.
- Tight infinite loops with no backoff. Same failure mode as the
  default `gh pr watch`, plus the agent has no chance to terminate
  on the exit-early signals above.

### Operational signals to watch for

For GitHub PR watchers specifically, the merge-readiness signals
worth handling explicitly:

```text
CLEAN       ready
UNSTABLE    ready (non-required check failing)
HAS_HOOKS   ready (waiting on optional hook)
BEHIND      needs rebase, then re-watch
DIRTY       conflict, stop and report
```

Without GitHub merge-queue / auto-merge enabled, concurrent merges
on a busy repo produce repeated `BEHIND` transitions; Campaign 27
saw five to six rebase cycles per PR. Merge queue removes that
class of loop entirely.

### Limitations

- The five-minute number is the current Anthropic prompt-cache TTL.
  If that window changes, or if the watcher runs on a different
  provider, the warm/danger/committed boundaries shift but the
  three-region structure does not.
- The exit-early signals above are GitHub-specific. The general
  principle — wake, check, exit on a small set of terminal states,
  back off otherwise — transfers to other coordination targets.
