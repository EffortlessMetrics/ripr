<!-- section: Added -->
- `.github/workflows/public-proof.yml` runs `cargo xtask public-proof --check`
  nightly, on demand and when a scoreboard, corpus, receipt or the page changes.
  It is advisory, so a corpus or scoreboard PR does not fail required CI. When a
  receipt has drifted from its source, `--check` now lists the page lines a
  refresh would change, so whoever bumps a scoreboard sees which published
  numbers move before committing. (#5494)

<!-- section: Fixed -->
- The public proof page names a repository whose speed or memory instrument
  failed in its shortfall lines instead of dropping it behind the worst
  repository that did measure. (#6730)
