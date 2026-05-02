# Scoped PR Contract

A work item usually produces one scoped PR.

A scoped PR is the mergeable review unit for `ripr`: one narrow production
delta, one acceptance criterion, and the evidence package needed to review it.

This contract belongs to work items and PRs. It is not the definition of Codex
`/goal`; a Codex goal is a multi-PR implementation campaign.

## Required Fields

Each work item or PR should define:

- work item ID
- scope
- production delta
- evidence/support delta
- single acceptance criterion
- required commands
- non-goals

The task should not say "make the analyzer better." It should name one
capability, public contract, or architecture seam.

## Task Template

```text
Work item:
<campaign work item ID>

Goal:
<one capability or seam>

Scope:
One scoped production behavior, public contract, or architecture seam.
Large fixture, docs, and golden support is fine when it supports this one slice.

Production delta:
<exact module, command, or contract being changed>

Evidence/support delta:
<specs, fixtures, tests, goldens, metrics, docs, ADRs, or learnings expected>

Acceptance:
<single reviewable claim>

Required commands:
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
git diff --check

If analyzer output changes:
cargo xtask fixtures
cargo xtask goldens check

If extension files change:
cd editors/vscode
npm ci
npm run compile
npm run package

Non-goals:
<explicit exclusions>

Do not:
- add panic-family shortcuts
- use mutation-runtime outcome language in static output
- add non-Rust implementation files outside allowlisted surfaces
- add shell scripts instead of xtask commands
- add new crates without an approved workspace-shape decision
```

## Done Criteria

A scoped PR is done when:

- the acceptance criterion passes
- the production delta remains scoped
- expected evidence artifacts are present
- required commands pass or the remaining blocker is documented
- the PR summary explains reviewer focus
- non-goals remain out of scope

If the work item cannot be completed cleanly, stop by writing a blocked report
or updating the relevant doc, spec, or learning note. Do not silently broaden the
PR.

## Evidence Package

Evidence should scale with risk and public surface. Depending on the work item,
the package may include:

- spec update
- unit test
- fixture BDD case
- golden human output
- golden JSON output
- context packet expectation
- LSP diagnostic or hover expectation
- output contract registry update
- capability metric update
- traceability manifest update
- changelog entry
- ADR or learning note

Large evidence diffs are welcome when they make one narrow production delta
reviewable.

## Review Boundary

Humans review and merge scoped PRs. Codex Goals may continue to another work
item only when the next item is independent or explicitly stackable.

For non-stackable work, the safe boundary is:

```text
open PR -> generate reports -> wait for human review/merge
```
