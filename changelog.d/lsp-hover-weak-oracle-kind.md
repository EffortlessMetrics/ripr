<!-- section: Fixed -->
- LSP hover: a related test that matched an oracle but still misses the change
  now keeps its oracle strength and kind (`weak relational_check oracle: ...;
  misses: ...`), as full CLI output does. Only a test with no matched oracle
  uses the `misses: ...; checked ...` form, and a test with no recorded oracle
  (for example one with no assertion) shows only the reason
  ([#5927](https://github.com/EffortlessMetrics/ripr-swarm/issues/5927)).
