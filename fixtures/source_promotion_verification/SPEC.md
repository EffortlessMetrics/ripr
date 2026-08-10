# Source-promotion verification adversarial fixture contract

The synthetic Git tests exercise a valid direct two-parent J and reject:

- equivalent-tree squash and appended ordinary or merge repair commits;
- reversed or substituted parents and rebased/cherry-picked ranges;
- ancestry count or ordered-digest drift;
- reviewed-tree mismatch and automatic preview-tree substitution;
- stale/tampered preflight bytes;
- incomplete, duplicate, extra, or out-of-inventory resolution rows;
- governed metadata changes and merged-main equivalents that do not reach J;
- floating, abbreviated, or uppercase identity arguments.

Fixtures use disposable local repositories and verify that the caller's refs,
index, worktree, and remote state remain unchanged. They are proof of graph and
tree identity only, not semantic conflict correctness, product correctness,
release readiness, publication, or K back-sync behavior.
