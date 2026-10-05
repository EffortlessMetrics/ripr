<!-- section: Changed -->
- The DX scoreboard and Rust corpus nightlies gate against baselines recorded
  on hosted runners, keyed on the runner's CPU model so a night on different
  hardware leaves wall time and memory uncompared instead of failing. Both
  lanes keep the corpus outside the cached `target/`, and the scoreboard
  refuses a corpus directory that git resolves to an enclosing repository,
  which could otherwise detach the caller's working tree (#6726).
