<!-- section: Fixed -->
- TypeScript preview: a Node test registered with an active `expectFailure`
  option no longer counts as ordinary test evidence, and a locally shadowed
  `undefined` no longer passes for the global value in a test's
  skip/todo/fails/expectFailure option. Such a test could otherwise lend its
  assertions to a seam it is expected to fail on
  ([#5436](https://github.com/EffortlessMetrics/ripr-swarm/pull/5436)).
