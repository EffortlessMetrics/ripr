//! Borrowed, byte-bounded classified-cache publication (#4999).
//!
//! Ordinary store memory is serializer scratch plus a bounded IO buffer.
//! Shard membership is chosen from encoded bytes first; record count is only a
//! secondary cap. Load/decode bounds remain the read-side sibling.

use super::{
    CACHE_ENVELOPE_DIGEST_DOMAIN, CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES, CacheStoreStatus,
    CachedSeamLimitInfo, RepoSeamCacheKey, RepoSeamFactCache,
    SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION, SHARDED_ENVELOPE_DIGEST_DOMAIN,
    ShardedCacheManifest, ShardedCacheShardRef, codec, encode_checksummed_pretty_to_writer,
    placeholder_payload_digest, resolve_sharded_cache_file,
};
use crate::analysis::seam_classification::ClassifiedSeam;
#[cfg(test)]
use crate::analysis::seam_classification::reset_classified_seam_clone_count;
use serde::Serialize;
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[cfg(test)]
std::thread_local! {
    static STORE_IO_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PLAN_ENCODE_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FAIL_AFTER_SHARDS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static FAIL_NEXT_FILL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_AFTER_PARK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AFTER_COMMIT_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `hook` once right after the next sharded manifest commit, standing in for a
/// competing process that restores an older single entry in that window.
#[cfg(test)]
pub(super) fn after_next_manifest_commit(hook: Box<dyn FnOnce()>) {
    AFTER_COMMIT_HOOK.with(|slot| *slot.borrow_mut() = Some(hook));
}

#[cfg(test)]
fn run_after_commit_hook() {
    if let Some(hook) = AFTER_COMMIT_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

#[cfg(test)]
pub(super) fn store_io_high_water() -> usize {
    STORE_IO_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn plan_encode_high_water() -> usize {
    PLAN_ENCODE_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn fail_after_writing_shards(count: usize) {
    FAIL_AFTER_SHARDS.with(|flag| flag.set(Some(count)));
}

#[cfg(test)]
fn fail_after_shards_value() -> Option<usize> {
    FAIL_AFTER_SHARDS.with(std::cell::Cell::get)
}

#[cfg(test)]
fn clear_fail_after_shards() {
    FAIL_AFTER_SHARDS.with(|flag| flag.set(None));
}

#[cfg(test)]
pub(super) fn fail_next_streamed_fill() {
    FAIL_NEXT_FILL.with(|flag| flag.set(true));
}

#[cfg(test)]
fn take_fail_next_fill() -> bool {
    FAIL_NEXT_FILL.with(std::cell::Cell::take)
}

#[cfg(test)]
pub(super) fn fail_after_parking_single_entry() {
    FAIL_AFTER_PARK.with(|flag| flag.set(true));
}

#[cfg(test)]
fn take_fail_after_park() -> bool {
    FAIL_AFTER_PARK.with(std::cell::Cell::take)
}

pub(super) fn publish_classified_generation(
    cache: &RepoSeamFactCache,
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    limit_info: Option<&CachedSeamLimitInfo>,
    lexical_fallback_files: &[PathBuf],
    record_limit: usize,
    byte_ceiling: usize,
) -> Result<CacheStoreStatus, String> {
    #[cfg(test)]
    {
        reset_classified_seam_clone_count();
        STORE_IO_HIGH_WATER.with(|water| water.set(0));
        PLAN_ENCODE_HIGH_WATER.with(|water| water.set(0));
    }
    if record_limit == 0 {
        return Err("classified seam cache store limit must be positive".to_string());
    }
    if byte_ceiling == 0 {
        return Err("classified seam cache encoded shard ceiling must be positive".to_string());
    }
    crate::analysis::cancellation::checkpoint()?;
    match plan_publication(
        key,
        seams,
        limit_info,
        lexical_fallback_files,
        record_limit,
        byte_ceiling,
    )? {
        PublicationPlan::SkipOversized {
            index,
            encoded_bytes,
        } => Ok(oversized_skip_status(index, encoded_bytes, byte_ceiling)),
        PublicationPlan::Single => {
            publish_single_entry(cache, key, seams, limit_info, lexical_fallback_files)
        }
        PublicationPlan::Sharded(ranges) => publish_sharded_generation(
            cache,
            key,
            seams,
            limit_info,
            lexical_fallback_files,
            record_limit,
            ranges,
        ),
    }
}

enum PublicationPlan {
    Single,
    Sharded(Vec<Range<usize>>),
    SkipOversized {
        /// Record that cannot fit; `None` when the metadata around the records overflows.
        index: Option<usize>,
        encoded_bytes: usize,
    },
}

fn plan_publication(
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    limit_info: Option<&CachedSeamLimitInfo>,
    lexical_fallback_files: &[PathBuf],
    record_limit: usize,
    byte_ceiling: usize,
) -> Result<PublicationPlan, String> {
    if seams.len() <= record_limit {
        let single = borrowed_cache_envelope(key, seams, limit_info, lexical_fallback_files);
        if checksummed_pretty_fits(&single, byte_ceiling)? {
            return Ok(PublicationPlan::Single);
        }
        if seams.is_empty() {
            return Ok(PublicationPlan::SkipOversized {
                index: None,
                encoded_bytes: checksummed_pretty_len(&single)?,
            });
        }
    }
    let ranges = match plan_shard_ranges(key, seams, record_limit, byte_ceiling)? {
        ShardPlan::Ranges(ranges) => ranges,
        ShardPlan::Oversized { index } => {
            let encoded_bytes = single_record_shard_len(key, seams, index)?;
            return Ok(PublicationPlan::SkipOversized {
                index: Some(index),
                encoded_bytes,
            });
        }
    };
    Ok(PublicationPlan::Sharded(ranges))
}

fn single_record_shard_len(
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    index: usize,
) -> Result<usize, String> {
    let Some(slice) = seams.get(index..index.saturating_add(1)) else {
        return Err("oversized classified cache record index is out of bounds".to_string());
    };
    checksummed_pretty_len(&borrowed_shard_envelope(key, index, seams.len(), slice))
}

fn oversized_skip_status(
    index: Option<usize>,
    encoded_bytes: usize,
    byte_ceiling: usize,
) -> CacheStoreStatus {
    let (label, subject) = match index {
        Some(index) => (
            format!("skipped_oversized_record_index_{index}_ceiling_{byte_ceiling}"),
            format!("seam record {index}"),
        ),
        None => (
            format!("skipped_oversized_metadata_ceiling_{byte_ceiling}"),
            "the cache metadata".to_string(),
        ),
    };
    CacheStoreStatus {
        label,
        advisory: Some(format!(
            "classified seam cache not stored: {subject} encodes to {encoded_bytes} bytes, above the {byte_ceiling}-byte ceiling, so every run recomputes; set RIPR_CLASSIFIED_SEAM_CACHE_SHARD_BYTES to at least {encoded_bytes} to restore warm runs"
        )),
    }
}

enum ShardPlan {
    Ranges(Vec<Range<usize>>),
    Oversized { index: usize },
}

fn plan_shard_ranges(
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    record_limit: usize,
    byte_ceiling: usize,
) -> Result<ShardPlan, String> {
    if seams.is_empty() {
        return Ok(ShardPlan::Ranges(Vec::new()));
    }
    let conservative_shard_count = seams.len();
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < seams.len() {
        let max_end = start.saturating_add(record_limit).min(seams.len());
        match largest_fitting_end(
            key,
            seams,
            start,
            max_end,
            ranges.len(),
            conservative_shard_count,
            byte_ceiling,
        )? {
            Some(end) => ranges.push(start..end),
            None => return Ok(ShardPlan::Oversized { index: start }),
        }
        start = ranges
            .last()
            .map(|range| range.end)
            .ok_or_else(|| "classified cache shard planner lost the current range".to_string())?;
    }
    Ok(ShardPlan::Ranges(ranges))
}

fn largest_fitting_end(
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    start: usize,
    max_end: usize,
    shard_index: usize,
    conservative_shard_count: usize,
    byte_ceiling: usize,
) -> Result<Option<usize>, String> {
    if start >= max_end {
        return Ok(None);
    }
    if !prefix_fits(
        key,
        seams,
        start,
        start.saturating_add(1),
        shard_index,
        conservative_shard_count,
        byte_ceiling,
    )? {
        return Ok(None);
    }
    let mut best = start.saturating_add(1);
    let mut step = 1usize;
    while best < max_end {
        let probe = best.saturating_add(step).min(max_end);
        if prefix_fits(
            key,
            seams,
            start,
            probe,
            shard_index,
            conservative_shard_count,
            byte_ceiling,
        )? {
            best = probe;
            if best == max_end {
                return Ok(Some(best));
            }
            step = step.saturating_mul(2).max(1);
            continue;
        }
        let mut low = best.saturating_add(1);
        let mut high = probe.saturating_sub(1);
        while low <= high {
            let mid = low.saturating_add(high.saturating_sub(low) / 2);
            if prefix_fits(
                key,
                seams,
                start,
                mid,
                shard_index,
                conservative_shard_count,
                byte_ceiling,
            )? {
                best = mid;
                low = mid.saturating_add(1);
            } else {
                high = mid.saturating_sub(1);
            }
        }
        return Ok(Some(best));
    }
    Ok(Some(best))
}

fn prefix_fits(
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    start: usize,
    end: usize,
    shard_index: usize,
    conservative_shard_count: usize,
    byte_ceiling: usize,
) -> Result<bool, String> {
    let Some(slice) = seams.get(start..end) else {
        return Ok(false);
    };
    let envelope = borrowed_shard_envelope(key, shard_index, conservative_shard_count, slice);
    checksummed_pretty_fits(&envelope, byte_ceiling)
}

fn publish_single_entry(
    cache: &RepoSeamFactCache,
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    limit_info: Option<&CachedSeamLimitInfo>,
    lexical_fallback_files: &[PathBuf],
) -> Result<CacheStoreStatus, String> {
    std::fs::create_dir_all(&cache.dir).map_err(|err| format!("create cache dir failed: {err}"))?;
    let envelope = borrowed_cache_envelope(key, seams, limit_info, lexical_fallback_files);
    let digest = super::semantic_body_digest(CACHE_ENVELOPE_DIGEST_DOMAIN, &envelope)?;
    stream_checksummed_cache_file(cache.entry_path(key), "cache", &envelope, digest)?;
    Ok(CacheStoreStatus {
        label: "ok".to_string(),
        advisory: None,
    })
}

fn publish_sharded_generation(
    cache: &RepoSeamFactCache,
    key: &RepoSeamCacheKey,
    seams: &[ClassifiedSeam],
    limit_info: Option<&CachedSeamLimitInfo>,
    lexical_fallback_files: &[PathBuf],
    record_limit: usize,
    ranges: Vec<Range<usize>>,
) -> Result<CacheStoreStatus, String> {
    if ranges.is_empty() {
        return Err("sharded classified cache publication requires at least one shard".to_string());
    }
    let shard_count = ranges.len();
    let started_at = std::time::SystemTime::now();
    let sharded_dir = cache.sharded_entry_dir(key);
    std::fs::create_dir_all(&sharded_dir)
        .map_err(|err| format!("create sharded cache dir failed: {err}"))?;
    let previous = read_previous_manifest(cache, key);
    let publication_id = publication_id();
    let generation_dir = sharded_dir.join(format!("g{publication_id}"));
    std::fs::create_dir_all(&generation_dir)
        .map_err(|err| format!("create sharded cache generation dir failed: {err}"))?;
    let mut unpublished = UnpublishedGeneration::new(generation_dir);
    let mut shard_refs = Vec::with_capacity(shard_count);
    for (index, range) in ranges.iter().enumerate() {
        crate::analysis::cancellation::checkpoint()?;
        #[cfg(test)]
        if fail_after_shards_value() == Some(index) {
            clear_fail_after_shards();
            return Err(
                "injected sharded cache publication failure before remaining shards".to_string(),
            );
        }
        let Some(chunk) = seams.get(range.start..range.end) else {
            return Err("classified cache shard range is out of bounds".to_string());
        };
        let file = format!("g{publication_id}/shard-{index:05}.json");
        let path = resolve_sharded_cache_file(&sharded_dir, &file)?;
        let envelope = borrowed_shard_envelope(key, index, shard_count, chunk);
        let digest = super::semantic_body_digest(SHARDED_ENVELOPE_DIGEST_DOMAIN, &envelope)?;
        stream_checksummed_cache_file(path, "sharded cache file", &envelope, digest)?;
        shard_refs.push(ShardedCacheShardRef {
            index,
            file,
            seams: chunk.len(),
        });
    }
    crate::analysis::cancellation::checkpoint()?;
    #[cfg(test)]
    if fail_after_shards_value() == Some(shard_count) {
        clear_fail_after_shards();
        return Err("injected sharded cache publication failure before manifest".to_string());
    }
    let mut parked = ParkedSingleEntry::park(
        cache.entry_path(key),
        cache.sharded_manifest_path(key),
        &publication_id,
    )?;
    #[cfg(test)]
    if take_fail_after_park() {
        return Err(
            "injected sharded cache publication failure after parking the single entry".to_string(),
        );
    }
    crate::analysis::cancellation::checkpoint()?;
    let manifest = ShardedCacheManifest::new(
        key.clone(),
        seams.len(),
        shard_count,
        shard_refs,
        limit_info.cloned(),
        lexical_fallback_files.to_vec(),
    );
    let digest = manifest.expected_digest()?;
    let manifest_path = cache.sharded_manifest_path(key);
    stream_checksummed_cache_file(manifest_path, "sharded cache manifest", &manifest, digest)?;
    if let Some(parked) = parked.as_mut() {
        parked.commit();
    }
    unpublished.retain();
    #[cfg(test)]
    run_after_commit_hook();
    remove_single_entry_older_than(&cache.entry_path(key), started_at);
    if let Some(previous) = previous {
        remove_replaced_generation_files(&sharded_dir, &previous, &publication_id);
    }
    sweep_orphan_generations(
        &sharded_dir,
        std::time::SystemTime::now(),
        ORPHAN_GENERATION_GRACE,
    );
    Ok(CacheStoreStatus {
        label: format!(
            "sharded_ok_seams_{}_shards_{}_limit_{}",
            seams.len(),
            shard_count,
            record_limit
        ),
        advisory: None,
    })
}

/// The loader prefers a single entry over a manifest. A competing writer that rolls
/// back after parking can restore an older single entry after this writer's manifest
/// commit (a re-check after its `hard_link` usually catches that, so this is defence
/// in depth). Remove only an entry last written before this publication began: a
/// single entry a later writer published is newer and stays.
fn remove_single_entry_older_than(entry: &Path, started_at: std::time::SystemTime) {
    let Ok(modified) = std::fs::symlink_metadata(entry).and_then(|meta| meta.modified()) else {
        return;
    };
    if modified < started_at {
        let _ = std::fs::remove_file(entry);
    }
}

fn read_previous_manifest(
    cache: &RepoSeamFactCache,
    key: &RepoSeamCacheKey,
) -> Option<ShardedCacheManifest> {
    let bytes = std::fs::read(cache.sharded_manifest_path(key)).ok()?;
    let manifest = codec::decode_sharded_manifest(&bytes).ok()?;
    // The digest is keyless, so it catches corruption, not a hostile writer; deletes
    // are bounded by only ever touching files under `g*/`.
    manifest.validate_integrity().ok()?;
    Some(manifest)
}

/// Generation directories a terminated or superseded writer left behind stay until they
/// are this old, so a concurrent writer's in-progress generation is not swept. A writer
/// stalled longer than this between creating its last shard and committing can lose its
/// generation; the loader checks every listed shard, so that is a cache miss, never wrong data.
const ORPHAN_GENERATION_GRACE: std::time::Duration = std::time::Duration::from_hours(1);

/// Remove `g*` generation directories under `sharded_dir` that the current manifest
/// does not reference and that have not been touched for `grace`. Any doubt (no
/// readable valid manifest, unreadable metadata, symlinks) keeps the directory.
fn sweep_orphan_generations(
    sharded_dir: &Path,
    now: std::time::SystemTime,
    grace: std::time::Duration,
) -> usize {
    let Ok(bytes) = std::fs::read(sharded_dir.join("manifest.json")) else {
        return 0;
    };
    let Ok(manifest) = codec::decode_sharded_manifest(&bytes) else {
        return 0;
    };
    if manifest.validate_integrity().is_err() {
        return 0;
    }
    let referenced: std::collections::BTreeSet<&str> = manifest
        .shards
        .iter()
        .filter_map(|shard| shard.file.split('/').next())
        .collect();
    let Ok(entries) = std::fs::read_dir(sharded_dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with('g') || referenced.contains(name) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_dir() {
            continue;
        }
        let old_enough = meta
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= grace);
        if old_enough && std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn remove_replaced_generation_files(
    sharded_dir: &Path,
    previous: &ShardedCacheManifest,
    retained_publication_id: &str,
) {
    let retained_prefix = format!("g{retained_publication_id}/");
    for shard in &previous.shards {
        // Only generation files are ours to delete; a listed manifest path is hostile.
        let in_generation_dir = shard
            .file
            .split_once('/')
            .is_some_and(|(dir, rest)| dir.starts_with('g') && !rest.is_empty());
        if shard.file.starts_with(&retained_prefix) || !in_generation_dir {
            continue;
        }
        if let Ok(path) = resolve_sharded_cache_file(sharded_dir, &shard.file) {
            let _ = std::fs::remove_file(&path);
            if let Some(parent) = path.parent().filter(|parent| *parent != sharded_dir) {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

struct UnpublishedGeneration {
    dir: PathBuf,
    retain: bool,
}

impl UnpublishedGeneration {
    fn new(dir: PathBuf) -> Self {
        Self { dir, retain: false }
    }

    fn retain(&mut self) {
        self.retain = true;
    }
}

impl Drop for UnpublishedGeneration {
    fn drop(&mut self) {
        if !self.retain {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

struct ParkedSingleEntry {
    original: PathBuf,
    parked: PathBuf,
    manifest: PathBuf,
    manifest_at_park: Option<Vec<u8>>,
    restore: bool,
}

impl ParkedSingleEntry {
    fn park(
        original: PathBuf,
        manifest: PathBuf,
        publication_id: &str,
    ) -> Result<Option<Self>, String> {
        if !original.exists() {
            return Ok(None);
        }
        let manifest_at_park = match std::fs::read(&manifest) {
            Ok(bytes) => Some(bytes),
            Err(err) if err.kind() == io::ErrorKind::NotFound => None,
            Err(err) => {
                return Err(format!(
                    "read sharded cache manifest before parking failed: {err}"
                ));
            }
        };
        let mut parked = original.clone().into_os_string();
        parked.push(format!(".parked-{publication_id}"));
        let parked = PathBuf::from(parked);
        std::fs::rename(&original, &parked)
            .map_err(|err| format!("park competing single classified cache entry failed: {err}"))?;
        Ok(Some(Self {
            original,
            parked,
            manifest,
            manifest_at_park,
            restore: true,
        }))
    }

    fn commit(&mut self) {
        self.restore = false;
        let _ = std::fs::remove_file(&self.parked);
    }

    fn manifest_unchanged_since_park(&self) -> bool {
        match std::fs::read(&self.manifest) {
            Ok(bytes) => self.manifest_at_park.as_deref() == Some(bytes.as_slice()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => self.manifest_at_park.is_none(),
            Err(_) => false,
        }
    }

    fn restore_if_uncontested(&self) {
        if !self.manifest_unchanged_since_park() {
            let _ = std::fs::remove_file(&self.parked);
            return;
        }
        match std::fs::hard_link(&self.parked, &self.original) {
            Ok(()) => {
                if !self.manifest_unchanged_since_park() {
                    let _ = std::fs::remove_file(&self.original);
                }
                let _ = std::fs::remove_file(&self.parked);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&self.parked);
            }
            Err(_) => {
                if self.original.exists() || !self.manifest_unchanged_since_park() {
                    let _ = std::fs::remove_file(&self.parked);
                    return;
                }
                let _ = std::fs::rename(&self.parked, &self.original);
                if !self.manifest_unchanged_since_park() {
                    let _ = std::fs::remove_file(&self.original);
                }
            }
        }
    }
}

impl Drop for ParkedSingleEntry {
    fn drop(&mut self) {
        if self.restore {
            self.restore_if_uncontested();
        }
    }
}

fn publication_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}-{seq}", std::process::id())
}

fn stream_checksummed_cache_file<T: Serialize>(
    path: PathBuf,
    label: &str,
    body: &T,
    payload_digest: String,
) -> Result<(), String> {
    crate::atomic_file::write_cache_streamed(&path, label, |file| {
        #[cfg(test)]
        if take_fail_next_fill() {
            return Err(io::Error::other("injected classified cache fill failure"));
        }
        super::note_integrity_envelope();
        let mut writer = BoundedBufWriter::new(file, CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES);
        encode_checksummed_pretty_to_writer(body, payload_digest.clone(), &mut writer)
            .map_err(io::Error::other)?;
        writer.flush()
    })
}

fn checksummed_pretty_len<T: Serialize>(body: &T) -> Result<usize, String> {
    let mut counter = DiscardingCounter::default();
    encode_checksummed_pretty_to_writer(body, placeholder_payload_digest(), &mut counter)?;
    Ok(counter.count)
}

fn checksummed_pretty_fits<T: Serialize>(body: &T, ceiling: usize) -> Result<bool, String> {
    let mut counter = CeilingCounter {
        count: 0,
        ceiling,
        exceeded: false,
    };
    match encode_checksummed_pretty_to_writer(body, placeholder_payload_digest(), &mut counter) {
        Ok(()) => Ok(true),
        Err(_) if counter.exceeded => Ok(false),
        Err(err) => Err(err),
    }
}

#[derive(Default)]
struct DiscardingCounter {
    count: usize,
}

impl Write for DiscardingCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.count = self.count.saturating_add(buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct CeilingCounter {
    count: usize,
    ceiling: usize,
    exceeded: bool,
}

impl Write for CeilingCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.count = self.count.saturating_add(buf.len());
        #[cfg(test)]
        PLAN_ENCODE_HIGH_WATER.with(|water| water.set(water.get().max(self.count)));
        if self.count > self.ceiling {
            self.exceeded = true;
            return Err(io::Error::other("encoded size exceeds ceiling"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BoundedBufWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,
    cap: usize,
    high_water: usize,
}

impl<W: Write> BoundedBufWriter<W> {
    fn new(inner: W, cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            inner,
            buf: Vec::with_capacity(cap),
            cap,
            high_water: 0,
        }
    }

    fn note_occupancy(&mut self) {
        self.high_water = self.high_water.max(self.buf.len());
        #[cfg(test)]
        STORE_IO_HIGH_WATER.with(|water| water.set(water.get().max(self.high_water)));
    }

    fn flush_buf(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.inner.write_all(&self.buf)?;
        self.buf.clear();
        Ok(())
    }
}

impl<W: Write> Write for BoundedBufWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut offset = 0;
        while offset < data.len() {
            if self.buf.len() >= self.cap {
                self.flush_buf()?;
            }
            let space = self.cap.saturating_sub(self.buf.len());
            if space == 0 {
                return Err(io::Error::other(
                    "classified cache IO buffer has no capacity",
                ));
            }
            let take = (data.len() - offset).min(space);
            self.buf.extend_from_slice(
                data.get(offset..offset.saturating_add(take))
                    .ok_or_else(|| {
                        io::Error::other("classified cache IO slice is out of bounds")
                    })?,
            );
            offset = offset.saturating_add(take);
            self.note_occupancy();
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

#[derive(Serialize)]
struct BorrowedCacheEnvelope<'a> {
    schema_version: &'a str,
    analyzer_version: &'a str,
    workspace_root_hash: &'a str,
    files_content_hash: &'a str,
    cfg_features_hash: &'a str,
    config_hash: &'a str,
    test_intent_hash: &'a str,
    suppressions_hash: &'a str,
    workspace_manifests_hash: &'a str,
    lockfile_hash: &'a str,
    toolchain_hash: &'a str,
    #[serde(serialize_with = "super::related_test_table::serialize")]
    classified_seams: &'a [ClassifiedSeam],
    seam_limit_info: Option<&'a CachedSeamLimitInfo>,
    lexical_fallback_files: &'a [PathBuf],
}

#[derive(Serialize)]
struct BorrowedShardedEnvelope<'a> {
    sharded_cache_schema_version: &'a str,
    schema_version: &'a str,
    analyzer_version: &'a str,
    workspace_root_hash: &'a str,
    files_content_hash: &'a str,
    cfg_features_hash: &'a str,
    config_hash: &'a str,
    test_intent_hash: &'a str,
    suppressions_hash: &'a str,
    workspace_manifests_hash: &'a str,
    lockfile_hash: &'a str,
    toolchain_hash: &'a str,
    shard_index: usize,
    shard_count: usize,
    #[serde(serialize_with = "super::related_test_table::serialize")]
    classified_seams: &'a [ClassifiedSeam],
}

fn borrowed_cache_envelope<'a>(
    key: &'a RepoSeamCacheKey,
    seams: &'a [ClassifiedSeam],
    limit_info: Option<&'a CachedSeamLimitInfo>,
    lexical_fallback_files: &'a [PathBuf],
) -> BorrowedCacheEnvelope<'a> {
    BorrowedCacheEnvelope {
        schema_version: &key.schema_version,
        analyzer_version: &key.analyzer_version,
        workspace_root_hash: &key.workspace_root_hash,
        files_content_hash: &key.files_content_hash,
        cfg_features_hash: &key.cfg_features_hash,
        config_hash: &key.config_hash,
        test_intent_hash: &key.test_intent_hash,
        suppressions_hash: &key.suppressions_hash,
        workspace_manifests_hash: &key.workspace_manifests_hash,
        lockfile_hash: &key.lockfile_hash,
        toolchain_hash: &key.toolchain_hash,
        classified_seams: seams,
        seam_limit_info: limit_info,
        lexical_fallback_files,
    }
}

fn borrowed_shard_envelope<'a>(
    key: &'a RepoSeamCacheKey,
    shard_index: usize,
    shard_count: usize,
    seams: &'a [ClassifiedSeam],
) -> BorrowedShardedEnvelope<'a> {
    BorrowedShardedEnvelope {
        sharded_cache_schema_version: SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION,
        schema_version: &key.schema_version,
        analyzer_version: &key.analyzer_version,
        workspace_root_hash: &key.workspace_root_hash,
        files_content_hash: &key.files_content_hash,
        cfg_features_hash: &key.cfg_features_hash,
        config_hash: &key.config_hash,
        test_intent_hash: &key.test_intent_hash,
        suppressions_hash: &key.suppressions_hash,
        workspace_manifests_hash: &key.workspace_manifests_hash,
        lockfile_hash: &key.lockfile_hash,
        toolchain_hash: &key.toolchain_hash,
        shard_index,
        shard_count,
        classified_seams: seams,
    }
}

impl RepoSeamFactCache {
    #[cfg(test)]
    pub(crate) fn store_classified_seams_with_record_and_byte_limits(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        limit_info: Option<&CachedSeamLimitInfo>,
        record_limit: usize,
        byte_ceiling: usize,
    ) -> Result<CacheStoreStatus, String> {
        publish_classified_generation(
            self,
            key,
            seams,
            limit_info,
            &[],
            record_limit,
            byte_ceiling,
        )
    }

    #[cfg(test)]
    pub(crate) fn sharded_shard_path(
        &self,
        key: &RepoSeamCacheKey,
        index: usize,
    ) -> Result<PathBuf, String> {
        let bytes = std::fs::read(self.sharded_manifest_path(key))
            .map_err(|err| format!("read sharded manifest for shard path: {err}"))?;
        let manifest = codec::decode_sharded_manifest(&bytes)?;
        let shard = manifest
            .shards
            .get(index)
            .ok_or_else(|| format!("sharded manifest has no shard {index}"))?;
        resolve_sharded_cache_file(&self.sharded_entry_dir(key), &shard.file)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        CLASSIFIED_SEAM_CACHE_ENCODED_SHARD_CEILING_BYTES,
        CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES, CacheEnvelope, CacheLoad,
        SHARDED_ENVELOPE_DIGEST_DOMAIN, WorkspaceState,
        classified_seam_cache_encoded_shard_ceiling_from_env, ignore_remove_dir_all,
        integrity_vec_high_water, reset_integrity_vec_high_water, resolve_sharded_cache_file,
        semantic_body_digest,
    };
    use super::*;
    use crate::analysis::seam_classification::{
        ClassifiedSeam, classified_seam_clone_count, reset_classified_seam_clone_count,
    };
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::TestGripEvidence;
    use crate::domain::{Confidence, StageEvidence, StageState};
    use std::path::{Path, PathBuf};

    fn empty_key() -> RepoSeamCacheKey {
        WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &[],
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key()
    }

    fn isolated_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ripr-store-{label}-{}-{nanos}", std::process::id()))
    }

    fn listed_generation_dirs(
        cache: &RepoSeamFactCache,
        key: &RepoSeamCacheKey,
    ) -> Result<Vec<String>, String> {
        let dir = cache.sharded_entry_dir(key);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            if !entry.file_type().map_err(|err| err.to_string())?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('g') {
                names.push(name.into_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    fn classified_with_pad(pad: &str) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            PathBuf::from("src/foo.rs"),
            "src/foo.rs::foo",
            SeamKind::PredicateBoundary,
            42,
            10,
            "x > 5".to_string(),
            RequiredDiscriminator::BoundaryValue {
                description: "x > 5".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: StageEvidence::new(StageState::Yes, Confidence::High, pad),
            activate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "activate"),
            propagate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "propagate"),
            observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observe"),
            discriminate: StageEvidence::new(StageState::No, Confidence::Low, "discriminate"),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
            new_test_target: None,
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::Ungripped,
        }
    }

    fn round_trip(
        cache: &RepoSeamFactCache,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
    ) -> Result<(), String> {
        match cache.load_classified_seams(key) {
            CacheLoad::Hit((loaded, _)) => {
                if loaded.len() != seams.len() {
                    return Err(format!(
                        "round-trip count {} != {}",
                        loaded.len(),
                        seams.len()
                    ));
                }
                for (actual, expected) in loaded.iter().zip(seams) {
                    let actual_bytes = serde_json::to_vec(actual).map_err(|err| err.to_string())?;
                    let expected_bytes =
                        serde_json::to_vec(expected).map_err(|err| err.to_string())?;
                    if actual_bytes != expected_bytes {
                        return Err("round-trip changed a classified seam".to_string());
                    }
                }
                Ok(())
            }
            other => Err(format!("expected classified cache hit, got {other:?}")),
        }
    }

    #[test]
    fn encoded_byte_ceiling_defaults_and_rejects_invalid_env() -> Result<(), String> {
        let default = classified_seam_cache_encoded_shard_ceiling_from_env(Err(
            std::env::VarError::NotPresent,
        ))?;
        assert_eq!(default, CLASSIFIED_SEAM_CACHE_ENCODED_SHARD_CEILING_BYTES);
        for value in ["", "0", "not-a-number"] {
            let err =
                match classified_seam_cache_encoded_shard_ceiling_from_env(Ok(value.to_string())) {
                    Ok(limit) => {
                        return Err(format!(
                            "invalid encoded ceiling {value:?} should fail, got {limit}"
                        ));
                    }
                    Err(err) => err,
                };
            assert!(
                err.contains("RIPR_CLASSIFIED_SEAM_CACHE_SHARD_BYTES"),
                "diagnostic should name env var for {value:?}: {err}"
            );
        }
        Ok(())
    }

    #[test]
    fn borrowed_single_entry_matches_owned_codec_bytes() -> Result<(), String> {
        let key = empty_key();
        let seams = vec![classified_with_pad("tiny")];
        let owned = CacheEnvelope::new_with_fallback(key.clone(), seams.clone(), None, Vec::new());
        let owned_bytes = codec::encode(&owned)?;
        let borrowed = borrowed_cache_envelope(&key, &seams, None, &[]);
        let digest = semantic_body_digest(CACHE_ENVELOPE_DIGEST_DOMAIN, &borrowed)?;
        let mut streamed = Vec::new();
        encode_checksummed_pretty_to_writer(&borrowed, digest, &mut streamed)?;
        assert_eq!(
            streamed, owned_bytes,
            "borrowed streaming encode must stay byte-identical to the owned codec"
        );
        Ok(())
    }

    #[test]
    fn borrowed_shard_matches_owned_codec_bytes() -> Result<(), String> {
        let key = empty_key();
        let seams = vec![classified_with_pad("shard-ident")];
        let owned = super::super::ShardedCacheEnvelope::new(key.clone(), 0, 2, seams.clone());
        let owned_bytes = codec::encode_shard(&owned)?;
        let borrowed = borrowed_shard_envelope(&key, 0, 2, &seams);
        let digest = semantic_body_digest(SHARDED_ENVELOPE_DIGEST_DOMAIN, &borrowed)?;
        let mut streamed = Vec::new();
        encode_checksummed_pretty_to_writer(&borrowed, digest, &mut streamed)?;
        assert_eq!(
            streamed, owned_bytes,
            "borrowed shard streaming encode must stay byte-identical to the owned codec"
        );
        Ok(())
    }

    /// The owned envelopes write `classified_seams` through the related-test
    /// table (#5291). The borrowed writers must use the same serializer, or a
    /// stored entry would not decode. Shared related tests make the table
    /// representation differ from plain serialization.
    #[test]
    fn borrowed_writers_match_owned_bytes_with_shared_related_tests() -> Result<(), String> {
        use super::super::related_test_table::tests::{related, seam};
        use crate::domain::RelationReason;
        let a = related("a", RelationReason::SameModule);
        let b = related("b", RelationReason::SameModule);
        let seams = vec![
            seam(1, vec![a.clone(), b.clone()]),
            seam(2, vec![b, a.clone()]),
            seam(3, vec![a]),
        ];
        let key = empty_key();

        let owned = CacheEnvelope::new_with_fallback(key.clone(), seams.clone(), None, Vec::new());
        let owned_bytes = codec::encode(&owned)?;
        let borrowed = borrowed_cache_envelope(&key, &seams, None, &[]);
        let digest = semantic_body_digest(CACHE_ENVELOPE_DIGEST_DOMAIN, &borrowed)?;
        let mut streamed = Vec::new();
        encode_checksummed_pretty_to_writer(&borrowed, digest, &mut streamed)?;
        assert_eq!(
            streamed, owned_bytes,
            "single-entry writer diverged from owned codec"
        );
        assert_eq!(
            checksummed_pretty_len(&borrowed)?,
            owned_bytes.len(),
            "byte planner must count the encoded table representation"
        );

        let owned_shard = super::super::ShardedCacheEnvelope::new(key.clone(), 0, 2, seams.clone());
        let owned_shard_bytes = codec::encode_shard(&owned_shard)?;
        let borrowed_shard = borrowed_shard_envelope(&key, 0, 2, &seams);
        let digest = semantic_body_digest(SHARDED_ENVELOPE_DIGEST_DOMAIN, &borrowed_shard)?;
        let mut streamed = Vec::new();
        encode_checksummed_pretty_to_writer(&borrowed_shard, digest, &mut streamed)?;
        assert_eq!(
            streamed, owned_shard_bytes,
            "shard writer diverged from owned codec"
        );
        assert_eq!(
            checksummed_pretty_len(&borrowed_shard)?,
            owned_shard_bytes.len()
        );
        Ok(())
    }

    #[test]
    fn below_exactly_and_one_byte_over_the_encoded_ceiling() -> Result<(), String> {
        let dir = isolated_dir("exact-ceiling");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let seams = vec![classified_with_pad("exact")];
        let envelope = borrowed_cache_envelope(&key, &seams, None, &[]);
        let encoded_len = checksummed_pretty_len(&envelope)?;

        let below = cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &seams,
            None,
            8,
            encoded_len.saturating_add(8),
        )?;
        assert_eq!(below.label, "ok");
        round_trip(&cache, &key, &seams)?;
        let file_len = std::fs::metadata(cache.entry_path(&key))
            .map_err(|err| err.to_string())?
            .len() as usize;
        assert_eq!(file_len, encoded_len);

        let exact = cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &seams,
            None,
            8,
            encoded_len,
        )?;
        assert_eq!(exact.label, "ok");
        round_trip(&cache, &key, &seams)?;

        let over = cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &seams,
            None,
            8,
            encoded_len.saturating_sub(1),
        )?;
        assert!(
            over.label
                .starts_with("skipped_oversized_record_index_0_ceiling_"),
            "one-byte-over single record must skip, got {}",
            over.label
        );
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn variable_size_records_force_byte_shards_when_record_count_would_not() -> Result<(), String> {
        let dir = isolated_dir("variable-size");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let seams = vec![
            classified_with_pad("a"),
            classified_with_pad("b"),
            classified_with_pad(&"M".repeat(4000)),
        ];
        let all_len = checksummed_pretty_len(&borrowed_cache_envelope(&key, &seams, None, &[]))?;
        let small_shard =
            checksummed_pretty_len(&borrowed_shard_envelope(&key, 0, 3, &seams[..1]))?;
        let medium_shard =
            checksummed_pretty_len(&borrowed_shard_envelope(&key, 0, 3, &seams[2..3]))?;
        let ceiling = medium_shard.max(small_shard);
        assert!(
            all_len > ceiling,
            "all-in-one encoding {all_len} must exceed the per-record shard ceiling {ceiling}"
        );
        reset_classified_seam_clone_count();
        reset_integrity_vec_high_water();
        let status = cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 8, ceiling)?;
        assert!(
            status.label.starts_with("sharded_ok_seams_3_shards_"),
            "record-count-only packing would keep one entry; got {}",
            status.label
        );
        assert!(
            classified_seam_clone_count() == 0,
            "ordinary publication must not clone classified seams, cloned {}",
            classified_seam_clone_count()
        );
        assert_eq!(
            integrity_vec_high_water(),
            0,
            "ordinary publication must not retain a complete encoded Vec"
        );
        assert!(
            !cache.entry_path(&key).exists(),
            "byte-bounded shards must not leave a competing single entry"
        );
        let mut saw_over_count_pack = false;
        for index in 0..8 {
            let Ok(path) = cache.sharded_shard_path(&key, index) else {
                break;
            };
            let len = std::fs::metadata(&path)
                .map_err(|err| err.to_string())?
                .len() as usize;
            assert!(
                len <= ceiling,
                "shard {index} encoded {} bytes above ceiling {ceiling}",
                len
            );
            saw_over_count_pack = true;
        }
        assert!(saw_over_count_pack, "expected at least one shard file");
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn many_small_records_empty_and_multi_shard_round_trip() -> Result<(), String> {
        let dir = isolated_dir("many-small");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();

        let empty_status =
            cache.store_classified_seams_with_record_and_byte_limits(&key, &[], None, 8, 4096)?;
        assert_eq!(empty_status.label, "ok");
        round_trip(&cache, &key, &[])?;

        let seams: Vec<_> = (0..12)
            .map(|i| classified_with_pad(&format!("r{i}")))
            .collect();
        let one = checksummed_pretty_len(&borrowed_shard_envelope(&key, 0, 12, &seams[..1]))?;
        let status = cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &seams,
            None,
            3,
            one.saturating_add(one / 2).max(one.saturating_add(64)),
        )?;
        assert!(
            status.label.contains("shards_"),
            "many small records should shard under a tight ceiling: {}",
            status.label
        );
        round_trip(&cache, &key, &seams)?;
        assert!(
            !cache.entry_path(&key).exists(),
            "replacing a single entry with shards must not leave the preferred single path"
        );
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn store_high_water_is_independent_of_total_shard_payload() -> Result<(), String> {
        let dir = isolated_dir("high-water");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let pad = "P".repeat(CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES.saturating_add(2048));
        let seams = vec![classified_with_pad(&pad), classified_with_pad("tail")];
        reset_classified_seam_clone_count();
        reset_integrity_vec_high_water();
        let status = cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &seams,
            None,
            8,
            CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES.saturating_mul(4),
        )?;
        assert_eq!(status.label, "ok");
        let file_len = std::fs::metadata(cache.entry_path(&key))
            .map_err(|err| err.to_string())?
            .len() as usize;
        assert!(
            file_len > CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES,
            "fixture must exceed the IO buffer, got {file_len}"
        );
        assert_eq!(
            classified_seam_clone_count(),
            0,
            "single-entry publication must not clone classified seams, cloned {}",
            classified_seam_clone_count()
        );
        assert_eq!(integrity_vec_high_water(), 0);
        assert!(store_io_high_water() > 0);
        assert!(store_io_high_water() <= CLASSIFIED_SEAM_CACHE_STORE_IO_BUFFER_BYTES);
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn oversized_record_skips_without_claiming_a_populated_cache() -> Result<(), String> {
        let dir = isolated_dir("oversized");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let seams = vec![classified_with_pad(&"Z".repeat(2048))];
        let status =
            cache.store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 8, 64)?;
        assert!(
            status
                .label
                .starts_with("skipped_oversized_record_index_0_ceiling_64"),
            "got {}",
            status.label
        );
        assert!(
            !cache.entry_path(&key).exists(),
            "skipped oversized store must not publish a single entry"
        );
        assert!(
            !cache.sharded_manifest_path(&key).exists(),
            "skipped oversized store must not publish a shard generation"
        );
        match cache.load_classified_seams(&key) {
            CacheLoad::Miss => {}
            other => return Err(format!("skipped store must remain a miss, got {other:?}")),
        }
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn failed_replacement_keeps_the_previous_valid_generation() -> Result<(), String> {
        let dir = isolated_dir("replace-fail");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let first = vec![
            classified_with_pad("first-a"),
            classified_with_pad("first-b"),
        ];
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &first, None, 1, 1_000_000)?;
        round_trip(&cache, &key, &first)?;
        let generations_before = listed_generation_dirs(&cache, &key)?;
        assert_eq!(
            generations_before.len(),
            1,
            "first publication should leave one generation dir: {generations_before:?}"
        );
        fail_after_writing_shards(1);
        let second = vec![
            classified_with_pad("second-a"),
            classified_with_pad("second-b"),
            classified_with_pad("second-c"),
        ];
        let err = match cache
            .store_classified_seams_with_record_and_byte_limits(&key, &second, None, 1, 1_000_000)
        {
            Ok(status) => {
                return Err(format!(
                    "injected replacement failure should not succeed: {}",
                    status.label
                ));
            }
            Err(err) => err,
        };
        assert!(
            err.contains("injected"),
            "replacement failure should keep the injected diagnostic: {err}"
        );
        assert_eq!(
            listed_generation_dirs(&cache, &key)?,
            generations_before,
            "failed publication must not leave an extra generation directory"
        );
        round_trip(&cache, &key, &first)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn injected_fill_failure_does_not_admit_a_partial_generation() -> Result<(), String> {
        let dir = isolated_dir("fill-fail");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let first = vec![classified_with_pad("keep")];
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &first, None, 8, 1_000_000)?;
        fail_next_streamed_fill();
        let err = match cache.store_classified_seams_with_record_and_byte_limits(
            &key,
            &[classified_with_pad("new")],
            None,
            8,
            1_000_000,
        ) {
            Ok(status) => {
                return Err(format!(
                    "injected fill failure succeeded as {}",
                    status.label
                ));
            }
            Err(err) => err,
        };
        assert!(
            err.contains("injected classified cache fill failure"),
            "fill failure should name the injected error: {err}"
        );
        round_trip(&cache, &key, &first)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn failed_sharded_replace_restores_the_previous_single_entry() -> Result<(), String> {
        let dir = isolated_dir("park-restore");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let first = vec![classified_with_pad("keep-single")];
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &first, None, 8, 1_000_000)?;
        assert!(
            cache.entry_path(&key).exists(),
            "fixture must start as a preferred single entry"
        );
        fail_after_parking_single_entry();
        let second: Vec<_> = (0..6)
            .map(|i| classified_with_pad(&format!("next-{i}")))
            .collect();
        let err = match cache
            .store_classified_seams_with_record_and_byte_limits(&key, &second, None, 2, 1_000_000)
        {
            Ok(status) => {
                return Err(format!(
                    "injected park failure should not succeed: {}",
                    status.label
                ));
            }
            Err(err) => err,
        };
        assert!(
            err.contains("after parking the single entry"),
            "park failure should keep the injected diagnostic: {err}"
        );
        assert!(
            cache.entry_path(&key).exists(),
            "failed sharded replace must restore the previous single entry"
        );
        assert!(
            listed_generation_dirs(&cache, &key)?.is_empty(),
            "failed sharded replace must not leave an unpublished generation"
        );
        round_trip(&cache, &key, &first)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn parked_restore_does_not_replace_a_newer_single_entry() -> Result<(), String> {
        let dir = isolated_dir("park-noreplace-single");
        ignore_remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let original = dir.join("entry.json");
        let manifest = dir.join("manifest.json");
        std::fs::write(&original, b"old-single").map_err(|err| err.to_string())?;
        let parked = ParkedSingleEntry::park(original.clone(), manifest, "1")?
            .ok_or_else(|| "expected a parked single entry".to_string())?;
        std::fs::write(&original, b"concurrent-single").map_err(|err| err.to_string())?;
        drop(parked);
        let body = std::fs::read(&original).map_err(|err| err.to_string())?;
        assert_eq!(
            body, b"concurrent-single",
            "rollback must not replace a single entry published after parking"
        );
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn parked_restore_does_not_hide_a_newer_sharded_manifest() -> Result<(), String> {
        let dir = isolated_dir("park-noreplace-manifest");
        ignore_remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let original = dir.join("entry.json");
        let manifest = dir.join("manifest.json");
        std::fs::write(&original, b"old-single").map_err(|err| err.to_string())?;
        let parked = ParkedSingleEntry::park(original.clone(), manifest.clone(), "1")?
            .ok_or_else(|| "expected a parked single entry".to_string())?;
        std::fs::write(&manifest, b"newer-manifest").map_err(|err| err.to_string())?;
        drop(parked);
        assert!(
            !original.exists(),
            "rollback must not restore a single entry over a newer sharded manifest"
        );
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn parked_restore_keeps_a_stale_leftover_manifest_from_blocking_restore() -> Result<(), String>
    {
        let dir = isolated_dir("park-leftover-manifest");
        ignore_remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let original = dir.join("entry.json");
        let manifest = dir.join("manifest.json");
        std::fs::write(&original, b"newer-single").map_err(|err| err.to_string())?;
        std::fs::write(&manifest, b"stale-leftover-manifest").map_err(|err| err.to_string())?;
        let parked = ParkedSingleEntry::park(original.clone(), manifest.clone(), "1")?
            .ok_or_else(|| "expected a parked single entry".to_string())?;
        drop(parked);
        let body = std::fs::read(&original).map_err(|err| err.to_string())?;
        assert_eq!(
            body, b"newer-single",
            "an unchanged leftover sharded manifest must not block restoring the parked single entry"
        );
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    fn sharded_fixture(
        label: &str,
        count: usize,
    ) -> Result<
        (
            PathBuf,
            RepoSeamFactCache,
            RepoSeamCacheKey,
            Vec<ClassifiedSeam>,
        ),
        String,
    > {
        let dir = isolated_dir(label);
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let seams: Vec<_> = (0..count)
            .map(|i| classified_with_pad(&format!("{label}-{i}")))
            .collect();
        Ok((dir, cache, key, seams))
    }

    #[test]
    fn oversized_record_skip_names_the_record_its_size_and_the_remedy() -> Result<(), String> {
        let (dir, cache, key, _) = sharded_fixture("oversized-advisory", 0)?;
        let seams = vec![classified_with_pad(&"x".repeat(4096))];
        let status = cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 8, 1_024)?;
        assert_eq!(
            status.label,
            "skipped_oversized_record_index_0_ceiling_1024"
        );
        let advisory = status
            .advisory
            .ok_or_else(|| "an oversized skip must carry an advisory".to_string())?;
        assert!(advisory.contains("seam record 0"), "{advisory}");
        assert!(
            advisory.contains("above the 1024-byte ceiling"),
            "{advisory}"
        );
        assert!(
            advisory.contains("RIPR_CLASSIFIED_SEAM_CACHE_SHARD_BYTES"),
            "{advisory}"
        );
        let size: usize = advisory
            .split("encodes to ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| format!("advisory must state the encoded size: {advisory}"))?;
        assert!(
            size > 4096,
            "size must be the real encoded record size: {size}"
        );
        // The advised value must actually restore warm runs.
        let retried = cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 8, size)?;
        assert!(retried.advisory.is_none(), "{retried:?}");
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn oversized_metadata_skip_names_metadata_not_a_record() -> Result<(), String> {
        let (dir, cache, key, _) = sharded_fixture("oversized-metadata", 0)?;
        let status =
            cache.store_classified_seams_with_record_and_byte_limits(&key, &[], None, 8, 8)?;
        assert_eq!(status.label, "skipped_oversized_metadata_ceiling_8");
        let advisory = status
            .advisory
            .ok_or_else(|| "an oversized skip must carry an advisory".to_string())?;
        assert!(advisory.contains("the cache metadata"), "{advisory}");
        assert!(!advisory.contains("seam record"), "{advisory}");
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn older_single_entry_restored_after_a_sharded_commit_never_hides_it() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("late-restore", 6)?;
        let older = vec![classified_with_pad("older-single")];
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &older, None, 8, 1_000_000)?;
        let stale_single = std::fs::read(cache.entry_path(&key)).map_err(|err| err.to_string())?;
        let entry = cache.entry_path(&key);
        // A competing writer rolls back after this commit and restores its parked entry.
        after_next_manifest_commit(Box::new(move || {
            if let Ok(file) = std::fs::File::create(&entry) {
                use std::io::Write;
                let mut file = file;
                let _ = file.write_all(&stale_single);
                let _ = file.set_modified(
                    std::time::SystemTime::now() - std::time::Duration::from_hours(1),
                );
            }
        }));
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        assert!(
            !cache.entry_path(&key).exists(),
            "a restored older single entry must not survive a newer sharded commit"
        );
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn single_entry_published_after_a_sharded_commit_is_kept() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("newer-single", 6)?;
        let newer = vec![classified_with_pad("newer-single")];
        let entry = cache.entry_path(&key);
        let bytes = {
            let scratch = RepoSeamFactCache::at_dir(isolated_dir("newer-single-scratch"));
            scratch.store_classified_seams_with_record_and_byte_limits(
                &key, &newer, None, 8, 1_000_000,
            )?;
            std::fs::read(scratch.entry_path(&key)).map_err(|err| err.to_string())?
        };
        // A competing single publication lands after this writer's manifest commit.
        after_next_manifest_commit(Box::new(move || {
            if let Some(parent) = entry.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&entry, bytes);
            // The kernel's coarse file clock can land just behind `started_at`; pin it.
            if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&entry) {
                let _ = file
                    .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5));
            }
        }));
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        assert!(
            cache.entry_path(&key).exists(),
            "a single entry published after the commit is newer and must stay"
        );
        round_trip(&cache, &key, &newer)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn orphan_generations_are_swept_only_when_old_and_unreferenced() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("orphan-sweep", 6)?;
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        let sharded = cache.sharded_entry_dir(&key);
        let live = listed_generation_dirs(&cache, &key)?;
        assert_eq!(
            live.len(),
            1,
            "fixture must publish one generation: {live:?}"
        );
        // A writer terminated mid-write leaves a generation no manifest names.
        let orphan = sharded.join("g99-1-0");
        std::fs::create_dir_all(&orphan).map_err(|err| err.to_string())?;
        std::fs::write(orphan.join("shard-00000.json"), b"partial")
            .map_err(|err| err.to_string())?;
        let now = std::time::SystemTime::now();
        assert_eq!(
            sweep_orphan_generations(&sharded, now, ORPHAN_GENERATION_GRACE),
            0,
            "a young orphan may belong to a writer still running"
        );
        assert!(orphan.exists());
        let later = now + std::time::Duration::from_hours(2);
        assert_eq!(
            sweep_orphan_generations(&sharded, later, ORPHAN_GENERATION_GRACE),
            1
        );
        assert!(
            !orphan.exists(),
            "an old unreferenced generation must be removed"
        );
        assert_eq!(listed_generation_dirs(&cache, &key)?, live);
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn sharded_publication_sweeps_an_old_orphan_generation() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("orphan-wired", 6)?;
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        let sharded = cache.sharded_entry_dir(&key);
        let old = sharded.join("g98-1-0");
        let young = sharded.join("g99-1-0");
        for orphan in [&old, &young] {
            std::fs::create_dir_all(orphan).map_err(|err| err.to_string())?;
            std::fs::write(orphan.join("shard-00000.json"), b"partial")
                .map_err(|err| err.to_string())?;
        }
        std::fs::File::open(&old)
            .and_then(|file| {
                file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_hours(2))
            })
            .map_err(|err| err.to_string())?;
        let next: Vec<_> = (0..6)
            .map(|i| classified_with_pad(&format!("wired-{i}")))
            .collect();
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &next, None, 2, 1_000_000)?;
        assert!(!old.exists(), "publication must sweep an old orphan");
        assert!(young.exists(), "a young orphan may belong to a live writer");
        round_trip(&cache, &key, &next)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn orphan_sweep_does_nothing_without_a_valid_manifest() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("orphan-sweep-doubt", 6)?;
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        let sharded = cache.sharded_entry_dir(&key);
        let orphan = sharded.join("g99-1-0");
        std::fs::create_dir_all(&orphan).map_err(|err| err.to_string())?;
        let manifest = sharded.join("manifest.json");
        let mut bytes = std::fs::read(&manifest).map_err(|err| err.to_string())?;
        bytes.push(b'x');
        std::fs::write(&manifest, bytes).map_err(|err| err.to_string())?;
        let later = std::time::SystemTime::now() + std::time::Duration::from_hours(2);
        assert_eq!(
            sweep_orphan_generations(&sharded, later, std::time::Duration::ZERO),
            0
        );
        assert!(
            orphan.exists(),
            "an unreadable manifest must keep every generation"
        );
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn replaced_generation_cleanup_only_deletes_files_inside_generation_directories()
    -> Result<(), String> {
        let dir = isolated_dir("cleanup-scope");
        ignore_remove_dir_all(&dir);
        let generation = dir.join("g1-1-0");
        std::fs::create_dir_all(&generation).map_err(|err| err.to_string())?;
        std::fs::write(generation.join("shard-00000.json"), b"old")
            .map_err(|err| err.to_string())?;
        for sibling in ["gfoo.json", "manifest.json"] {
            std::fs::write(dir.join(sibling), b"keep").map_err(|err| err.to_string())?;
        }
        let refs = ["g1-1-0/shard-00000.json", "gfoo.json", "manifest.json"]
            .iter()
            .enumerate()
            .map(|(index, file)| ShardedCacheShardRef {
                index,
                file: (*file).to_string(),
                seams: 1,
            })
            .collect();
        let previous = ShardedCacheManifest::new(empty_key(), 3, 3, refs, None, Vec::new());
        remove_replaced_generation_files(&dir, &previous, "2-2-0");
        assert!(
            !generation.join("shard-00000.json").exists(),
            "a replaced generation file is removed"
        );
        for sibling in ["gfoo.json", "manifest.json"] {
            assert!(
                dir.join(sibling).exists(),
                "{sibling} is outside any g*/ directory"
            );
        }
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn tampered_previous_manifest_never_deletes_the_live_manifest() -> Result<(), String> {
        let (dir, cache, key, seams) = sharded_fixture("tampered-manifest", 6)?;
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &seams, None, 2, 1_000_000)?;
        let manifest = cache.sharded_manifest_path(&key);
        let text = std::fs::read_to_string(&manifest).map_err(|err| err.to_string())?;
        let marker = "shard-00000.json";
        let start = text
            .find(marker)
            .and_then(|end| text[..end].rfind('"'))
            .ok_or_else(|| "fixture manifest must list a first shard".to_string())?;
        let end = text.find(marker).map_or(start, |at| at + marker.len());
        // A path inside a `g*/` directory passes the deletion-scope guard, so only
        // the integrity check on the previous manifest can protect this file.
        let victim = cache
            .sharded_entry_dir(&key)
            .join("gzz")
            .join("victim.json");
        std::fs::create_dir_all(victim.parent().ok_or("victim parent")?)
            .map_err(|err| err.to_string())?;
        std::fs::write(&victim, b"must survive").map_err(|err| err.to_string())?;
        let tampered = format!("{}\"gzz/victim.json{}", &text[..start], &text[end..]);
        std::fs::write(&manifest, tampered).map_err(|err| err.to_string())?;
        let next: Vec<_> = (0..6)
            .map(|i| classified_with_pad(&format!("after-tamper-{i}")))
            .collect();
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &next, None, 2, 1_000_000)?;
        assert!(manifest.exists(), "the new manifest must survive");
        assert!(
            victim.exists(),
            "a file named only by a tampered previous manifest must not be deleted"
        );
        round_trip(&cache, &key, &next)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn cancellation_before_manifest_preserves_the_prior_generation() -> Result<(), String> {
        use crate::analysis::cancellation::{
            AnalysisAbortKind, AnalysisCancellationToken, with_token,
        };
        let dir = isolated_dir("cancel");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_key();
        let first = vec![classified_with_pad("warm-a"), classified_with_pad("warm-b")];
        cache
            .store_classified_seams_with_record_and_byte_limits(&key, &first, None, 1, 1_000_000)?;
        let generations_before = listed_generation_dirs(&cache, &key)?;
        let token = AnalysisCancellationToken::new();
        token.cancel(AnalysisAbortKind::Cancelled);
        let err = with_token(&token, || {
            cache.store_classified_seams_with_record_and_byte_limits(
                &key,
                &[
                    classified_with_pad("cancelled-a"),
                    classified_with_pad("cancelled-b"),
                ],
                None,
                1,
                1_000_000,
            )
        });
        match err {
            Ok(status) => {
                return Err(format!(
                    "cancelled store should not succeed: {}",
                    status.label
                ));
            }
            Err(err) => assert!(
                err.contains("analysis cancelled"),
                "cancellation should be named: {err}"
            ),
        }
        assert_eq!(
            listed_generation_dirs(&cache, &key)?,
            generations_before,
            "cancelled publication must not leave an extra generation directory"
        );
        round_trip(&cache, &key, &first)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn publication_ids_stay_unique_across_concurrent_calls() -> Result<(), String> {
        let handles: Vec<_> = (0..32)
            .map(|_| std::thread::spawn(publication_id))
            .collect();
        let mut ids = Vec::with_capacity(handles.len());
        for handle in handles {
            let id = handle
                .join()
                .map_err(|_join| "publication_id thread did not finish".to_string())?;
            ids.push(id);
        }
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != ids.len() {
            return Err(format!(
                "same-process concurrent publication ids collided: {ids:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn size_probe_stops_once_the_encoded_ceiling_is_exceeded() -> Result<(), String> {
        let key = empty_key();
        let seams: Vec<_> = (0..80)
            .map(|i| classified_with_pad(&format!("n{i}")))
            .collect();
        let all = borrowed_cache_envelope(&key, &seams, None, &[]);
        let all_len = checksummed_pretty_len(&all)?;
        let ceiling = (all_len / 8).max(64);
        assert!(
            all_len > ceiling.saturating_mul(2),
            "fixture must be well above the probe ceiling: all={all_len} ceiling={ceiling}"
        );
        PLAN_ENCODE_HIGH_WATER.with(|water| water.set(0));
        assert!(
            !checksummed_pretty_fits(&all, ceiling)?,
            "full payload {all_len} must not fit under {ceiling}"
        );
        let probe = plan_encode_high_water();
        assert!(
            probe > ceiling,
            "the stopping write may overshoot by one serde chunk, got {probe} ceiling {ceiling}"
        );
        assert!(
            probe < all_len,
            "size probe must stop before serializing the full payload: probe={probe} all={all_len}"
        );

        let dir = isolated_dir("bounded-probe");
        ignore_remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let status = cache.store_classified_seams_with_record_and_byte_limits(
            &key, &seams, None, 100_000, ceiling,
        )?;
        assert!(
            status.label.contains("shards_"),
            "a 100k record cap must still split on the byte ceiling: {}",
            status.label
        );
        round_trip(&cache, &key, &seams)?;
        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn sharded_cache_paths_reject_drive_prefix_and_parent_components() -> Result<(), String> {
        let dir = Path::new("/tmp/ripr-classified-cache");
        let ok = resolve_sharded_cache_file(dir, "g1-2-3/shard-00000.json")?;
        assert!(
            ok.ends_with("g1-2-3/shard-00000.json") || ok.ends_with("g1-2-3\\shard-00000.json"),
            "expected a cache-relative generation path, got {}",
            ok.display()
        );
        for unsafe_name in [
            "",
            "..",
            "../x",
            "g1/../x",
            "C:evil.json",
            "g1/C:x",
            "g1\\x",
        ] {
            let err = resolve_sharded_cache_file(dir, unsafe_name)
                .err()
                .ok_or_else(|| {
                    format!("unsafe sharded cache file {unsafe_name:?} should be rejected")
                })?;
            assert!(
                err.contains("unsafe") || err.contains("empty"),
                "unexpected diagnostic for {unsafe_name:?}: {err}"
            );
        }
        Ok(())
    }
}
