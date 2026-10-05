# RIPR-SPEC-0222: Byte-bounded classified-cache store publication

Status: proposed

Owner: product-analysis

Created: 2026-10-03

Linked issues: #4999 (write-side store amplification; sibling load bound is #5124).
Renumbered from RIPR-SPEC-0218 after #5370 landed that id, and from
RIPR-SPEC-0220 after #5366 landed the vscode shared-attempt status under
that id. Open #5416 occupies RIPR-SPEC-0221.

Support-tier impact:

- None. This is an internal cache-publication bound. It does not change
  analysis classes, public JSON findings, or support claims.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact: none. No new process, network, or file-policy surface. The new
env var is a store-path ceiling, not a process-RSS or load bound.

## Problem

Classified-seam cache publication still selected the single-entry versus
sharded path from record count alone. Both paths then deep-cloned the
chosen `ClassifiedSeam` slice and materialized the complete pretty-encoded
body as a `Vec<u8>` before the atomic write. A record count that is safe
for small records can still produce a multi-hundred-megabyte transient
shard. Historical host evidence around #4291 recorded an approximately
5.2 GB classified cache and an OOM during `cache_store` at roughly
11.7 GB anonymous RSS; that motivates a bounded store path without
proving a universal RSS threshold.

## Behavior

- Ordinary classified-cache publication serializes borrowed
  `ClassifiedSeam` records. It does not build `chunk.to_vec()`,
  `seams.to_vec()`, or an equivalent deep-cloned shard payload.
- A cache write that succeeds also removes `.ripr-atomic-<pid>-<nanos>-<seq>.tmp`
  files in its directory that a terminated run stranded, once per directory
  per process: regular files only, matching that exact name, untouched for
  ten minutes. A younger file may belong to a live writer; a writer stalled
  longer than that can lose its temporary file if a sweep reaches the
  directory while it is stale, and its rename then fails and the cache
  write is skipped.
- Encoding writes through a bounded IO buffer into the existing atomic
  temporary-file protocol. The store path does not retain the complete
  encoded entry or shard as a `Vec<u8>`.
- The primary shard/single-entry bound is encoded bytes:
  `RIPR_CLASSIFIED_SEAM_CACHE_SHARD_BYTES` (default 8 MiB,
  `CLASSIFIED_SEAM_CACHE_ENCODED_SHARD_CEILING_BYTES`). Record count
  (`RIPR_REPO_SEAM_CACHE_LIMIT` / `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS`)
  remains a secondary cap, not the sole selector.
- A payload may still be written as one file when it fits both the byte
  ceiling and the record cap. Otherwise it is published as byte-bounded
  shards. Manifest admission remains the generation gate: replacement
  shards use a new generation path so a failed publication leaves the
  previous valid generation or a miss, never a mixed authoritative
  manifest. An unpublished generation directory is removed on those
  failure paths. A prior single entry is parked off the loader's
  preferred path until the new manifest is admitted, then removed; a
  pre-commit failure restores it only when no newer single entry occupies
  the preferred path and the sharded manifest is unchanged since park
  (a leftover manifest from an earlier single-entry publish does not
  block restore; a replaced or unreadable manifest does).
- If one classified seam cannot fit under the configured byte ceiling,
  the store returns `skipped_oversized_record_index_{i}_ceiling_{n}` and
  does not claim a populated cache. Analysis output stays usable.
- Semantic digests, schema/analyzer identity, shard order, checksums,
  corruption handling, and warm-load reconstruction stay the current
  contracts. This spec does not bound cache *load* auxiliary memory.

## Non-Goals

