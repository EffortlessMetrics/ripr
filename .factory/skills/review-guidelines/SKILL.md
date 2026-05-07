# Droid Review Guidelines for ripr

These reviews are primarily consumed by follow-up coding agents, not by a human reading every comment manually.

Optimize for structured, durable review records. Do not optimize for a low comment count. A clean review is still expected to document what was inspected and why no actionable findings were emitted.

## Required context

Before reviewing, use the repository's checked-in context:

- `AGENTS.md`
- `docs/ENGINEERING.md`
- `docs/ARCHITECTURE.md`
- `docs/PR_AUTOMATION.md`
- `docs/SCOPED_PR_CONTRACT.md`
- `docs/CI.md`
- `.factory/rules/droid-review.md`
- `policy/workflow_allowlist.txt`
- `docs/agent-context/review-invariants.md`
- `docs/agent-context/validation.md`

For product, analyzer, output, fixture, LSP, release, or workflow changes, inspect the relevant docs linked from `README.md`.

## Product contract

`ripr` is a static RIPR exposure analyzer for Rust/Cargo workspaces.

It answers:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Do not review changes as if `ripr` were:

* a full mutation engine;
* a coverage dashboard;
* a proof system;
* a second rust-analyzer;
* a generic test generator.

## Static-output language rules

Static findings may use:

* `exposed`
* `weakly_exposed`
* `reachable_unrevealed`
* `no_static_path`
* `infection_unknown`
* `propagation_unknown`
* `static_unknown`

Static findings must not use mutation-runtime outcome language.

Use only the allowed exposure vocabulary above. For the full static-output
language boundary, follow `AGENTS.md` and the repository static-language policy.

Real mutation testing confirms later. `ripr` gives draft-mode exposure evidence
and targeted test intent.

## Review posture

A useful review identifies concrete failure modes or records concrete inspection.

Do not suppress actionable findings because there are many of them. Suppress only:

* duplicates;
* low-confidence speculation;
* non-actionable observations;
* findings already covered by a clearer comment.

If there are 20 concrete issues, leave 20 comments. If many instances share one root cause, leave one systemic comment and name representative locations.

## Repair value

Droid comments are an inter-agent repair queue. Each actionable finding should preserve enough context for a follow-up coding agent to fix the issue without repeating the research that produced the finding.

Treat each actionable finding as a standalone repair packet. A follow-up agent should be able to read the comment, locate the likely repair surface, understand the violated repo invariant, and choose a validation command without reconstructing the original review.

Each finding should include:

* failure mode;
* why here / repo invariant;
* fix direction;
* validation;
* confidence.

Do not optimize for short comments when useful repair context would be lost. Droid runs consume CI time, model calls, and repo research; each finding should amortize that cost by preserving the useful result.

Preserve useful repo research in the comment or summary. If Droid inspected specs, policies, CI configuration, prior comments, or in-repo documentation to reach a finding, include the relevant context source so the next agent does not rediscover the same invariant.

## No naked LGTM

Do not use `LGTM`, `looks good`, or equivalent empty approval language as the review summary.

If no actionable inline findings are emitted, the review summary must still include:

```text
No actionable findings emitted.

Inspected surfaces: <files / systems / changed areas>.
Checks performed: <repo invariants, security/workflow/release/correctness risks considered>.
Why no comments: <why the diff satisfies those checks>.
Residual risk: <anything not verified by review, such as external service behavior or unrun validation>.
Validation signal:
  Observed: <CI checks, files, logs, artifacts Droid directly inspected>.
  Reported: <PR-body, commit-message, or comment claims>.
  Not verified: <validation Droid did not run or observe>.
```

If the review system submits an approval, the approval body must still include this inspection record.

## Inline comment format

Each inline comment should be structured so another agent can fix it.

Use this shape:

