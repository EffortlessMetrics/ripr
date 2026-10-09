<!-- section: Changed -->
- File-level call facts are derived from per-function calls instead of
  stored as a second copy. Peak `check` RSS falls about 70 MB on a
  mid-sized workspace (717 MB to 647 MB on the #5415 repro, warm cache)
  with byte-identical output, and the on-disk file-fact cache shrinks
  about 60 MB. The cache moves to schema 1.22, so existing 1.21 entries
  miss by version and are rebuilt; if legacy-shaped bytes carrying the
  removed copy ever reach the 1.22 decode, derivation (not the stored
  copy) wins (#5415).
