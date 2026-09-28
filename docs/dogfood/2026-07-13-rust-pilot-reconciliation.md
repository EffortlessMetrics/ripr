# Rust pilot reconciliation — 2026-07-13

This reconciliation follows the fresh six-candidate analysis-only scout
recorded in [issue #1560 comment 4950688510](https://github.com/EffortlessMetrics/ripr-swarm/issues/1560#issuecomment-4950688510).
It updates the governed corpus without promoting any candidate to an eligible
repair attempt.

## Identity decision

The existing corpus already contains `ub-747-static-limitation` for
`EffortlessMetrics/ub-review`, source `PR #747`, head
`fc486dc2be564c34b3047e4b5e90accc853ec13e`, and canonical candidate
`EffortlessMetrics/ub-review#747`. The follow-up #747 observation has the same
repository, head, source, and candidate identity, so it is recorded in the
`observations` ledger as `duplicate_observation` rather than appended as a
second exclusion.

| Observation | Repository/head | Treatment | Reason |
| --- | --- | --- | --- |
| ripr #1545 | `ripr-swarm@0db4754aa57845d9473ada2e73f40ede2d75138f` | new exclusion | static limitation |
| ripr #1515 | `ripr-swarm@11192c85cbf5016aaf45d44abd9ce53e0b7557d7` | new exclusion | analysis timeout |
| perl-lsp #3703 | `perl-lsp-swarm@f4b3095e81f16a2e931acccaf2a5d479719fbdfc` | new exclusion | static limitation |
| perl-lsp #3738 | `perl-lsp-swarm@81025bcc89515eb30659e1e7388927d2bb1f9d99` | new exclusion | static limitation |
| ub-review #738 | `ub-review@98aea1868f92c6c0ffe89d9faae83fba11de3019` | new exclusion | static limitation |
| ub-review #747 | `ub-review@fc486dc2be564c34b3047e4b5e90accc853ec13e` | duplicate of `ub-747-static-limitation` | static limitation |

## Counts

The report distinguishes the observation stream from the unique exclusion and
attempt denominators:

- observed runs: **19** (13 existing exclusion observations + 6 follow-up observations);
- unique exclusions: **18** (13 existing + 5 new);
- duplicate observations: **1** (the repeated ub-review #747 candidate);
- timeout observations: **4** (3 existing repo-wide timeouts + ripr #1515);
- eligible repair attempts: **0**;
- cases: **[]**.

The five new candidate heads are explicitly listed under each repository's
`authorized_observation_heads`; repository authorization revisions are refreshed
to the current main heads observed on 2026-07-13. The candidate artifacts were
analysis-only and no consumer repository was edited or pushed.

This reconciliation does not establish route quality, repair usefulness, or a
support-tier claim. The next packet is a fresh two-candidate pilot per
authorized repository, with analysis-only eligibility checks before any repair
attempt can enter `cases`.
