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

For one complete local review and package pass, run `cargo xtask ci-full`. It
runs the review-ready `check-pr` lane, the evidence gates (`fixtures`,
`goldens check`, `test-oracle-report`, `dogfood`, and `metrics`), then the
package listing and publish dry-run. The explicit commands below remain the
inventory for targeted reruns.

The following report commands are advisory and do not independently block a
merge: `cargo xtask pr-triage-report`, `cargo xtask metrics`,
`cargo xtask check-pr-shape`, and `cargo xtask module-health`.

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask pr-triage-report # advisory
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask metrics # advisory
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
cargo xtask check-pr-shape # advisory
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
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250 --json
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
- **False-confidence gates and fields.** A gate, field, or command whose
  stated contract is stronger than its enforcement is a false-confidence
  surface — the policy-layer mirror of a wrong `repair_packet_ready: true`.
  When you write or touch a gate, field, or command, bind the enforcement to
  the claim: if the schema says "burn-down ready," the gate must compare
  against the current date; if the field is named `analyzed`, it must reflect
  actual analysis; if the manifest points at `path::tests::fn`, the gate must
  resolve `fn`; if the gate claims to detect network calls, it must cover the
  common networking crates. A gate whose stated contract is stronger than its
  code misleads every future reader who trusts it — including agents resuming
  campaigns from repository artifacts.
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
- **Resolve all review threads before merging.** The repo's branch ruleset
  requires `required_review_thread_resolution: true` — any unresolved review
  comment thread blocks `gh pr merge` with "Repository rule violations found."
  Automated review bots (codex, gemini, droid, github-actions) leave threads on
  nearly every PR. Before attempting to merge, address or reject each finding
  (fix the code, push, or explain why the suggestion is wrong), then resolve the
  thread via the GraphQL API:
  ```bash
  # list unresolved threads
  gh api graphql -f query='query($o:String!,$r:String!,$p:Int!){repository(owner:$o,name:$r){pullRequest(number:$p){reviewThreads(first:50){nodes{id isResolved}}}}}' -F o=EffortlessMetrics -F r=ripr-swarm -F p=<PR_NUMBER> --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | .id'
  # resolve each
  gh api graphql -f query='mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){thread{isResolved}}}' -f id=<THREAD_ID>
  ```
  Do not resolve a thread without first addressing the finding. If the bot's
  suggestion is correct, fix it and push. If it is wrong, leave a short reply
  explaining why before resolving. Resolving without engagement defeats the
  review-feedback loop.
- **Verify `required_approving_review_count` matches repo reality.** This is a
  single-maintainer repo; the count should be `0`. If it drifts back to `1`
  (e.g. after a GitHub UI change or a ruleset re-apply), PRs silently BLOCK with
  no actionable error. Check and fix:
  ```bash
  gh api repos/EffortlessMetrics/ripr-swarm/branches/main/protection/required_pull_request_reviews --jq '.required_approving_review_count'
  # if it is not 0, fix it:
  gh api -X PATCH repos/EffortlessMetrics/ripr-swarm/branches/main/protection/required_pull_request_reviews -F required_approving_review_count=0 -F dismiss_stale_reviews=false -F require_code_owner_reviews=false -F require_last_push_approval=false
  ```
  Also check the GitHub **ruleset** (not just branch protection), which is a
  separate enforcement layer:
  ```bash
  gh api repos/EffortlessMetrics/ripr-swarm/rulesets --jq '.[].id'
  gh api repos/EffortlessMetrics/ripr-swarm/rulesets/<ID> --jq '.rules[] | select(.type == "pull_request") | .parameters'
  ```

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


## Goal delivery and candidate workflow

Preserve the user's original goal, constraints, non-goals, assumptions, and
acceptance predicates. The durable issue, specification, plan, policy, receipt,
and closeout graph is the repository's source of truth for long-running work.

Use the six Codex procedures under `.agents/skills/**` as the operational
entrypoints. Select the narrowest procedure for the current claim, and keep one
current branch/worktree/PR per coherent claim. A waiting PR is still in flight;
advance a distinct claim when useful and revisit it after a material transition.

Use focused readers or reviewers only when they add a different source,
oracle, context, threat model, or verification method. Their findings are leads
until checked against current repository and GitHub evidence. Reasonable,
reversible engineering choices should proceed without an owner pause.

Keep PR head, integration basis, squash result, proof, review, and release
state as separate judgments. Refresh only the proof dimensions affected by a
changed head, conflict, implementation, oracle, public claim, generated
relationship, or integration basis. Unrelated movement on `main` does not
invalidate proof or review by itself.

Before publication use `cargo xtask precommit`, then the required changed-
surface and release gates. After merge, verify current `main`, reconcile the
issue and campaign predicates, refresh generated evidence, capture genuine
follow-ups, and remove only lane-created worktrees, branches, and residue.

Do not create fixed actor rosters, repository-global goal or writer state,
reservation systems, candidate tournaments, provider-crossing skill wrappers,
or Kiro lifecycle routes. GitHub and committed repository artifacts carry
durable state; transient model context does not.
