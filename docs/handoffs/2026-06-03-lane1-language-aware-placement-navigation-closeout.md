# Closeout: Lane 1 Language-Aware Placement Navigation

Date: 2026-06-03

Branch: `campaign-language-aware-placement-closeout`

Closed goal: `lane1-language-aware-placement-navigation`

Latest merged prerequisite PR: #941 `goals: advance language-aware placement
closeout` at `3bbb6a7fc8160d1e094735b401096fe643113af9`

## Current State

The Lane 1 language-aware placement navigation campaign is closed. The active
manifest now records `status = "closed"` and `no_current_goal = true`; the
archived manifest is
`.ripr/goals/archive/2026-06-03-lane1-language-aware-placement-navigation.toml`.

This was a post-0.8.0 trust-debt campaign for #911. It does not claim that the
already-published 0.8.0 tag contains behavior that landed afterward, and it
does not move release, signing, publishing, or distribution authority from
source `ripr` to `ripr-swarm`.

## PR Chain

| PR | Merged commit | Result |
| --- | --- | --- |
| #937 `goals: select language-aware placement campaign` | `8fd61978b0b6e4f7355bb711c30ab3002574cd81` | Selected #911 as the next Lane 1 successor after large-repo runtime completeness closed without selecting a successor. |
| #938 `report: surface external targets as navigation-only limitations` | `f3830512d0fec3ad02c595fe824984141bcae239` | Review-comments, LSP static-limit notes, and packet-adjacent targeted-test briefs can surface explicit configured external observer target evidence as navigation-only limitation context. Unknown external targets keep a named blocked route and no repair action, verify command, receipt command, or allowed edit surface. |
| #940 `report: summarize language-aware placement route quality` | `75af3079b94f3dc3714750b6afce8065aefb884c` | Readiness and evidence-quality scorecard surfaces summarize language-aware placement limitations and navigation-only external target evidence without promoting unresolved or preview external targets into public repair packets. |
| #941 `goals: advance language-aware placement closeout` | `3bbb6a7fc8160d1e094735b401096fe643113af9` | Marked the route-quality slice done and made this closeout the next ready work item. |

## Issue State

| Issue | Closeout state |
| --- | --- |
| #911 | This closeout supports closing #911 for the scoped suggested-test placement safety issue: binding, FFI, and externally tested seams no longer get confident unrelated Rust repair placement from the covered surfaces, and explicit external observer targets remain navigation-only limitation context. Future language-aware repair cards or inferred external test targets should open fresh issue-backed work. |
| #908 | Remains open for broader cross-language oracle graph work. This campaign did not prove external test-suite coverage or make cross-language evidence actionable. |
| #910 | Remains open for TS discriminator plus binding/FFI reachability proof. This campaign did not classify TS-tested Rust seams as externally observed; it only preserved fail-closed placement and route-quality behavior while that graph is unresolved. |

## Closeout Validation

Closeout validation in this PR:

```bash
rtk cargo xtask check-goals
rtk cargo xtask goals next
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

The merged PRs above carried the focused product validation:

```bash
rtk cargo test -p ripr suggested_test -- --test-threads=1
rtk cargo test -p ripr typescript_preview_card_projects_bun_cross_language_grip -- --test-threads=1
rtk cargo test -p ripr lsp --lib
rtk cargo xtask ripr-swarm readiness
rtk cargo xtask evidence-quality-scorecard
rtk cargo xtask check-output-contracts
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
```

## Claim Boundary

This closeout permits these claims for `ripr-swarm/main` after #941:

- Suggested-test placement for binding, FFI, and externally tested seams fails
  closed when RIPR lacks explicit target evidence.
- Explicit configured external observer target evidence may appear as
  navigation-only limitation context.
- Navigation-only external target evidence does not create public repair
  packets, verify commands, receipt commands, or allowed edit surfaces.
- LSP, review-comments, packet-adjacent briefs, readiness, and scorecard
  surfaces preserve language-aware placement limitations as limitations.
- Ordinary Rust seams with local Rust-side test context remain outside this
  limitation closeout and keep their existing locally supported behavior.

This closeout does not permit claims that:

- 0.8.0 on crates.io includes these post-release changes.
- full cross-language oracle tracing exists;
- TS, Python, or other external test targets can be inferred without explicit
  bridge or observer evidence;
- TS-tested Rust seams are classified as externally observed;
- static limitations or navigation-only targets are repair packets;
- RIPR invents verify commands, receipt commands, candidate values, or allowed
  edit surfaces for unresolved external targets;
- RIPR runs mutation execution, autonomous code editing, provider integration,
  default blocking CI, or a badge semantic switch;
- source release, signing, publishing, or distribution authority moved to
  `ripr-swarm`.

## Policy Ledger Changes

No support-tier, release, package, network, badge, no-panic, dependency,
provider, autonomous-edit, mutation-execution, or default CI blocking policy
ledger changed in this closeout.

## Remaining Work

No work items remain in the closed campaign. The active manifest intentionally
records `no_current_goal = true`; future work should be selected from current
repo-owned state rather than continuing this #911 manifest.

Expected successor themes remain outside this closeout:

- cross-language oracle graph analysis, tracked by #908 and #910;
- language-aware repair cards or external target inference only when a fresh
  issue names the measured gap after the navigation-only closeout;
- any source release or hotfix publication, which still requires explicit
  release authorization in source `ripr`.

## Archive Updates

- Active goal manifest closed:
  `.ripr/goals/active.toml`
- Archived active goal manifest:
  `.ripr/goals/archive/2026-06-03-lane1-language-aware-placement-navigation.toml`
- Handoff:
  `docs/handoffs/2026-06-03-lane1-language-aware-placement-navigation-closeout.md`
