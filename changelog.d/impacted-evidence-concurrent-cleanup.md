<!-- section: Fixed -->
- `ripr impacted-evidence`: a run that refuses bad `--pr-evidence` no longer
  deletes `latest.{json,md}` that a concurrent run wrote after it started. It
  removes only outputs that already existed, unchanged, when it began. If
  either output is newer, it leaves both in place and says so. An output whose
  metadata it cannot read is reported as a cleanup failure instead of being
  skipped as absent. This narrows the race but does not close it: the check
  and the removal are not atomic, and a same-size rewrite within one
  modification-time tick still reads as unchanged
  ([#5307](https://github.com/EffortlessMetrics/ripr-swarm/issues/5307)).
