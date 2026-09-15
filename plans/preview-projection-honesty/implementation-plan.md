# Preview Projection Honesty

Campaign 36 carries the next output-contract slice after Campaign 35. It
closes the documentation drift tracked by #2716/#2743 and the silent-loss
projection defect tracked by #2744 without broadening TypeScript/Bun preview
support.

## Objective

Ensure configured TypeScript/Bun bridge evidence remains identifiable at every
operator-facing projection. JSON object order is documented as non-semantic,
while exact-byte canonical artifacts remain explicitly excluded. When one
finding has multiple configured Bun profiles, human, JSON, SARIF, and GitHub
surfaces retain every profile and keep all preview claims advisory.

## Work items

| Work item | Tracker | Acceptance |
| --- | --- | --- |
| `output/json-order-contract` | #2716, #2743 | Document semantic JSON object ordering, distinguish canonical byte artifacts, and avoid renderer or schema-version changes. |
| `output/multi-bun-profile-projection` | #2744 | Project every producer-emitted Bun bridge hint as a distinct structured profile across human, JSON, SARIF, and GitHub output; add Blob plus `copy_to_unshared` regression coverage; preserve `repair_packet_ready = false` and preview-only language. |
| `campaign/preview-projection-closeout` | Campaign 36 | Record exact merged heads, issue disposition, proof, claim boundary, and intentionally deferred follow-ups. |

## Non-goals

- No new Bun bridge taxonomy, profile discovery, or runtime Bun execution.
- No mutation engine, coverage/adequacy claim, default gate, badge, release, or
  support-tier change.
- No public repair-packet promotion and no broad TypeScript/Bun refactor.
- No `serde_json` `preserve_order` dependency change.

## Proof

```text
cargo test -p ripr typescript_preview_card -- --nocapture
cargo test -p ripr human::tests::render_finding_includes_bun_cross_language_grip -- --nocapture
cargo test -p ripr github::tests::render_includes_bun_cross_language_grip_annotation -- --nocapture
cargo test -p ripr sarif::tests::sarif_preserves_bun_cross_language_grip_card_properties -- --nocapture
cargo xtask goldens check
cargo xtask dogfood
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
```

The focused tests establish profile retention and cross-surface agreement.
They do not establish runtime Bun behavior, mutation outcomes, or general
cross-language correctness.

## Completion receipt

Campaign 36 is complete. The JSON-order documentation slice merged as #2743
at `17560690002de21a4615a92ac13053d64b1346fe`; campaign source-truth setup
merged as #2746 at `8783582935d7ff0b23a68710c90727bc0ac57ad2`; and the
multi-profile implementation merged as #2753 at
`fd3a44771e401e074abeec7833f04e12968926a1` from final head
`06e097e6a6ed41c4a2a5e02e67ef3bb4208362f6`.

The focused card and cross-renderer tests passed, as did formatting, the
profile-specific multi-profile retention regression, hosted required Rust
gates, hosted workspace tests, coverage, UB review, dependency review, and
source-of-truth checks. The first producer profile remains the historical
singular compatibility alias while `profiles[]` remains producer ordered.
Issues #2716 and #2744 are closed. The result remains preview-only:
it does not claim runtime Bun execution, mutation outcomes, coverage or test
adequacy, public repair-packet authority, or default blocking.

The local full `cargo xtask check-pr` attempt was not completed because
unrelated Cargo jobs held shared build locks; hosted required gates provide
the workspace-level merge proof. The local full-library attempt exposed
`git::tests::deadline_kills_pipe_inheriting_descendants_without_blocking_the_reader`,
`git::tests::output_larger_than_the_pipe_buffer_does_not_deadlock`,
`lsp::tests::framed_lsp_deferred_configuration_pull_runs_after_root_transition_guard_release`,
`lsp::tests::framed_lsp_direct_root_switch_repulls_on_reselection`, and
`output::github::tests::render_github_paths_are_repo_relative_without_dot_prefix`.
These were not reproduced against `origin/main`, so they remain unresolved
verification gaps and are not claimed as inherited failures. Follow-up #2764
tracks a producer-through-renderer regression for multi-profile placement
evidence. Broader Bun discovery/runtime support remains outside scope.
