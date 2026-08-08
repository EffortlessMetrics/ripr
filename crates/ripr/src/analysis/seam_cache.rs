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
//! A companion corpus fingerprint cache
//! (`repo-corpus-fingerprint/{schema_version}/{fingerprint}.json`, issue
//! #2108) maps a stat-only corpus signature — sorted `(path, mtime, size)`
//! tuples, plus the inode change time on unix — to the aggregate `files_content_hash` previously computed for
//! that signature, so a warm cache-key computation does not re-read the
//! corpus. A fingerprint miss costs nothing: the content hash is then
//! computed exactly as before and the mapping is stored.
//!
//! When `RIPR_CACHE_DIR` is set (non-empty), all cache writes and reads
//! use `{RIPR_CACHE_DIR}/...` as the cache base instead. When unset,
//! behaviour is unchanged — default `{workspace_root}/target/ripr/cache`.
//!
//! `{key_hash}` is the FNV-1a 64-bit hash of the canonical key fields,
//! so different keys land in different files and a v1 cache hit on a
//! v0.5 entry is impossible.

use super::facts::FileFacts;
use super::seam_classification::ClassifiedSeam;
#[cfg(test)]
use super::seam_classification::SeamGripClassCounts;
use super::seam_inventory::{SeamLimitSource, repo_exposure_seam_limit};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// On-disk representation of seam-limit metadata embedded in the cache envelope.
/// Mirrors `SeamLimitInfo` but lives in the cache module to avoid a circular dep.
/// `#[serde(default)]` ensures old cache entries without this field deserialize
/// as `None` (= complete run; correct, since pre-Slice-B caches were full runs).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CachedSeamLimitInfo {
    pub(crate) analyzed: usize,
    pub(crate) total: usize,
    pub(crate) source: SeamLimitSource,
}

/// Cache schema version. Bump when the on-disk file shape changes; old
/// directories can be deleted on `cargo clean` or manually.
///
/// `0.1` → `0.2`: `RelatedTestGrip` gained `relation_reason` and
/// `relation_confidence` fields in `analysis/related-test-precision-v1`.
/// Old envelopes lack those fields and would fail serde deserialization
/// of the new shape; the version bump routes new entries to a fresh
/// directory and lets old entries go orphaned (gc'd on `cargo clean`).
/// `0.3` → `0.4`: `RelatedTestGrip` gained producer-owned
/// `TestTargetEvidence`; old envelopes deserialize with a missing target and
/// would incorrectly turn valid indexed tests into static limitations.
/// `0.4` → `0.5`: error-variant discriminators changed from surrounding
/// expressions to producer-owned exact identities; old classified seams must
/// not be reused by full or compact consumers.
pub(crate) const CACHE_SCHEMA_VERSION: &str = "0.6";
const SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION: &str = "0.2";

/// Compact-classified seam cache schema. This cache stores the same
/// `ClassifiedSeam` envelope shape as the full repo exposure cache, but
/// under a separate directory because the evidence payload is intentionally
/// compact and must never satisfy full repo-exposure consumers.
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION: &str = "0.3";

/// Compact class-count cache used by repo badge rendering. It keys off
/// the same workspace state as the full fact cache, but stores only
/// per-class counts so badge endpoints never need to deserialize the
/// multi-hundred-megabyte evidence cache.
#[cfg(test)]
const COUNT_CACHE_SCHEMA_VERSION: &str = "0.1";

/// Per-file fact cache schema. This is intentionally separate from the
/// workspace-level classified seam cache so warm compute can reuse parser facts
/// even when a full classified seam entry has not been written yet.
pub(crate) const FILE_FACT_CACHE_SCHEMA_VERSION: &str = "0.2";

/// Keep the best-effort classified-seam cache from turning a successful live
/// analysis into an unbounded post-analysis stall on large repos. Larger live
/// audits should surface a named cache-store limitation instead of spending the
/// remaining audit budget on full-evidence JSON serialization.
pub(crate) const CLASSIFIED_SEAM_CACHE_STORE_LIMIT: usize = 20_000;
pub(crate) const CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV: &str = "RIPR_REPO_SEAM_CACHE_LIMIT";
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT: usize = 100_000;
pub(crate) const COMPACT_CLASSIFIED_SEAM_CACHE_STORE_LIMIT_ENV: &str =
    "RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS";

/// Environment variable that relocates the cache base directory. When set
/// to a non-empty path, all cache reads and writes use that path as the
/// root instead of `{workspace_root}/target/ripr/cache`. When unset or
/// empty the default `{workspace_root}/target/ripr/cache` is used.
///
/// This is useful for read-only or immutable source checkouts where
/// writing into `target/` is not permitted, and for redirecting the
/// cache to a faster or larger volume.
///
/// # Example
///
/// ```text
/// RIPR_CACHE_DIR=/var/cache/ripr ripr check --diff path/to/file.diff
/// ```
pub(crate) const CACHE_DIR_ENV: &str = "RIPR_CACHE_DIR";

/// Resolve the cache base directory.
///
/// When `RIPR_CACHE_DIR` is set to a non-empty value that base is used
/// directly (no sub-path is appended). When unset or empty the default
/// `{workspace_root}/target/ripr/cache` is returned, which is
/// byte-identical to the pre-relocation behaviour.
///
/// Takes the env-var value as a parameter rather than calling
/// `std::env::var` internally so tests can exercise both branches
/// without mutating process state.
pub(crate) fn cache_base_dir_from_env(
    workspace_root: &std::path::Path,
    env_value: Result<String, std::env::VarError>,
) -> PathBuf {
    match env_value {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
        _ => workspace_root.join("target").join("ripr").join("cache"),
    }
}

/// Resolve the cache base directory using the live process environment.
pub(crate) fn cache_base_dir(workspace_root: &std::path::Path) -> PathBuf {
    cache_base_dir_from_env(workspace_root, std::env::var(CACHE_DIR_ENV))
}

/// Read-only summary of a cache directory for status and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CacheStatus {
    pub(crate) state: &'static str,
    pub(crate) total_size_bytes: u64,
    pub(crate) entry_count: usize,
}

/// Inspect a cache directory without following symlinks or fabricating counts.
///
/// This is best-effort diagnostic output, not a security boundary or an atomic
/// snapshot. The standard-library path-based traversal has no portable
/// directory-handle-relative API, so a concurrent rename can still make a
/// reported total stale. Symlink entries are skipped and traversal failures
/// are surfaced as `partial`; callers must not use this report to authorize
/// access or make security decisions.
pub(crate) fn inspect_cache_dir(cache_dir: &Path) -> CacheStatus {
    let metadata = match std::fs::symlink_metadata(cache_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CacheStatus {
                state: "not_found",
                total_size_bytes: 0,
                entry_count: 0,
            };
        }
        Err(_) => {
            return CacheStatus {
                state: "unavailable",
                total_size_bytes: 0,
                entry_count: 0,
            };
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return CacheStatus {
            state: "unavailable",
            total_size_bytes: 0,
            entry_count: 0,
        };
    }

    let mut total_size_bytes = 0u64;
    let mut entry_count = 0usize;
    let mut partially_readable = false;
    let mut stack = vec![cache_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            partially_readable = true;
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                partially_readable = true;
                continue;
            };
            let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                partially_readable = true;
                continue;
            };
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                total_size_bytes = total_size_bytes.saturating_add(metadata.len());
                entry_count = entry_count.saturating_add(1);
            }
        }
    }

    CacheStatus {
        state: if partially_readable { "partial" } else { "ok" },
        total_size_bytes,
        entry_count,
    }
}

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
    pub(crate) workspace_manifests_hash: String,
    pub(crate) lockfile_hash: String,
    pub(crate) toolchain_hash: String,
    /// Encodes the effective seam limit: `"unlimited"` when the operator
    /// set `RIPR_REPO_EXPOSURE_SEAM_LIMIT=0` (unbounded opt-out), or
    /// `"limit_N"` for any positive limit (default or configured).
    /// Different limits produce different filenames, so a capped cache
    /// entry is never served for an unbounded run and vice-versa.
    pub(crate) seam_limit_key: String,
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
    /// `seam_limit_key` is included so capped runs and unbounded runs
    /// never share a cache file.
    pub(crate) fn filename(&self) -> String {
        let parts: [&str; 12] = [
            &self.schema_version,
            &self.analyzer_version,
            &self.workspace_root_hash,
            &self.files_content_hash,
            &self.cfg_features_hash,
            &self.config_hash,
            &self.test_intent_hash,
            &self.suppressions_hash,
            &self.workspace_manifests_hash,
            &self.lockfile_hash,
            &self.toolchain_hash,
            &self.seam_limit_key,
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

/// Aggregate content hash over the corpus file set — the exact
/// `files_content_hash` derivation `WorkspaceState::cache_key` has always
/// used, extracted so the corpus fingerprint store can persist and reuse it
/// (issue #2108). The corpus fingerprint fast path rebuilds a byte-identical
/// cache key from this stored value instead of re-reading every file.
pub(crate) fn files_content_hash(files: &[(PathBuf, Vec<u8>)]) -> String {
    // Sort by path so file walk order does not change the hash.
    let mut sorted_files: Vec<(&PathBuf, &Vec<u8>)> = files.iter().map(|(p, b)| (p, b)).collect();
    sorted_files.sort_by(|a, b| a.0.cmp(b.0));
    let mut files_buf = String::new();
    for (path, content) in sorted_files {
        files_buf.push_str(&path.to_string_lossy().replace('\\', "/"));
        files_buf.push('\0');
        files_buf.push_str(&hash_bytes(content));
        files_buf.push('\n');
    }
    hash_str(&files_buf)
}

/// Cache-key inputs other than the corpus content hash. The fingerprint
/// fast path (issue #2108) holds no file bytes, so it rebuilds the key from
/// these small inputs plus the stored `files_content_hash`. Keeping the key
/// derivation here — shared with [`WorkspaceState::cache_key`] — is what
/// guarantees a fingerprint-rebuilt key is byte-identical to a freshly
/// computed one.
pub(crate) struct WorkspaceKeyContext<'a> {
    pub(crate) workspace_root: &'a Path,
    pub(crate) cfg_features: Option<&'a str>,
    pub(crate) config_text: Option<&'a str>,
    pub(crate) test_intent_text: Option<&'a str>,
    pub(crate) suppressions_text: Option<&'a str>,
}

impl WorkspaceKeyContext<'_> {
    pub(crate) fn cache_key(&self, files_content_hash: String) -> RepoSeamCacheKey {
        let workspace_root_hash = hash_str(&self.workspace_root.to_string_lossy());

        // Encode the effective seam limit into the key so capped runs and
        // unbounded runs never share a cache file.
        let seam_limit_key = match repo_exposure_seam_limit() {
            None => "unlimited".to_string(),
            Some((n, _)) => format!("limit_{n}"),
        };

        let workspace_manifests_hash =
            hash_named_workspace_files(self.workspace_root, "Cargo.toml");
        let lockfile_hash = hash_named_workspace_files(self.workspace_root, "Cargo.lock");
        let toolchain_hash = hash_str(
            std::env::var("RUSTUP_TOOLCHAIN")
                .or_else(|_| std::env::var("RIPR_TOOLCHAIN"))
                .unwrap_or_else(|_| "unavailable".to_string())
                .as_str(),
        );

        RepoSeamCacheKey {
            schema_version: CACHE_SCHEMA_VERSION.to_string(),
            analyzer_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_root_hash,
            files_content_hash,
            cfg_features_hash: hash_str(self.cfg_features.unwrap_or("")),
            config_hash: hash_str(self.config_text.unwrap_or("")),
            test_intent_hash: hash_str(self.test_intent_text.unwrap_or("")),
            suppressions_hash: hash_str(self.suppressions_text.unwrap_or("")),
            workspace_manifests_hash,
            lockfile_hash,
            toolchain_hash,
            seam_limit_key,
        }
    }
}

