//! Fact-layer cache for repo seam analysis (`cache/repo-seam-facts-v1`).
//!
//! Caches `Vec<ClassifiedSeam>` keyed on the aggregate workspace state
//! (per-file content hashes, cfg/features, config, test intent,
//! suppressions, analyzer version, schema version). The cold path
//! computes the inventory from scratch and writes the entry; the warm
//! path returns the cached entry when the key matches; corrupt entries
//! degrade to `Miss` so analysis never fails because of cache state.
//!
//! Per Campaign 5A acceptance:
//!
//! - cache fact layers only — `FileFacts`, owner index, `RepoSeam` facts,
//!   `TestGripEvidence`, `ClassifiedSeam` summaries. v1 caches the
//!   workspace-level `Vec<ClassifiedSeam>` (which transitively covers
//!   the listed layers) and per-file `FileFacts` so timed-out cold paths can
//!   still make the next run cheaper.
//! - never cache rendered JSON, Markdown, diagnostics, hover, or packet
//!   strings. The renderers re-render from the cached facts.
//! - codec stays behind a module boundary
//!   ([`codec::encode`] / [`codec::decode`]).
//! - never `bincode`. v1 uses `serde_json` (inspectable, easy to debug).
//!   `postcard` is the binary path if profiling later proves it
//!   necessary; the codec module is the only place that needs to change.
//!
//! The cache directory lives at:
//!
//! ```text
//! {workspace_root}/target/ripr/cache/repo-seam-facts/{schema_version}/{key_hash}.json
//! ```
//!
//! `{key_hash}` is the FNV-1a 64-bit hash of the canonical key fields,
//! so different keys land in different files and a v1 cache hit on a
//! v0.5 entry is impossible.

use super::facts::FileFacts;
use super::seam_classification::ClassifiedSeam;
#[cfg(test)]
use super::seam_classification::SeamGripClassCounts;
use std::path::{Path, PathBuf};

/// Cache schema version. Bump when the on-disk file shape changes; old
/// directories can be deleted on `cargo clean` or manually.
///
/// `0.1` → `0.2`: `RelatedTestGrip` gained `relation_reason` and
/// `relation_confidence` fields in `analysis/related-test-precision-v1`.
/// Old envelopes lack those fields and would fail serde deserialization
/// of the new shape; the version bump routes new entries to a fresh
/// directory and lets old entries go orphaned (gc'd on `cargo clean`).
pub(crate) const CACHE_SCHEMA_VERSION: &str = "0.2";

/// Compact-classified seam cache schema. This cache stores the same
/// `ClassifiedSeam` envelope shape as the full repo exposure cache, but
/// under a separate directory because the evidence payload is intentionally
/// compact and must never satisfy full repo-exposure consumers.
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION: &str = "0.1";

/// Compact class-count cache used by repo badge rendering. It keys off
/// the same workspace state as the full fact cache, but stores only
/// per-class counts so badge endpoints never need to deserialize the
/// multi-hundred-megabyte evidence cache.
#[cfg(test)]
const COUNT_CACHE_SCHEMA_VERSION: &str = "0.1";

/// Per-file fact cache schema. This is intentionally separate from the
/// workspace-level classified seam cache so warm compute can reuse parser facts
/// even when a full classified seam entry has not been written yet.
const FILE_FACT_CACHE_SCHEMA_VERSION: &str = "0.1";

/// Keep the best-effort classified-seam cache from turning a successful live
/// analysis into an unbounded post-analysis stall on large repos. Larger live
/// audits should surface a named cache-store limitation instead of spending the
/// remaining audit budget on full-evidence JSON serialization.
pub(crate) const CLASSIFIED_SEAM_CACHE_STORE_LIMIT: usize = 20_000;
pub(crate) const CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV: &str = "RIPR_REPO_SEAM_CACHE_LIMIT";
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT: usize = 100_000;
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV: &str =
    "RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS";

pub(crate) fn classified_seam_cache_store_limit() -> Result<usize, String> {
    classified_seam_cache_store_limit_from_env(std::env::var(CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV))
}

fn classified_seam_cache_store_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    seam_cache_store_limit_from_env(
        value,
        CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV,
        CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
    )
}

pub(crate) fn compact_classified_seam_cache_store_limit() -> Result<usize, String> {
    compact_classified_seam_cache_store_limit_from_env(std::env::var(
        COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV,
    ))
}

fn compact_classified_seam_cache_store_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    seam_cache_store_limit_from_env(
        value,
        COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV,
        COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
    )
}

fn seam_cache_store_limit_from_env(
    value: Result<String, std::env::VarError>,
    env_name: &str,
    default_limit: usize,
) -> Result<usize, String> {
    match value {
        Ok(value) => parse_positive_seam_cache_store_limit(&value, env_name),
        Err(std::env::VarError::NotPresent) => Ok(default_limit),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{env_name} must be valid UTF-8")),
    }
}

