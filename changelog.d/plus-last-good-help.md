<!-- section: Fixed -->
- `ripr plus`: help now lists the `ripr-plus.last-good.json` and
  `ripr-plus.last-good.md` files a run may keep when its artifact cannot be
  read or composed (a `--check` failure keeps none), and the failure message
  names only the files it actually kept. Help and the message both
  say a kept receipt describes an earlier run, may be stale for the current
  HEAD, and is not current evidence
  ([#5595](https://github.com/EffortlessMetrics/ripr-swarm/issues/5595)).
