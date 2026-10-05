<!-- section: Fixed -->
- A malformed diff is now reported as `malformed_diff` when a supplied partial
  Perl fact packet also produces findings. The packet's partial limitation
  describes only that supplied evidence, so it no longer suppresses the
  malformed-diff limitation
  ([#6703](https://github.com/EffortlessMetrics/ripr-swarm/issues/6703)).
