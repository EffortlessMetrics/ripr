<!-- section: Fixed -->
- A writer that commits a sharded classified-cache generation now removes an
  older single entry left by a competing writer's late rollback, so it cannot
  hide the newer generation. A single entry published later is kept.
- Generation directories left by a writer that was terminated mid-store are
  removed once they are an hour old and no manifest references them. The
  previous manifest is integrity-checked before any of its files are deleted.
- When one classified record (or the cache metadata) exceeds
  `RIPR_CLASSIFIED_SEAM_CACHE_SHARD_BYTES`, ripr now prints one stderr line
  naming the record, its encoded size and the value that restores warm runs.
  Before, every run recomputed with no visible reason.
