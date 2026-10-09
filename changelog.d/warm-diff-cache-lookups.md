<!-- section: Changed -->
- Warm `ripr check` runs on large workspaces are faster. File-fact cache
  lookups now run on all cores, and a diff run with a changed file no longer
  decodes every cache entry ripr has stored to label the miss. Rerunning after
  a one-line edit drops from 1.61 s to 0.74 s on rust-analyzer and from
  2.19 s to 1.70 s on bevy. The module-graph walk compares plain paths by
  bytes, and the dependent-package scans run on all cores, so bevy's pinned
  upstream diff reruns in about 1.85 s instead of 2.4 s.
