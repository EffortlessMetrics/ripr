<!-- section: Changed -->
- A cache miss no longer decodes every stored file-fact entry to name which
  files changed. ripr reads each entry's header and fully decodes and
  integrity-checks only the files that missed. On ripr-swarm (about 4,500
  cached entries) one edit took the cached-parse phase of `ripr pilot` from
  5.7 s to 2.5 s; output is unchanged. The saving grows with the cache.