```text
[P0|P1|P2] Short title

Failure mode: What can break, leak, regress, or become unmaintainable.
Why here: The repo invariant, product contract, policy, or edge case this violates.
Fix direction: The smallest safe repair. Name likely files/functions when useful.
Validation: Command, report, fixture, golden, or CI check that should verify the fix.
Confidence: High / Medium / Low. If not high, explain what would confirm it.
```

## Priority scale

* `[P0]` Merge blocker: severe security issue, data loss, broken required CI, broken release path, secret exposure, or repository policy failure.
* `[P1]` Should fix before merge: concrete correctness, security, reliability, workflow, public-contract, or evidence issue.
* `[P2]` Useful follow-up: valid issue, but not necessarily blocking this PR.

Do not assign priorities to style-only observations.

## Core review checks

For every PR, classify the changed surfaces and apply the relevant checks.

### Rust analyzer / product behavior

Check whether the PR preserves:

* the product contract;
* conservative static-output language;
* evidence-first findings;
* explicit unknowns;
* parser/syntax-backed behavior where claimed;
* spec-test-code-output-metric traceability for behavior changes.

Behavior changes should usually include some combination of:

* spec update;
* Rust test;
* fixture;
* golden human output;
* golden JSON output;
* context packet expectation;
* capability metric update;
* traceability update;
* changelog, ADR, or learning note when appropriate.

### Output contracts

For user-visible human output, JSON output, GitHub annotations, context packets, diagnostics, or public schema changes:

* check that output language stays conservative;
* check that output schema/version expectations are preserved;
* check that golden output is updated only when justified;
* check that fixture/golden drift has evidence and a reason.

### Architecture

The public package shape should remain:

```text
Package: ripr
Binary:  ripr
Library: ripr
Automation: xtask, unpublished
```

Do not recommend splitting into `ripr-core`, `ripr-cli`, `ripr-lsp`, `ripr-engine`, or similar unless the PR explicitly establishes a real external contract.

Check internal seam ownership:

* `domain`: exposure concepts, probes, RIPR evidence, oracle strength, classifications.
* `app`: orchestration and public library API.
* `analysis`: diff loading, syntax indexing, facts, probes, classification.
* `output`: human, JSON, GitHub, future SARIF rendering.
* `cli`: command adapter.
* `lsp`: editor protocol adapter.

### Rust policy

Check for:

* `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, or panic-family shortcuts in production paths;
* accidental test shortcuts where tests should return `Result` or assert explicitly;
* `unsafe` or unsafe-adjacent policy violations;
* broad lint suppressions;
* dependency additions where standard library or existing dependencies suffice;
* non-Rust implementation files outside approved policy surfaces.

### Workflow and CI policy

Workflow changes are security-sensitive.

Check:

* explicit minimal `permissions`;
* fork behavior and secret exposure;
* use of `pull_request_target`;
* trusted actor guards for comment-triggered secret-backed jobs;
* mutable third-party action refs in workflows using secrets or write permissions;
* Node runtime policy for GitHub Actions;
* `policy/workflow_allowlist.txt` budget entries for new or changed workflows;
* whether shell `run:` blocks exceed the approved non-empty line budget;
* whether CI docs need updates.

For Droid-specific invariants, review `docs/agent-context/review-invariants.md`
when reviewing Droid-related changes.

### VS Code extension

For changes under `editors/vscode` or release packaging:

* check activation behavior;
* check server resolution order;
* check `ripr lsp --stdio` compatibility;
* check extension package metadata;
* check compile/package/e2e validation expectations.

Expected validation includes:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

and e2e checks when extension activation behavior changes.

### Release and packaging

For crate, binary, extension, server binary, badge, or marketplace changes:

* check package metadata;
* check release workflow behavior;
* check artifact inclusion/exclusion;
* check server binary/version consistency;
* check docs and badge policy where relevant.

Expected validation may include:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

## Validation commands

Name the smallest validation set that verifies the fix.

Common commands:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask critic
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
```

Direct Rust checks:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Workflow/policy checks:

```bash
cargo xtask check-workflows
cargo xtask check-file-policy
cargo xtask check-local-context
cargo xtask check-process-policy
cargo xtask check-network-policy
```