fn parse_positive_seam_cache_store_limit(value: &str, env_name: &str) -> Result<usize, String> {
    let trimmed = value.trim();
    let parsed = trimmed
        .parse::<usize>()
        .map_err(|err| format!("{env_name} must be a positive integer: {err}"))?;
    if parsed == 0 {
        return Err(format!("{env_name} must be a positive integer"));
    }
    Ok(parsed)
}

/// Aggregate cache key — every field that, when changed, must invalidate
/// the workspace-level classified seam cache.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RepoSeamCacheKey {
    pub(crate) schema_version: String,
    pub(crate) analyzer_version: String,
    pub(crate) workspace_root_hash: String,
    pub(crate) files_content_hash: String,
    pub(crate) cfg_features_hash: String,
    pub(crate) config_hash: String,
    pub(crate) test_intent_hash: String,
    pub(crate) suppressions_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RepoFileFactCacheKey {
    schema_version: String,
    analyzer_version: String,
    file_path: PathBuf,
    content_hash: String,
}

impl RepoFileFactCacheKey {
    pub(crate) fn new(file_path: &Path, content: &[u8]) -> Self {
        Self {
            schema_version: FILE_FACT_CACHE_SCHEMA_VERSION.to_string(),
            analyzer_version: env!("CARGO_PKG_VERSION").to_string(),
            file_path: file_path.to_path_buf(),
            content_hash: hash_bytes(content),
        }
    }

    fn filename(&self) -> String {
        let file_path = self.file_path.to_string_lossy();
        let parts = [
            self.schema_version.as_str(),
            self.analyzer_version.as_str(),
            file_path.as_ref(),
            self.content_hash.as_str(),
        ];
        let mut buf = String::new();
        for (idx, part) in parts.iter().enumerate() {
            if idx > 0 {
                buf.push('\0');
            }
            buf.push_str(part);
        }
        format!("{:016x}.json", fnv1a_64(buf.as_bytes()))
    }
}

impl RepoSeamCacheKey {
    /// Filename component derived from the canonical key fields. The
    /// FNV-1a 64-bit hash is stable across releases (unlike
    /// `DefaultHasher`) and produces a 16-char lowercase hex string.
    pub(crate) fn filename(&self) -> String {
        let parts: [&str; 8] = [
            &self.schema_version,
            &self.analyzer_version,
            &self.workspace_root_hash,
            &self.files_content_hash,
            &self.cfg_features_hash,
            &self.config_hash,
            &self.test_intent_hash,
            &self.suppressions_hash,
        ];
        let mut buf = String::new();
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                buf.push('\0');
            }
            buf.push_str(p);
        }
        format!("{:016x}.json", fnv1a_64(buf.as_bytes()))
    }
}

/// Outcome of a cache load. `CorruptIgnored` exists so analysis can
/// continue when an entry is unreadable, malformed, or references a
/// schema we no longer accept.
#[derive(Debug)]
pub(crate) enum CacheLoad<T> {
    Hit(T),
    Miss,
    CorruptIgnored { reason: String },
}

/// Inputs the analysis pipeline collects to derive the cache key. Held
/// separately so the test pyramid can construct a known state without
/// touching the filesystem.
pub(crate) struct WorkspaceState<'a> {
    pub(crate) workspace_root: &'a Path,
    /// `(canonical relative path, content bytes)` for every Rust file
    /// the inventory will index — production **seam sources** plus test
    /// **evidence sources**. `ClassifiedSeam` carries `TestGripEvidence`
    /// derived from test files, so a test-only edit must invalidate the
    /// cache; restricting this to production files would let stale grip
    /// evidence survive a test rewrite. Order does not matter — the
    /// hash sorts before mixing.
    pub(crate) files: &'a [(PathBuf, Vec<u8>)],
    pub(crate) cfg_features: Option<&'a str>,
    pub(crate) config_text: Option<&'a str>,
    pub(crate) test_intent_text: Option<&'a str>,
    pub(crate) suppressions_text: Option<&'a str>,
}

impl<'a> WorkspaceState<'a> {
    pub(crate) fn cache_key(&self) -> RepoSeamCacheKey {
        let workspace_root_hash = hash_str(&self.workspace_root.to_string_lossy());

        // Sort by path so file walk order does not change the hash.
        let mut sorted_files: Vec<(&PathBuf, &Vec<u8>)> =
            self.files.iter().map(|(p, b)| (p, b)).collect();
        sorted_files.sort_by(|a, b| a.0.cmp(b.0));
        let mut files_buf = String::new();
        for (path, content) in sorted_files {
            files_buf.push_str(&path.to_string_lossy().replace('\\', "/"));
            files_buf.push('\0');
            files_buf.push_str(&hash_bytes(content));
            files_buf.push('\n');
        }
        let files_content_hash = hash_str(&files_buf);

        RepoSeamCacheKey {
            schema_version: CACHE_SCHEMA_VERSION.to_string(),
            analyzer_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_root_hash,
            files_content_hash,
            cfg_features_hash: hash_str(self.cfg_features.unwrap_or("")),
            config_hash: hash_str(self.config_text.unwrap_or("")),
            test_intent_hash: hash_str(self.test_intent_text.unwrap_or("")),
            suppressions_hash: hash_str(self.suppressions_text.unwrap_or("")),
        }
    }
}

