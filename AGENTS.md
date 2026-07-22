# Agent Instructions

This repository is the product repo for `ripr`: a static mutation-exposure
analyzer for Rust/Cargo workspaces.

## Product Contract

`ripr` answers this question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Keep all work aligned with that contract. Do not turn `ripr` into a full
mutation engine, a coverage dashboard, a proof system, a second rust-analyzer,
or a generic test generator.

## Language Rules

Static findings must use conservative language:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Do not claim:

- `killed`
- `survived`
- `untested`
- `proven`
- `adequate`

Real mutation testing confirms later. `ripr` gives draft-mode exposure evidence
and targeted test intent.

## Architecture Rules

Keep the public surface as one published package:

```text
Package: ripr
Binary:  ripr
Library: ripr
Automation: xtask, unpublished
```

Do not split into `ripr-core`, `ripr-cli`, `ripr-lsp`, `ripr-engine`, or
`ripr-schema` until there is a real external contract.

The current internal shape is:

- `domain`: probe, RIPR evidence, oracle strength, exposure classification,
  fix-instruction state, candidate relations, test-evidence summary
- `app`: use-case orchestration and public library API
- `analysis`: diff loading, syntax indexing, probe generation, classification,
  repair-route readiness, seam inventory, test-grip evidence
- `output`: human, JSON, SARIF, GitHub annotation, gate decision, repair
  packet, receipt, badge, and evidence-record rendering
- `cli`: command-line adapter (dispatch, help, doctor, parse)
- `lsp`: experimental sidecar adapter (backend, diagnostics, hover, actions,
  capabilities, position encoding, diagnostic budget, refresh scheduler,
  input identity, agent protocol, typed component-outcome degradation
  authority)
- `agent`: repair-loop commands (loop commands, provenance)
- `config`: `ripr.toml` loading, typed model, language detection

## Rust Baseline

- Edition: Rust 2024
- Minimum Rust version: 1.95
- Keep `unsafe_code = "forbid"`

## Rust-First File Policy

Rust is the default implementation language for repo automation, production
logic, test harnesses, fixture runners, release checks, and policy checks.

Do not add shell, Python, JavaScript, TypeScript, or other programming files
outside approved surfaces. Prefer `cargo xtask` for repo automation. If a
non-Rust file is necessary, update `policy/non-rust-allowlist.toml` and explain
the exception in the PR.

The VS Code extension, GitHub Actions declarations, fixture inputs,
documentation examples, generated outputs, and assets are explicit exceptions
when covered by policy metadata.

## Required Gates

Run these before claiming the branch is ready:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask pr-triage-report
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask metrics
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-droid-review-config
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-fixture-contracts
cargo xtask check-evidence-promotion-honesty
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
cargo xtask check-proof-packs
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo xtask check-command-catalog
```

`cargo xtask shape` is allowed to make safe local edits: run `cargo fmt`, sort
policy allowlists, ensure `target/ripr/reports`, and write a shape report.
`cargo xtask pr-summary` writes a local reviewer packet from git diff/status.
`cargo xtask pr-triage-report` writes an advisory open-PR board hygiene report.
`cargo xtask gh-pr-status --pr <number>` writes a read-only merge-readiness
packet for one PR after it exists.
`cargo xtask ci-budget [--workflow <name>] [--limit <n>] [--input <path>]`
writes an advisory CI budget and merge-queue hygiene report that separates
disk-guard infrastructure tempfails (issue #1058) from product failures; it
reads recent routed-workflow runs through `gh` (or a supplied `--input` JSON
file) and changes no CI behavior.
`cargo xtask fix-pr` runs safe shaping and then refreshes the PR summary.
`cargo xtask precommit` is the cheap non-mutating guardrail.
`cargo xtask worktree doctor` reports dirty main, branches behind main,
generated residue, and broad untracked scope before PR work proceeds.
`cargo xtask check-pr` is the review-ready non-release gate.

See `docs/PR_AUTOMATION.md` for the shape/check/guide model, current automation
entrypoint, and repo-ops report packets.

Large-repo RIPR scans are build-heavy in this repo. Prefer `repo-badge-json`,
generated receipts, an explicit gap ledger, or
`cargo xtask repo-exposure-summary-report` for ordinary summary counts; do not
use full `repo-exposure-json` for normal badge, receipt, top-file, or packet
queue paths. Run at most one no-ledger repo-wide RIPR scan at a time, scope
`RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` only to intentional full refreshes, and
clean up ad-hoc large JSON outputs after inspection.

Useful runtime checks:

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:error_path:8ee9f771
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:error_path:8ee9f771 --json
```