impl WorkspaceState<'_> {
    pub(crate) fn cache_key(&self) -> RepoSeamCacheKey {
        WorkspaceKeyContext {
            workspace_root: self.workspace_root,
            cfg_features: self.cfg_features,
            config_text: self.config_text,
            test_intent_text: self.test_intent_text,
            suppressions_text: self.suppressions_text,
        }
        .cache_key(files_content_hash(self.files))
    }
}

/// Corpus fingerprint cache schema. Separate directory from the classified
/// seam cache so the tiny mapping files can evolve independently; old
/// directories are gc'd on `cargo clean` like the other cache layers.
///
/// `0.1` → `0.2`: the fingerprint mixes in the unix ctime (inode change
/// time) on unix platforms, closing the mtime-preserving-rewrite residual
/// there. Old `0.1` mappings were computed without ctime and must never
/// match a `0.2` fingerprint, so they are orphaned in the `0.1` directory.
pub(crate) const CORPUS_FINGERPRINT_CACHE_SCHEMA_VERSION: &str = "0.2";

/// Cheap corpus signature: FNV-1a over sorted
/// `(relative path, mtime secs, mtime nanos, size)` tuples for every file in
/// `files` (paths relative to `root`), extended with the unix ctime (inode
/// change time secs + nanos) on unix platforms. Stat-only — no file
/// contents are read. Returns `None` when any file cannot be stat'd or has
/// no portable mtime; callers must then fall back to the full content
/// read, which is always correct.
///
/// Residual (issue #2108, narrowed by the ctime mix-in): on unix, ctime
/// bumps on ANY content or metadata write — including writes by tools that
/// restore mtime (`rsync -a`, `cp --preserve=timestamps`, archive
/// extractors) — so a same-size content rewrite with a preserved mtime
/// still invalidates the fingerprint. The only way to change bytes without
/// changing this signature is to write without touching any file metadata
/// at all, which no ordinary file API or tool can do. On non-unix
/// platforms the signature remains mtime+size only, and the original
/// caveat stands there: a content change preserving both size and mtime
/// (to the filesystem's mtime granularity) keeps the old fingerprint and
/// can serve the stale mapping.
///
/// Either way, the direction is bounded by the write path, which only
/// stores a mapping when the fingerprint is identical before and after the
/// content read, so a fingerprint hit can never select a hash derived from
/// a corpus with a *different* signature — only from a signature-identical
/// corpus whose bytes changed invisibly to the signature's fields.
pub(crate) fn corpus_fingerprint(root: &Path, files: &[PathBuf]) -> Option<String> {
    let mut entries: Vec<String> = Vec::with_capacity(files.len());
    for path in files {
        let metadata = std::fs::metadata(root.join(path)).ok()?;
        let modified = metadata.modified().ok()?;
        let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
        let entry = format!(
            "{}\0{}\0{}\0{}",
            path.to_string_lossy().replace('\\', "/"),
            since_epoch.as_secs(),
            since_epoch.subsec_nanos(),
            metadata.len()
        );
        // Unix hardening: ctime (inode change time) cannot be preserved by
        // mtime-restoring tools, so mixing it in closes the
        // preserved-mtime rewrite residual on unix (see the doc comment).
        #[cfg(unix)]
        let entry = {
            use std::os::unix::fs::MetadataExt;
            let mut entry = entry;
            entry.push('\0');
            entry.push_str(&metadata.ctime().to_string());
            entry.push('\0');
            entry.push_str(&metadata.ctime_nsec().to_string());
            entry
        };
        entries.push(entry);
    }
    entries.sort();
    let mut buf = String::new();
    for entry in &entries {
        buf.push_str(entry);
        buf.push('\n');
    }
    Some(hash_str(&buf))
}

/// On-disk shape for the corpus fingerprint mapping. One file per
/// fingerprint, mirroring the one-file-per-key layout of the other cache
/// layers; the embedded fields are re-verified on load so a hash collision
/// or a stale file degrades to a miss instead of a wrong hash.
#[derive(serde::Serialize, serde::Deserialize)]
struct CorpusFingerprintEnvelope {
    fingerprint_cache_schema_version: String,
    workspace_root_hash: String,
    fingerprint: String,
    files_content_hash: String,
}

/// Maps a corpus fingerprint (stat-only signature) to the aggregate
/// `files_content_hash` previously computed for that exact signature
/// (issue #2108). A hit lets the cache-key path skip reading the whole
/// corpus; a miss costs nothing beyond the stats already performed.
///
/// Layout:
///
/// ```text
/// {workspace_root}/target/ripr/cache/repo-corpus-fingerprint/{schema_version}/{fingerprint}.json
/// ```
pub(crate) struct RepoCorpusFingerprintCache {
    dir: PathBuf,
}

impl RepoCorpusFingerprintCache {
    pub(crate) fn at(workspace_root: &Path) -> Self {
        Self {
            dir: cache_base_dir(workspace_root)
                .join("repo-corpus-fingerprint")
                .join(CORPUS_FINGERPRINT_CACHE_SCHEMA_VERSION),
        }
    }

    /// Construct a cache at an explicit directory (tests use this to
    /// avoid touching the real workspace).
    #[cfg(test)]
    pub(crate) fn at_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Return the stored aggregate content hash for `fingerprint`, or
    /// `None` when no entry exists, the entry is unreadable/corrupt, or
    /// the entry belongs to a different schema version, workspace root, or
    /// fingerprint. Every failure mode is a conservative miss: the caller
    /// then recomputes the hash from file contents exactly as before.
    pub(crate) fn lookup(&self, workspace_root: &Path, fingerprint: &str) -> Option<String> {
        let path = self.entry_path(fingerprint);
        let bytes = std::fs::read(path).ok()?;
        let envelope = codec::decode_corpus_fingerprint(&bytes).ok()?;
        if envelope.fingerprint_cache_schema_version == CORPUS_FINGERPRINT_CACHE_SCHEMA_VERSION
            && envelope.workspace_root_hash == hash_str(&workspace_root.to_string_lossy())
            && envelope.fingerprint == fingerprint
        {
            Some(envelope.files_content_hash)
        } else {
            None
        }
    }

    /// Persist `fingerprint -> files_content_hash`. Written to a temp file
    /// in the same directory and renamed into place so a concurrent reader
    /// never observes a torn mapping (the fingerprint and the hash are
    /// updated atomically). Best-effort semantics mirror the other cache
    /// layers: an error is reported to the caller, which logs and moves on.
    pub(crate) fn store(
        &self,
        workspace_root: &Path,
        fingerprint: &str,
        files_content_hash: &str,
    ) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|err| format!("create corpus fingerprint cache dir failed: {err}"))?;
        let envelope = CorpusFingerprintEnvelope {
            fingerprint_cache_schema_version: CORPUS_FINGERPRINT_CACHE_SCHEMA_VERSION.to_string(),
            workspace_root_hash: hash_str(&workspace_root.to_string_lossy()),
            fingerprint: fingerprint.to_string(),
            files_content_hash: files_content_hash.to_string(),
        };
        let bytes = codec::encode_corpus_fingerprint(&envelope)?;
        let path = self.entry_path(fingerprint);
        crate::atomic_file::write_cache(&path, &bytes, "corpus fingerprint cache")?;
        Ok(())
    }

    fn entry_path(&self, fingerprint: &str) -> PathBuf {
        self.dir.join(format!("{fingerprint}.json"))
    }
}

