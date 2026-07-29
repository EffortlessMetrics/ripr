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