Editor extension checks:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
code --install-extension dist/ripr-0.10.0.vsix --force
```

The extension should resolve the server in this order:

```text
ripr.server.path
bundled server binary        (not yet shipped — no platform VSIX carries one
                              today; planned under #1443 / #1624)
downloaded cached server binary
verified first-run download
ripr on PATH
actionable error
```

Do not make `cargo install ripr` a requirement for the normal editor install
path. It is a fallback for offline, pinned, or controlled environments.

## Implementation Bias

Prefer small, high-signal changes:

- Changed behavior first, not whole-repo abstract adequacy.
- Evidence paths before scores.
- Unknown is valid and should be explicit.
- Human output should be actionable.
- JSON output should be stable and versioned.
- Agent context should state the exact missing discriminator.
- Do not credit reach-plus-a-strong-oracle as `exposed`: a strong oracle must
  observe the changed sink (see `docs/STATIC_EXPOSURE_MODEL.md` § Discrimination
  vs Coverage). Crediting proximity as discrimination is the coverage mistake.
- Align on identity, not tokens. Before crediting `exposed` from a name match,
  resolve that the test reaches the *same entity*, not just the same *string*: a
  bare `.method(` on any receiver, or the owner's bare method-name appearing in an
  oracle, can belong to a different class. Token coincidence — substring
  (`buffer⊂buffered_stream`) or whole-word-wrong-owner — is the recurring
  false-`exposed` family; when you touch one token-matching alignment/relation
  site, audit the others, and pin each confirmed over-credit as a
  should-stay-`weakly_exposed` golden (see `docs/LEARNINGS.md` § Token coincidence
  is a false-`exposed` family).
- Real producers only: do not flip a not-available field to a fabricated
  taxonomy or a fake-zero. Until a real production condition populates the
  inspected field, defer the named limitation to a code comment rather than
  emit invented evidence (see `docs/LEARNINGS.md` § Detection needs a real
  producer).
- The actionability flip is the cardinal-sin seam: a wrong
  `repair_packet_ready: true` is worse than ten advisory findings. Under-emit
  before you over-emit — keep the flip fail-closed and let the shared validator
  be the only authority.
- Reuse the shared enforcement layer (validators, renderers, route helpers)
  across every surface; do not fork a parallel validator. Reconcile derived
  messaging in the layer that owns the final decision so all surfaces agree
  (see `docs/adr/0019-language-adapters-reuse-shared-packet-contract.md`).
- Graduate every confirmed false-promotion into the evidence-promotion corpus
  (`fixtures/evidence-promotion-honesty-corpus/corpus.json` +
  `cargo xtask check-evidence-promotion-honesty`, RIPR-SPEC-0108), not just a unit
  test. The gate pins the non-promotion expectation *independent of the golden*,
  so it catches a dishonest re-bless that `goldens check` would accept — goldens
  can encode dishonesty. Share the invariant + corpus across languages; do **not**
  unify the per-language matchers (different taxonomies, different edge policies).
- Performance is part of honesty: an interactive path that is too slow, or that
  defers expensive analysis off the keystroke path, must **disclose** its state
  (e.g. `run_status: "seams_deferred"`, RIPR-SPEC-0105) and never present a
  partial/deferred run as complete. A fast path may be partial only if the status
  says so.

When a target file is already a monolith — flagged by `cargo xtask
module-health` (advisory, exits 0) — the first PR of a capability wave should
be a behaviour-preserving decomposition. Zero golden drift is the proof that it
is pure structure: each new capability then lands in a focused module with a
clear single responsibility, and the blast radius of future changes shrinks.

Do not add deep semantic dependencies, persistent databases, or broad LSP
features unless the basic CLI, schema, packaging, and tests remain green.

### Verification bias

- Treat sub-agent and scout findings as leads, not facts. Verify control-flow
  claims against the code, and turn a suspected bug into a PR only behind a
  failing fixture. Fast, confident producers — sub-agents included — are
  unreliable on precise logic; a cheap finding is a lead that needs a slower
  verifier beneath it.
- For classifier or `analysis/**` behavior changes, work fixture-first and
  measure golden blast radius (`cargo xtask goldens check` + `cargo xtask
  dogfood`) before finalizing. The golden corpus is the regression net and will
  catch an over-corrected heuristic; an in-repo corpus that already passes is not
  evidence the change is accurate on external code.
- Verify the artifact, not the report: every PR, RUN the command and READ the
  output before claiming it works. Gates passing, tests passing, and a builder's
  own "all gates pass" are weak oracles for behavior. Never merge on a
  sub-agent's or builder's self-report.
- Do not hide a gate's exit code behind a pipeline. `cargo test … | grep … ;
  echo done` reports the exit status of `echo` (always 0), so a real failure
  reads as success. Run the gate directly, or capture `${PIPESTATUS[0]}` (bash)
  before the pipe. Likewise, a green required check is not proof an analyzer fix
  is correct — CI can pass on a fix the adversarial review knows is partial;
  judge the fix on its semantics, not its exit code.
- Run the full `routed-rust.yml` `cargo xtask check-*` list, not `precommit` and
  not a hand-picked subset. A partial list silently skips `check-network-policy`,
  `check-dependencies`, and `check-generated`; CI will fail what local guessing
  missed.
- Verify with the *right* harness — "verify the artifact" cuts both ways, since a
  wrong harness manufactures false **negatives**. Run the **absolute** worktree
  binary (`<worktree>/target/debug/ripr.exe`, not a long `../` that escapes to the
  main checkout's stale binary) and `cargo fmt --check` under the pinned 1.95.0 /
  rustfmt 1.9.0 toolchain. When a fix "doesn't work" but the builder insists it
  does, suspect your own harness before the builder — inject a unique marker string
  into the output to confirm your edits are even in the binary you are running. And
  terminate any `ripr lsp --stdio` you spawn for an LSP behavioral test; an
  orphaned server holds a Windows file lock and breaks the next build.

### Status-comment verification contract

When posting a status, triage, or "issue update" comment on a GitHub issue,
bind every claim to verifiable evidence. The open-issue list is the durable
campaign record; low-truth status comments bury substantive signal and
mislead future agents who consume prior comments as context.

Rules:

- Every status claim must cite a **verifiable artifact**: a `file:line`
  reference, a merged PR number, a `gh run` / `gh release` result, or a `git
  log --grep` output. "Not started" is not a valid verdict without evidence.
- "No PR references it" is **forbidden** without an **all-state PR search**
  attached inline: `gh pr list --state all --search <issue-number>` (or the
  MCP `search_pull_requests` equivalent). Many issues have merged PRs that
  cite them only in the PR body, not the commit subject, so `git log --all
  --grep <issue-number>` alone misses these — it is supplemental, not the
  primary check.
- Use the closed `status/*` label set as the primary status signal (labels
  don't bury signal; a status comment is secondary):
  - `status/done-open` — delivered; the issue is intentionally kept open.
  - `status/blocked-upstream` / `status/blocked-repo` — waiting on an external
    or in-repo dependency.
  - `status/needs-work` — actionable and **not started**. Do **not** use it
    for partially landed work; that understates delivery and invites
    duplicate implementation.
  - `status/partial` — a **bounded portion has merged** to `main` (or another
    authoritative repository) and the residual acceptance plus next owner are
    recorded. Apply it only when a merged deliverable exists — never merely
    because a branch or PR is open.
  - `status/mis-scoped` — the issue needs re-scoping before work proceeds.
- A partially landed slice gets **one** evidence-bound reconciliation comment
  (landed PR + exact merge SHA, acceptance covered, acceptance remaining, next
  owner/dependency, claim boundary). Closing a child issue never closes its
  parent capability. See #1863 for the canonical reconciliation format.
- One status comment per issue per pass. A second pass must **edit** (or
  minimize/hide) the prior comment, not append a near-duplicate. Re-posting
  the same review minutes apart is noise that buries substantive comments.
- Do not fabricate file paths or issue numbers. Verify paths exist (`ls`,
  `find`) and issues exist (`gh issue view`) before citing them.
- Do not post a status review on a **closed** issue without first checking
  `gh issue view <N> --json state`. Describing the pre-fix state of an
  already-closed issue is a credibility failure.

### Finding verification contract

The status-comment contract above covers updates on existing issues. The same
evidence discipline applies **before filing or materially updating a code
finding** (#2026): an inaccurate issue becomes durable context for future
agents and invites duplicate implementation.

A finding record must name:

- repository, inspected branch/ref, and full source SHA;
- file path and exact lines or symbol, and the observation timestamp;
- the reproduction command, or the explicit reason no executable
  reproduction exists (a design question does not need a failing command,
  but still requires current source identity and accurate behavior
  description);
- actual result vs. expected result or invariant;
- an all-state, finding-specific issue/PR search (`gh issue list
  --state all --search <term>` and `gh pr list --state all --search
  <term>` — both default to open-only, and a bare list is not evidence
  bound to the finding);
- known concurrent PRs touching the seam;
- confidence, remaining uncertainty, and a classification:
  `verified_current | historical | cannot_reproduce | superseded |
  design_question`.

Pre-filing rules:

1. Re-read the exact current file/symbol after all scouts return — scout
   output is a lead, never the finding.
2. Run the smallest deterministic reproduction where practical.
3. Treat line numbers from an earlier commit as stale until re-resolved.
4. Do not promote a grep absence into an architectural fact without
   checking the search command and relevant alternate paths.
5. Check open PRs for a branch that already changes the seam.
6. When the premise changed during the audit, narrow or close the draft
   instead of preserving the original claim; corrections edit or
   prominently amend the original record rather than burying it in a later
   summary.
7. Source SHA binds evidence, but a moved main requires a premise recheck
   — not automatic abandonment of a valid finding.

## PR Scope Doctrine

Do not optimize PRs for low line count. Optimize for narrow production risk and
complete evidence.

A large fixture, golden-output, spec, docs, ADR, metrics, or traceability diff
is welcome when it makes one production behavior reviewable. A small code diff
is not acceptable if it changes multiple contracts without a spec-test-code
trail.

Every material behavior change should preserve this chain:

```text
spec -> test or fixture -> code -> output contract -> metric
```

Make production delta, evidence delta, acceptance criterion, and non-goals
explicit in PRs and planning docs.

## Commit, PR, and Merge Boundary

Do not pause merely to commit, push, open a PR, update a PR, or merge a clean
PR.

For scoped implementation, docs, tests, and refactors, use this default flow:

```text
review -> improve -> validate -> commit -> push -> open/update PR -> merge when ready
```

A PR is ready when the branch is current, required checks pass, real review
findings are addressed, the diff matches the stated scope, and repo policy does
not require a different sequence.

Merge-safety rules, learned the hard way:

- Treat CI checks like oracles. A check that *runs* but is not *required* is not
  a discriminator for merge safety — an advisory red can still merge. Before
  saying a policy is protected by a gate, verify the required check actually
  depends on it
  (`gh api repos/<owner>/<repo>/branches/main/protection/required_status_checks`).
- Do not treat `mergeStateStatus=UNSTABLE` as a merge decision by itself. Inspect
  required checks, advisory checks, and branch protection separately; merge only
  when the required discriminator is green, and explain advisory failures rather
  than waving them through.
- When a PR fails on a file or spec it did not touch, reproduce against
  `origin/main`. If `main` is already broken, fix `main` in a tiny unblock PR
  first, then rebase the dependent work — do not debug your own diff for an
  inherited failure.
- A pass with zero analyzed subjects is `not_run`, not evidence. Preserve
  denominators in reports; a green state with an empty denominator proves nothing.
- An output-shape change invalidates every golden. A PR that adds or renames a
  field in `ripr check` / report JSON must re-bless **all** affected goldens in
  the same PR — and a golden PR that merges concurrently with such a change goes
  stale the moment both land, breaking `goldens check` (the required gate) for
  every subsequent PR. This is the single-writer-collision family (cf. spec
  numbers) on goldens. When `goldens check` fails on `main`, diff
  `expected/check.json` against the actual output: an additive missing field
  points at a concurrent output-shape PR. Fix by re-blessing on the *current*
  `main` (after the latest output-shape change), not an older base — re-blessing
  on a stale base just re-breaks it. If a later output-shape PR has already
  re-blessed everything, an earlier in-flight re-bless PR becomes redundant *and*
  harmful (merging it reverts the golden): close it.
- Distinguish an infra tempfail from a real failure before reacting. A quick
  `runner_api_failed` / runner-selection error is infra — re-run. A gate that
  *ran* and failed (e.g. `xtask: goldens check failed`, a `FAILED` test) is real
  — read the report and fix it. Re-running a real failure wastes a CI cycle;
  debugging your own diff for an infra flake wastes a turn.

`stackable = false` means do not build the next dependent work item on top of
the current branch. It does not create an approval gate.

`blocked_by` is a dependency rule. If a work item depends on another item, wait
until that dependency is landed or explicitly update the manifest. Do not invent
a separate merge rule.

Ask before proceeding only when continuing would change public schema, output
contracts, security/workflows/secrets, dependencies, release or publish
behavior, architecture boundaries, campaign ordering, or duplicate-PR
selection.

## Review posture

Automated review comments are primarily consumed by follow-up coding agents.
Do not optimize for a human reading every comment. Optimize for concrete,
structured, actionable findings that another agent can fix.

A clean review must still document what was inspected.
Do not treat "LGTM" as a useful review result. If there are no actionable
findings, produce a short inspection record that names:

- changed surfaces inspected;
- risks considered;
- repo invariants checked;
- validation signals;
- residual assumptions.

When reviewing or repairing code, read these files first:

- `.factory/skills/review-guidelines/SKILL.md`
- `.factory/rules/rust.md`
- `.factory/rules/github-actions.md`
- `.factory/rules/security.md`
- `docs/agent-context/repo-map.md`
- `docs/agent-context/review-invariants.md`
- `docs/agent-context/validation.md`

## Orchestration Operating Model

Orchestrated work is a staged pipeline, not one monolithic session:

```text
reconcile current repo and work-item truth
-> compile bounded context
-> run read-only discovery and adversarial checks
-> root-thread synthesis and decision
-> isolated implementation
-> independent verification
-> integration, review, and targeted proof
-> cleanup and durable handoff
```

The main or root session is the **orchestrator and integrator**. It owns the
objective, accepted premises, constraints, non-goals, dependency order,
acceptance coverage, contradictions, product or architecture decisions, PR
scope, and final integration judgment. Broad file searches, CI logs, broad
diffs, test inventories, and failed hypotheses belong in subagents that return
bounded structured results, not in the root context. During verification, the
root may inspect targeted evidence directly when binding a claim to an exact
head, command, denominator, artifact, or changed line; this targeted inspection
does not turn the root into a raw-log sink.

### When to delegate

Prefer subagents when:

- two or more independent read-heavy questions can be answered in parallel;
- a log, diff, search, or inventory would otherwise flood the root context;
- an independent adversarial or verification pass materially changes trust;
- a broad audit separates naturally by surface, risk, or evidence family;
- two write tasks have already-proven disjoint edit cages and semantic resources,
  plus real worktree, CI, review, and merge capacity.

Stay single-agent when:

- the task is one narrow edit or one serial decision chain;
- the root still needs product, architecture, security, policy, release, or
  public-contract judgment before decomposition;
- context or premises are stale, contradictory, or incomplete;
- writers would share source, schema, golden, fixture, policy, workflow,
  release, active-goal, traceability, report-root, or Cargo-target resources;
- CI, review, or merge capacity is already saturated;
- coordination would cost more than doing the bounded task directly.

Do not treat the client or model's maximum thread count as a target. Cap each
wave by useful independent work and the repository's integration capacity.
Read-only fan-out comes before write fan-out. One writer is the default.

### Subagent roles

Use role-specific workers instead of generic "help with this" prompts:

- **Scout:** read-only inventory, repository/PR surface mapping, schema/spec
  tracing, test discovery, log summarization, and premise checks. Returns
  observations, evidence references, missing proof, risks, and next questions;
  never claims implementation or verification complete.
- **Adversary:** read-only challenge to one named premise, plan, result, or claim.
  Returns concrete discrepancies with references, or `none_found` plus the exact
  inspected scope. One supported contradiction is enough to stop dependent work.
- **Builder:** implements exactly one accepted production/evidence delta in a
  prepared isolated worktree and explicit edit cage. Returns the actual changed
  files, diff/commit identity, commands attempted, artifacts, blockers, and
  author claims; never self-verifies.
- **Verifier:** checks the exact builder diff/commit with the named proof routes.
  Does not repair source. Records actual exit status, denominator, output
  artifact, harness identity, residue, and whether the claim is supported,
  contradicted, limited, or not run.
- **Reviewer:** independently inspects the scoped diff and evidence contract for
  correctness, boundary violations, unsupported claims, and missing tests. A
  clean result still lists inspected surfaces, risks, invariants, validation
  signals, and residual assumptions.
- **Cleanup auditor:** read-only inventory of claims, worktrees, branches,
  generated output, caches, and temporary residue. Produces a cleanup plan; the
  root/operator performs any destructive cleanup explicitly.

Root-to-direct-child delegation is the supported default. Do not ask subagents
to spawn descendants unless a future measured repository contract explicitly
authorizes recursive fan-out.

### Wave discipline

Before spawning a wave:

1. Reconcile live Git, open PRs, open issues, linked plans/specs, current
   base/head, and working-tree state. Live source beats transcript or stale
   planning prose. `.ripr/goals/` was deleted; goal files never select or
   authorize work.
2. Give every subtask one bounded objective, decision contribution, input/base
   identity, read scope, edit policy, dependency list, conflict resources,
   stop conditions, expected evidence, and result budget.
3. Run independent read-only scouts and adversaries first. Do not start a
   builder while its premise or contract remains disputed.
4. Have the root synthesize results. A contradiction, stale identity, missing
   required contract, or scope expansion forces stop/recompile/replan; do not
   smooth it into consensus.
5. Start a writer only after its exact base, worktree, edit cage, forbidden
   paths, and semantic single-writer resources are known.
6. Run a separate verifier after the builder stops mutating the result.
7. Integrate only current, in-cage, independently verified work. Then run the
   normal review/CI/PR/merge flow and clean up every orchestration artifact.

The durable campaign and work-item authority remains the existing issue/spec/
plan graph. Subtasks are ephemeral execution detail inside one work
item; do not create a second committed task hierarchy from subagent threads.

### Context and result discipline

Keep subagent packets reference-first and role-scoped. Inline only the facts
needed for that role. Large documents, logs, search dumps, complete diffs, and
reports stay in the child thread or an ignored artifact path; return stable
path/line, JSON-pointer, command, digest, or issue/PR references instead.

A useful bounded result contains:

```text
status and role
input/base identity actually consumed
short summary
observations or claims with claim type
exact evidence references
contradictions and assumptions
missing evidence and stop reason
files read and files changed
commands attempted with actual status and denominator
produced artifact references
selected / omitted / total counts when bounded
one recommended next action
```

Do not return raw chain-of-thought. Do not paste full logs into the root summary.
If output is truncated or stored out of band, name the omitted counts and the
complete retrieval route. Repeated unchanged packets/results should remain
byte-stable once the planned tooling exists; until then, preserve deterministic
ordering and omit timestamps or volatile paths from semantic summaries.

### Verification and synthesis rules

- A subagent report is a lead, not authority. Verify control-flow and behavioral
  claims against current source and executable evidence.
- A builder's "fixed" or "tests pass" is an `author_claim`, not a verified fact.
- Verification must bind the exact diff/commit, command, denominator, harness,
  and produced artifact. A pass with zero tests or subjects is `not_run`.
- Repetition is not independent evidence when agents consumed the same source.
  Do not majority-vote truth or average confidence prose.
- Preserve contradictions, stale/rejected results, partial coverage, and missing
  evidence as first-class states. One concrete contradiction blocks dependent
  work until the root resolves or recompiles it.
- Task completion count is not acceptance coverage. The root tracks each
  acceptance requirement, proof obligation, non-goal, verification state, and
  unresolved gap explicitly.
- The root owns final synthesis and judgment. Subagents do not change campaign
  order, schemas, policy, architecture, release state, credentials, or merge
  authority silently.

### Writer isolation and semantic conflicts

**Background workflow agents may share the main working directory.** Even a
read-only-by-intent worker can have shell access and create or modify files.
Therefore:

- prompts for shared-tree research must explicitly forbid file creation and
  mutation, and the root must compare `git status --short` before and after;
- every source-editing builder and any command that may mutate repository state
  uses a task-specific worktree at the exact declared base;
- parallel writers require disjoint path cages **and** disjoint semantic
  resources; different files do not make two tasks independent;
- treat output schemas and their goldens, fixture corpora, spec-number
  allocation, traceability, policy ledgers,
  workflows, release assets, and default mutable report/Cargo/npm roots as
  single-writer resources;
- never run `git add -A` while a background workflow is live; stage explicit
  paths and inspect all tracked/untracked changes before commit;
- workers must not run `git checkout`, `git switch`, `git stash`,
  `git reset --hard`, branch deletion, or worktree removal to repair unexpected
  state. Stop and report the mismatch instead;
- a stale base, changed work item, conflicting landed PR, unexpected file, or
  boundary violation invalidates the worker result until the root replans;
- builder and verifier must not mutate the same worktree concurrently.

Every fan-out ends with cleanup: worker threads/results accounted for,
worktrees and branches dispositioned, locks/claims released when present,
`target/ripr` growth inspected, temporary/generated/npm/Cargo residue reviewed,
and the root worktree proven uncontaminated.

The typed orchestration tooling described by issue #1631 and the child issue
range 1632–1639 is not yet an assumed command surface. Apply these rules manually
until each command and schema lands; do not invent or cite planned commands as
current evidence. See `docs/AGENT_OPERATING_MODEL.md`,
`docs/CODEX_GOALS.md`, `docs/agent-context/CONTEXT_SYSTEM.md`, and
`docs/reference/AGENT_HANDOFF_PROTOCOL.md` for the durable surrounding model.

## Long-Context Agent Workflow

This repo is intentionally organized so agents can resume long-running goals
from repository artifacts instead of chat history.

When picking up work:

- start from `docs/ROADMAP.md` and `docs/IMPLEMENTATION_PLAN.md`
- use `docs/IMPLEMENTATION_CAMPAIGNS.md` for campaign history
- use `docs/CAPABILITY_MATRIX.md` to identify current capability status
- use `docs/PR_AUTOMATION.md` to understand local shaping and PR reports
- use `docs/CODEX_GOALS.md` for the multi-PR campaign model
- use `docs/SCOPED_PR_CONTRACT.md` for one work item's PR-sized evidence bar
- use `.allow/spec-system/slices/` for one PR's scope of record: each
  behavior-changing PR owns a small PR-local `ImplementationSliceV1` there
  (requirement IDs, change class, seams, evidence obligations, non-goals,
  claim boundary) — never worker, branch, CI, or progress state
- use `docs/specs/` and `.ripr/traceability.toml` to map spec -> tests -> code
- choose the smallest vertical slice with one production delta and one evidence
  package
- update `docs/LEARNINGS.md` when repo knowledge or blockers should survive

See `docs/AGENT_WORKFLOWS.md` for the detailed handoff model.

See `docs/AGENT_OPERATING_MODEL.md` for the orchestration operating model:
agent economics, verify-don't-trust discipline, CI hygiene, and the rationale
for why constraints enable autonomy.

See `docs/LSP_AGENT_REPAIR_WORKFLOW.md` for the end-to-end LSP-first
repair/receipt loop: Show Status → Copy Top Repair Packet → edit in cage →
verify → receipt → Show Receipt Status → Show Route Quality.

See `docs/LIBRARY.md` for the curated knowledge library: agentic learnings,
repo domain learnings, and a dated timeline of major learning milestones across
all campaigns.