/// Crate-private cache I/O surface. Holds the directory the cache lives
/// in but not in-memory state; safe to construct cheaply per call.
pub(crate) struct RepoSeamFactCache {
    dir: PathBuf,
    sharded_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CacheStoreStatus {
    pub(crate) label: String,
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
        let cache_root = cache_base_dir(workspace_root);
        Self {
            dir: cache_root.join(cache_name).join(schema_version),
            sharded_dir: cache_root
                .join(format!("{cache_name}-sharded"))
                .join(schema_version)
                .join(SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION),
        }
    }

    /// Construct a cache at an explicit directory (tests use this to
    /// avoid touching the real workspace).
    #[cfg(test)]
    pub(crate) fn at_dir(dir: PathBuf) -> Self {
        Self {
            sharded_dir: dir.join("sharded"),
            dir,
        }
    }

    /// Look up classified seams by key. Returns the seams AND the cached
    /// `SeamLimitInfo` so the caller can surface correct `run_status` on
    /// a cache hit. `Miss` is returned for both "no file" and "different
    /// key"; `CorruptIgnored` carries a reason for logs.
    #[cfg(test)]
    pub(crate) fn load_classified_seams(
        &self,
        key: &RepoSeamCacheKey,
    ) -> CacheLoad<(Vec<ClassifiedSeam>, Option<CachedSeamLimitInfo>)> {
        match self.load_classified_seams_with_fallback(key) {
            CacheLoad::Hit((seams, limit_info, _)) => CacheLoad::Hit((seams, limit_info)),
            CacheLoad::Miss => CacheLoad::Miss,
            CacheLoad::CorruptIgnored { reason } => CacheLoad::CorruptIgnored { reason },
        }
    }

    pub(crate) fn load_classified_seams_with_fallback(
        &self,
        key: &RepoSeamCacheKey,
    ) -> CacheLoad<(
        Vec<ClassifiedSeam>,
        Option<CachedSeamLimitInfo>,
        Vec<PathBuf>,
    )> {
        match self.load_single_classified_seams(key) {
            CacheLoad::Miss => self.load_sharded_classified_seams(key),
            other => other,
        }
    }