/// Crate-private cache I/O surface. Holds the directory the cache lives
/// in but not in-memory state; safe to construct cheaply per call.
pub(crate) struct RepoSeamFactCache {
    dir: PathBuf,
}

impl RepoSeamFactCache {
    /// Construct a cache rooted at the workspace's `target/ripr/cache/...`.
    pub(crate) fn at(workspace_root: &Path) -> Self {
        Self::at_named(workspace_root, "repo-seam-facts", CACHE_SCHEMA_VERSION)
    }

    /// Construct the separate compact-classified cache used by repo badge
    /// projection. It deliberately does not share entries with full repo
    /// exposure because compact evidence omits the large related-test payload.
    pub(crate) fn at_compact_classified(workspace_root: &Path) -> Self {
        Self::at_named(
            workspace_root,
            "repo-compact-classified-seams",
            COMPACT_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION,
        )
    }

    fn at_named(workspace_root: &Path, cache_name: &str, schema_version: &str) -> Self {
        Self {
            dir: workspace_root
                .join("target")
                .join("ripr")
                .join("cache")
                .join(cache_name)
                .join(schema_version),
        }
    }

    /// Construct a cache at an explicit directory (tests use this to
    /// avoid touching the real workspace).
    #[cfg(test)]
    pub(crate) fn at_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Look up classified seams by key. `Miss` is returned for both
    /// "no file" and "different key", so callers do not have to
    /// distinguish in v1. `CorruptIgnored` carries a reason for logs.
    pub(crate) fn load_classified_seams(
        &self,
        key: &RepoSeamCacheKey,
    ) -> CacheLoad<Vec<ClassifiedSeam>> {
        let path = self.entry_path(key);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return CacheLoad::Miss,
            Err(err) => {
                return CacheLoad::CorruptIgnored {
                    reason: format!("read failed: {err}"),
                };
            }
        };
        match codec::decode(&bytes) {
            Ok(envelope) => {
                if envelope.matches_key(key) {
                    CacheLoad::Hit(envelope.classified_seams)
                } else {
                    // Key collision is unlikely (16-char FNV file
                    // names + 8 fields hashed in), but possible. Treat
                    // as miss without failing analysis.
                    CacheLoad::Miss
                }
            }
            Err(reason) => CacheLoad::CorruptIgnored { reason },
        }
    }

    pub(crate) fn store_compact_classified_seams_with_limit(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        store_limit: usize,
    ) -> Result<(), String> {
        self.store_classified_seams_with_limit(key, seams, store_limit)
    }

    pub(crate) fn store_classified_seams_with_limit(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        store_limit: usize,
    ) -> Result<(), String> {
        if seams.len() > store_limit {
            return Err(format!(
                "skipped_large_entry_seams_{}_limit_{}",
                seams.len(),
                store_limit
            ));
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|err| format!("create cache dir failed: {err}"))?;
        let envelope = CacheEnvelope::new(key.clone(), seams.to_vec());
        let bytes = codec::encode(&envelope)?;
        let path = self.entry_path(key);
        std::fs::write(&path, &bytes).map_err(|err| format!("write cache failed: {err}"))?;
        Ok(())
    }

    fn entry_path(&self, key: &RepoSeamCacheKey) -> PathBuf {
        self.dir.join(key.filename())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FileFactCacheStats {
    pub(crate) hits: usize,
    pub(crate) misses: usize,
    pub(crate) corrupt_ignored: usize,
    pub(crate) stores: usize,
    pub(crate) store_errors: usize,
}

impl FileFactCacheStats {
    pub(crate) fn status_label(&self) -> String {
        format!(
            "hits_{}_misses_{}_corrupt_{}_store_errors_{}",
            self.hits, self.misses, self.corrupt_ignored, self.store_errors
        )
    }
}

pub(crate) struct RepoFileFactCache {
    dir: PathBuf,
}

impl RepoFileFactCache {
    pub(crate) fn at(workspace_root: &Path) -> Self {
        Self {
            dir: workspace_root
                .join("target")
                .join("ripr")
                .join("cache")
                .join("repo-file-facts")
                .join(FILE_FACT_CACHE_SCHEMA_VERSION),
        }
    }

    #[cfg(test)]
    pub(crate) fn at_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub(crate) fn load_file_facts(&self, key: &RepoFileFactCacheKey) -> CacheLoad<FileFacts> {
        let path = self.entry_path(key);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return CacheLoad::Miss,
            Err(err) => {
                return CacheLoad::CorruptIgnored {
                    reason: format!("read failed: {err}"),
                };
            }
        };
        match codec::decode_file_facts(&bytes) {
            Ok(envelope) => {
                if envelope.matches_key(key) {
                    CacheLoad::Hit(envelope.file_facts)
                } else {
                    CacheLoad::Miss
                }
            }
            Err(reason) => CacheLoad::CorruptIgnored { reason },
        }
    }

    pub(crate) fn store_file_facts(
        &self,
        key: &RepoFileFactCacheKey,
        facts: &FileFacts,
    ) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|err| format!("create file fact cache dir failed: {err}"))?;
        let envelope = FileFactCacheEnvelope::new(key.clone(), facts.clone());
        let bytes = codec::encode_file_facts(&envelope)?;
        std::fs::write(self.entry_path(key), bytes)
            .map_err(|err| format!("write file fact cache failed: {err}"))?;
        Ok(())
    }

    fn entry_path(&self, key: &RepoFileFactCacheKey) -> PathBuf {
        self.dir.join(key.filename())
    }
}

