# Gap Decision Ledger Corpus

This manifest-only corpus pins the planned `RIPR-SPEC-0046` gap-decision
vocabulary before a production ledger exists.

The corpus is not analyzer truth and does not generate findings. It defines the
reviewable contract for the future gap-decision layer:

- evidence classes are converted into gap decisions before projection;
- repairable gaps carry stable anchors, repair routes, and verification
  commands;
- preview-language and static-limit evidence stays visibly advisory;
- missing artifacts carry regeneration commands;
- gate candidates satisfy the safe gate predicate;
- output and presentation text gaps route to output/golden repair.

This directory is exempt from ordinary fixture `input` / `expected` layout by
`cargo xtask check-fixture-contracts` because it is a manifest-only corpus.
