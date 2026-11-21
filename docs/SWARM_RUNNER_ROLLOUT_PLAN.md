# Swarm Runner Rollout Plan

This plan standardizes routed CI across `*-swarm` repositories using the HL7v2 PR #73 pattern as the reference implementation.

## Core routing contract

- Medium Rust lanes: `CPX42 -> CX43 -> CX53 -> GitHub-hosted`.
- Heavy Rust lanes: `CX53 -> CX43 -> GitHub-hosted`.
- Fork PRs must never run on self-hosted runners.
- One normalized required check per repository: `<Repo> Rust Small Result`.

## Router discovery rule

Use org-level runner discovery only:

```bash
gh api "orgs/${ORG}/actions/runners?per_page=100"
```

Required environment:

- `GH_TOKEN: ${{ secrets.EM_RUNNER_READ_TOKEN }}`
- `ORG: EffortlessMetrics`

Do **not** use repository runner discovery:

```text
repos/<owner>/<repo>/actions/runners
```

## Stable router outputs

- `router_target=cpx42|cx43|cx53|github`
- `router_reason=cpx42_idle|cx43_idle|cx53_idle|no_idle_runner|runner_image_unavailable|runner_token_missing|runner_token_unauthorized|runner_token_forbidden|runner_api_failed|parse_failed|fork_or_untrusted_pr`
- Optional `router_error=false|true` only for repos that already expose an error-classification output.

Do not rename existing reason values without an explicit migration that updates
the router summary, normalized result summary, rollout evidence, and downstream
issue comments. In particular, `fork_or_untrusted_pr` and
`runner_image_unavailable` are already used by `ripr-swarm` and must remain
valid routed-result vocabulary.

## CPX42 execution model

- Use direct Rust toolchain setup (`dtolnay/rust-toolchain@v1`, Rust `1.95.0`).
- Prepare `TMPDIR` and `CARGO_TARGET_DIR` before toolchain setup.
- Do not assume local `em-ci-rust:1.95` Docker image on CPX42.
- Run repo Rust gate on host and clean scratch afterwards.

## CX43/CX53 execution model

- Preserve known-good local Docker image path where already working.
- Do not rewrite working CX43/CX53 jobs to host toolchain without specific failures.

## Rollout order

1. Batch 1 medium pilots: `tokmd-swarm`, `OpenRacing-swarm`.
2. Batch 2 mixed lanes: `perl-lsp-swarm` (split small vs corpus), `adze-swarm` (lane-size dependent).
3. Batch 3 heavy hold: keep `bitnet-rs-swarm` heavy lane on `CX53 -> CX43 -> GitHub-hosted` until tiny lane exists.
4. Batch 4 long tail: `uselesskey-swarm`, `ripr-swarm`, `unsafe-review-swarm`, `perfgate-swarm`, `shiplog-swarm`, `shipper-swarm`, `atlasctl-swarm`.

## Stop conditions

Do not merge if any are true:

- Repository runner endpoint is used.
- CPX42 assumes `em-ci-rust:1.95` image.
- `TMPDIR` created after toolchain step.
- Result job does not include conditional runner jobs.
- Fork PR can reach self-hosted jobs.
- Hosted fallback removed.
- Branch protection changed in same PR.
- Release/publish/signing workflows are modified.

## Merge conditions

Merge when all are true:

- Org runner discovery is used.
- Expected primary route is selected for lane class when the PR is explicitly
  proving a primary self-hosted rollout and an idle image-ready runner is
  discoverable.
- GitHub-hosted fallback is accepted when the router records an allowed fallback
  reason such as `no_idle_runner`, `runner_image_unavailable`,
  `runner_api_failed`, `runner_token_missing`, `runner_token_unauthorized`,
  `runner_token_forbidden`, `parse_failed`, or `fork_or_untrusted_pr`.
- Selected implementation lane succeeds.
- Non-selected implementation lanes are skipped in the normal success path.
- Normalized result check succeeds.
- Guardrail checks pass.