/// Compact cache for [`SeamGripClassCounts`].
#[cfg(test)]
pub(crate) struct RepoSeamCountCache {
    dir: PathBuf,
}

#[cfg(test)]
impl RepoSeamCountCache {
    /// Construct a count cache rooted at the workspace's
    /// `target/ripr/cache/...`.
    pub(crate) fn at(workspace_root: &Path) -> Self {
        Self {
            dir: workspace_root
                .join("target")
                .join("ripr")
                .join("cache")
                .join("repo-seam-counts")
                .join(COUNT_CACHE_SCHEMA_VERSION),
        }
    }

    pub(crate) fn load_counts(&self, key: &RepoSeamCacheKey) -> CacheLoad<SeamGripClassCounts> {
        let path = self.entry_path(key);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return CacheLoad::Miss,
            Err(err) => {
                return CacheLoad::CorruptIgnored {
                    reason: format!("read failed: {err}"),
                };
            }
        };
        match codec::decode_counts(&bytes) {
            Ok(envelope) => {
                if envelope.matches_key(key) {
                    CacheLoad::Hit(envelope.counts)
                } else {
                    CacheLoad::Miss
                }
            }
            Err(reason) => CacheLoad::CorruptIgnored { reason },
        }
    }

    pub(crate) fn store_counts(
        &self,
        key: &RepoSeamCacheKey,
        counts: &SeamGripClassCounts,
    ) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|err| format!("create count cache dir failed: {err}"))?;
        let envelope = CountCacheEnvelope::new(key.clone(), counts.clone());
        let bytes = codec::encode_counts(&envelope)?;
        let path = self.entry_path(key);
        std::fs::write(&path, &bytes).map_err(|err| format!("write count cache failed: {err}"))?;
        Ok(())
    }

    fn entry_path(&self, key: &RepoSeamCacheKey) -> PathBuf {
        self.dir.join(key.filename())
    }
}

/// On-disk shape. The key is embedded so callers can verify on read
/// even though the filename already encodes a hash of the same fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct CacheEnvelope {
    schema_version: String,
    analyzer_version: String,
    workspace_root_hash: String,
    files_content_hash: String,
    cfg_features_hash: String,
    config_hash: String,
    test_intent_hash: String,
    suppressions_hash: String,
    classified_seams: Vec<ClassifiedSeam>,
}

#[cfg(test)]
#[derive(serde::Serialize, serde::Deserialize)]
struct CountCacheEnvelope {
    count_cache_schema_version: String,
    schema_version: String,
    analyzer_version: String,
    workspace_root_hash: String,
    files_content_hash: String,
    cfg_features_hash: String,
    config_hash: String,
    test_intent_hash: String,
    suppressions_hash: String,
    counts: SeamGripClassCounts,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileFactCacheEnvelope {
    file_fact_cache_schema_version: String,
    analyzer_version: String,
    file_path: PathBuf,
    content_hash: String,
    file_facts: FileFacts,
}

impl FileFactCacheEnvelope {
    fn new(key: RepoFileFactCacheKey, file_facts: FileFacts) -> Self {
        Self {
            file_fact_cache_schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            file_path: key.file_path,
            content_hash: key.content_hash,
            file_facts,
        }
    }