- No redesign of cache load, lazy paging, or the final
  `Vec<ClassifiedSeam>` consumer API (#5124).
- No change to classification semantics or truncation of valid analysis
  output.
- No mmap requirement, process-RSS guarantee, or release operation.
- No weakening of checksum, digest, schema, or atomic-publication
  integrity.

## Required Evidence

- Owned versus borrowed streaming encodes are byte-identical for single
  entries and shards. Borrowed store envelopes serialize
  `classified_seams` through the same `related_test_table` adapter owned
  envelopes use.
- Variable-size records with a record cap that would keep one entry still
  split when the encoded-byte ceiling requires it.
- Below-limit, exactly-at-limit, and one-byte-over single-record cases.
- Empty cache, many small records, and multi-shard round-trips preserve
  semantic order.
- Zero `ClassifiedSeam` clones and zero complete encoded `Vec<u8>` on the
  ordinary store path; IO high-water stays within the bounded buffer
  while the published file exceeds that buffer.
- One oversized record skips without publishing a generation.
- Failed replacement and cancellation leave the previous generation and
  do not retain an extra unpublished `g*` directory.
- A failed sharded replacement of a single entry restores that entry;
  a successful replacement leaves no preferred single path.
- Rollback restore does not replace a newer single entry or hide a
  newer sharded manifest published after parking. An unchanged leftover
  sharded manifest from an earlier single-entry publish does not block
  restore.
- Size probes stop once encoded bytes exceed the ceiling; a large record
  cap still splits on the byte bound without serializing a full
  record-limit window.
- Shard relative paths reject parent, separator, and drive-prefixed
  components.
- Injected fill failure, mid-generation failure, and cancellation leave
  the previous valid generation.
- Existing integrity, missing-shard, and warm-hit controls keep passing.

## Acceptance Examples

- Two tiny records plus one medium record with record cap 8 and a byte
  ceiling equal to one medium shard publish multiple shards, not one
  count-sized entry.
- A single record whose encoded size equals the ceiling publishes as
  `ok`; one byte less skips without replacing a prior valid entry.
- Replacing a valid sharded generation and failing before the new
  manifest keeps the prior warm hit.

## Test Mapping

- `crates/ripr/src/analysis/seam_cache/store.rs::tests::borrowed_single_entry_matches_owned_codec_bytes`
- `crates/ripr/src/atomic_file.rs::tests::stranded_temp_names_match_only_what_this_module_creates`
- `crates/ripr/src/atomic_file.rs::tests::sweep_removes_only_old_matching_regular_files`
- `crates/ripr/src/atomic_file.rs::tests::sweep_does_not_follow_or_remove_a_planted_symlink`
- `crates/ripr/src/atomic_file.rs::tests::write_cache_sweeps_an_aged_stranded_temp_file_once_per_directory`
- `crates/ripr/src/atomic_file.rs::tests::write_cache_streamed_sweeps_an_aged_stranded_temp_file`
- `crates/ripr/tests/stale_temp_sweep.rs::next_run_removes_an_old_stranded_temp_file_and_keeps_a_young_one`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::borrowed_shard_matches_owned_codec_bytes`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::borrowed_writers_match_owned_bytes_with_shared_related_tests`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::below_exactly_and_one_byte_over_the_encoded_ceiling`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::variable_size_records_force_byte_shards_when_record_count_would_not`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::many_small_records_empty_and_multi_shard_round_trip`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::store_high_water_is_independent_of_total_shard_payload`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::oversized_record_skips_without_claiming_a_populated_cache`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::failed_replacement_keeps_the_previous_valid_generation`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::injected_fill_failure_does_not_admit_a_partial_generation`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::failed_sharded_replace_restores_the_previous_single_entry`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::parked_restore_does_not_replace_a_newer_single_entry`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::parked_restore_does_not_hide_a_newer_sharded_manifest`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::parked_restore_keeps_a_stale_leftover_manifest_from_blocking_restore`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::cancellation_before_manifest_preserves_the_prior_generation`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::encoded_byte_ceiling_defaults_and_rejects_invalid_env`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::publication_ids_stay_unique_across_concurrent_calls`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::size_probe_stops_once_the_encoded_ceiling_is_exceeded`
- `crates/ripr/src/analysis/seam_cache/store.rs::tests::sharded_cache_paths_reject_drive_prefix_and_parent_components`
- Existing `crates/ripr/src/analysis/seam_cache.rs` integrity, missing-shard,
  and sharded warm-hit tests

## Implementation Mapping

- `crates/ripr/src/analysis/seam_cache/store.rs` — publication planner,
  borrowed envelopes, bounded writer, generation-atomic shard names
- `crates/ripr/src/analysis/seam_cache.rs` — store entry, encoded-byte
  env, streaming checksummed encode helper, safe shard-path join
- `crates/ripr/src/atomic_file.rs` — `write_cache_streamed`
- `crates/ripr/src/analysis/seam_classification.rs` — test-only clone
  counter on `ClassifiedSeam`

## Metrics

- Ordinary encoded shard/single-entry bytes `<=` configured ceiling
- Auxiliary live store memory `<=` bounded IO buffer + serializer scratch
  + bounded metadata
- Host-scoped 10k/self-dogfood store-phase RSS remains
  `not_established` in this change; #3794 retains that observation
