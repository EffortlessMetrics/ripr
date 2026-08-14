# Campaign 36 Closeout: Preview Projection Honesty

Date: 2026-07-29  
Campaign: `preview-projection-honesty`  
Status: complete

## Delivered

- #2743 (`17560690002de21a4615a92ac13053d64b1346fe`) documented that JSON
  object member order is semantic-insensitive and excluded exact-byte
  canonical artifacts from that claim.
- #2746 (`8783582935d7ff0b23a68710c90727bc0ac57ad2`) carried the Campaign 36
  plan, non-goals, acceptance criteria, and proof path into the repository.
- #2753 (`fd3a44771e401e074abeec7833f04e12968926a1`) merged final head
  `06e097e6a6ed41c4a2a5e02e67ef3bb4208362f6`, retaining every configured Bun
  bridge profile across human, JSON, SARIF, and GitHub output. The first
  producer profile remains the singular v1 compatibility alias; multi-profile
  JSON keeps `profiles[]` in producer order. Profile placement evidence is
  associated by producer order when it is present.

- #2757 (this closeout PR) carries the exact merge heads, proof results, claim
  boundary, local verification gaps, and follow-up disposition into the repo.

Issues #2716 and #2744 are closed.

## Acceptance and proof

- Blob plus `copy_to_unshared` focused fixtures prove two profiles survive the
  card projection, preserve the first-profile compatibility alias, and retain
  separate per-profile data. The Blob placement reason is producer-backed; the
  copy-specific placement string in the renderer fixture proves ordering and
  association only, not a production placement producer.
- Human, GitHub, JSON/SARIF, and preview-card tests passed; the focused
  cross-renderer suite passed 8 tests and the preview-card suite passed 14.
- `cargo fmt --all -- --check` and
  `cargo clippy -p ripr --lib --tests -- -D warnings` passed locally.
- Hosted required Rust gates, workspace tests, coverage, UB review, dependency
  review, and source-of-truth checks passed on the final head. The required
  `Ripr Rust Small Result` check passed before auto-merge.
- Golden, dogfood, output-contract, static-language, no-panic, and fixture
  contract checks passed during the lane; hosted required gates revalidated
  the final pushed head after review fixes.

## Claim boundary

The projection remains static preview evidence. It does not claim runtime Bun
execution, mutation outcomes, test adequacy or coverage, public repair-packet
authority, default blocking, or release readiness. The campaign did not add a
new Bun taxonomy, discovery engine, runtime, dependency, or broad refactor.

## Verification gaps and disposition

The local full `cargo xtask check-pr` attempt did not complete because
unrelated Cargo jobs held shared build locks. The local full-library attempt
reported these failures: `git::tests::deadline_kills_pipe_inheriting_descendants_without_blocking_the_reader`,
`git::tests::output_larger_than_the_pipe_buffer_does_not_deadlock`,
`lsp::tests::framed_lsp_deferred_configuration_pull_runs_after_root_transition_guard_release`,
`lsp::tests::framed_lsp_direct_root_switch_repulls_on_reselection`, and
`output::github::tests::render_github_paths_are_repo_relative_without_dot_prefix`.
They were not reproduced against `origin/main`, so they remain unresolved local
verification gaps rather than an inherited-baseline conclusion. Hosted
workspace tests passed, and the failures are not attributed to this campaign.
GitHub's automated review rate limit prevented a CodeRabbit review; the
available review threads were addressed and resolved, including the profile
alias compatibility finding.

Follow-up #2764 records the missing producer-through-renderer regression for
multi-profile placement evidence. Runtime Bun support, broader profile
discovery, and non-preview repair-packet promotion remain intentionally outside
this campaign.