    fn matches_key(&self, key: &RepoFileFactCacheKey) -> bool {
        self.file_fact_cache_schema_version == key.schema_version
            && self.analyzer_version == key.analyzer_version
            && self.file_path == key.file_path
            && self.content_hash == key.content_hash
    }
}

#[cfg(test)]
impl CountCacheEnvelope {
    fn new(key: RepoSeamCacheKey, counts: SeamGripClassCounts) -> Self {
        Self {
            count_cache_schema_version: COUNT_CACHE_SCHEMA_VERSION.to_string(),
            schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            workspace_root_hash: key.workspace_root_hash,
            files_content_hash: key.files_content_hash,
            cfg_features_hash: key.cfg_features_hash,
            config_hash: key.config_hash,
            test_intent_hash: key.test_intent_hash,
            suppressions_hash: key.suppressions_hash,
            counts,
        }
    }

    fn matches_key(&self, key: &RepoSeamCacheKey) -> bool {
        self.count_cache_schema_version == COUNT_CACHE_SCHEMA_VERSION
            && self.schema_version == key.schema_version
            && self.analyzer_version == key.analyzer_version
            && self.workspace_root_hash == key.workspace_root_hash
            && self.files_content_hash == key.files_content_hash
            && self.cfg_features_hash == key.cfg_features_hash
            && self.config_hash == key.config_hash
            && self.test_intent_hash == key.test_intent_hash
            && self.suppressions_hash == key.suppressions_hash
    }
}

impl CacheEnvelope {
    fn new(key: RepoSeamCacheKey, classified_seams: Vec<ClassifiedSeam>) -> Self {
        Self {
            schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            workspace_root_hash: key.workspace_root_hash,
            files_content_hash: key.files_content_hash,
            cfg_features_hash: key.cfg_features_hash,
            config_hash: key.config_hash,
            test_intent_hash: key.test_intent_hash,
            suppressions_hash: key.suppressions_hash,
            classified_seams,
        }
    }

    fn matches_key(&self, key: &RepoSeamCacheKey) -> bool {
        self.schema_version == key.schema_version
            && self.analyzer_version == key.analyzer_version
            && self.workspace_root_hash == key.workspace_root_hash
            && self.files_content_hash == key.files_content_hash
            && self.cfg_features_hash == key.cfg_features_hash
            && self.config_hash == key.config_hash
            && self.test_intent_hash == key.test_intent_hash
            && self.suppressions_hash == key.suppressions_hash
    }
}

/// Codec module — the only place serialization format is decided.
/// Switching to `postcard` for binary v2 is a localized change here.
mod codec {
    #[cfg(test)]
    use super::CountCacheEnvelope;
    use super::{CacheEnvelope, FileFactCacheEnvelope};

    pub(super) fn encode(envelope: &CacheEnvelope) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope).map_err(|err| format!("encode failed: {err}"))
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<CacheEnvelope, String> {
        serde_json::from_slice(bytes).map_err(|err| format!("decode failed: {err}"))
    }

    #[cfg(test)]
    pub(super) fn encode_counts(envelope: &CountCacheEnvelope) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope).map_err(|err| format!("encode counts failed: {err}"))
    }

    #[cfg(test)]
    pub(super) fn decode_counts(bytes: &[u8]) -> Result<CountCacheEnvelope, String> {
        serde_json::from_slice(bytes).map_err(|err| format!("decode counts failed: {err}"))
    }

    pub(super) fn encode_file_facts(envelope: &FileFactCacheEnvelope) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope)
            .map_err(|err| format!("encode file facts failed: {err}"))
    }

    pub(super) fn decode_file_facts(bytes: &[u8]) -> Result<FileFactCacheEnvelope, String> {
        serde_json::from_slice(bytes).map_err(|err| format!("decode file facts failed: {err}"))
    }
}

fn hash_str(s: &str) -> String {
    hash_bytes(s.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a_64(bytes))
}

