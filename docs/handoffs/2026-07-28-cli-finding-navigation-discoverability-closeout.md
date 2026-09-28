# Closeout: CLI Finding Navigation Discoverability

Date: 2026-07-28

Goal: complete the issue-backed CLI discoverability campaign from #2598 through
the follow-up scope-preservation work in #2659.

## Decision

The campaign is closed for the current repository state. The default human
`ripr check` path now gives an operator an executable route from a finding to
`ripr explain` and `ripr context`, and the route preserves the analysis scope
needed to make those follow-up commands describe the same finding.

The campaign deliberately stayed within the product contract:

```text
check finding
-> copy the finding-specific explain/context command
-> replay the same scope and artifact inputs
-> inspect the evidence path and missing discriminator
```

## What Landed

| Work item | Result |
| --- | --- |
| #2598 / #2620 | Added the human-output follow-up route for `explain` and `context --at`, including quiet empty and fully suppressed states. |
| #2659 / #2681 | Preserved the analyzed scope in copied navigation commands and made the replay path executable for supported config, mode, artifact, and shell-argument combinations. |
| Golden corpus | Refreshed the governed human outputs for the changed command guidance and verified zero remaining golden drift. |
| Follow-up tracking | Closed #2659 automatically through #2681. Issue #2598 remains open with `status/done-open` because it is the delivered product tracking issue. |

## Validation

The final #2681 head was `624288421659e11b6d9d3f16873b7a7b6775243a`; it merged to
`main` as `89ce52c6`. Hosted proof passed:

- required `Ripr Rust Small Result` (`30411789655`, result `90451516232`);
- routed CX43 required gates (`90449365102`);
- workspace tests (`rust-tests-junit`, 3893/3893);
- coverage, source-of-truth, dependency, security, UB review, Codecov, and
  routing checks.

Focused local proof also covered navigation unit tests, CLI help consistency,
custom-scope replay, artifact reuse with non-default mode, and the governed
golden check.

## Claim Boundary and Follow-Up

This proves executable, scope-preserving static-evidence navigation. It does
not prove runtime mutation outcomes, test adequacy, or release readiness.

The inherited all-target Clippy baseline failure at
`crates/ripr/tests/cli_smoke.rs:3398` is tracked separately in #2679. It was
not widened into this campaign because it predates the navigation change and
does not invalidate the merged required proof for #2681.

