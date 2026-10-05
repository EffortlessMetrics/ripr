<!-- section: Fixed -->
- Rust match arms: when a related test calls or otherwise reaches the changed
  function, a test related only by sharing its file or module, and calling
  nothing that could reach the function, no longer confirms a match arm by
  naming the arm's enum variant. A same-file
  `assert!(matches!(Unit::from_str(..), Ok(Unit::Fortnight)))` used to mark the
  `Unit::Fortnight =>` arm of an unrelated `seconds` function `exposed`, so
  rewriting that assertion moved the arm to `weakly_exposed` although the test
  never runs `seconds`. Same-file tests still credit strength, and still
  confirm when no related test calls or otherwise reaches the function. The
  discriminator summary now says such a test cannot confirm the arm, instead
  of claiming no assertion names it
  ([#6297](https://github.com/EffortlessMetrics/ripr-swarm/issues/6297)).