    fn load_single_classified_seams(
        &self,
        key: &RepoSeamCacheKey,
    ) -> CacheLoad<(
        Vec<ClassifiedSeam>,
        Option<CachedSeamLimitInfo>,
        Vec<PathBuf>,
    )> {
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
                    CacheLoad::Hit((
                        envelope.classified_seams,
                        envelope.seam_limit_info,
                        envelope.lexical_fallback_files,
                    ))
                } else {
                    // Key collision is unlikely (16-char FNV file
                    // names + 12 fields hashed in), but possible. Treat
                    // as miss without failing analysis.
                    CacheLoad::Miss
                }
            }
            Err(reason) => CacheLoad::CorruptIgnored { reason },
        }
    }

    #[cfg(test)]
    pub(crate) fn store_compact_classified_seams_with_limit(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        store_limit: usize,
    ) -> Result<CacheStoreStatus, String> {
        self.store_compact_classified_seams_with_limit_and_fallback(key, seams, &[], store_limit)
    }

    pub(crate) fn store_compact_classified_seams_with_limit_and_fallback(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        lexical_fallback_files: &[PathBuf],
        store_limit: usize,
    ) -> Result<CacheStoreStatus, String> {
        self.store_classified_seams_with_limit_and_fallback(
            key,
            seams,
            None,
            lexical_fallback_files,
            store_limit,
        )
    }

    /// Store classified seams with the given cache-store limit.
    ///
    /// `limit_info` is `Some(...)` when this is a truncated run (seam limit
    /// was applied); `None` for a complete run. The value is persisted in the
    /// cache envelope so a warm-path load can return the correct `run_status`.
    #[cfg(test)]
    pub(crate) fn store_classified_seams_with_limit(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        limit_info: Option<&CachedSeamLimitInfo>,
        store_limit: usize,
    ) -> Result<CacheStoreStatus, String> {
        self.store_classified_seams_with_limit_and_fallback(
            key,
            seams,
            limit_info,
            &[],
            store_limit,
        )
    }

    pub(crate) fn store_classified_seams_with_limit_and_fallback(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        limit_info: Option<&CachedSeamLimitInfo>,
        lexical_fallback_files: &[PathBuf],
        store_limit: usize,
    ) -> Result<CacheStoreStatus, String> {
        if store_limit == 0 {
            return Err("classified seam cache store limit must be positive".to_string());
        }
        if seams.len() > store_limit {
            return self.store_sharded_classified_seams_with_limit(
                key,
                seams,
                limit_info,
                lexical_fallback_files,
                store_limit,
            );
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|err| format!("create cache dir failed: {err}"))?;
        let envelope = CacheEnvelope::new_with_fallback(
            key.clone(),
            seams.to_vec(),
            limit_info.cloned(),
            lexical_fallback_files.to_vec(),
        );
        let bytes = codec::encode(&envelope)?;
        let path = self.entry_path(key);
        crate::atomic_file::write_cache(&path, &bytes, "cache")?;
        Ok(CacheStoreStatus {
            label: "ok".to_string(),
        })
    }

    fn entry_path(&self, key: &RepoSeamCacheKey) -> PathBuf {
        self.dir.join(key.filename())
    }

    fn load_sharded_classified_seams(
        &self,
        key: &RepoSeamCacheKey,
    ) -> CacheLoad<(
        Vec<ClassifiedSeam>,
        Option<CachedSeamLimitInfo>,
        Vec<PathBuf>,
    )> {
        let manifest_path = self.sharded_manifest_path(key);
        let bytes = match std::fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return CacheLoad::Miss,
            Err(err) => {
                return CacheLoad::CorruptIgnored {
                    reason: format!("read sharded manifest failed: {err}"),
                };
            }
        };
        let manifest = match codec::decode_sharded_manifest(&bytes) {
            Ok(manifest) => manifest,
            Err(reason) => return CacheLoad::CorruptIgnored { reason },
        };
        if !manifest.matches_key(key) {
            return CacheLoad::Miss;
        }
        if manifest.shards.is_empty() && manifest.total_seams != 0 {
            return CacheLoad::CorruptIgnored {
                reason: "sharded manifest has no shards for non-empty seam payload".to_string(),
            };
        }
        if manifest.shard_count != manifest.shards.len() {
            return CacheLoad::CorruptIgnored {
                reason: format!(
                    "sharded manifest expected {} shards but listed {}",
                    manifest.shard_count,
                    manifest.shards.len()
                ),
            };
        }

        let mut seams = Vec::with_capacity(manifest.total_seams);
        for (index, shard) in manifest.shards.iter().enumerate() {
            if shard.index != index {
                return CacheLoad::CorruptIgnored {
                    reason: format!(
                        "sharded manifest index mismatch at position {index}: {}",
                        shard.index
                    ),
                };
            }
            let shard_path = self.sharded_entry_dir(key).join(&shard.file);
            let bytes = match std::fs::read(&shard_path) {
                Ok(bytes) => bytes,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    return CacheLoad::CorruptIgnored {
                        reason: format!("missing sharded cache file {}", shard_path.display()),
                    };
                }
                Err(err) => {
                    return CacheLoad::CorruptIgnored {
                        reason: format!("read sharded cache file failed: {err}"),
                    };
                }
            };
            let envelope = match codec::decode_shard(&bytes) {
                Ok(envelope) => envelope,
                Err(reason) => return CacheLoad::CorruptIgnored { reason },
            };
            if !envelope.matches_key(key) {
                return CacheLoad::CorruptIgnored {
                    reason: format!("sharded cache key mismatch in {}", shard.file),
                };
            }
            if envelope.sharded_cache_schema_version != manifest.sharded_cache_schema_version
                || envelope.shard_index != shard.index
                || envelope.shard_count != manifest.shard_count
            {
                return CacheLoad::CorruptIgnored {
                    reason: format!("sharded cache metadata mismatch in {}", shard.file),
                };
            }
            if envelope.classified_seams.len() != shard.seams {
                return CacheLoad::CorruptIgnored {
                    reason: format!(
                        "sharded cache file {} carried {} seams but manifest expected {}",
                        shard.file,
                        envelope.classified_seams.len(),
                        shard.seams
                    ),
                };
            }
            seams.extend(envelope.classified_seams);
        }
        if seams.len() != manifest.total_seams {
            return CacheLoad::CorruptIgnored {
                reason: format!(
                    "sharded cache loaded {} seams but manifest expected {}",
                    seams.len(),
                    manifest.total_seams
                ),
            };
        }
        CacheLoad::Hit((
            seams,
            manifest.seam_limit_info,
            manifest.lexical_fallback_files,
        ))
    }

    fn store_sharded_classified_seams_with_limit(
        &self,
        key: &RepoSeamCacheKey,
        seams: &[ClassifiedSeam],
        limit_info: Option<&CachedSeamLimitInfo>,
        lexical_fallback_files: &[PathBuf],
        store_limit: usize,
    ) -> Result<CacheStoreStatus, String> {
        std::fs::create_dir_all(self.sharded_entry_dir(key))
            .map_err(|err| format!("create sharded cache dir failed: {err}"))?;
        let shard_count = seams.len().div_ceil(store_limit);
        let mut shard_refs = Vec::with_capacity(shard_count);
        for (index, chunk) in seams.chunks(store_limit).enumerate() {
            let file = format!("shard-{index:05}.json");
            let envelope =
                ShardedCacheEnvelope::new(key.clone(), index, shard_count, chunk.to_vec());
            let bytes = codec::encode_shard(&envelope)?;
            let path = self.sharded_entry_dir(key).join(&file);
            crate::atomic_file::write_cache(&path, &bytes, "sharded cache file")?;
            shard_refs.push(ShardedCacheShardRef {
                index,
                file,
                seams: chunk.len(),
            });
        }
        let manifest = ShardedCacheManifest::new(
            key.clone(),
            seams.len(),
            shard_count,
            shard_refs,
            limit_info.cloned(),
            lexical_fallback_files.to_vec(),
        );
        let bytes = codec::encode_sharded_manifest(&manifest)?;
        let manifest_path = self.sharded_manifest_path(key);
        crate::atomic_file::write_cache(&manifest_path, &bytes, "sharded cache manifest")?;
        Ok(CacheStoreStatus {
            label: format!(
                "sharded_ok_seams_{}_shards_{}_limit_{}",
                seams.len(),
                shard_count,
                store_limit
            ),
        })
    }

    fn sharded_entry_dir(&self, key: &RepoSeamCacheKey) -> PathBuf {
        self.sharded_dir
            .join(key.filename().trim_end_matches(".json"))
    }

    fn sharded_manifest_path(&self, key: &RepoSeamCacheKey) -> PathBuf {
        self.sharded_entry_dir(key).join("manifest.json")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FileFactCacheStats {
    pub(crate) hits: usize,
    pub(crate) misses: usize,
    pub(crate) corrupt_ignored: usize,
    pub(crate) stores: usize,
    pub(crate) store_errors: usize,
    /// Files whose current content missed the keyed entry while an older
    /// envelope for the same path was present. This is the narrow, owned
    /// content-invalidation signal; other input families remain explicit
    /// `not_available` limitations until they have equivalent provenance.
    pub(crate) invalidated_files: BTreeSet<PathBuf>,
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
            dir: cache_base_dir(workspace_root)
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

    /// Snapshot paths with valid cached envelopes before a build starts. The
    /// caller uses this set for O(1) miss attribution and deliberately does not
    /// observe entries created during the same build.
    pub(crate) fn known_file_paths(&self) -> HashSet<PathBuf> {
        let mut paths = HashSet::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return paths;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            if let Ok(envelope) = codec::decode_file_facts(&bytes) {
                paths.insert(envelope.file_path);
            }
        }
        paths
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
        crate::atomic_file::write_cache(&self.entry_path(key), &bytes, "file fact cache")?;
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
    /// `target/ripr/cache/...` (or `RIPR_CACHE_DIR` when set).
    pub(crate) fn at(workspace_root: &Path) -> Self {
        Self {
            dir: cache_base_dir(workspace_root)
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
        crate::atomic_file::write_cache(&path, &bytes, "count cache")?;
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
    workspace_manifests_hash: String,
    lockfile_hash: String,
    toolchain_hash: String,
    classified_seams: Vec<ClassifiedSeam>,
    /// `None` means this is a complete run (all seams were analyzed).
    /// `Some(...)` means the run was capped; the renderer uses this to
    /// emit `run_status: "seam_limit_applied"` on a cache hit.
    /// `#[serde(default)]` ensures old cache entries without this field
    /// deserialize as `None` (correct: pre-Slice-B caches were full runs).
    #[serde(default)]
    seam_limit_info: Option<CachedSeamLimitInfo>,
    #[serde(default)]
    lexical_fallback_files: Vec<PathBuf>,
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
    workspace_manifests_hash: String,
    lockfile_hash: String,
    toolchain_hash: String,
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
            workspace_manifests_hash: key.workspace_manifests_hash,
            lockfile_hash: key.lockfile_hash,
            toolchain_hash: key.toolchain_hash,
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
            && self.workspace_manifests_hash == key.workspace_manifests_hash
            && self.lockfile_hash == key.lockfile_hash
            && self.toolchain_hash == key.toolchain_hash
    }
}

impl CacheEnvelope {
    #[cfg(test)]
    fn new(
        key: RepoSeamCacheKey,
        classified_seams: Vec<ClassifiedSeam>,
        seam_limit_info: Option<CachedSeamLimitInfo>,
    ) -> Self {
        Self::new_with_fallback(key, classified_seams, seam_limit_info, Vec::new())
    }

    fn new_with_fallback(
        key: RepoSeamCacheKey,
        classified_seams: Vec<ClassifiedSeam>,
        seam_limit_info: Option<CachedSeamLimitInfo>,
        lexical_fallback_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            workspace_root_hash: key.workspace_root_hash,
            files_content_hash: key.files_content_hash,
            cfg_features_hash: key.cfg_features_hash,
            config_hash: key.config_hash,
            test_intent_hash: key.test_intent_hash,
            suppressions_hash: key.suppressions_hash,
            workspace_manifests_hash: key.workspace_manifests_hash,
            lockfile_hash: key.lockfile_hash,
            toolchain_hash: key.toolchain_hash,
            classified_seams,
            seam_limit_info,
            lexical_fallback_files,
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
            && self.workspace_manifests_hash == key.workspace_manifests_hash
            && self.lockfile_hash == key.lockfile_hash
            && self.toolchain_hash == key.toolchain_hash
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ShardedCacheManifest {
    sharded_cache_schema_version: String,
    schema_version: String,
    analyzer_version: String,
    workspace_root_hash: String,
    files_content_hash: String,
    cfg_features_hash: String,
    config_hash: String,
    test_intent_hash: String,
    suppressions_hash: String,
    workspace_manifests_hash: String,
    lockfile_hash: String,
    toolchain_hash: String,
    total_seams: usize,
    shard_count: usize,
    shards: Vec<ShardedCacheShardRef>,
    /// See `CacheEnvelope::seam_limit_info`. `#[serde(default)]` provides
    /// backward-compat with pre-Slice-B manifests.
    #[serde(default)]
    seam_limit_info: Option<CachedSeamLimitInfo>,
    #[serde(default)]
    lexical_fallback_files: Vec<PathBuf>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ShardedCacheShardRef {
    index: usize,
    file: String,
    seams: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ShardedCacheEnvelope {
    sharded_cache_schema_version: String,
    schema_version: String,
    analyzer_version: String,
    workspace_root_hash: String,
    files_content_hash: String,
    cfg_features_hash: String,
    config_hash: String,
    test_intent_hash: String,
    suppressions_hash: String,
    workspace_manifests_hash: String,
    lockfile_hash: String,
    toolchain_hash: String,
    shard_index: usize,
    shard_count: usize,
    classified_seams: Vec<ClassifiedSeam>,
}

impl ShardedCacheManifest {
    fn new(
        key: RepoSeamCacheKey,
        total_seams: usize,
        shard_count: usize,
        shards: Vec<ShardedCacheShardRef>,
        seam_limit_info: Option<CachedSeamLimitInfo>,
        lexical_fallback_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            sharded_cache_schema_version: SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION.to_string(),
            schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            workspace_root_hash: key.workspace_root_hash,
            files_content_hash: key.files_content_hash,
            cfg_features_hash: key.cfg_features_hash,
            config_hash: key.config_hash,
            test_intent_hash: key.test_intent_hash,
            suppressions_hash: key.suppressions_hash,
            workspace_manifests_hash: key.workspace_manifests_hash,
            lockfile_hash: key.lockfile_hash,
            toolchain_hash: key.toolchain_hash,
            total_seams,
            shard_count,
            shards,
            seam_limit_info,
            lexical_fallback_files,
        }
    }

    fn matches_key(&self, key: &RepoSeamCacheKey) -> bool {
        self.sharded_cache_schema_version == SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION
            && self.schema_version == key.schema_version
            && self.analyzer_version == key.analyzer_version
            && self.workspace_root_hash == key.workspace_root_hash
            && self.files_content_hash == key.files_content_hash
            && self.cfg_features_hash == key.cfg_features_hash
            && self.config_hash == key.config_hash
            && self.test_intent_hash == key.test_intent_hash
            && self.suppressions_hash == key.suppressions_hash
            && self.workspace_manifests_hash == key.workspace_manifests_hash
            && self.lockfile_hash == key.lockfile_hash
            && self.toolchain_hash == key.toolchain_hash
    }
}

impl ShardedCacheEnvelope {
    fn new(
        key: RepoSeamCacheKey,
        shard_index: usize,
        shard_count: usize,
        classified_seams: Vec<ClassifiedSeam>,
    ) -> Self {
        Self {
            sharded_cache_schema_version: SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION.to_string(),
            schema_version: key.schema_version,
            analyzer_version: key.analyzer_version,
            workspace_root_hash: key.workspace_root_hash,
            files_content_hash: key.files_content_hash,
            cfg_features_hash: key.cfg_features_hash,
            config_hash: key.config_hash,
            test_intent_hash: key.test_intent_hash,
            suppressions_hash: key.suppressions_hash,
            workspace_manifests_hash: key.workspace_manifests_hash,
            lockfile_hash: key.lockfile_hash,
            toolchain_hash: key.toolchain_hash,
            shard_index,
            shard_count,
            classified_seams,
        }
    }

    fn matches_key(&self, key: &RepoSeamCacheKey) -> bool {
        self.sharded_cache_schema_version == SHARDED_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION
            && self.schema_version == key.schema_version
            && self.analyzer_version == key.analyzer_version
            && self.workspace_root_hash == key.workspace_root_hash
            && self.files_content_hash == key.files_content_hash
            && self.cfg_features_hash == key.cfg_features_hash
            && self.config_hash == key.config_hash
            && self.test_intent_hash == key.test_intent_hash
            && self.suppressions_hash == key.suppressions_hash
            && self.workspace_manifests_hash == key.workspace_manifests_hash
            && self.lockfile_hash == key.lockfile_hash
            && self.toolchain_hash == key.toolchain_hash
    }
}

/// Codec module — the only place serialization format is decided.
/// Switching to `postcard` for binary v2 is a localized change here.
mod codec {
    #[cfg(test)]
    use super::CountCacheEnvelope;
    use super::{
        CacheEnvelope, CorpusFingerprintEnvelope, FileFactCacheEnvelope, ShardedCacheEnvelope,
        ShardedCacheManifest,
    };

    pub(super) fn encode(envelope: &CacheEnvelope) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope).map_err(|err| format!("encode failed: {err}"))
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<CacheEnvelope, String> {
        serde_json::from_slice(bytes).map_err(|err| format!("decode failed: {err}"))
    }

    pub(super) fn encode_sharded_manifest(
        manifest: &ShardedCacheManifest,
    ) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(manifest)
            .map_err(|err| format!("encode sharded manifest failed: {err}"))
    }

    pub(super) fn decode_sharded_manifest(bytes: &[u8]) -> Result<ShardedCacheManifest, String> {
        serde_json::from_slice(bytes)
            .map_err(|err| format!("decode sharded manifest failed: {err}"))
    }

    pub(super) fn encode_shard(envelope: &ShardedCacheEnvelope) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope)
            .map_err(|err| format!("encode sharded cache file failed: {err}"))
    }

    pub(super) fn decode_shard(bytes: &[u8]) -> Result<ShardedCacheEnvelope, String> {
        serde_json::from_slice(bytes)
            .map_err(|err| format!("decode sharded cache file failed: {err}"))
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

    pub(super) fn encode_corpus_fingerprint(
        envelope: &CorpusFingerprintEnvelope,
    ) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(envelope)
            .map_err(|err| format!("encode corpus fingerprint failed: {err}"))
    }

    pub(super) fn decode_corpus_fingerprint(
        bytes: &[u8],
    ) -> Result<CorpusFingerprintEnvelope, String> {
        serde_json::from_slice(bytes)
            .map_err(|err| format!("decode corpus fingerprint failed: {err}"))
    }
}

fn hash_str(s: &str) -> String {
    hash_bytes(s.as_bytes())
}

pub(crate) fn stable_input_hash(bytes: &[u8]) -> String {
    hash_bytes(bytes)
}

/// Producer-owned provenance for the local Cargo package and feature graphs.
/// External dependency metadata is never resolved here: targeted reruns must
/// not perform network work or imply that registry graph facts were observed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkspaceGraphProvenance {
    pub(crate) package_graph_status: String,
    pub(crate) package_graph_hash: Option<String>,
    pub(crate) package_graph_detail: Option<String>,
    pub(crate) feature_graph_status: String,
    pub(crate) feature_graph_hash: Option<String>,
    pub(crate) feature_graph_detail: Option<String>,
    pub(crate) external_dependency_graph_status: String,
    pub(crate) external_dependency_graph_detail: String,
    /// #2968: path dependencies discovered in Cargo.toml `[dependencies]`
    /// tables with `path = "..."` values. Each entry is `(from_manifest, dep_name, resolved_path)`.
    /// Preserved but not yet walked (#2969/#2970 consume this).
    pub(crate) path_dependency_edges: Vec<(String, String, String)>,
}

