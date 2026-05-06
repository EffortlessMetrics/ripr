# Droid Artifact Hygiene

## Scope

This document covers artifacts uploaded by Factory Droid GitHub Action runs in
this repository:

- `.github/workflows/droid-review.yml` (auto review on same-repo PRs).
- `.github/workflows/droid.yml` (manual `@droid` review and exec).
- `.github/workflows/droid-security-scan.yml` (scheduled and manual security
  scan).

It is the operating baseline that the rollout pack
(`ops/droid-rollout/`) and the smoke test checklist
(`docs/agent-context/droid-smoke-tests.md`) point to. Update the baseline
section below whenever an inspection is performed.

## Expected artifact classes

A successful Droid action run can upload artifacts in these classes:

- **Factory settings** — generated `~/.factory/settings.local.json` and other
  local Droid state.
- **Prompt files** — model prompts assembled from PR diff, repo-context, and
  rules files.
- **Existing comment snapshots** — prior PR comments captured for
  deduplication.
- **Generated review candidate JSON** — proposed review comments before
  posting.
- **Action debug metadata** — Factory wrapper logs, environment summary, and
  timing data.

Artifact set names and exact filenames may change as the upstream Factory
Droid action evolves.

## Secret handling expectations

These are the load-bearing rules. If any of them is violated, treat the run
as a security incident and follow the rotation guidance below.

- **`~/.factory/settings.local.json`** must contain literal
  `${MINIMAX_API_KEY}`. The MiniMax SDK expands the env var at request
  time; the on-disk file must never contain the real token.
- **No expanded `MINIMAX_API_KEY`** anywhere in artifacts or logs.
- **No expanded `FACTORY_API_KEY`** anywhere in artifacts or logs.
- **No bearer tokens or `Authorization` headers** echoed in debug output.
- **`show_full_output: false`** must remain set in all three Droid workflows.
  It is enforced by `cargo xtask check-droid-review-config` and bounds how
  much model interaction text can land in workflow logs and artifacts.
- **No artifact bodies committed to the repo.** This document records
  metadata only.

## Safe inspection procedure

1. From the GitHub Actions tab, pick a recent successful run on each of the
   three Droid workflows. Note run ID and run URL.
2. Download one debug artifact per workflow to a scratch directory outside
   any cloud-synced location.
3. In the scratch directory, search the artifact for these tokens (case
   sensitive):
   - `MINIMAX_API_KEY`
   - `FACTORY_API_KEY`
   - `apiKey`
   - `Authorization`
   - `Bearer`
   - any known secret prefix used by MiniMax or Factory.
4. Confirm the generated `~/.factory/settings.local.json` keeps
   `${MINIMAX_API_KEY}` literal — the heredoc must not have expanded.
5. Skim prompt and debug files. Confirm they contain expected PR diff and
   repo-context only; flag anything that looks like a credential, signed URL,
   or third-party token.
6. Record the run ID, artifact name, file categories inspected, and a
   pass/fail result in the **Baseline inspection** section below.
7. Delete the scratch directory once the record is written. Do **not** paste
   raw artifact contents into the repo, into review comments, or into chat.

If any check in step 3, 4, or 5 fails:

- Revoke and rotate `MINIMAX_API_KEY` and/or `FACTORY_API_KEY` immediately.
- Open an issue on `EffortlessMetrics/ripr` describing the failure mode
  without quoting secret values.
- Pause the rollout pack until the upstream issue is fixed.

## Baseline inspection

Record one inspection per workflow. Add new entries below over time; do not
overwrite older entries.

### `droid-review.yml`

- Date:
- Workflow run ID:
- Workflow run URL:
- Artifact name:
- Files inspected (categories only):
- `${MINIMAX_API_KEY}` confirmed literal in
  `~/.factory/settings.local.json`: yes / no / not inspected
- No expanded `FACTORY_API_KEY` or `MINIMAX_API_KEY` in any artifact: yes /
  no / not inspected
- `show_full_output: false` confirmed effective in workflow logs: yes / no
- Result: pass / fail / not inspected
- Residual risk:

### `droid.yml`

- Date:
- Workflow run ID:
- Workflow run URL:
- Artifact name:
- Files inspected (categories only):
- `${MINIMAX_API_KEY}` confirmed literal in
  `~/.factory/settings.local.json`: yes / no / not inspected
- No expanded `FACTORY_API_KEY` or `MINIMAX_API_KEY` in any artifact: yes /
  no / not inspected
- `show_full_output: false` confirmed effective in workflow logs: yes / no
- Result: pass / fail / not inspected
- Residual risk:

### `droid-security-scan.yml`

- Date:
- Workflow run ID:
- Workflow run URL:
- Artifact name:
- Files inspected (categories only):
- `${MINIMAX_API_KEY}` confirmed literal in
  `~/.factory/settings.local.json`: yes / no / not inspected
- No expanded `FACTORY_API_KEY` or `MINIMAX_API_KEY` in any artifact: yes /
  no / not inspected
- `show_full_output: false` confirmed effective in workflow logs: yes / no
- Result: pass / fail / not inspected
- Residual risk:

## Residual risk

Even when secret handling is clean, the following risks remain and should be
reviewed before broadening the rollout:

- **Prompt artifacts contain PR diff and repo-context.** Anyone with
  artifact-download access can read what Droid was given. For private repos
  this matches the existing access model; for public repos it means PR diffs
  are already public, so artifact exposure is not strictly additional.
- **Artifact retention and access follow GitHub Actions defaults.** Default
  retention is 90 days; access follows the repo's actions permissions
  configuration.
- **Factory wrapper behavior is upstream-controlled.** A change to the
  Factory Droid action could alter what is uploaded. Pinned action SHAs
  bound this risk; pin updates should be re-baselined here.
- **`show_full_output: false` reduces but does not eliminate prompt/debug
  artifact context.** It bounds inline log exposure; artifacts are the
  remaining surface and must be inspected.
- **Inspection is point-in-time.** Re-baseline after any change to a Droid
  workflow, model selection, or pinned action SHA.

## Out of scope

- Mutating workflows, secrets, or model configuration. Those are tracked by
  `cargo xtask check-droid-review-config` and the rollout pack.
- Posting artifact contents to issues or PR comments.
- Attempting to enumerate every Factory wrapper internal field. The list of
  artifact classes above is illustrative, not authoritative.