/// FNV-1a 64-bit. Same algorithm `seams::compute_seam_id` uses; chosen
/// for its dependency-free determinism across Rust releases.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash: u64 = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seam_classification::ClassifiedSeam;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::TestGripEvidence;
    use crate::domain::{Confidence, StageEvidence, StageState};
    use std::path::PathBuf;

    fn sample_classified() -> ClassifiedSeam {
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
            reach: StageEvidence::new(StageState::Yes, Confidence::High, "reach"),
            activate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "activate"),
            propagate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "propagate"),
            observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observe"),
            discriminate: StageEvidence::new(StageState::No, Confidence::Low, "discriminate"),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::Ungripped,
        }
    }

    fn empty_state() -> WorkspaceState<'static> {
        WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &[],
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
    }

    fn isolated_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ripr-cache-{label}-{}", uuid_like()))
    }

    #[test]
    fn given_no_cache_when_load_runs_then_miss_is_returned() -> Result<(), String> {
        let dir = isolated_dir("cold");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir);
        let key = empty_state().cache_key();
        match cache.load_classified_seams(&key) {
            CacheLoad::Miss => Ok(()),
            other => Err(format!("expected Miss on missing cache dir, got {other:?}")),
        }
    }

    #[test]
    fn given_unchanged_inputs_when_cache_is_warm_then_classified_seams_are_reused()
    -> Result<(), String> {
        let dir = isolated_dir("warm");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified()];
        cache
            .store_classified_seams_with_limit(&key, &seams, CLASSIFIED_SEAM_CACHE_STORE_LIMIT)
            .map_err(|err| format!("store should succeed: {err}"))?;
        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit(loaded) => {
                if loaded.len() != seams.len() {
                    Err(format!(
                        "warm path should return stored seams, got {} vs {}",
                        loaded.len(),
                        seams.len()
                    ))
                } else if loaded[0].seam.id().as_str() != seams[0].seam.id().as_str() {
                    Err(format!(
                        "round-trip should preserve seam id, got {} vs {}",
                        loaded[0].seam.id().as_str(),
                        seams[0].seam.id().as_str()
                    ))
                } else if loaded[0].class != seams[0].class {
                    Err(format!(
                        "round-trip should preserve class, got {:?} vs {:?}",
                        loaded[0].class, seams[0].class
                    ))
                } else {
                    Ok(())
                }
            }
            other => Err(format!("expected Hit on warm cache, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_large_classified_entry_when_cache_store_runs_then_write_is_skipped()
    -> Result<(), String> {
        let dir = isolated_dir("large-skip");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];
        let err = match cache.store_classified_seams_with_limit(&key, &seams, 1) {
            Ok(()) => return Err("large classified seam cache entries should be skipped".into()),
            Err(err) => err,
        };

        assert!(
            err.contains("skipped_large_entry_seams_2_limit_1"),
            "skip reason should be machine-readable: {err}"
        );
        assert!(
            !cache.entry_path(&key).exists(),
            "skipped cache store should not write a classified seam entry"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn classified_cache_store_limit_defaults_to_20k_when_env_missing() -> Result<(), String> {
        let limit =
            classified_seam_cache_store_limit_from_env(Err(std::env::VarError::NotPresent))?;

        assert_eq!(limit, CLASSIFIED_SEAM_CACHE_STORE_LIMIT);
        Ok(())
    }

    #[test]
    fn classified_cache_store_limit_accepts_positive_env_override() -> Result<(), String> {
        let limit = classified_seam_cache_store_limit_from_env(Ok("25000".to_string()))?;

        assert_eq!(limit, 25_000);
        Ok(())
    }

    #[test]
    fn classified_cache_store_limit_rejects_invalid_env_override() -> Result<(), String> {
        for value in ["", "0", "not-a-number"] {
            let err = match classified_seam_cache_store_limit_from_env(Ok(value.to_string())) {
                Ok(limit) => {
                    return Err(format!(
                        "invalid classified cache env value {value:?} should fail, got limit {limit}"
                    ));
                }
                Err(err) => err,
            };
            assert!(
                err.contains(CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV),
                "diagnostic should name env var for {value:?}: {err}"
            );
            assert!(
                err.contains("positive integer"),
                "diagnostic should describe expected value for {value:?}: {err}"
            );
        }
        Ok(())
    }

    #[test]
    fn classified_cache_store_limit_can_be_raised_for_large_entries() -> Result<(), String> {
        let dir = isolated_dir("classified-raised-limit");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];

        cache
            .store_classified_seams_with_limit(&key, &seams, 2)
            .map_err(|err| format!("raised classified cache limit should allow store: {err}"))?;

        match cache.load_classified_seams(&key) {
            CacheLoad::Hit(loaded) if loaded.len() == 2 => {}
            other => {
                return Err(format!(
                    "expected classified cache hit after raised limit: {other:?}"
                ));
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn compact_cache_store_limit_defaults_to_100k_when_env_missing() -> Result<(), String> {
        let limit = compact_classified_seam_cache_store_limit_from_env(Err(
            std::env::VarError::NotPresent,
        ))?;

        assert_eq!(limit, COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT);
        Ok(())
    }

    #[test]
    fn compact_cache_store_limit_accepts_positive_env_override() -> Result<(), String> {
        let limit = compact_classified_seam_cache_store_limit_from_env(Ok("200000".to_string()))?;

        assert_eq!(limit, 200_000);
        Ok(())
    }

    #[test]
    fn compact_cache_store_limit_rejects_invalid_env_override() -> Result<(), String> {
        for value in ["", "0", "not-a-number"] {
            let err = match compact_classified_seam_cache_store_limit_from_env(
                Ok(value.to_string()),
            ) {
                Ok(limit) => {
                    return Err(format!(
                        "invalid compact cache env value {value:?} should fail, got limit {limit}"
                    ));
                }
                Err(err) => err,
            };
            assert!(
                err.contains(COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV),
                "diagnostic should name env var for {value:?}: {err}"
            );
            assert!(
                err.contains("positive integer"),
                "diagnostic should describe expected value for {value:?}: {err}"
            );
        }
        Ok(())
    }

    #[test]
    fn compact_cache_store_limit_controls_skip_reason() -> Result<(), String> {
        let dir = isolated_dir("compact-large-skip");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];

        let err = match cache.store_compact_classified_seams_with_limit(&key, &seams, 1) {
            Ok(()) => {
                return Err("configured compact cache limit should skip oversized entry".into());
            }
            Err(err) => err,
        };

        assert!(
            err.contains("skipped_large_entry_seams_2_limit_1"),
            "skip reason should carry configured limit: {err}"
        );
        assert!(
            !cache.entry_path(&key).exists(),
            "skipped compact cache store should not write an entry"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn compact_cache_store_limit_can_be_raised_for_large_entries() -> Result<(), String> {
        let dir = isolated_dir("compact-raised-limit");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];

        cache
            .store_compact_classified_seams_with_limit(&key, &seams, 2)
            .map_err(|err| format!("raised compact cache limit should allow store: {err}"))?;

        match cache.load_classified_seams(&key) {
            CacheLoad::Hit(loaded) if loaded.len() == 2 => {}
            other => {
                return Err(format!(
                    "expected compact cache hit after raised limit: {other:?}"
                ));
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn given_changed_file_content_hash_when_cache_is_loaded_then_old_entry_is_treated_as_miss()
    -> Result<(), String> {
        let dir = isolated_dir("changed");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let path = PathBuf::from("src/foo.rs");
        let original_files = [(path.clone(), b"fn foo() {}\n".to_vec())];
        let original_key = WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &original_files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();
        cache
            .store_classified_seams_with_limit(
                &original_key,
                &[sample_classified()],
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store original: {err}"))?;
        let new_files = [(path, b"fn foo() { let x = 1; }\n".to_vec())];
        let new_key = WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &new_files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();
        if original_key.files_content_hash == new_key.files_content_hash {
            return Err("different file content must produce different files_content_hash".into());
        }
        let result = match cache.load_classified_seams(&new_key) {
            CacheLoad::Miss => Ok(()),
            other => Err(format!(
                "expected Miss after file content change, got {other:?}"
            )),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_test_file_content_changes_when_cache_key_is_built_then_classified_seam_cache_is_invalidated()
    -> Result<(), String> {
        // The cache hashes the same Rust file set fed to `build_index`
        // — production *and* test files. `ClassifiedSeam` carries
        // `TestGripEvidence` derived from test files, so a test-only
        // edit must change the key. This test pins that contract by
        // varying only a test file's content (no test_intent.toml,
        // no suppressions.toml, no production change).
        let prod = PathBuf::from("src/foo.rs");
        let prod_bytes = b"pub fn foo() -> i32 { 1 }\n".to_vec();
        let test_path = PathBuf::from("tests/foo_test.rs");

        let baseline_files = [
            (prod.clone(), prod_bytes.clone()),
            (
                test_path.clone(),
                b"#[test] fn smoke() { assert_eq!(1, 1); }\n".to_vec(),
            ),
        ];
        let baseline = WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &baseline_files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();

        let updated_files = [
            (prod, prod_bytes),
            (
                test_path,
                b"#[test] fn smoke() { assert_eq!(super::foo(), 1); }\n".to_vec(),
            ),
        ];
        let updated = WorkspaceState {
            workspace_root: Path::new("/repo"),
            files: &updated_files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();

        if baseline.files_content_hash == updated.files_content_hash {
            return Err(
                "test-only file content change must change files_content_hash so stale \
                 TestGripEvidence cannot survive in the cache"
                    .into(),
            );
        }
        if baseline.filename() == updated.filename() {
            return Err(
                "test-only file content change must produce a different cache filename".into(),
            );
        }
        Ok(())
    }

    #[test]
    fn given_test_intent_hash_change_when_cache_is_loaded_then_classified_seam_cache_is_invalidated()
    -> Result<(), String> {
        let baseline = WorkspaceState {
            test_intent_text: Some(""),
            ..empty_state()
        }
        .cache_key();
        let updated = WorkspaceState {
            test_intent_text: Some("[[test]] name = \"smoke\""),
            ..empty_state()
        }
        .cache_key();
        if baseline.test_intent_hash == updated.test_intent_hash {
            return Err("different test intent must produce different test_intent_hash".into());
        }
        if baseline.filename() == updated.filename() {
            return Err(
                "different test_intent_hash must produce a different cache filename".into(),
            );
        }
        Ok(())
    }

    #[test]
    fn given_suppression_hash_change_when_cache_is_loaded_then_classified_seam_cache_is_invalidated()
    -> Result<(), String> {
        let baseline = WorkspaceState {
            suppressions_text: Some(""),
            ..empty_state()
        }
        .cache_key();
        let updated = WorkspaceState {
            suppressions_text: Some("[[suppression]] kind = \"exposure_gap\""),
            ..empty_state()
        }
        .cache_key();
        if baseline.suppressions_hash == updated.suppressions_hash {
            return Err(
                "different suppressions text must produce different suppressions_hash".into(),
            );
        }
        if baseline.filename() == updated.filename() {
            return Err(
                "different suppressions_hash must produce a different cache filename".into(),
            );
        }
        Ok(())
    }

    #[test]
    fn given_corrupt_cache_entry_when_loading_then_corrupt_ignored_is_reported_without_failing()
    -> Result<(), String> {
        let dir = isolated_dir("corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir: {err}"))?;
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let path = cache.entry_path(&key);
        std::fs::write(&path, b"{not valid json")
            .map_err(|err| format!("write corrupt entry: {err}"))?;
        let result = match cache.load_classified_seams(&key) {
            CacheLoad::CorruptIgnored { reason } => {
                if !reason.contains("decode failed") {
                    Err(format!(
                        "corrupt reason should explain decode failure, got {reason}"
                    ))
                } else {
                    Ok(())
                }
            }
            other => Err(format!(
                "expected CorruptIgnored on bad json, got {other:?}"
            )),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_envelope_key_mismatch_when_loading_then_miss_is_returned_without_failing()
    -> Result<(), String> {
        let dir = isolated_dir("keymismatch");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key_a = WorkspaceState {
            cfg_features: Some("a"),
            ..empty_state()
        }
        .cache_key();
        let key_b = WorkspaceState {
            cfg_features: Some("b"),
            ..empty_state()
        }
        .cache_key();
        cache
            .store_classified_seams_with_limit(
                &key_a,
                &[sample_classified()],
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store under key_a: {err}"))?;
        // Write key_a's envelope under key_b's filename — simulates a
        // hash collision or stale entry.
        let envelope = CacheEnvelope::new(key_a.clone(), vec![sample_classified()]);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir: {err}"))?;
        let bytes = codec::encode(&envelope)?;
        std::fs::write(cache.entry_path(&key_b), bytes)
            .map_err(|err| format!("write under wrong filename: {err}"))?;
        let result = match cache.load_classified_seams(&key_b) {
            CacheLoad::Miss => Ok(()),
            other => Err(format!(
                "expected Miss when envelope key mismatches request, got {other:?}"
            )),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_file_facts_cached_when_loading_same_file_bytes_then_hit_is_returned()
    -> Result<(), String> {
        let dir = isolated_dir("file-facts-warm");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoFileFactCache::at_dir(dir.clone());
        let path = PathBuf::from("src/lib.rs");
        let key = RepoFileFactCacheKey::new(&path, b"pub fn cached() {}\n");
        let facts = FileFacts {
            path: path.clone(),
            source: "pub fn cached() {}\n".to_string(),
            ..FileFacts::default()
        };

        cache
            .store_file_facts(&key, &facts)
            .map_err(|err| format!("store file facts should succeed: {err}"))?;

        let result = match cache.load_file_facts(&key) {
            CacheLoad::Hit(loaded) => {
                if loaded != facts {
                    Err("loaded file facts should match stored facts".to_string())
                } else {
                    Ok(())
                }
            }
            other => Err(format!("expected file fact cache hit, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_file_content_changes_when_file_facts_load_then_miss_is_returned() -> Result<(), String>
    {
        let dir = isolated_dir("file-facts-invalidates");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoFileFactCache::at_dir(dir.clone());
        let path = PathBuf::from("src/lib.rs");
        let original_key = RepoFileFactCacheKey::new(&path, b"pub fn cached() -> i32 { 1 }\n");
        let changed_key = RepoFileFactCacheKey::new(&path, b"pub fn cached() -> i32 { 2 }\n");
        let facts = FileFacts {
            path: path.clone(),
            source: "pub fn cached() -> i32 { 1 }\n".to_string(),
            ..FileFacts::default()
        };

        cache
            .store_file_facts(&original_key, &facts)
            .map_err(|err| format!("store original file facts: {err}"))?;

        let result = match cache.load_file_facts(&changed_key) {
            CacheLoad::Miss => Ok(()),
            other => Err(format!(
                "expected Miss after file content change, got {other:?}"
            )),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn file_fact_cache_stats_status_label_is_trace_safe() {
        let stats = FileFactCacheStats {
            hits: 2,
            misses: 3,
            corrupt_ignored: 1,
            stores: 3,
            store_errors: 0,
        };
        assert_eq!(
            stats.status_label(),
            "hits_2_misses_3_corrupt_1_store_errors_0"
        );
    }

    /// Tiny non-crypto unique-ish suffix for tempdir naming. Avoids
    /// depending on `tempfile` and avoids tests racing each other when
    /// run with `--test-threads`.
    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{}-{:x}", std::process::id(), nanos)
    }
}