/// Read local Cargo manifests and derive deterministic package/feature graph
/// facts without invoking Cargo, rustc, a registry, or a network client.
pub(crate) fn workspace_graph_provenance(root: &Path) -> WorkspaceGraphProvenance {
    let mut manifests = Vec::new();
    collect_named_workspace_files(root, root, "Cargo.toml", &mut manifests);
    manifests.sort_by(|left, right| left.0.cmp(&right.0));

    if manifests.is_empty() {
        return WorkspaceGraphProvenance {
            package_graph_status: "unavailable".to_string(),
            package_graph_detail: Some("no local Cargo.toml manifest was found".to_string()),
            feature_graph_status: "unavailable".to_string(),
            feature_graph_detail: Some("no local Cargo.toml manifest was found".to_string()),
            external_dependency_graph_status: "unavailable".to_string(),
            external_dependency_graph_detail:
                "external dependency metadata is not resolved; no network access was used"
                    .to_string(),
            path_dependency_edges: Vec::new(),
            ..WorkspaceGraphProvenance::default()
        };
    }

    let mut package_facts = Vec::new();
    let mut path_dep_edges: Vec<(String, String, String)> = Vec::new();
    let mut feature_facts = Vec::new();
    let mut parse_errors = Vec::new();
    for (path, bytes) in &manifests {
        let path_text = path.to_string_lossy().replace('\\', "/");
        let value = match std::str::from_utf8(bytes)
            .map_err(|err| err.to_string())
            .and_then(|text| toml::from_str::<toml::Value>(text).map_err(|err| err.to_string()))
        {
            Ok(value) => value,
            Err(err) => {
                parse_errors.push(format!("{path_text}: {err}"));
                continue;
            }
        };
        let package = value.get("package").and_then(toml::Value::as_table);
        let package_name = package
            .and_then(|table| table.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or("<workspace-only>");
        let workspace_members = value
            .get("workspace")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("members"))
            .map(canonical_toml_value)
            .unwrap_or_default();
        let dependencies = ["dependencies", "dev-dependencies", "build-dependencies"]
            .into_iter()
            .filter_map(|section| value.get(section).and_then(toml::Value::as_table))
            .flat_map(|table| table.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        // #2968: also preserve path-dep values so #2969 can build the adjacency graph.
        let manifest_dir = path.parent().unwrap_or(Path::new("."));
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = value.get(section).and_then(toml::Value::as_table) {
                for (dep_name, dep_value) in table {
                    if let Some(path_str) = dep_value
                        .as_table()
                        .and_then(|t| t.get("path"))
                        .and_then(toml::Value::as_str)
                    {
                        let resolved = manifest_dir.join(path_str);
                        let resolved_str = resolved.to_string_lossy().replace('\\', "/");
                        path_dep_edges.push((path_text.clone(), dep_name.clone(), resolved_str));
                    }
                }
            }
        }
        package_facts.push(format!(
            "{path_text}\0package={package_name}\0members={workspace_members}\0dependencies={dependencies:?}"
        ));

        let feature_values = value
            .get("features")
            .and_then(toml::Value::as_table)
            .map(|table| {
                table
                    .iter()
                    .map(|(name, value)| format!("{name}={}", canonical_toml_value(value)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        feature_facts.push(format!("{path_text}\0features={feature_values:?}"));
    }

    let parse_detail = (!parse_errors.is_empty()).then(|| parse_errors.join("; "));
    let package_graph_status = if package_facts.is_empty() {
        "unavailable"
    } else if parse_detail.is_some() {
        "limited"
    } else {
        "complete"
    };
    let feature_graph_status = if parse_detail.is_some() {
        "limited"
    } else {
        "complete"
    };
    package_facts.sort();
    feature_facts.sort();
    WorkspaceGraphProvenance {
        package_graph_status: package_graph_status.to_string(),
        package_graph_hash: (!package_facts.is_empty())
            .then(|| stable_input_hash(package_facts.join("\n").as_bytes())),
        package_graph_detail: parse_detail.clone(),
        feature_graph_status: feature_graph_status.to_string(),
        feature_graph_hash: Some(stable_input_hash(feature_facts.join("\n").as_bytes())),
        feature_graph_detail: parse_detail,
        external_dependency_graph_status: "unavailable".to_string(),
        external_dependency_graph_detail:
            "external dependency metadata is not resolved; no network access was used".to_string(),
        path_dependency_edges: path_dep_edges,
    }
}

fn canonical_toml_value(value: &toml::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a_64(bytes))
}

fn hash_named_workspace_files(root: &Path, file_name: &str) -> String {
    workspace_named_file_identity(root, file_name)
        .unwrap_or_else(|| hash_str("<no matching workspace files>"))
}

/// Return the deterministic identity of all matching workspace files.
///
/// LSP input identity uses the same path-and-content boundary as the seam
/// cache so a manifest or lockfile change cannot be treated as equivalent to
/// the previous analysis input. `None` means no matching workspace file was
/// found; unreadable files retain the seam-cache placeholder behavior.
pub(crate) fn workspace_named_file_identity(root: &Path, file_name: &str) -> Option<String> {
    let mut files = Vec::new();
    collect_named_workspace_files(root, root, file_name, &mut files);
    workspace_file_identity(files)
}

/// Return the Cargo manifest and lockfile identities with one workspace walk.
///
/// Refresh scheduling runs this on every analysis request. Keeping the two
/// identities on the same traversal avoids doubling blocking filesystem work
/// on the interactive path while preserving each per-name identity boundary.
pub(crate) fn workspace_named_file_identities(root: &Path) -> (Option<String>, Option<String>) {
    let mut files = [Vec::new(), Vec::new()];
    collect_named_workspace_files_by_name(root, root, &mut files);
    let [manifest_files, lockfile_files] = files;
    (
        workspace_file_identity(manifest_files),
        workspace_file_identity(lockfile_files),
    )
}

/// Fail-closed portable variant for the repo-exposure artifact input identity
/// (#2823). The collector strips the checkout root from each collected path
/// (falling back to the absolute spelling on a strip miss), so the ordinary
/// identity above is root-relative in practice but can silently reintroduce a
/// root-bound absolute path on a miss. This variant instead degrades the
/// whole identity to `None` when any collected path is still absolute after
/// collection — a `None` renders into the caller's canonical string as a
/// stable, root-independent placeholder, never as checkout-instance evidence.
pub(crate) fn workspace_named_file_identities_relative(
    root: &Path,
) -> (Option<String>, Option<String>) {
    let mut files = [Vec::new(), Vec::new()];
    collect_named_workspace_files_by_name(root, root, &mut files);
    let [manifest_files, lockfile_files] = files;
    (
        workspace_file_identity_portable(manifest_files),
        workspace_file_identity_portable(lockfile_files),
    )
}

/// Portable identity: collected paths must already be root-relative. A path
/// that is still absolute means the collector's root strip missed; fail
/// closed rather than hash the absolute — and therefore root-bound —
/// spelling into a supposedly portable identity.
fn workspace_file_identity_portable(files: Vec<(PathBuf, Vec<u8>)>) -> Option<String> {
    if files.iter().any(|(path, _)| path.is_absolute()) {
        return None;
    }
    workspace_file_identity(files)
}

fn workspace_file_identity(mut files: Vec<(PathBuf, Vec<u8>)>) -> Option<String> {
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut input = String::new();
    for (path, bytes) in files {
        input.push_str(&path.to_string_lossy().replace('\\', "/"));
        input.push('\0');
        input.push_str(&hash_bytes(&bytes));
        input.push('\n');
    }
    (!input.is_empty()).then(|| hash_str(&input))
}

fn collect_named_workspace_files_by_name(
    root: &Path,
    directory: &Path,
    files: &mut [Vec<(PathBuf, Vec<u8>)>; 2],
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            if matches!(
                name,
                ".git" | ".ripr" | "target" | "fixtures" | ".direnv" | "node_modules"
            ) {
                continue;
            }
            collect_named_workspace_files_by_name(root, &path, files);
        } else if matches!(name, "Cargo.toml" | "Cargo.lock") {
            let index = usize::from(name == "Cargo.lock");
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let bytes =
                std::fs::read(&path).unwrap_or_else(|_| b"<workspace input unreadable>".to_vec());
            files[index].push((relative, bytes));
        }
    }
}

fn collect_named_workspace_files(
    root: &Path,
    directory: &Path,
    file_name: &str,
    files: &mut Vec<(PathBuf, Vec<u8>)>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            if matches!(
                name,
                ".git" | ".ripr" | "target" | "fixtures" | ".direnv" | "node_modules"
            ) {
                continue;
            }
            collect_named_workspace_files(root, &path, file_name, files);
        } else if name == file_name {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let bytes =
                std::fs::read(&path).unwrap_or_else(|_| b"<workspace input unreadable>".to_vec());
            files.push((relative, bytes));
        }
    }
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
            .store_classified_seams_with_limit(
                &key,
                &seams,
                None,
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store should succeed: {err}"))?;
        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit((loaded, limit_info)) => {
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
                } else if limit_info.is_some() {
                    Err(format!(
                        "complete run should store None limit_info, got {limit_info:?}"
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
    fn given_fallback_cache_entry_when_warm_load_runs_then_provenance_is_replayed()
    -> Result<(), String> {
        let dir = isolated_dir("fallback-provenance");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let fallback_files = vec![PathBuf::from("src/z.rs"), PathBuf::from("src/a.rs")];
        cache
            .store_classified_seams_with_limit_and_fallback(
                &key,
                &[sample_classified()],
                None,
                &fallback_files,
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store fallback provenance: {err}"))?;

        let result = match cache.load_classified_seams_with_fallback(&key) {
            CacheLoad::Hit((_, None, loaded_files)) => {
                if loaded_files != fallback_files {
                    Err(format!(
                        "warm cache must replay fallback files, got {loaded_files:?}"
                    ))
                } else {
                    Ok(())
                }
            }
            other => Err(format!(
                "expected fallback provenance cache hit, got {other:?}"
            )),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn given_large_classified_entry_when_cache_store_runs_then_shards_are_written()
    -> Result<(), String> {
        let dir = isolated_dir("large-shard");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];
        let status = cache
            .store_classified_seams_with_limit(&key, &seams, None, 1)
            .map_err(|err| format!("large classified seam cache should shard: {err}"))?;

        assert_eq!(status.label, "sharded_ok_seams_2_shards_2_limit_1");
        assert!(
            !cache.entry_path(&key).exists(),
            "sharded cache store should not write a monolithic classified seam entry"
        );
        assert!(
            cache.sharded_manifest_path(&key).exists(),
            "sharded cache store should write a manifest"
        );
        assert!(
            cache
                .sharded_entry_dir(&key)
                .join("shard-00000.json")
                .exists(),
            "sharded cache store should write the first shard"
        );
        assert!(
            cache
                .sharded_entry_dir(&key)
                .join("shard-00001.json")
                .exists(),
            "sharded cache store should write the second shard"
        );

        match cache.load_classified_seams(&key) {
            CacheLoad::Hit((loaded, _)) if loaded.len() == 2 => {}
            other => return Err(format!("expected sharded cache hit, got {other:?}")),
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn classified_cache_store_limit_rejects_zero_direct_limit() -> Result<(), String> {
        let dir = isolated_dir("zero-limit");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let err =
            match cache.store_classified_seams_with_limit(&key, &[sample_classified()], None, 0) {
                Ok(status) => {
                    return Err(format!(
                        "zero direct cache limit should fail, got {}",
                        status.label
                    ));
                }
                Err(err) => err,
            };

        assert!(
            err.contains("positive"),
            "zero direct cache limit should produce positive-limit diagnostic: {err}"
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
            .store_classified_seams_with_limit(&key, &seams, None, 2)
            .map_err(|err| format!("raised classified cache limit should allow store: {err}"))?;

        match cache.load_classified_seams(&key) {
            CacheLoad::Hit((loaded, _)) if loaded.len() == 2 => {}
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
    fn compact_cache_store_limit_controls_shard_size() -> Result<(), String> {
        let dir = isolated_dir("compact-large-shard");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];

        let status = cache
            .store_compact_classified_seams_with_limit(&key, &seams, 1)
            .map_err(|err| format!("configured compact cache limit should shard: {err}"))?;

        assert_eq!(status.label, "sharded_ok_seams_2_shards_2_limit_1");
        assert!(
            !cache.entry_path(&key).exists(),
            "sharded compact cache store should not write a monolithic entry"
        );
        match cache.load_classified_seams(&key) {
            CacheLoad::Hit((loaded, _)) if loaded.len() == 2 => {}
            other => return Err(format!("expected compact sharded cache hit: {other:?}")),
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn given_missing_sharded_cache_file_when_loading_then_corrupt_ignored_is_reported()
    -> Result<(), String> {
        let dir = isolated_dir("missing-shard");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified(); 2];

        cache
            .store_classified_seams_with_limit(&key, &seams, None, 1)
            .map_err(|err| format!("large classified seam cache should shard: {err}"))?;
        std::fs::remove_file(cache.sharded_entry_dir(&key).join("shard-00001.json"))
            .map_err(|err| format!("remove shard fixture: {err}"))?;

        match cache.load_classified_seams(&key) {
            CacheLoad::CorruptIgnored { reason } => {
                assert!(
                    reason.contains("missing sharded cache file"),
                    "missing shard should be named in corrupt reason: {reason}"
                );
            }
            other => {
                return Err(format!(
                    "expected CorruptIgnored for missing sharded cache file, got {other:?}"
                ));
            }
        }

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
            CacheLoad::Hit((loaded, _)) if loaded.len() == 2 => {}
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
                None,
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
    fn workspace_manifest_and_lockfile_changes_change_cache_identity() -> Result<(), String> {
        let root = isolated_dir("workspace-inputs");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|err| format!("create workspace: {err}"))?;
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .map_err(|err| format!("write manifest: {err}"))?;
        std::fs::write(root.join("Cargo.lock"), "version = 4\n")
            .map_err(|err| format!("write lockfile: {err}"))?;
        let files: [(PathBuf, Vec<u8>); 0] = [];
        let baseline = WorkspaceState {
            workspace_root: &root,
            files: &files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();

        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate\"]\n",
        )
        .map_err(|err| format!("change manifest: {err}"))?;
        std::fs::write(root.join("Cargo.lock"), "version = 4\n# changed\n")
            .map_err(|err| format!("change lockfile: {err}"))?;
        let updated = WorkspaceState {
            workspace_root: &root,
            files: &files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key();

        assert_ne!(
            baseline.workspace_manifests_hash,
            updated.workspace_manifests_hash
        );
        assert_ne!(baseline.lockfile_hash, updated.lockfile_hash);
        assert_ne!(baseline.filename(), updated.filename());
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// (#2823) The portable identity fails closed: collected paths are
    /// root-relative by construction (the collector strips the checkout
    /// root), so a path that is still absolute means the strip missed, and
    /// the whole identity must degrade to `None` instead of hashing the
    /// root-bound absolute spelling into a supposedly portable identity.
    #[test]
    fn workspace_relative_file_identity_fails_closed_on_strip_miss() -> Result<(), String> {
        let relative_files = vec![(PathBuf::from("Cargo.toml"), b"[workspace]\n".to_vec())];
        // The plain identity accepts whatever spelling it is given (that is
        // its contract for a single live checkout).
        if workspace_file_identity(relative_files.clone()).is_none() {
            return Err("the plain identity must hash relative paths".to_string());
        }
        let portable = workspace_file_identity_portable(relative_files.clone());
        if portable != workspace_file_identity(relative_files) {
            return Err(
                "root-relative collected paths must produce the same portable and plain identities"
                    .to_string(),
            );
        }
        // An absolute collected path — the strip-miss shape — must NOT fall
        // back to an absolute-path identity: the whole computation degrades.
        let strip_missed = vec![(
            PathBuf::from("/checkout/a/Cargo.toml"),
            b"[workspace]\n".to_vec(),
        )];
        if workspace_file_identity_portable(strip_missed.clone()).is_some() {
            return Err(
                "a still-absolute collected path must degrade the portable identity to None, not an absolute-path identity"
                    .to_string(),
            );
        }
        if workspace_file_identity_portable(strip_missed.clone())
            == workspace_file_identity(strip_missed)
        {
            return Err(
                "a strip miss must not silently reproduce the absolute-path identity".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn graph_provenance_reports_local_package_and_feature_facts_without_network()
    -> Result<(), String> {
        let root = isolated_dir("graph-provenance");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("crates/app"))
            .map_err(|err| format!("create workspace: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/app\"]\n",
        )
        .map_err(|err| format!("write root manifest: {err}"))?;
        std::fs::write(
            root.join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[features]\ndefault = []\nfast = []\n",
        )
        .map_err(|err| format!("write member manifest: {err}"))?;

        let first = workspace_graph_provenance(&root);
        assert_eq!(first.package_graph_status, "complete");
        assert_eq!(first.feature_graph_status, "complete");
        assert!(first.package_graph_hash.is_some());
        assert!(first.feature_graph_hash.is_some());
        assert_eq!(first.external_dependency_graph_status, "unavailable");
        assert!(
            first
                .external_dependency_graph_detail
                .contains("no network")
        );

        std::fs::write(
            root.join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[features]\ndefault = []\nslow = []\n",
        )
        .map_err(|err| format!("change feature manifest: {err}"))?;
        let second = workspace_graph_provenance(&root);
        assert_ne!(first.feature_graph_hash, second.feature_graph_hash);
        assert_eq!(first.package_graph_hash, second.package_graph_hash);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn graph_provenance_names_unavailable_or_malformed_manifests() -> Result<(), String> {
        let root = isolated_dir("graph-provenance-limited");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|err| format!("create workspace: {err}"))?;
        let missing = workspace_graph_provenance(&root);
        assert_eq!(missing.package_graph_status, "unavailable");
        assert_eq!(missing.feature_graph_status, "unavailable");

        std::fs::write(root.join("Cargo.toml"), "[package\nname = \"broken\"\n")
            .map_err(|err| format!("write malformed manifest: {err}"))?;
        let malformed = workspace_graph_provenance(&root);
        assert_eq!(malformed.package_graph_status, "unavailable");
        assert_eq!(malformed.feature_graph_status, "limited");
        assert!(malformed.package_graph_detail.is_some());
        assert!(malformed.feature_graph_detail.is_some());

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
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
                None,
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store under key_a: {err}"))?;
        // Write key_a's envelope under key_b's filename — simulates a
        // hash collision or stale entry.
        let envelope = CacheEnvelope::new(key_a.clone(), vec![sample_classified()], None);
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
        if !cache.known_file_paths().contains(&path) {
            return Err("changed content should identify a prior same-path version".to_string());
        }

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
            invalidated_files: BTreeSet::new(),
        };
        assert_eq!(
            stats.status_label(),
            "hits_2_misses_3_corrupt_1_store_errors_0"
        );
    }

    #[test]
    fn cache_base_dir_returns_default_when_env_is_unset() {
        let root = PathBuf::from("/some/workspace");
        let result = cache_base_dir_from_env(&root, Err(std::env::VarError::NotPresent));
        assert_eq!(
            result,
            root.join("target").join("ripr").join("cache"),
            "unset env must return default cache base"
        );
    }

    #[test]
    fn cache_base_dir_returns_default_when_env_is_empty() {
        let root = PathBuf::from("/some/workspace");
        let result = cache_base_dir_from_env(&root, Ok(String::new()));
        assert_eq!(
            result,
            root.join("target").join("ripr").join("cache"),
            "empty RIPR_CACHE_DIR must return default cache base"
        );
    }

    #[test]
    fn cache_base_dir_returns_env_value_when_set() {
        let root = PathBuf::from("/some/workspace");
        let override_dir = "/tmp/my-ripr-cache";
        let result = cache_base_dir_from_env(&root, Ok(override_dir.to_string()));
        assert_eq!(
            result,
            PathBuf::from(override_dir),
            "non-empty RIPR_CACHE_DIR must override the default cache base"
        );
    }

    #[test]
    fn cache_base_dir_trims_whitespace_from_env_value() {
        let root = PathBuf::from("/some/workspace");
        let result = cache_base_dir_from_env(&root, Ok("  /tmp/trimmed-cache  ".to_string()));
        assert_eq!(
            result,
            PathBuf::from("/tmp/trimmed-cache"),
            "RIPR_CACHE_DIR value must be trimmed before use"
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

    // ---- cache envelope + limit_info round-trip tests (Slice B) -----------

    #[test]
    fn cache_envelope_with_limit_info_round_trips() -> Result<(), String> {
        let dir = isolated_dir("envelope-limit-info");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified()];
        let limit_info = CachedSeamLimitInfo {
            analyzed: 1,
            total: 5,
            source: SeamLimitSource::Configured,
        };
        cache
            .store_classified_seams_with_limit(
                &key,
                &seams,
                Some(&limit_info),
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store with limit_info should succeed: {err}"))?;

        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit((loaded, loaded_limit)) => {
                if loaded.len() != seams.len() {
                    Err(format!(
                        "seam count mismatch: {} vs {}",
                        loaded.len(),
                        seams.len()
                    ))
                } else {
                    match loaded_limit {
                        None => Err("expected Some(limit_info) on hit, got None".to_string()),
                        Some(li) => {
                            if li.analyzed != limit_info.analyzed {
                                Err(format!(
                                    "analyzed mismatch: {} vs {}",
                                    li.analyzed, limit_info.analyzed
                                ))
                            } else if li.total != limit_info.total {
                                Err(format!(
                                    "total mismatch: {} vs {}",
                                    li.total, limit_info.total
                                ))
                            } else if li.source.as_str() != limit_info.source.as_str() {
                                Err(format!(
                                    "source mismatch: {} vs {}",
                                    li.source.as_str(),
                                    limit_info.source.as_str()
                                ))
                            } else {
                                Ok(())
                            }
                        }
                    }
                }
            }
            other => Err(format!("expected Hit with limit_info, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn cache_envelope_missing_limit_info_field_deserializes_as_none() -> Result<(), String> {
        // Simulate a pre-Slice-B cache file (no `seam_limit_info` field):
        // it should deserialize cleanly with seam_limit_info = None.
        let dir = isolated_dir("envelope-compat");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified()];

        // Store normally (no limit_info → None).
        cache
            .store_classified_seams_with_limit(
                &key,
                &seams,
                None,
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store: {err}"))?;

        // Load the raw JSON and strip out the seam_limit_info key to simulate
        // an old cache entry that never had the field.
        let entry_path = cache.entry_path(&key);
        let raw = std::fs::read(&entry_path).map_err(|err| format!("read entry: {err}"))?;
        let mut json_val: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|err| format!("parse entry: {err}"))?;
        json_val
            .as_object_mut()
            .map(|m| m.remove("seam_limit_info"));
        let rewritten = serde_json::to_vec(&json_val).map_err(|err| format!("re-encode: {err}"))?;
        std::fs::write(&entry_path, rewritten).map_err(|err| format!("rewrite: {err}"))?;

        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit((_, limit_info)) => {
                if limit_info.is_some() {
                    Err(format!(
                        "old cache entry without seam_limit_info should deserialize as None, got {limit_info:?}"
                    ))
                } else {
                    Ok(())
                }
            }
            other => Err(format!("expected Hit on compat cache entry, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn complete_run_stores_none_limit_info_in_envelope() -> Result<(), String> {
        let dir = isolated_dir("envelope-complete");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = RepoSeamFactCache::at_dir(dir.clone());
        let key = empty_state().cache_key();
        let seams = vec![sample_classified()];

        cache
            .store_classified_seams_with_limit(
                &key,
                &seams,
                None,
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store complete: {err}"))?;

        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit((_, None)) => Ok(()),
            CacheLoad::Hit((_, Some(info))) => Err(format!(
                "complete run should return None limit_info, got analyzed={} total={}",
                info.analyzed, info.total
            )),
            other => Err(format!("expected Hit on complete cache, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    // ---- corpus fingerprint cache tests (issue #2108) -------------------

    fn write_corpus_file(root: &Path, relative: &str, content: &str) -> Result<PathBuf, String> {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("mkdir: {err}"))?;
        }
        std::fs::write(&path, content).map_err(|err| format!("write {relative}: {err}"))?;
        Ok(PathBuf::from(relative))
    }

    /// Rewrite `path` with identical content until the inode change time
    /// advances, so subsequent ctime assertions hold on filesystems with
    /// coarse timestamp granularity (~1ms is common; some fuse/overlay or
    /// FAT-like filesystems are coarser). Bounded so a filesystem that
    /// never bumps ctime fails the test loudly instead of hanging.
    #[cfg(unix)]
    fn wait_for_ctime_tick(path: &Path) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        let reference = std::fs::metadata(path)
            .map(|m| (m.ctime(), m.ctime_nsec()))
            .map_err(|err| format!("stat ctime: {err}"))?;
        let content = std::fs::read(path).map_err(|err| format!("read for tick: {err}"))?;
        for _ in 0..1_000 {
            std::fs::write(path, &content).map_err(|err| format!("tick rewrite: {err}"))?;
            let current = std::fs::metadata(path)
                .map(|m| (m.ctime(), m.ctime_nsec()))
                .map_err(|err| format!("stat ctime: {err}"))?;
            if current != reference {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Err("filesystem ctime never advanced; ctime assertions cannot run here".to_string())
    }

    #[test]
    fn corpus_fingerprint_is_stable_for_unchanged_files() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-stable");
        let _ = std::fs::remove_dir_all(&dir);
        let a = write_corpus_file(&dir, "src/a.rs", "pub fn a() -> i32 { 1 }\n")?;
        let b = write_corpus_file(&dir, "src/b.rs", "pub fn b() -> i32 { 2 }\n")?;

        let first = corpus_fingerprint(&dir, &[a.clone(), b.clone()]);
        let second = corpus_fingerprint(&dir, &[b, a]);
        assert_eq!(
            first, second,
            "fingerprint must not depend on discovery order"
        );
        assert!(first.is_some(), "fingerprint should be stat-able");

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn corpus_fingerprint_changes_with_size_or_mtime_or_path_set() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-drift");
        let _ = std::fs::remove_dir_all(&dir);
        let a = write_corpus_file(&dir, "src/a.rs", "pub fn a() -> i32 { 1 }\n")?;
        let baseline = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("baseline fingerprint should compute")?;

        // Size change (mtime preserved via set_modified): must invalidate.
        let original_mtime = std::fs::metadata(dir.join(&a))
            .and_then(|m| m.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        std::fs::write(dir.join(&a), "pub fn a() -> i32 { 12345 }\n// padding\n")
            .map_err(|err| format!("rewrite: {err}"))?;
        let file = std::fs::File::options()
            .write(true)
            .open(dir.join(&a))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;
        let size_changed = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("size-changed fingerprint should compute")?;
        assert_ne!(
            baseline, size_changed,
            "size change with preserved mtime must change the fingerprint"
        );

        // Same size, bumped mtime: must invalidate. The ctime tick wait
        // first guarantees the unix ctime has advanced even on filesystems
        // with coarse timestamp granularity, so the unix assertion below
        // is deterministic.
        #[cfg(unix)]
        wait_for_ctime_tick(&dir.join(&a))?;
        std::fs::write(dir.join(&a), "pub fn a() -> i32 { 1 }\n")
            .map_err(|err| format!("restore: {err}"))?;
        let restored = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("restored fingerprint should compute")?;
        assert_ne!(
            size_changed, restored,
            "restoring the original content changes the size back"
        );
        let file = std::fs::File::options()
            .write(true)
            .open(dir.join(&a))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;
        let restored_signature = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("restored-signature fingerprint should compute")?;
        #[cfg(not(unix))]
        assert_eq!(
            baseline, restored_signature,
            "non-unix: same path + mtime + size must reproduce the baseline fingerprint"
        );
        #[cfg(unix)]
        assert_ne!(
            baseline, restored_signature,
            "unix: the rewrites above bumped ctime, so restoring content + mtime must NOT reproduce the baseline fingerprint"
        );

        // Path-set change: must invalidate.
        let b = write_corpus_file(&dir, "src/b.rs", "pub fn b() -> i32 { 2 }\n")?;
        let with_b = corpus_fingerprint(&dir, &[a.clone(), b])
            .ok_or("two-file fingerprint should compute")?;
        assert_ne!(
            baseline, with_b,
            "adding a file must change the fingerprint"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Unix hardening (codex P2 on #2175): a same-size content rewrite
    /// with the mtime explicitly restored — exactly what `rsync -a`,
    /// `cp --preserve=timestamps`, or an archive extractor does — bumps
    /// the inode change time, so the fingerprint must change.
    #[cfg(unix)]
    #[test]
    fn corpus_fingerprint_unix_ctime_invalidates_mtime_preserving_rewrite() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-ctime");
        let _ = std::fs::remove_dir_all(&dir);
        let a = write_corpus_file(&dir, "src/a.rs", "pub fn a() -> i32 { 1 }\n")?;
        let baseline = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("baseline fingerprint should compute")?;

        let original_mtime = std::fs::metadata(dir.join(&a))
            .and_then(|m| m.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        // Guarantee at least one filesystem timestamp tick has elapsed so
        // the rewrite below bumps ctime even on coarse-granularity
        // filesystems; the helper rewrites identical bytes, so only the
        // signature — never the content — is affected.
        wait_for_ctime_tick(&dir.join(&a))?;
        // Same length, different bytes — mtime and size preserved.
        std::fs::write(dir.join(&a), "pub fn a() -> i32 { 2 }\n")
            .map_err(|err| format!("rewrite: {err}"))?;
        let file = std::fs::File::options()
            .write(true)
            .open(dir.join(&a))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;

        let after = corpus_fingerprint(&dir, std::slice::from_ref(&a))
            .ok_or("post-rewrite fingerprint should compute")?;
        assert_ne!(
            baseline, after,
            "unix ctime must invalidate the fingerprint on a same-size rewrite with restored mtime"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn corpus_fingerprint_returns_none_when_a_file_cannot_be_statted() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir: {err}"))?;
        let missing = PathBuf::from("src/missing.rs");
        assert_eq!(
            corpus_fingerprint(&dir, &[missing]),
            None,
            "an un-stat-able file must degrade to None, not a fabricated fingerprint"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn fingerprint_cache_roundtrip_returns_stored_hash() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let root = Path::new("/repo");
        let cache = RepoCorpusFingerprintCache::at_dir(dir.clone());
        cache
            .store(root, "0123456789abcdef", "fedcba9876543210")
            .map_err(|err| format!("store: {err}"))?;

        assert_eq!(
            cache.lookup(root, "0123456789abcdef"),
            Some("fedcba9876543210".to_string()),
            "lookup should return the stored aggregate hash"
        );
        assert_eq!(
            cache.lookup(root, "aaaaaaaaaaaaaaaa"),
            None,
            "a different fingerprint must miss"
        );
        assert_eq!(
            cache.lookup(Path::new("/other-repo"), "0123456789abcdef"),
            None,
            "an entry from a different workspace root must miss"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn fingerprint_cache_store_is_atomic_and_leaves_no_temp_file() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-atomic");
        let _ = std::fs::remove_dir_all(&dir);
        let root = Path::new("/repo");
        let cache = RepoCorpusFingerprintCache::at_dir(dir.clone());
        cache
            .store(root, "0123456789abcdef", "fedcba9876543210")
            .map_err(|err| format!("store: {err}"))?;
        // Overwrite the same fingerprint: the mapping is replaced atomically.
        cache
            .store(root, "0123456789abcdef", "1111111111111111")
            .map_err(|err| format!("re-store: {err}"))?;

        let entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|err| format!("read dir: {err}"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<_, _>>()
            .map_err(|err| format!("read entry: {err}"))?;
        assert_eq!(
            entries,
            vec![dir.join("0123456789abcdef.json")],
            "store must leave exactly one renamed entry and no temp file"
        );
        assert_eq!(
            cache.lookup(root, "0123456789abcdef"),
            Some("1111111111111111".to_string()),
            "the atomic overwrite must surface the latest mapping"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn fingerprint_cache_lookup_ignores_corrupt_entry() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir: {err}"))?;
        std::fs::write(dir.join("0123456789abcdef.json"), b"{not valid json")
            .map_err(|err| format!("write corrupt entry: {err}"))?;
        let cache = RepoCorpusFingerprintCache::at_dir(dir.clone());
        assert_eq!(
            cache.lookup(Path::new("/repo"), "0123456789abcdef"),
            None,
            "a corrupt entry must degrade to a conservative miss"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn fingerprint_rebuilt_key_is_byte_identical_to_freshly_computed_key() -> Result<(), String> {
        let dir = isolated_dir("fingerprint-key-parity");
        let _ = std::fs::remove_dir_all(&dir);
        let a = write_corpus_file(&dir, "src/a.rs", "pub fn a() -> i32 { 1 }\n")?;
        let b = write_corpus_file(&dir, "tests/b.rs", "#[test] fn t() {}\n")?;
        let files: Vec<(PathBuf, Vec<u8>)> = [a.clone(), b.clone()]
            .into_iter()
            .map(|relative| {
                std::fs::read(dir.join(&relative))
                    .map(|bytes| (relative, bytes))
                    .map_err(|err| format!("read corpus file: {err}"))
            })
            .collect::<Result<_, _>>()?;

        let state = WorkspaceState {
            workspace_root: &dir,
            files: &files,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        };
        let fresh_key = state.cache_key();

        // Persist the mapping the cold path would store, then rebuild the
        // key the way the fingerprint fast path does — with no file bytes.
        let fingerprint =
            corpus_fingerprint(&dir, &[a, b]).ok_or("fingerprint should compute for the corpus")?;
        let cache = RepoCorpusFingerprintCache::at_dir(dir.join("fp-cache"));
        cache
            .store(&dir, &fingerprint, &fresh_key.files_content_hash)
            .map_err(|err| format!("store mapping: {err}"))?;
        let stored_hash = cache
            .lookup(&dir, &fingerprint)
            .ok_or("stored mapping should load")?;
        let rebuilt_key = WorkspaceKeyContext {
            workspace_root: &dir,
            cfg_features: None,
            config_text: None,
            test_intent_text: None,
            suppressions_text: None,
        }
        .cache_key(stored_hash);

        assert_eq!(
            fresh_key, rebuilt_key,
            "fingerprint-rebuilt key must be byte-identical to the computed key"
        );
        assert_eq!(fresh_key.filename(), rebuilt_key.filename());

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
