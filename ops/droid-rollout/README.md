# Droid Rollout Pack

This directory contains the known-good Factory Droid + MiniMax M2.7 BYOK setup
used by `ripr`. It is a **template**: the YAML files here are not active
workflows. To roll the setup out to another EffortlessMetrics repository, copy
the files into `.github/workflows/` and the docs into the target repo's docs
tree, then validate against the smoke and invariant checklists.

## What this pack contains

| Source file                                     | Destination in target repo                          |
| ----------------------------------------------- | --------------------------------------------------- |
| `ops/droid-rollout/droid-review.yml`            | `.github/workflows/droid-review.yml`                |
| `ops/droid-rollout/droid.yml`                   | `.github/workflows/droid.yml`                       |
| `ops/droid-rollout/droid-security-scan.yml`     | `.github/workflows/droid-security-scan.yml`         |
| `ops/droid-rollout/review-guidelines.SKILL.md`  | repo skill / agent-context (location is repo-local) |
| `ops/droid-rollout/droid-review-rules.md`       | repo agent-context                                  |
| `ops/droid-rollout/review-invariants.md`        | repo agent-context                                  |
| `ops/droid-rollout/droid-smoke-tests.md`        | repo agent-context                                  |
| `ops/droid-rollout/repo-rollout-checklist.md`   | use during rollout, not committed                   |

## Required secrets

Add the following repository secrets (Settings → Secrets and variables →
Actions):

- `FACTORY_API_KEY` — Factory Droid API key for the org's Factory account.
- `MINIMAX_API_KEY` — MiniMax API key with access to the M2.7 model.

Do not commit `.factory/settings.local.json` or any expanded form of
`MINIMAX_API_KEY`.

## Required GitHub App

Install the **Factory Droid GitHub App** on the target repository. Without the
app, the manual `@droid` flow and PR comment integration will not work.

## Required workflow controls (do not weaken)

These are load-bearing. They are enforced upstream by `ripr`'s
`cargo xtask check-droid-review-config`; reproduce them faithfully:

- **Same-repo guard** for the automatic review workflow:
  `head.repo.full_name == github.repository`.
- **Trusted actor guard** for the manual `@droid` workflow:
  `OWNER`, `MEMBER`, or `COLLABORATOR` only.
- **`show_full_output: false`** on every Droid action step.
- **Pinned action SHAs** — every `uses:` ref is a 40-character commit SHA, not
  a tag or branch.
- **Runtime BYOK** — Factory settings are written to
  `~/.factory/settings.local.json` in a step before the Droid action runs. Do
  not use the Droid action's `settings:` input for BYOK custom models.
- **Quoted heredoc** — the heredoc must be `<<'JSON'` (single-quoted) so
  `${MINIMAX_API_KEY}` is preserved as a literal in the file. The MiniMax SDK
  expands it at request time.
- **`review_depth: shallow`** unless you have a documented reason to change.
- **`custom:MiniMax-M2.7-0`** for both `review_model` and `security_model`.
- **`MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}`** as a job-level env, so
  the heredoc references the runtime env var.

## Branch protection considerations

If branch protection requires status checks before merge:

- Do not list the Droid Auto Review workflow as a required check unless you
  accept that flaky model runs can block merges.
- Required reviews from CODEOWNERS still apply; Droid review is advisory.

## Fork PR behavior

The same-repo guard (`head.repo.full_name == github.repository`) means Droid
**will not** review PRs from forks. This is intentional — the workflow uses
secrets and write permissions, neither of which should be exposed to fork PRs.
External contributors can request a review by mentioning a maintainer.

## Draft PR behavior

Draft PRs **are reviewed**. The automatic review workflow's `pull_request`
trigger includes `ready_for_review`, but the same-repo guard does not exclude
drafts. This matches `ripr`'s queueing invariants (drafts are reviewable; do
not cancel an active review).

## `[skip-review]` escape hatch

To skip Droid Auto Review on a particular PR, include `[skip-review]` in the
PR title. The automatic workflow's `if:` clause checks for this token. Use it
sparingly — for example, mechanical refactors with extensive evidence
elsewhere.

## Smoke test procedure

After installing the pack and before relying on it, run through
`droid-smoke-tests.md`.

## Artifact hygiene check

After the first successful Droid run, inspect one debug artifact and confirm:

- generated Factory settings keep `${MINIMAX_API_KEY}` literal;
- no expanded MiniMax or Factory API token appears in any artifact;
- `show_full_output: false` is in effect for all Droid action steps.

`ripr`'s `docs/operations/droid-artifacts.md` documents the inspection
procedure.

## Adapting repo-specific review guidance

The skill and rules templates contain repo-neutral guidance. Tailor:

- **Glossary** — domain terms specific to the target repo.
- **Review priorities** — what reviewers should look at first (e.g. schema
  drift, public API, performance hot paths).
- **Validation commands** — replace `cargo xtask` references with the target
  repo's equivalent gates.

Preserve these review-output invariants verbatim:

- no naked LGTM;
- no arbitrary comment cap;
- repair-ready findings (failure mode / why here / fix direction / validation
  / confidence);
- Observed / Reported / Not verified evidence labels;
- no extra `@mentions` in Droid-generated review bodies.