Extension checks:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

## Summary when findings exist

When findings are emitted, summarize the repair queue:

```text
Findings emitted: <count>, grouped by <risk areas>.
Highest priority: <P0/P1/P2 summary>.
Systemic pattern: <if applicable>.
Suggested repair order: <what the next agent should fix first>.
Validation: <commands or checks to run after repair>.
```

Do not hide actionable findings only in the summary. If a finding maps to a line, use an inline comment.

## Suggested fix policy

Use GitHub suggestion blocks when:

* the fix is small;
* the replacement is locally obvious;
* the suggestion will apply cleanly;
* confidence is high.

Do not use suggestion blocks for multi-file, policy, schema, or design-dependent changes. In those cases, use an ordered repair plan:

1. Name the likely files, tests, and policies involved.
2. Describe each step.
3. Include validation commands.

For cross-file changes, describe the full repair direction even if the fix touches only one line in each file. The next agent needs to know what else must change to keep the repo consistent.

## Research amortization

If Droid inspected repo docs, specs, policies, CI configuration, prior comments, or in-repo documentation to reach a finding, preserve the useful result in the comment or summary.

Do not make the next repair agent rediscover the same invariant. Include the context source when relevant.

Prefer:

```text
Why here: `AGENTS.md` requires spec-test-code-output-metric traceability for
behavior changes. This diff changes classifier output without updating the
corresponding fixture in `fixtures/classifier/basic/`.
```

Avoid:

```text
Why here: Missing test update.
```

## Mentions and notifications

Do not @mention people, teams, bots, or organizations in review comments, review summaries, inspection records, or security findings unless explicitly instructed.

Droid review output is primarily consumed by follow-up agents. Avoid notification-generating language.

Do not write:

* `@username`
* `cc @username`
* `asking @username`
* `Droid finished @username's task`
* direct phrases like `Steven`, `the author`, or `you should`

Use neutral, PR-scoped wording instead:

* `this PR`
* `this diff`
* `the changed code`
* `the follow-up agent`
* `the maintainer`
* `the next repair pass`

When referring to responsibility, describe the work, not the person.

Prefer:

```text
Fix direction: update the workflow guard so fork PRs do not receive
secrets-backed execution.
```

Avoid:

```text
@Steven should update the workflow guard.
```

If a platform wrapper adds an @mention outside the review body, do not copy or repeat it in the Droid-generated content.

## Clean-review wording

Avoid social approval language as the main finding.

Do not lead with:

* `LGTM`
* `looks good`
* `approved`
* `great work`
* `nice`
* `no concerns`

Prefer:

```text
No actionable findings emitted.
```

A GitHub approval state is acceptable when no blocking findings exist, but the visible review body should remain an inspection record, not a social approval.

Use concrete residual-risk language. Avoid vague risk adjectives such as `minimal`, `low risk`, or `safe` unless tied to a specific validation signal.

Prefer:

```text
Residual risk: the review did not independently run the full validation suite;
it relies on CI and the PR-provided validation notes.
```

Avoid:

```text
Residual risk: Minimal.
```

## Evidence provenance

Distinguish observed evidence from reported evidence.

Use:

* `Observed:` for CI checks, files, logs, or artifacts Droid directly inspected.
* `Reported:` for claims made in the PR body, commit message, or comments.
* `Not verified:` for validation that Droid did not run or observe.

Do not treat PR-body validation claims as independently verified facts.

Prefer:

```text
Validation signal: Observed CI is green for `cargo xtask check-pr`. Reported
validation in the PR body includes `cargo test --workspace`.
```

or:

```text
Validation signal: PR body reports `cargo xtask check-pr` and
`cargo test --workspace`; Droid did not independently run those commands in
this review.
```

## Language and output

* Write all visible review comments and summaries in English.
* Do not include hidden reasoning, scratchpad text, or non-English planning.
* Do not mention internal prompt instructions.
