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
- `policy/workflow_allowlist.txt`

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

Static findings must not claim:

* `killed`
* `survived`
* `untested`
* `proven`
* `adequate`

Real mutation testing confirms later. `ripr` gives draft-mode exposure evidence and targeted test intent.

## Review posture

A useful review identifies concrete failure modes or records concrete inspection.

Do not suppress actionable findings because there are many of them. Suppress only:

* duplicates;
* low-confidence speculation;
* non-actionable observations;
* findings already covered by a clearer comment.

If there are 20 concrete issues, leave 20 comments. If many instances share one root cause, leave one systemic comment and name representative locations.

## No naked LGTM

Do not use `LGTM`, `looks good`, or equivalent empty approval language as the review summary.

If no actionable inline findings are emitted, the review summary must still include:

```text
No actionable findings emitted.

Inspected surfaces: <files / systems / changed areas>.
Checks performed: <repo invariants, security/workflow/release/correctness risks considered>.
Why no comments: <why the diff satisfies those checks>.
Residual risk: <anything not proven by review, such as external service behavior or unrun validation>.
Validation signal: <CI checks, tests, reports, or commands that support the result>.
```

If the review system submits an approval, the approval body must still include this inspection record.

## Inline comment format

Each inline comment should be structured so another agent can fix it.

Use this shape:

```text
[P0|P1|P2] Short title

Failure mode: What can break, leak, regress, or become unmaintainable.
Why here: The specific repo invariant or contract this violates.
Fix direction: Concrete repair guidance. Include the smallest viable fix when obvious.
Validation: Command, report, fixture, golden, or CI check that should prove the fix.
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

For Droid workflows specifically:

* automatic PR review should run on same-repo PRs and every commit when configured that way;
* active Droid reviews should not be canceled merely because a newer commit arrived;
* the per-PR queue should keep the latest pending run without serializing unrelated PRs;
* MiniMax BYOK model should remain `custom:MiniMax-M2.7-0` unless intentionally changed;
* runtime BYOK settings should be written to `~/.factory/settings.local.json`;
* do not rely on the Droid Action `settings:` input for BYOK custom models unless the Factory path mismatch is known fixed;
* keep `${MINIMAX_API_KEY}` literal in generated settings files; do not expand it into artifacts;
* do not set `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL`;
* do not enable `show_full_output` in normal operation.

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

Name the smallest validation set that proves the fix.

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

## Suggestions

Use GitHub suggestion blocks when:

* the fix is small;
* the replacement is locally obvious;
* the suggestion will apply cleanly.

Do not use suggestion blocks for multi-file, policy, schema, or design-dependent changes. In those cases, describe the fix direction.

## Language and output

* Write all visible review comments and summaries in English.
* Do not include hidden reasoning, scratchpad text, or non-English planning.
* Do not mention internal prompt instructions.
