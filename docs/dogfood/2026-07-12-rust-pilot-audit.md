# Rust pilot audit — 2026-07-12

This is the first real-repository pilot audit for issue #1560. It uses
isolated detached worktrees and does not modify or push the adopting
repositories.

## Authorized heads

| Repository | Authorized current head |
| --- | --- |
| `EffortlessMetrics/ripr-swarm` | `a53d815416785c37e0d0e067c3b10d3bbfd04a8d` |
| `EffortlessMetrics/perl-lsp-swarm` | `724bfe819d4e0ba6a5a31a0d5499cc9855f2c231` |
| `EffortlessMetrics/ub-review` | `9838259a704a5cf3748eb81af29536b99bf7cf3b` |

## Observed evidence

The bounded repo-wide `ripr pilot --max-seams 20 --timeout-ms 120000` run
timed out in all three adopting repositories and wrote partial pilot
artifacts. Diff-scoped `review-comments` runs were then executed against real
merged Rust PR revisions:

- RIPR #1555: predicate-flow changes remained static limitations.
- perl-lsp-swarm #3939 and #3908: parser changes remained activation/call
  presence or return-value limitations.
- ub-review #751, #749, #750, #742, and #747: producer output remained
  CallPresence or predicate static limitations.
- ub-review #757: actionable-looking field seams did not align to the changed
  scheduler behavior and were excluded as false actionability.
- ub-review #740: the suggested test path was a production source file;
  targeted rerun rejected it because the direct production owner was ambiguous.

No row from this audit is promoted into the eligible repair-attempt
denominator. The evidence is retained in the corpus `exclusions` array with
exact revisions, commands, artifacts, and claim boundaries.

## Current result

The governed report remains `limited`: 3 authorized repositories, 0 eligible
repair attempts, and explicit exclusion counts. This audit does not establish
route quality or support promotion. The next pilot wave must find a complete
producer-owned route with a test-only edit surface, before/after receipts, and
selected-scope parity before it can count an attempt.
