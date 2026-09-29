//! Repo seam inventory walker per RIPR-SPEC-0005.
//!
//! Walks production Rust files via the existing syntax adapter
//! (`rust_index::build_index`) and emits a deterministic
//! `Vec<RepoSeam>` from the `ProbeShapeFact` records each file already
//! produces. This is the v1 implementation; future PRs add test-grip
//! evidence (`analysis/test-grip-evidence-v1`) and seam classification
//! (`analysis/repo-ripr-classification-v1`).
//!
//! Determinism contract per the spec:
//!
//! 1. Two runs over the same source tree must produce the same seams in
//!    the same order regardless of file walk order.
//! 2. Test files do not generate production seams (they are filtered by
//!    the shared source-role model).
//!
//! Both contracts are pinned by tests in this file.

use super::classify::exact_error_variant;
use super::rust_index::{
    self, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, RustIndex,
};
#[cfg(test)]
use super::seam_cache::CLASSIFIED_SEAM_CACHE_STORE_LIMIT;
#[cfg(test)]
use super::seam_cache::RepoSeamCountCache;
use super::seam_cache::{
    CacheLoad, CachedSeamLimitInfo, CorpusFingerprintLookup, FileFactCacheStats,
    RepoCorpusFingerprintCache, RepoSeamCacheKey, RepoSeamFactCache, WorkspaceKeyContext,
    WorkspaceState, classified_seam_cache_store_limit, compact_classified_seam_cache_store_limit,
    corpus_fingerprint,
};
#[cfg(test)]
use super::seam_classification::SeamGripClassCounts;
use super::seam_classification::{self, ClassifiedSeam};
use super::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use super::test_grip_evidence;
use super::workspace;
use crate::analysis::cancellation;
use crate::config::RiprConfig;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const REPO_EXPOSURE_SEAM_LIMIT_ENV: &str = "RIPR_REPO_EXPOSURE_SEAM_LIMIT";

/// Default cap on the number of seams analyzed in a single full-repo
/// `repo-exposure-json` run. This prevents pathological 41-minute runs
/// on giant workspaces. Operators can opt out via `RIPR_REPO_EXPOSURE_SEAM_LIMIT=0`.
pub(crate) const DEFAULT_REPO_EXPOSURE_SEAM_LIMIT: usize = 10_000;

/// Environment variable that overrides the pilot artifact seam budget.
/// Set to `0` to remove the cap (unbounded); any positive integer sets
/// the budget explicitly.  When unset, `DEFAULT_PILOT_SEAM_BUDGET` applies.
pub(crate) const PILOT_SEAM_BUDGET_ENV: &str = "RIPR_PILOT_SEAM_BUDGET";

/// Default cap on the number of seams written to pilot artifacts
/// (`repo-exposure.json` and `agent-seam-packets.json`).  2 000 seams
/// stay comfortably below 10 MB at typical per-seam evidence sizes.
/// Operators can raise or remove the cap via `RIPR_PILOT_SEAM_BUDGET`.
pub(crate) const DEFAULT_PILOT_SEAM_BUDGET: usize = 2_000;

const LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";

/// Walk production Rust files at `root` and emit the raw seam inventory.
/// Used by the `repo-seams-*` formats; the classified inventory used by
/// `repo-exposure-*` formats lives in [`inventory_classified_seams_at`].
pub(crate) fn inventory_seams_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<Vec<RepoSeam>, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    // Producer-owned source role (#3283).
    let mut context =
        workspace::context_for_files(root, rust_files.iter().map(|path| path.as_path()));
    context.production_like_targets = config.analysis().production_like_targets().clone();
    context.harness_targets = harness_targets_from_config(root, config);
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::classify_with(p, &context).seeds_production_findings())
        .cloned()
        .collect();

    // Index the full set so `find_owner_function` can resolve owners
    // even when the seam appears in a file the production filter
    // includes but tests reference.
    let index = rust_index::build_index_with_test_harnesses(
        root,
        &rust_files,
        harness_registrations(config),
    )?;
    Ok(inventory_seams_from_index(&production_files, &index))
}

/// Whether a seam limit was the built-in default or explicitly configured
/// via the environment variable.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SeamLimitSource {
    /// The limit came from `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT` (env var unset).
    Default,
    /// The limit came from an explicit `RIPR_REPO_EXPOSURE_SEAM_LIMIT` setting.
    Configured,
}

impl SeamLimitSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Configured => "configured",
        }
    }
}

/// Carries information about a seam-limit truncation so the output
/// layer can self-declare when a run analyzed fewer seams than were
/// available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SeamLimitInfo {
    pub(crate) analyzed: usize,
    pub(crate) total: usize,
    pub(crate) source: SeamLimitSource,
}

impl From<CachedSeamLimitInfo> for SeamLimitInfo {
    fn from(cached: CachedSeamLimitInfo) -> Self {
        Self {
            analyzed: cached.analyzed,
            total: cached.total,
            source: cached.source,
        }
    }
}

impl From<&SeamLimitInfo> for CachedSeamLimitInfo {
    fn from(info: &SeamLimitInfo) -> Self {
        Self {
            analyzed: info.analyzed,
            total: info.total,
            source: info.source.clone(),
        }
    }
}

/// Walk production Rust files at `root` and emit per-seam evidence and
/// classification. This is the input to `output/repo-exposure-report-v1`.
/// The discard hook in `inventory_seams_at` from #237 is replaced by
/// this real consumer; evidence and classification are no longer
/// computed for the diff-free seam-only formats.
///
/// Consults the on-disk fact-layer cache
/// (`target/ripr/cache/repo-seam-facts/...`) before computing. Cache
/// hits skip the file walk, parse, evidence build, and classification
/// pipeline entirely. Misses and corrupt entries fall through to a
/// fresh compute and write the result for the next run. Cache
/// failures never fail the analysis.
#[cfg(test)]
pub(crate) fn inventory_classified_seams_at(root: &Path) -> Result<Vec<ClassifiedSeam>, String> {
    inventory_classified_seams_at_with_config(root, &RiprConfig::default())
        .map(|(classified, _)| classified)
}

pub(crate) fn inventory_classified_seams_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<(Vec<ClassifiedSeam>, Option<SeamLimitInfo>), String> {
    let total_started = Instant::now();
    let cache = RepoSeamFactCache::at(root);
    let store_limit = classified_seam_cache_store_limit()?;
    let collect_started = Instant::now();
    let (rust_files, fingerprint) = match scan_corpus_fingerprint(root) {
        Ok(scan) => scan,
        Err(err) => {
            trace_latency_phase(
                "collect_workspace_state",
                "error",
                collect_started.elapsed(),
            );
            trace_latency_phase("total", "error", total_started.elapsed());
            return Err(err);
        }
    };
    let inputs = workspace_key_inputs(root, config);

    // Stat-only fast path (issue #2108): when the corpus fingerprint store
    // holds a mapping for the current (path, mtime, size, ctime on unix)
    // signature, the cache key is rebuilt without reading any file contents.
    if let Some(key) = fingerprint_cached_workspace_key(root, &inputs, fingerprint.as_deref()) {
        trace_latency_phase(
            "collect_workspace_state",
            "fingerprint",
            collect_started.elapsed(),
        );
        let cache_started = Instant::now();
        // Cooperative cancellation (#1972): a superseded or
        // deadline-expired refresh stops before the cache load path.
        cancellation::checkpoint()?;
        match cache.load_classified_seams_with_fallback(&key) {
            CacheLoad::Hit((cached, cached_limit_info, lexical_fallback_files)) => {
                trace_latency_phase("cache_load", "hit", cache_started.elapsed());
                trace_latency_phase("total", "cache_hit", total_started.elapsed());
                if let Some(disclosure) =
                    rust_index::lexical_fallback_disclosure_at(root, &lexical_fallback_files)
                {
                    eprintln!("{disclosure}");
                }
                // Preserve the run_status from the original run: a capped run
                // stored its SeamLimitInfo in the envelope; a complete run
                // stored None.
                return Ok((cached, cached_limit_info.map(SeamLimitInfo::from)));
            }
            CacheLoad::Miss => {
                trace_latency_phase("cache_load", "miss", cache_started.elapsed());
            }
            CacheLoad::CorruptIgnored { reason } => {
                trace_latency_phase("cache_load", "corrupt_ignored", cache_started.elapsed());
                eprintln!("ripr: repo seam cache entry ignored ({reason})");
            }
        }
    }

    let state = match collect_workspace_state_from_files(root, config, rust_files) {
        Ok(state) => {
            trace_latency_phase("collect_workspace_state", "ok", collect_started.elapsed());
            state
        }
        Err(err) => {
            trace_latency_phase(
                "collect_workspace_state",
                "error",
                collect_started.elapsed(),
            );
            trace_latency_phase("total", "error", total_started.elapsed());
            return Err(err);
        }
    };
    let key = state.cache_key();
    store_corpus_fingerprint_mapping(&state, fingerprint, &key);
    // NOTE: the seam-limit key is baked into `key.filename()`, so a capped run
    // and an unbounded run never share a cache file — no fast-path bypass needed.
    let cache_started = Instant::now();
    trace_latency_phase(
        "cache_load",
        &format!("start_files_{}", state.files.len()),
        Duration::ZERO,
    );
    // Cooperative cancellation (#1972): a superseded or deadline-expired
    // refresh stops before the post-collect cache load.
    cancellation::checkpoint()?;
    match cache.load_classified_seams_with_fallback(&key) {
        CacheLoad::Hit((cached, cached_limit_info, lexical_fallback_files)) => {
            trace_latency_phase("cache_load", "hit", cache_started.elapsed());
            trace_latency_phase("total", "cache_hit", total_started.elapsed());
            if let Some(disclosure) =
                rust_index::lexical_fallback_disclosure_at(root, &lexical_fallback_files)
            {
                eprintln!("{disclosure}");
            }
            // Preserve the run_status from the original run: a capped run stored
            // its SeamLimitInfo in the envelope; a complete run stored None.
            return Ok((cached, cached_limit_info.map(SeamLimitInfo::from)));
        }
        CacheLoad::Miss => {
            trace_latency_phase("cache_load", "miss", cache_started.elapsed());
        }
        CacheLoad::CorruptIgnored { reason } => {
            trace_latency_phase("cache_load", "corrupt_ignored", cache_started.elapsed());
            // Advisory: surface the reason so operators can see why a
            // warm path degraded to cold. Never fail analysis.
            eprintln!("ripr: repo seam cache entry ignored ({reason})");
        }
    }
    let compute_started = Instant::now();
    trace_latency_phase("cold_compute", "start", Duration::ZERO);
    let (classified, limit_info, lexical_fallback_files) =
        match inventory_classified_seams_from_state_with_config(&state, config) {
            Ok(pair) => {
                trace_latency_phase("cold_compute", "ok", compute_started.elapsed());
                pair
            }
            Err(err) => {
                trace_latency_phase("cold_compute", "error", compute_started.elapsed());
                trace_latency_phase("total", "error", total_started.elapsed());
                return Err(err);
            }
        };
    // Best-effort write: a write failure does not fail analysis. The
    // result is already in memory; the next run just sees a miss again.
    // Persist the limit_info so a warm-path load returns the correct run_status.
    let cached_limit_info: Option<CachedSeamLimitInfo> = limit_info.as_ref().map(Into::into);
    let store_started = Instant::now();
    trace_latency_phase(
        "cache_store",
        &format!(
            "start_classified_{}_limit_{}",
            classified.len(),
            store_limit
        ),
        Duration::ZERO,
    );
    let store_status = match cache.store_classified_seams_with_limit_and_fallback(
        &key,
        &classified,
        cached_limit_info.as_ref(),
        &lexical_fallback_files,
        store_limit,
    ) {
        Ok(status) => status.label,
        Err(reason) => {
            eprintln!("ripr: repo seam cache store ignored ({reason})");
            cache_store_status_label(&reason)
        }
    };
    trace_latency_phase("cache_store", &store_status, store_started.elapsed());
    trace_latency_phase("total", "computed", total_started.elapsed());
    Ok((classified, limit_info))
}

/// Return the workspace cache identity used by the full classified inventory.
///
/// Targeted rerun parity uses this explicit identity check so a narrow result
/// cannot be accepted against a different manifest, policy, or toolchain
/// state than the full pipeline invocation.
pub(crate) fn workspace_cache_key_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<super::seam_cache::RepoSeamCacheKey, String> {
    let (rust_files, fingerprint) = scan_corpus_fingerprint(root)?;
    let inputs = workspace_key_inputs(root, config);
    // Fingerprint fast path (issue #2108): a stored mapping yields the
    // byte-identical key without reading any file contents.
    if let Some(key) = fingerprint_cached_workspace_key(root, &inputs, fingerprint.as_deref()) {
        return Ok(key);
    }
    let state = collect_workspace_state_from_files(root, config, rust_files)?;
    let key = state.cache_key();
    store_corpus_fingerprint_mapping(&state, fingerprint, &key);
    Ok(key)
}

fn trace_latency_phase(phase: &str, status: &str, duration: Duration) {
    if std::env::var_os(LATENCY_TRACE_ENV).is_some() {
        eprintln!("{}", latency_trace_line(phase, status, duration));
    }
}

/// A bounded, typed diagnostic channel for the latency runner. The ordinary
/// human phase label cannot represent failure identities or overflow.
fn trace_file_fact_cache(stats: &FileFactCacheStats) {
    if std::env::var_os(LATENCY_TRACE_ENV).is_none() {
        return;
    }
    eprintln!(
        "ripr_file_fact_cache_receipt {}",
        file_fact_cache_receipt(stats)
    );
}

fn file_fact_cache_receipt(stats: &FileFactCacheStats) -> serde_json::Value {
    let rows: Vec<_> = stats
        .store_failures
        .iter()
        .map(|failure| {
            serde_json::json!({
                "path": portable_store_failure_path(&failure.path),
                "stage": failure.stage,
                "error": failure.error,
            })
        })
        .collect();
    serde_json::json!({
        "schema_version": "0.1",
        "hits": stats.hits,
        "misses": stats.misses,
        "invalidated": stats.invalidated_files.len(),
        "corrupt_ignored": stats.corrupt_ignored,
        "stores": stats.stores,
        "store_errors": stats.store_errors,
        "store_failures": rows,
        "store_failures_dropped": stats.store_failures_dropped,
    })
}

/// Join path components with `/`. Components come from the native parser, so
/// a Windows `\\` separator splits while a literal `\\` in a Unix file name
/// does not. A name that is not UTF-8 or that holds a `\\` has no portable
/// spelling: it is `null` rather than a lossy rewrite that names another file.
fn portable_store_failure_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let part = component.as_os_str().to_str()?;
        if part.contains('\\') {
            return None;
        }
        parts.push(part);
    }
    Some(parts.join("/"))
}

fn cache_store_status_label(reason: &str) -> String {
    let mut label = String::from("ignored_");
    for ch in reason.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            label.push(ch);
        } else {
            label.push('_');
        }
        if label.len() >= 160 {
            break;
        }
    }
    label
}

fn latency_trace_line(phase: &str, status: &str, duration: Duration) -> String {
    format!(
        "ripr_repo_exposure_latency phase={phase} status={status} duration_ms={}",
        duration.as_millis()
    )
}

/// Cold-path inventory + classify with no cache. Used by the cached
/// entry point on miss and by tests that want to drive the pipeline
/// directly. Stays crate-private; the public entry is the cached
/// function above.
#[cfg(test)]
pub(crate) fn inventory_classified_seams_uncached_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<Vec<ClassifiedSeam>, String> {
    let discover_started = Instant::now();
    let rust_files = match workspace::discover_rust_files(root) {
        Ok(files) => {
            trace_latency_phase("discover_rust_files", "ok", discover_started.elapsed());
            files
        }
        Err(err) => {
            trace_latency_phase("discover_rust_files", "error", discover_started.elapsed());
            return Err(err);
        }
    };
    let filter_started = Instant::now();
    // Same shared role authority as the production paths (#3285).
    let mut uncached_context =
        workspace::context_for_files(root, rust_files.iter().map(|path| path.as_path()));
    uncached_context.production_like_targets = config.analysis().production_like_targets().clone();
    uncached_context.harness_targets = harness_targets_from_config(root, config);
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::classify_with(p, &uncached_context).seeds_production_findings())
        .cloned()
        .collect();
    trace_latency_phase("filter_production_files", "ok", filter_started.elapsed());

    let index_started = Instant::now();
    let mut index = match rust_index::build_index_with_test_harnesses(
        root,
        &rust_files,
        harness_registrations(config),
    ) {
        Ok(index) => {
            trace_latency_phase("build_index", "ok", index_started.elapsed());
            index
        }
        Err(err) => {
            trace_latency_phase("build_index", "error", index_started.elapsed());
            return Err(err);
        }
    };
    let policy_started = Instant::now();
    rust_index::apply_oracle_policy(&mut index, config.oracles());
    trace_latency_phase("apply_oracle_policy", "ok", policy_started.elapsed());
    let seams_started = Instant::now();
    let seams = inventory_seams_from_index(&production_files, &index);
    trace_latency_phase("inventory_seams", "ok", seams_started.elapsed());
    let evidence_started = Instant::now();
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &index);
    trace_latency_phase("evidence_for_seams", "ok", evidence_started.elapsed());
    let classify_started = Instant::now();
    let classified = seam_classification::classify_seams_owned(seams, evidence);
    trace_latency_phase("classify_seams", "ok", classify_started.elapsed());
    Ok(classified)
}

/// Walk production Rust files at `root` and return compact seam grip
/// class counts. Repo badges use this path because they need headline
/// counts but not full per-seam evidence or related-test payloads.
#[cfg(test)]
pub(crate) fn inventory_seam_grip_class_counts_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<SeamGripClassCounts, String> {
    let cache = RepoSeamCountCache::at(root);
    let state = collect_workspace_state(root, config)?;
    let key = state.cache_key();
    match cache.load_counts(&key) {
        CacheLoad::Hit(cached) => return Ok(cached),
        CacheLoad::Miss => {}
        CacheLoad::CorruptIgnored { reason } => {
            eprintln!("ripr: repo seam count cache entry ignored ({reason})");
        }
    }
    let counts = inventory_seam_grip_class_counts_from_state_with_config(&state, config)?;
    let _ = cache.store_counts(&key, &counts);
    Ok(counts)
}

/// Walk production Rust files at `root` and return compact classified seams.
///
/// Public badge projection needs canonical gap grouping and actionability, but
/// not the full related-test/evidence payload carried by repo exposure. This
/// mirrors the compact repo-badge count path while retaining the seam records
/// needed to deduplicate canonical repair items.
pub(crate) fn inventory_compact_classified_seams_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<Vec<ClassifiedSeam>, String> {
    let total_started = Instant::now();
    let store_limit = compact_classified_seam_cache_store_limit()?;
    let cache = RepoSeamFactCache::at_compact_classified(root);
    let (rust_files, fingerprint) = scan_corpus_fingerprint(root)?;
    let inputs = workspace_key_inputs(root, config);

    // Stat-only fast path (issue #2108): rebuild the byte-identical cache
    // key from the corpus fingerprint store when the signature is unchanged.
    if let Some(key) = fingerprint_cached_workspace_key(root, &inputs, fingerprint.as_deref()) {
        let cache_started = Instant::now();
        match cache.load_classified_seams_with_fallback(&key) {
            CacheLoad::Hit((cached, _limit_info, lexical_fallback_files)) => {
                trace_latency_phase("compact_cache_load", "hit", cache_started.elapsed());
                trace_latency_phase("total", "compact_cache_hit", total_started.elapsed());
                if let Some(disclosure) =
                    rust_index::lexical_fallback_disclosure_at(root, &lexical_fallback_files)
                {
                    eprintln!("{disclosure}");
                }
                return Ok(cached);
            }
            CacheLoad::Miss => {
                trace_latency_phase("compact_cache_load", "miss", cache_started.elapsed());
            }
            CacheLoad::CorruptIgnored { reason } => {
                trace_latency_phase(
                    "compact_cache_load",
                    "corrupt_ignored",
                    cache_started.elapsed(),
                );
                eprintln!("ripr: compact repo seam cache entry ignored ({reason})");
            }
        }
    }

    let state = collect_workspace_state_from_files(root, config, rust_files)?;
    let key = state.cache_key();
    store_corpus_fingerprint_mapping(&state, fingerprint, &key);
    let cache_started = Instant::now();
    match cache.load_classified_seams_with_fallback(&key) {
        CacheLoad::Hit((cached, _limit_info, lexical_fallback_files)) => {
            trace_latency_phase("compact_cache_load", "hit", cache_started.elapsed());
            trace_latency_phase("total", "compact_cache_hit", total_started.elapsed());
            if let Some(disclosure) =
                rust_index::lexical_fallback_disclosure_at(root, &lexical_fallback_files)
            {
                eprintln!("{disclosure}");
            }
            return Ok(cached);
        }
        CacheLoad::Miss => {
            trace_latency_phase("compact_cache_load", "miss", cache_started.elapsed());
        }
        CacheLoad::CorruptIgnored { reason } => {
            trace_latency_phase(
                "compact_cache_load",
                "corrupt_ignored",
                cache_started.elapsed(),
            );
            eprintln!("ripr: compact repo seam cache entry ignored ({reason})");
        }
    }

    let (classified, lexical_fallback_files) =
        inventory_compact_classified_seams_from_state_with_config(&state, config)?;
    let store_started = Instant::now();
    trace_latency_phase(
        "compact_cache_store",
        &format!(
            "start_classified_{}_limit_{}",
            classified.len(),
            store_limit
        ),
        Duration::ZERO,
    );
    let store_status = match cache.store_compact_classified_seams_with_limit_and_fallback(
        &key,
        &classified,
        &lexical_fallback_files,
        store_limit,
    ) {
        Ok(status) => status.label,
        Err(reason) => {
            eprintln!("ripr: compact repo seam cache store ignored ({reason})");
            cache_store_status_label(&reason)
        }
    };
    trace_latency_phase(
        "compact_cache_store",
        &store_status,
        store_started.elapsed(),
    );
    trace_latency_phase("total", "compact_computed", total_started.elapsed());
    Ok(classified)
}

#[cfg(test)]
fn inventory_seam_grip_class_counts_uncached_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<SeamGripClassCounts, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    // Producer-owned source role (#3283).
    let mut context =
        workspace::context_for_files(root, rust_files.iter().map(|path| path.as_path()));
    context.production_like_targets = config.analysis().production_like_targets().clone();
    context.harness_targets = harness_targets_from_config(root, config);
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::classify_with(p, &context).seeds_production_findings())
        .cloned()
        .collect();

    let mut index = rust_index::build_index_with_test_harnesses(
        root,
        &rust_files,
        harness_registrations(config),
    )?;
    rust_index::apply_oracle_policy(&mut index, config.oracles());
    let seams = inventory_seams_from_index(&production_files, &index);
    let mut counts = SeamGripClassCounts::new(seams.len());
    let context = test_grip_evidence::CompactGripContext::new(&index);
    for seam in &seams {
        let evidence = test_grip_evidence::compact_evidence_for_seam(seam, &context);
        let class = seam_classification::classify_seam(seam, &evidence);
        counts.increment(class);
    }
    Ok(counts)
}

fn inventory_compact_classified_seams_from_state_with_config(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Result<(Vec<ClassifiedSeam>, Vec<PathBuf>), String> {
    let production_files = production_files_from_state_with_role(state, config);
    let build_started = Instant::now();
    trace_latency_phase(
        "file_fact_cache",
        &format!(
            "start_files_{}_production_{}",
            state.files.len(),
            production_files.len()
        ),
        Duration::ZERO,
    );
    let mut cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &state.workspace_root,
        &state.files,
        harness_registrations(config),
    )?;
    cancellation::checkpoint()?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    let lexical_fallback_files = rust_index::lexical_fallback_files(&cached.index);
    let seams = inventory_seams_from_index(&production_files, &cached.index);
    let context = test_grip_evidence::CompactGripContext::new(&cached.index);
    let mut classified = Vec::with_capacity(seams.len());
    for seam in seams {
        cancellation::checkpoint()?;
        let evidence = test_grip_evidence::compact_evidence_for_seam(&seam, &context);
        let class = seam_classification::classify_seam(&seam, &evidence);
        classified.push(ClassifiedSeam {
            evidence,
            seam,
            class,
        });
    }
    Ok((classified, lexical_fallback_files))
}

type ClassifiedSeamInventory = (Vec<ClassifiedSeam>, Option<SeamLimitInfo>, Vec<PathBuf>);

fn inventory_classified_seams_from_state_with_config(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Result<ClassifiedSeamInventory, String> {
    let production_files = production_files_from_state_with_role(state, config);
    let build_started = Instant::now();
    trace_latency_phase(
        "file_fact_cache",
        &format!(
            "start_files_{}_production_{}",
            state.files.len(),
            production_files.len()
        ),
        Duration::ZERO,
    );
    let mut cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &state.workspace_root,
        &state.files,
        harness_registrations(config),
    )?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    trace_file_fact_cache(&cached.file_fact_cache);
    let policy_started = Instant::now();
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    let lexical_fallback_files = rust_index::lexical_fallback_files(&cached.index);
    trace_latency_phase("apply_oracle_policy", "ok", policy_started.elapsed());
    let seams_started = Instant::now();
    let mut seams = inventory_seams_from_index(&production_files, &cached.index);
    cancellation::checkpoint()?;
    trace_latency_phase("inventory_seams", "ok", seams_started.elapsed());
    let limit_info = apply_repo_exposure_seam_limit(&mut seams);
    let evidence_started = Instant::now();
    trace_latency_phase(
        "evidence_for_seams",
        &format!("start_seams_{}", seams.len()),
        Duration::ZERO,
    );
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &cached.index);
    cancellation::checkpoint()?;
    trace_latency_phase("evidence_for_seams", "ok", evidence_started.elapsed());
    let classify_started = Instant::now();
    let classified = seam_classification::classify_seams_owned(seams, evidence);
    cancellation::checkpoint()?;
    trace_latency_phase("classify_seams", "ok", classify_started.elapsed());
    Ok((classified, limit_info, lexical_fallback_files))
}

#[derive(Clone, Debug)]
pub(crate) struct ScopedClassifiedSeamInventory {
    pub(crate) classified: Vec<ClassifiedSeam>,
    pub(crate) file_fact_cache: FileFactCacheStats,
    pub(crate) workspace_cache_key: super::seam_cache::RepoSeamCacheKey,
    pub(crate) total_rust_files: usize,
    pub(crate) total_production_files: usize,
    pub(crate) scoped_production_files: Vec<PathBuf>,
    pub(crate) changed_production_files: Vec<PathBuf>,
    pub(crate) immediate_caller_files: Vec<PathBuf>,
    /// Scoped seams left without evidence because the first
    /// [`DiffScopeEvidenceStages`] stage was sufficient; zero when
    /// every scoped seam was classified.
    pub(crate) unevaluated_seams: usize,
}

/// Two-stage evidence for the diff-scoped inventory.
///
/// Test-grip evidence costs tens of milliseconds per seam, and a diff
/// touching a large file plus its callers can scope thousands of seams
/// (#4292). A consumer whose result depends only on a known subset
/// names that subset as `first`; when `sufficient` accepts the
/// classified subset, the remaining seams are never evaluated. When it
/// declines, every scoped seam is classified exactly as without stages.
/// The analysis layer stays ignorant of why a subset is enough.
pub(crate) struct DiffScopeEvidenceStages<'a> {
    pub(crate) first: &'a dyn Fn(&RepoSeam) -> bool,
    pub(crate) sufficient: &'a dyn Fn(&[ClassifiedSeam]) -> bool,
}

/// Cache-backed inventory for one edited test file.
///
/// The whole workspace still contributes file facts and ownership context, but
/// only seams owned by functions directly called from the selected test file
/// receive fresh relation/evidence/classification work. This intentionally
/// recomputes selected edges after a test edit rather than serving an invalid
/// workspace-level classified-seam cache.
#[derive(Clone, Debug)]
pub(crate) struct TargetedTestClassifiedSeamInventory {
    pub(crate) classified: Vec<ClassifiedSeam>,
    pub(crate) selected_test_count: usize,
    pub(crate) direct_call_names: Vec<String>,
    pub(crate) file_fact_cache: FileFactCacheStats,
    pub(crate) workspace_cache_key: super::seam_cache::RepoSeamCacheKey,
}

/// Why a `--changed-test` selector could not name the seams to recompute.
/// The selector failures are named limitations for the rerun report
/// (RIPR-SPEC-0123); `Analysis` is a workspace failure the caller reports as
/// an error.
#[derive(Debug)]
pub(crate) enum TargetedTestInventoryError {
    Selector {
        kind: &'static str,
        message: String,
        selected_test_count: usize,
        /// The index was already built when the selector failed, so the
        /// report still discloses the cache work that ran.
        cache: Box<TargetedSelectorCacheEvidence>,
    },
    Analysis(String),
}

/// File-fact cache work completed before a changed-test selector failed.
#[derive(Clone, Debug)]
pub(crate) struct TargetedSelectorCacheEvidence {
    pub(crate) file_fact_cache: FileFactCacheStats,
    pub(crate) workspace_cache_key: super::seam_cache::RepoSeamCacheKey,
}

impl From<String> for TargetedTestInventoryError {
    fn from(message: String) -> Self {
        Self::Analysis(message)
    }
}

impl From<TargetedTestInventoryError> for String {
    fn from(error: TargetedTestInventoryError) -> Self {
        match error {
            TargetedTestInventoryError::Selector { message, .. }
            | TargetedTestInventoryError::Analysis(message) => message,
        }
    }
}

pub(crate) const CHANGED_TEST_UNRESOLVED: &str = "changed_test_unresolved";
pub(crate) const CHANGED_TEST_OWNER_UNRESOLVED: &str = "changed_test_owner_unresolved";
pub(crate) const CHANGED_TEST_OWNER_AMBIGUOUS: &str = "changed_test_owner_ambiguous";

pub(crate) fn inventory_changed_test_classified_seams_at_with_config_node(
    root: &Path,
    config: &RiprConfig,
    changed_test: &Path,
    test_node: Option<&str>,
) -> Result<TargetedTestClassifiedSeamInventory, TargetedTestInventoryError> {
    let state = collect_workspace_state(root, config)?;
    let workspace_cache_key = state.cache_key();
    let changed_test = normalized_inventory_path(changed_test);
    let mut cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &state.workspace_root,
        &state.files,
        harness_registrations(config),
    )?;
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    let selector_limitation =
        |kind, message, selected_test_count| TargetedTestInventoryError::Selector {
            kind,
            message,
            selected_test_count,
            cache: Box::new(TargetedSelectorCacheEvidence {
                file_fact_cache: cached.file_fact_cache.clone(),
                workspace_cache_key: workspace_cache_key.clone(),
            }),
        };

    let selected_tests = cached
        .index
        .tests
        .iter()
        .filter(|test| normalized_inventory_path(&test.file) == changed_test)
        .filter(|test| test_node.is_none_or(|node| test.name == node))
        .collect::<Vec<_>>();
    if selected_tests.is_empty() {
        return Err(selector_limitation(
            CHANGED_TEST_UNRESOLVED,
            format!(
                "targeted rerun changed test `{}{}` did not resolve to a parsed test",
                changed_test,
                test_node.map_or(String::new(), |node| format!("::{node}"))
            ),
            0,
        ));
    }

    let direct_call_names = selected_tests
        .iter()
        .flat_map(|test| {
            test.calls
                .iter()
                .filter(move |call| call.name != test.name)
                .map(|call| call.name.trim())
        })
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if direct_call_names.is_empty() {
        return Err(selector_limitation(
            CHANGED_TEST_OWNER_UNRESOLVED,
            format!(
                "targeted rerun changed test `{changed_test}` has no direct owner-call selector"
            ),
            selected_tests.len(),
        ));
    }

    let candidate_functions = cached
        .index
        .functions
        .iter()
        .filter(|function| {
            !function.source_role.is_evidence_role() && direct_call_names.contains(&function.name)
        })
        .collect::<Vec<_>>();
    let matched_call_names = candidate_functions
        .iter()
        .map(|function| function.name.clone())
        .collect::<BTreeSet<_>>();
    if matched_call_names.is_empty() {
        return Err(selector_limitation(
            CHANGED_TEST_OWNER_UNRESOLVED,
            format!(
                "targeted rerun changed test `{changed_test}` did not resolve a direct production owner"
            ),
            selected_tests.len(),
        ));
    }
    for call_name in &matched_call_names {
        let matching_owners = candidate_functions
            .iter()
            .filter(|function| function.name == *call_name)
            .count();
        if matching_owners > 1 {
            return Err(selector_limitation(
                CHANGED_TEST_OWNER_AMBIGUOUS,
                format!(
                    "targeted rerun changed test `{changed_test}` has ambiguous direct production owner `{call_name}`"
                ),
                selected_tests.len(),
            ));
        }
    }
    let production_files = candidate_functions
        .into_iter()
        .map(|function| function.file.clone())
        .collect::<Vec<_>>();
    let seams = inventory_seams_from_index(&production_files, &cached.index)
        .into_iter()
        .filter(|seam| {
            seam.owner()
                .rsplit("::")
                .next()
                .is_some_and(|name| matched_call_names.contains(name))
        })
        .collect::<Vec<_>>();
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &cached.index);
    let classified = seam_classification::classify_seams_owned(seams, evidence);

    Ok(TargetedTestClassifiedSeamInventory {
        classified,
        selected_test_count: selected_tests.len(),
        direct_call_names: matched_call_names.into_iter().collect(),
        file_fact_cache: cached.file_fact_cache,
        workspace_cache_key,
    })
}

/// Kill-switch for the warm no-impact fast path (issue #3859, removal
/// control 12): `RIPR_NO_IMPACT_FAST_PATH=0` restores the
/// read-everything scoped path unconditionally. Any other value (or
/// unset) leaves the fast path eligible.
const NO_IMPACT_FAST_PATH_ENV: &str = "RIPR_NO_IMPACT_FAST_PATH";

/// Typed reason the warm no-impact fast path declined a diff (issue
/// #3859). Every reason runs the full conservative scoped path; the
/// reason is traced, never silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoImpactFallbackReason {
    ChangedRustSource,
    ChangedTestOrHarness,
    ChangedManifestOrConfig,
    ChangedSourceRoleAuthority,
    UnsupportedOrUnprovenChangeIdentity,
    MissingFingerprintMapping,
    IncompatibleWorkspaceIdentity,
    CorruptMetadata,
    CorpusChangedDuringResolution,
}

impl NoImpactFallbackReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ChangedRustSource => "changed_rust_source",
            Self::ChangedTestOrHarness => "changed_test_or_harness",
            Self::ChangedManifestOrConfig => "changed_manifest_or_config",
            Self::ChangedSourceRoleAuthority => "changed_source_role_authority",
            Self::UnsupportedOrUnprovenChangeIdentity => "unsupported_or_unproven_change_identity",
            Self::MissingFingerprintMapping => "missing_fingerprint_mapping",
            Self::IncompatibleWorkspaceIdentity => "incompatible_workspace_identity",
            Self::CorruptMetadata => "corrupt_metadata",
            Self::CorpusChangedDuringResolution => "corpus_changed_during_resolution",
        }
    }
}

/// Outcome of the warm no-impact probe (issue #3859). The
/// inventory is boxed: it outweighs the reason by two orders of
/// magnitude and the enum crosses the call boundary by value.
enum NoImpactOutcome {
    Fast(Box<ScopedClassifiedSeamInventory>),
    Declined(NoImpactFallbackReason),
}

fn no_impact_fast_path_disabled() -> bool {
    std::env::var(NO_IMPACT_FAST_PATH_ENV)
        .ok()
        .is_some_and(|value| value == "0")
}

/// Warm no-impact fast path (issue #3859). For a diff established not to
/// affect Rust seam/evidence semantics, rebuild the exact workspace
/// cache key from the admitted corpus fingerprint mapping and return
/// an empty scoped inventory without reading Rust source contents or
/// building the Rust index. Manifest reads still occur (source-role
/// context needs declared targets); Rust content reads, file-fact
/// recomputes, index construction, and semantic work are all zero.
///
/// Returns `Declined` with a typed reason for every ineligible input;
/// the caller traces the reason and runs the full conservative path.
/// Returns `Err` only when corpus discovery itself fails, matching the
/// full path's failure exactly.
fn try_no_impact_fast_path(
    root: &Path,
    config: &RiprConfig,
    changed_files: &[PathBuf],
    changed_owner_names: &[String],
) -> Result<NoImpactOutcome, String> {
    let fast_started = Instant::now();
    if changed_owner_names
        .iter()
        .any(|name| !name.trim().is_empty())
    {
        return Ok(NoImpactOutcome::Declined(
            NoImpactFallbackReason::ChangedRustSource,
        ));
    }
    // Discovery walks directories and stats files; it reads no file
    // contents. A discovery failure matches the full path's failure,
    // so propagate it rather than declining into an identical error.
    let (rust_files, fingerprint) = scan_corpus_fingerprint(root)?;
    let Some(fingerprint) = fingerprint else {
        // No stat field on this platform witnesses every content write
        // (issue #3848), so no signature is reusable evidence here.
        return Ok(NoImpactOutcome::Declined(
            NoImpactFallbackReason::UnsupportedOrUnprovenChangeIdentity,
        ));
    };
    let context = production_role_context(root, config, rust_files.iter().map(PathBuf::as_path));
    if let Some(reason) =
        no_impact_changed_path_fallback(changed_files, &rust_files, config, &context)
    {
        return Ok(NoImpactOutcome::Declined(reason));
    }
    let files_content_hash =
        match RepoCorpusFingerprintCache::at(root).lookup_detailed(root, &fingerprint) {
            CorpusFingerprintLookup::Hit(files_content_hash) => files_content_hash,
            CorpusFingerprintLookup::Missing => {
                return Ok(NoImpactOutcome::Declined(
                    NoImpactFallbackReason::MissingFingerprintMapping,
                ));
            }
            CorpusFingerprintLookup::Corrupt(_) => {
                return Ok(NoImpactOutcome::Declined(
                    NoImpactFallbackReason::CorruptMetadata,
                ));
            }
            CorpusFingerprintLookup::Incompatible(_) => {
                return Ok(NoImpactOutcome::Declined(
                    NoImpactFallbackReason::IncompatibleWorkspaceIdentity,
                ));
            }
        };
    let workspace_cache_key =
        workspace_key_inputs(root, config).cache_key(root, files_content_hash);
    let total_rust_files = rust_files.len();
    let total_production_files = rust_files
        .iter()
        .filter(|path| workspace::classify_with(path, &context).seeds_production_findings())
        .count();
    // The corpus may have moved between the scan and this point. A
    // re-stat is cheap (no content reads); any drift declines into the
    // full path rather than pairing the key with a changed corpus.
    if corpus_fingerprint(root, &rust_files).as_deref() != Some(fingerprint.as_str()) {
        return Ok(NoImpactOutcome::Declined(
            NoImpactFallbackReason::CorpusChangedDuringResolution,
        ));
    }
    trace_latency_phase(
        "diff_scoped_inventory",
        "no_impact_fast_path",
        fast_started.elapsed(),
    );
    Ok(NoImpactOutcome::Fast(Box::new(
        ScopedClassifiedSeamInventory {
            classified: Vec::new(),
            file_fact_cache: FileFactCacheStats::zero_work(),
            workspace_cache_key,
            total_rust_files,
            total_production_files,
            scoped_production_files: Vec::new(),
            changed_production_files: Vec::new(),
            immediate_caller_files: Vec::new(),
            unevaluated_seams: 0,
        },
    )))
}

/// Path-only eligibility for the warm no-impact fast path (issue
/// #3859). Returns the typed fallback reason for the first changed
/// path that is a represented analysis input; `None` means no changed
/// path can affect Rust seam/evidence semantics.
///
/// This is deliberately not a `.rs`-only extension shortcut: changed
/// tests invalidate evidence, and manifest/config/source-role movement
/// widens conservatively, each with its own reason.
fn no_impact_changed_path_fallback(
    changed_files: &[PathBuf],
    rust_files: &[PathBuf],
    config: &RiprConfig,
    context: &workspace::SourceRoleContext,
) -> Option<NoImpactFallbackReason> {
    let corpus: BTreeSet<String> = rust_files
        .iter()
        .map(|path| normalized_inventory_path(path))
        .collect();
    let as_set = |paths: &BTreeSet<PathBuf>| {
        paths
            .iter()
            .map(|path| normalized_inventory_path(path))
            .collect::<BTreeSet<_>>()
    };
    let production_like = as_set(config.analysis().production_like_targets());
    let declared_tests = as_set(&context.declared_test_targets);
    let declared_benches = as_set(&context.declared_bench_targets);
    let harness_targets: BTreeSet<String> = config
        .analysis()
        .test_harnesses()
        .iter()
        .map(|registration| normalized_inventory_path(registration.target.as_path()))
        .collect();
    let suppressions = normalized_inventory_path(config.suppressions().path());
    for changed in changed_files {
        let normalized = normalized_inventory_path(changed);
        if corpus.contains(&normalized) {
            // Indexed corpus member: any change reaches the index or
            // its test-grip evidence, whatever the role.
            return Some(rust_change_reason(workspace::classify_with(
                Path::new(&normalized),
                context,
            )));
        }
        if production_like.contains(&normalized) {
            return Some(NoImpactFallbackReason::ChangedRustSource);
        }
        if harness_targets.contains(&normalized) {
            return Some(NoImpactFallbackReason::ChangedTestOrHarness);
        }
        // Declared target membership precedes the extension check: a
        // deleted declared test target is no longer in the corpus, yet
        // its role authority moved, which the extension branch alone
        // would report as an ordinary test change.
        if declared_tests.contains(&normalized) || declared_benches.contains(&normalized) {
            return Some(NoImpactFallbackReason::ChangedSourceRoleAuthority);
        }
        if is_rust_source_file_name(&normalized) {
            // Rust source outside the discovered corpus (deleted file,
            // ignored directory): the full path would still resolve
            // owners or roles against it, so never shortcut.
            return Some(rust_change_reason(workspace::classify_with(
                Path::new(&normalized),
                context,
            )));
        }
        if is_manifest_or_config_path(&normalized, &suppressions) {
            return Some(NoImpactFallbackReason::ChangedManifestOrConfig);
        }
    }
    None
}

/// Fallback reason for a changed Rust source path from its
/// source role. Only an explicit evidence role earns the test
/// reason; a fixture role the model will not vouch for fails closed
/// as source.
fn rust_change_reason(role: workspace::SourceRole) -> NoImpactFallbackReason {
    if role.seeds_production_findings() {
        NoImpactFallbackReason::ChangedRustSource
    } else {
        match role {
            workspace::SourceRole::TestEvidence
            | workspace::SourceRole::BenchEvidence
            | workspace::SourceRole::ExampleEvidence => {
                NoImpactFallbackReason::ChangedTestOrHarness
            }
            _ => NoImpactFallbackReason::ChangedRustSource,
        }
    }
}

/// Rust source by file name, case-insensitive: an uppercase `.RS`
/// must never slip past as a documentation edit.
fn is_rust_source_file_name(normalized: &str) -> bool {
    normalized
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .rsplit('.')
        .next()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

/// Cargo manifest/lock/toolchain inputs plus the repository inputs the
/// workspace cache key derives from: `ripr.toml`, the test-intent
/// file, and the configured suppressions file.
fn is_manifest_or_config_path(normalized: &str, suppressions: &str) -> bool {
    if normalized == suppressions
        || normalized == crate::config::CONFIG_FILE_NAME
        || normalized == ".ripr/test_intent.toml"
    {
        return true;
    }
    if normalized.starts_with(".cargo/") {
        return true;
    }
    matches!(
        normalized.rsplit('/').next().unwrap_or_default(),
        "Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
    )
}

pub(crate) fn inventory_diff_scoped_classified_seams_at_with_config(
    root: &Path,
    config: &RiprConfig,
    changed_files: &[PathBuf],
    changed_owner_names: &[String],
) -> Result<ScopedClassifiedSeamInventory, String> {
    inventory_diff_scoped_classified_seams_inner(
        root,
        config,
        changed_files,
        changed_owner_names,
        !no_impact_fast_path_disabled(),
        None,
    )
}

/// [`inventory_diff_scoped_classified_seams_at_with_config`] with staged
/// evidence; see [`DiffScopeEvidenceStages`].
pub(crate) fn inventory_diff_scoped_classified_seams_staged_at_with_config(
    root: &Path,
    config: &RiprConfig,
    changed_files: &[PathBuf],
    changed_owner_names: &[String],
    stages: &DiffScopeEvidenceStages<'_>,
) -> Result<ScopedClassifiedSeamInventory, String> {
    inventory_diff_scoped_classified_seams_inner(
        root,
        config,
        changed_files,
        changed_owner_names,
        !no_impact_fast_path_disabled(),
        Some(stages),
    )
}

/// Shared body behind [`inventory_diff_scoped_classified_seams_at_with_config`].
/// The explicit `fast_path_enabled` flag is the removal control (issue
/// #3859, control 12): tests drive the disabled route directly instead
/// of mutating the process environment.
fn inventory_diff_scoped_classified_seams_inner(
    root: &Path,
    config: &RiprConfig,
    changed_files: &[PathBuf],
    changed_owner_names: &[String],
    fast_path_enabled: bool,
    stages: Option<&DiffScopeEvidenceStages<'_>>,
) -> Result<ScopedClassifiedSeamInventory, String> {
    cancellation::checkpoint()?;
    if fast_path_enabled {
        match try_no_impact_fast_path(root, config, changed_files, changed_owner_names) {
            Ok(NoImpactOutcome::Fast(inventory)) => {
                cancellation::checkpoint()?;
                return Ok(*inventory);
            }
            Ok(NoImpactOutcome::Declined(reason)) => {
                trace_latency_phase(
                    "diff_scoped_inventory",
                    &format!("no_impact_fallback_{}", reason.as_str()),
                    Duration::ZERO,
                );
            }
            Err(error) => return Err(error),
        }
    } else {
        trace_latency_phase(
            "diff_scoped_inventory",
            "no_impact_fast_path_disabled",
            Duration::ZERO,
        );
    }
    let state = collect_workspace_state(root, config)?;
    cancellation::checkpoint()?;
    let workspace_cache_key = state.cache_key();
    let total_rust_files = state.files.len();
    let production_files = production_files_from_state_with_role(&state, config);
    let total_production_files = production_files.len();
    let production_file_set = production_files
        .iter()
        .map(|path| normalized_inventory_path(path))
        .collect::<BTreeSet<_>>();
    let changed_file_set = changed_files
        .iter()
        .map(|path| normalized_inventory_path(path))
        .filter(|path| production_file_set.contains(path))
        .collect::<BTreeSet<_>>();

    let build_started = Instant::now();
    trace_latency_phase(
        "file_fact_cache",
        &format!(
            "start_review_scope_files_{}_production_{}",
            state.files.len(),
            production_files.len()
        ),
        Duration::ZERO,
    );
    let mut cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &state.workspace_root,
        &state.files,
        harness_registrations(config),
    )?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    cancellation::checkpoint()?;
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());

    cancellation::checkpoint()?;
    let caller_file_set = immediate_caller_file_set(
        &cached.index,
        &production_file_set,
        &changed_file_set,
        changed_owner_names,
    );
    let mut scoped_file_set = changed_file_set.clone();
    scoped_file_set.extend(caller_file_set.iter().cloned());

    let scoped_production_files = production_files
        .iter()
        .filter(|path| scoped_file_set.contains(&normalized_inventory_path(path)))
        .cloned()
        .collect::<Vec<_>>();
    let changed_production_files = production_files
        .iter()
        .filter(|path| changed_file_set.contains(&normalized_inventory_path(path)))
        .cloned()
        .collect::<Vec<_>>();
    let immediate_caller_files = production_files
        .iter()
        .filter(|path| caller_file_set.contains(&normalized_inventory_path(path)))
        .cloned()
        .collect::<Vec<_>>();

    cancellation::checkpoint()?;
    let seams_started = Instant::now();
    let seams = inventory_seams_from_index(&scoped_production_files, &cached.index);
    trace_latency_phase(
        "inventory_seams",
        "review_scope_ok",
        seams_started.elapsed(),
    );
    cancellation::checkpoint()?;
    let (classified, unevaluated_seams) = classify_scoped_seams(seams, &cached.index, stages)?;

    cancellation::checkpoint()?;
    Ok(ScopedClassifiedSeamInventory {
        classified,
        file_fact_cache: cached.file_fact_cache,
        workspace_cache_key,
        total_rust_files,
        total_production_files,
        scoped_production_files,
        changed_production_files,
        immediate_caller_files,
        unevaluated_seams,
    })
}

/// Classify the scoped seams, first stage only when `stages` accepts it.
/// Returns the classified seams and how many scoped seams were skipped.
/// Each stage rejects cancellation before classifying the evidence it
/// built, as [`classify_complete_scoped_evidence`] does for the full scope.
fn classify_scoped_seams(
    seams: Vec<RepoSeam>,
    index: &RustIndex,
    stages: Option<&DiffScopeEvidenceStages<'_>>,
) -> Result<(Vec<ClassifiedSeam>, usize), String> {
    let evidence_started = Instant::now();
    let pass = test_grip_evidence::EvidencePass::new(index);
    let mut evidence = Vec::new();
    let mut evaluated = vec![false; seams.len()];
    if let Some(stages) = stages {
        let first = seams
            .iter()
            .enumerate()
            .filter(|(_, seam)| (stages.first)(seam))
            .map(|(position, seam)| {
                evaluated[position] = true;
                seam.clone()
            })
            .collect::<Vec<_>>();
        trace_latency_phase(
            "evidence_for_seams",
            &format!(
                "review_scope_first_stage_seams_{}_of_{}",
                first.len(),
                seams.len()
            ),
            Duration::ZERO,
        );
        evidence = pass.evidence_for(&first);
        cancellation::checkpoint()?;
        let classified_first = seam_classification::classify_seams(&first, &evidence);
        if (stages.sufficient)(&classified_first) {
            trace_latency_phase(
                "evidence_for_seams",
                "review_scope_first_stage_sufficient",
                evidence_started.elapsed(),
            );
            return Ok((classified_first, seams.len() - first.len()));
        }
    }
    let rest = seams
        .iter()
        .zip(&evaluated)
        .filter(|(_, evaluated)| !**evaluated)
        .map(|(seam, _)| seam.clone())
        .collect::<Vec<_>>();
    trace_latency_phase(
        "evidence_for_seams",
        &format!("review_scope_start_seams_{}", rest.len()),
        Duration::ZERO,
    );
    // Evidence is per seam and pairs by id, so first-stage evidence
    // joins the rest without changing any seam's classification or the
    // original seam order.
    evidence.extend(pass.evidence_for(&rest));
    trace_latency_phase(
        "evidence_for_seams",
        "review_scope_ok",
        evidence_started.elapsed(),
    );
    Ok((classify_complete_scoped_evidence(seams, evidence)?, 0))
}

/// Non-fallible evidence construction can stop with a partial vector. This
/// admission boundary rejects cancellation before classifying that vector.
fn classify_complete_scoped_evidence(
    seams: Vec<RepoSeam>,
    evidence: Vec<test_grip_evidence::TestGripEvidence>,
) -> Result<Vec<ClassifiedSeam>, String> {
    cancellation::checkpoint()?;
    Ok(seam_classification::classify_seams_owned(seams, evidence))
}

fn immediate_caller_file_set(
    index: &RustIndex,
    production_file_set: &BTreeSet<String>,
    changed_file_set: &BTreeSet<String>,
    changed_owner_names: &[String],
) -> BTreeSet<String> {
    let owner_call_names = changed_owner_names
        .iter()
        .filter_map(|owner| owner.rsplit("::").next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if owner_call_names.is_empty() {
        return BTreeSet::new();
    }

    index
        .functions
        .iter()
        .filter(|function| !function.source_role.is_evidence_role())
        .filter_map(|function| {
            let file = normalized_inventory_path(&function.file);
            (production_file_set.contains(&file)
                && !changed_file_set.contains(&file)
                && function
                    .calls
                    .iter()
                    .any(|call| owner_call_names.contains(&call.name)))
            .then_some(file)
        })
        .collect()
}

fn normalized_inventory_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Return the effective seam limit and its source.
///
/// - Env var unset → `Some((DEFAULT_REPO_EXPOSURE_SEAM_LIMIT, Default))` — always-on cap.
/// - Env var = "0" (or parses to 0) → `None` — operator opt-out: unbounded.
/// - Env var = N > 0 → `Some((N, Configured))`.
pub(crate) fn repo_exposure_seam_limit() -> Option<(usize, SeamLimitSource)> {
    match std::env::var(REPO_EXPOSURE_SEAM_LIMIT_ENV) {
        Ok(value) => {
            // Explicit env: "0" means opt-out (unbounded); N>0 means configured.
            parse_repo_exposure_seam_limit(&value).map(|n| (n, SeamLimitSource::Configured))
        }
        Err(_) => {
            // Env var not set → apply the default cap.
            Some((DEFAULT_REPO_EXPOSURE_SEAM_LIMIT, SeamLimitSource::Default))
        }
    }
}

fn parse_repo_exposure_seam_limit(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|limit| *limit > 0)
}

pub(crate) fn apply_repo_exposure_seam_limit(seams: &mut Vec<RepoSeam>) -> Option<SeamLimitInfo> {
    let (limit, source) = repo_exposure_seam_limit()?;
    apply_repo_exposure_seam_limit_inner(seams, limit, source)
}

fn apply_repo_exposure_seam_limit_inner(
    seams: &mut Vec<RepoSeam>,
    limit: usize,
    source: SeamLimitSource,
) -> Option<SeamLimitInfo> {
    let total = seams.len();
    if total <= limit {
        return None;
    }
    seams.truncate(limit);
    trace_latency_phase(
        "repo_exposure_seam_limit",
        &format!("limit_{}_of_{total}", seams.len()),
        Duration::ZERO,
    );
    Some(SeamLimitInfo {
        analyzed: seams.len(),
        total,
        source,
    })
}

#[cfg(test)]
fn apply_repo_exposure_seam_limit_for_test(
    seams: &mut Vec<RepoSeam>,
    limit_and_source: Option<(usize, SeamLimitSource)>,
) -> Option<SeamLimitInfo> {
    let (limit, source) = limit_and_source?;
    apply_repo_exposure_seam_limit_inner(seams, limit, source)
}

/// Apply the pilot seam budget to an already-classified slice, returning
/// a `SeamLimitInfo` when the slice was truncated, or `None` when the full
/// slice fits within the budget.
///
/// The budget is resolved in priority order:
/// 1. `RIPR_PILOT_SEAM_BUDGET=0` → unbounded (operator opt-out).
/// 2. `RIPR_PILOT_SEAM_BUDGET=N` (N > 0) → configured cap N.
/// 3. Env var unset → `DEFAULT_PILOT_SEAM_BUDGET` (always-on default).
pub(crate) fn apply_pilot_seam_budget(
    classified: &mut Vec<super::seam_classification::ClassifiedSeam>,
) -> Option<SeamLimitInfo> {
    let (limit, source) = pilot_seam_budget()?;
    apply_pilot_seam_budget_inner(classified, limit, source)
}

fn apply_pilot_seam_budget_inner(
    classified: &mut Vec<super::seam_classification::ClassifiedSeam>,
    limit: usize,
    source: SeamLimitSource,
) -> Option<SeamLimitInfo> {
    let total = classified.len();
    if total <= limit {
        return None;
    }
    classified.truncate(limit);
    Some(SeamLimitInfo {
        analyzed: classified.len(),
        total,
        source,
    })
}

/// Return the effective pilot seam budget and its source.
///
/// - Env var unset → `Some((DEFAULT_PILOT_SEAM_BUDGET, Default))`.
/// - Env var = `"0"` → `None` (operator opt-out: unbounded).
/// - Env var = N > 0 → `Some((N, Configured))`.
pub(crate) fn pilot_seam_budget() -> Option<(usize, SeamLimitSource)> {
    match std::env::var(PILOT_SEAM_BUDGET_ENV) {
        Ok(value) => {
            parse_repo_exposure_seam_limit(&value).map(|n| (n, SeamLimitSource::Configured))
        }
        Err(_) => Some((DEFAULT_PILOT_SEAM_BUDGET, SeamLimitSource::Default)),
    }
}

#[cfg(test)]
fn inventory_seam_grip_class_counts_from_state_with_config(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Result<SeamGripClassCounts, String> {
    let production_files = production_files_from_state_with_role(state, config);
    let build_started = Instant::now();
    trace_latency_phase(
        "file_fact_cache",
        &format!(
            "start_files_{}_production_{}",
            state.files.len(),
            production_files.len()
        ),
        Duration::ZERO,
    );
    let mut cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &state.workspace_root,
        &state.files,
        harness_registrations(config),
    )?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    let seams = inventory_seams_from_index(&production_files, &cached.index);
    let mut counts = SeamGripClassCounts::new(seams.len());
    let context = test_grip_evidence::CompactGripContext::new(&cached.index);
    for seam in &seams {
        let evidence = test_grip_evidence::compact_evidence_for_seam(seam, &context);
        let class = seam_classification::classify_seam(seam, &evidence);
        counts.increment(class);
    }
    Ok(counts)
}

/// Exact registered test-harness target identities (#3532), normalized to
/// the forward-slashed workspace-relative form the role context compares.
/// Validated against parsed Cargo target metadata (#3608): only
/// registrations whose owning manifest declares the target
/// `harness = false` keep the file-wide evidence-role grant.
fn harness_targets_from_config(
    root: &Path,
    config: &RiprConfig,
) -> std::collections::BTreeSet<PathBuf> {
    rust_index::validated_file_wide_harness_targets(root, harness_registrations(config))
}

/// Registration list for the harness-aware index builds (#3532). Empty
/// without registrations; the index build is then a no-op for the
/// registry and every output stays byte-identical.
fn harness_registrations(config: &RiprConfig) -> &[crate::config::TestHarnessRegistration] {
    config.analysis().test_harnesses()
}

fn production_files_from_state_with_role(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Vec<PathBuf> {
    let context = production_role_context(
        &state.workspace_root,
        config,
        state.files.iter().map(|(path, _)| path.as_path()),
    );
    state
        .files
        .iter()
        .map(|(path, _)| path)
        .filter(|path| workspace::classify_with(path, &context).seeds_production_findings())
        .cloned()
        .collect()
}

/// Producer-owned source role (#3283): layout plus declared Cargo
/// targets plus the repository production-like opt-in. Shared by the
/// full inventory path and the warm no-impact fast path (issue #3859)
/// so both derive the identical production set; the fast path feeds it
/// discovered paths instead of read file contents, which is exact
/// because role classification never consults file bytes.
fn production_role_context<'a, I>(
    workspace_root: &Path,
    config: &RiprConfig,
    files: I,
) -> workspace::SourceRoleContext
where
    I: IntoIterator<Item = &'a Path>,
{
    let mut context = workspace::context_for_files(workspace_root, files);
    context.production_like_targets = config.analysis().production_like_targets().clone();
    context.harness_targets = harness_targets_from_config(workspace_root, config);
    context
}

/// Collect the per-file content + intent + suppressions inputs the
/// cache key derives from. The repo exposure cold path reuses these
/// bytes when building cached file facts so file discovery and file
/// reads are not repeated after a classified-seam cache miss.
///
/// Hashes the **same Rust file set fed to `build_index`** — production
/// seam sources *and* test evidence sources. `ClassifiedSeam` carries
/// `TestGripEvidence` derived from test files, so a test-only edit must
/// invalidate the cache; filtering to production-only here would let
/// stale grip evidence survive a test rewrite.
fn collect_workspace_state(
    root: &Path,
    config: &RiprConfig,
) -> Result<OwnedWorkspaceState, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    collect_workspace_state_from_files(root, config, rust_files)
}

/// Read the contents of a pre-discovered corpus file list. Callers that
/// already ran discovery for the corpus fingerprint scan (issue #2108)
/// pass the list through so the directory walk is not repeated.
fn collect_workspace_state_from_files(
    root: &Path,
    config: &RiprConfig,
    rust_files: Vec<PathBuf>,
) -> Result<OwnedWorkspaceState, String> {
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(rust_files.len());
    for path in rust_files {
        cancellation::checkpoint()?;
        let bytes = std::fs::read(root.join(&path))
            .map_err(|err| format!("read {} failed: {err}", path.display()))?;
        files.push((path, bytes));
    }
    Ok(OwnedWorkspaceState {
        workspace_root: root.to_path_buf(),
        files,
        config_text: config.source_text().map(str::to_string),
        test_intent_text: read_optional(&root.join(".ripr").join("test_intent.toml")),
        suppressions_text: read_optional(&root.join(config.suppressions().path())),
    })
}

/// Discover the corpus and compute its stat-only fingerprint in one pass
/// (issue #2108). The fingerprint is `None` when any file cannot be
/// stat'd, and on any platform where [`corpus_fingerprint`] refuses to
/// sign because no field of the stat tuple is a content-change witness
/// (issue #3848); callers then fall back to the always-correct content
/// read.
fn scan_corpus_fingerprint(root: &Path) -> Result<(Vec<PathBuf>, Option<String>), String> {
    let rust_files = workspace::discover_rust_files(root)?;
    let fingerprint = corpus_fingerprint(root, &rust_files);
    Ok((rust_files, fingerprint))
}

/// Cache-key inputs other than the corpus content hash, in owned form so
/// the fingerprint fast path can build a key without holding file bytes.
struct WorkspaceKeyInputs {
    cfg_features: Option<String>,
    config_text: Option<String>,
    test_intent_text: Option<String>,
    suppressions_text: Option<String>,
}

fn workspace_key_inputs(root: &Path, config: &RiprConfig) -> WorkspaceKeyInputs {
    WorkspaceKeyInputs {
        cfg_features: std::env::var("RIPR_CFG_FEATURES").ok(),
        config_text: config.source_text().map(str::to_string),
        test_intent_text: read_optional(&root.join(".ripr").join("test_intent.toml")),
        suppressions_text: read_optional(&root.join(config.suppressions().path())),
    }
}

impl WorkspaceKeyInputs {
    fn cache_key(&self, root: &Path, files_content_hash: String) -> RepoSeamCacheKey {
        WorkspaceKeyContext {
            workspace_root: root,
            cfg_features: self.cfg_features.as_deref(),
            config_text: self.config_text.as_deref(),
            test_intent_text: self.test_intent_text.as_deref(),
            suppressions_text: self.suppressions_text.as_deref(),
        }
        .cache_key(files_content_hash)
    }
}

/// Resolve the workspace cache key from the corpus fingerprint store
/// (issue #2108). Returns `Some(key)` only when the store holds a mapping
/// for the corpus's current stat-only signature; that key is byte-identical
/// to the key a full content read would compute, so a cache hit through it
/// is exactly as authoritative as one through the read-everything path.
fn fingerprint_cached_workspace_key(
    root: &Path,
    inputs: &WorkspaceKeyInputs,
    fingerprint: Option<&str>,
) -> Option<RepoSeamCacheKey> {
    let stored_hash = RepoCorpusFingerprintCache::at(root).lookup(root, fingerprint?)?;
    Some(inputs.cache_key(root, stored_hash))
}

/// Persist `fingerprint -> files_content_hash` after a full content read
/// (issue #2108). The mapping is stored only when the corpus signature is
/// identical before and after the read, so a fingerprint can never be
/// paired with a hash derived from a corpus with a *different* signature.
/// Best-effort: a store failure degrades the next run to the old
/// read-everything path and is logged, never fatal.
fn store_corpus_fingerprint_mapping(
    state: &OwnedWorkspaceState,
    pre_read_fingerprint: Option<String>,
    key: &RepoSeamCacheKey,
) {
    let Some(fingerprint) = pre_read_fingerprint else {
        return;
    };
    let paths: Vec<PathBuf> = state.files.iter().map(|(path, _)| path.clone()).collect();
    let post_read = corpus_fingerprint(&state.workspace_root, &paths);
    if post_read.as_deref() != Some(fingerprint.as_str()) {
        // The corpus changed while it was being read; the signature no
        // longer describes the hashed bytes, so storing would be dishonest.
        return;
    }
    if let Err(reason) = RepoCorpusFingerprintCache::at(&state.workspace_root).store(
        &state.workspace_root,
        &fingerprint,
        &key.files_content_hash,
    ) {
        eprintln!("ripr: corpus fingerprint cache store ignored ({reason})");
    }
}

fn read_optional(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Owned form of `WorkspaceState` so the inventory function can return
/// it across the cache call boundary. `WorkspaceState` borrows; this
/// converts to it on demand.
struct OwnedWorkspaceState {
    workspace_root: PathBuf,
    files: Vec<(PathBuf, Vec<u8>)>,
    config_text: Option<String>,
    test_intent_text: Option<String>,
    suppressions_text: Option<String>,
}

impl OwnedWorkspaceState {
    fn cache_key(&self) -> super::seam_cache::RepoSeamCacheKey {
        let cfg_features = std::env::var("RIPR_CFG_FEATURES").ok();
        WorkspaceState {
            workspace_root: &self.workspace_root,
            files: &self.files,
            cfg_features: cfg_features.as_deref(),
            config_text: self.config_text.as_deref(),
            test_intent_text: self.test_intent_text.as_deref(),
            suppressions_text: self.suppressions_text.as_deref(),
        }
        .cache_key()
    }
}

/// Inventory seams from a pre-built index. Public(crate) so tests can
/// drive the walker without re-running file discovery.
pub(crate) fn inventory_seams_from_index(
    production_files: &[PathBuf],
    index: &RustIndex,
) -> Vec<RepoSeam> {
    if let Some(disclosure) = rust_index::lexical_fallback_disclosure(index) {
        eprintln!("{disclosure}");
    }
    if let Some(disclosure) = rust_index::include_resolution_disclosure(index) {
        eprintln!("{disclosure}");
    }
    if let Some(disclosure) = rust_index::module_composition_disclosure(index) {
        eprintln!("{disclosure}");
    }
    let mut seams: Vec<RepoSeam> = Vec::new();

    // Iterate `production_files` in caller-given order, but the final
    // sort below makes the output independent of that order anyway.
    for path in production_files {
        let Some(facts) = index.files.get(path) else {
            continue;
        };
        for shape in &facts.probe_shapes {
            let Some(seam) = build_seam_from_shape(path, shape, index) else {
                continue;
            };
            seams.push(seam);
        }
    }

    // Stable order: file, byte offset, kind, owner — matches the
    // canonical seam ID fields exactly so the sort key and the dedup
    // key agree. Without `owner` in the sort, two seams with the same
    // (file, byte_offset, kind) but different owners would still be
    // adjacent after sorting (one byte belongs to one function), but
    // having the keys aligned makes the contract explicit.
    seams.sort_by(|a, b| {
        a.file()
            .cmp(b.file())
            .then(a.byte_offset().cmp(&b.byte_offset()))
            .then(a.kind().as_str().cmp(b.kind().as_str()))
            .then(a.owner().cmp(b.owner()))
    });

    // Two probe shapes can land at the same byte offset with the same
    // kind (e.g., a predicate counted by multiple traversal passes).
    // Dedup by canonical seam fields so the output is set-like.
    seams.dedup_by(|a, b| {
        a.file() == b.file()
            && a.byte_offset() == b.byte_offset()
            && a.kind() == b.kind()
            && a.owner() == b.owner()
    });

    seams
}

fn build_seam_from_shape(
    path: &Path,
    shape: &ProbeShapeFact,
    index: &RustIndex,
) -> Option<RepoSeam> {
    let kind = seam_kind_from_probe_shape(&shape.kind)?;
    let owner_fact = rust_index::find_owner_function(index, path, shape.start_line)?;
    // Skip shapes whose owner is itself a test function (e.g.,
    // `#[test] fn ...` inside an in-file `#[cfg(test)] mod tests`).
    // the source-role model already excludes physical test files;
    // this catches inline test modules.
    if owner_fact.source_role.is_evidence_role() {
        return None;
    }
    // `FunctionFact.id` is built from `path.display()`, which uses native
    // separators (`\` on Windows, `/` elsewhere). Normalize so seam IDs
    // are stable across platforms.
    let owner = owner_fact.id.0.replace('\\', "/");
    let expression = shape.text.clone();
    let required_discriminator = required_discriminator_for(kind, &expression);
    let expected_sink = expected_sink_for(kind);
    Some(RepoSeam::new(
        path,
        owner,
        kind,
        shape.start_byte,
        shape.start_line,
        expression,
        required_discriminator,
        expected_sink,
    ))
}

fn seam_kind_from_probe_shape(kind: &str) -> Option<SeamKind> {
    match kind {
        PROBE_SHAPE_PREDICATE => Some(SeamKind::PredicateBoundary),
        PROBE_SHAPE_RETURN_VALUE => Some(SeamKind::ReturnValue),
        PROBE_SHAPE_ERROR_PATH => Some(SeamKind::ErrorVariant),
        PROBE_SHAPE_FIELD_CONSTRUCTION => Some(SeamKind::FieldConstruction),
        PROBE_SHAPE_SIDE_EFFECT => Some(SeamKind::SideEffect),
        PROBE_SHAPE_MATCH_ARM => Some(SeamKind::MatchArm),
        // The diff-scoped probe shape "call_deletion" represents the
        // syntax of a call site. In repo scope the same shape is the
        // seam asking "are tests verifying this call happens at all?"
        // — i.e. `SeamKind::CallPresence`.
        PROBE_SHAPE_CALL_DELETION => Some(SeamKind::CallPresence),
        _ => None,
    }
}

fn required_discriminator_for(kind: SeamKind, expression: &str) -> RequiredDiscriminator {
    match kind {
        SeamKind::PredicateBoundary => RequiredDiscriminator::BoundaryValue {
            description: expression.to_string(),
        },
        SeamKind::ErrorVariant => RequiredDiscriminator::ErrorVariant {
            // Store the producer-owned identity, not the surrounding return
            // expression. Activation evidence and route compatibility both
            // speak in terms of the exact error variant. Preserve an
            // unparseable expression so downstream checks remain fail-closed.
            variant: exact_error_variant(expression).unwrap_or_else(|| expression.to_string()),
        },
        SeamKind::ReturnValue => RequiredDiscriminator::ReturnValue {
            description: expression.to_string(),
        },
        SeamKind::FieldConstruction => RequiredDiscriminator::FieldValue {
            field: expression.to_string(),
        },
        SeamKind::SideEffect => RequiredDiscriminator::Effect {
            sink: expression.to_string(),
        },
        SeamKind::MatchArm => RequiredDiscriminator::MatchArmTaken {
            arm: expression.to_string(),
        },
        SeamKind::CallPresence => RequiredDiscriminator::CallSite {
            target: expression.to_string(),
        },
    }
}

fn expected_sink_for(kind: SeamKind) -> ExpectedSink {
    match kind {
        SeamKind::PredicateBoundary | SeamKind::ReturnValue | SeamKind::MatchArm => {
            ExpectedSink::ReturnValue
        }
        SeamKind::ErrorVariant => ExpectedSink::ErrorChannel,
        SeamKind::FieldConstruction => ExpectedSink::OutputField,
        SeamKind::SideEffect | SeamKind::CallPresence => ExpectedSink::SideEffect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;

    #[test]
    fn cache_receipt_retains_producer_order_and_portable_failure_rows() {
        let mut stats = FileFactCacheStats::zero_work();
        stats.record_store_failure(
            PathBuf::from("src/foo..rs"),
            super::super::seam_cache::FileFactStoreError {
                stage: super::super::seam_cache::FileFactStoreStage::Write,
                message: "portable failure".to_string(),
            },
        );
        stats.record_store_failure(
            PathBuf::from("src\\bar.rs"),
            super::super::seam_cache::FileFactStoreError {
                stage: super::super::seam_cache::FileFactStoreStage::Encode,
                message: "second failure".to_string(),
            },
        );
        let value = file_fact_cache_receipt(&stats);
        assert_eq!(value["store_errors"], 2);
        assert_eq!(value["store_failures"][0]["path"], "src/foo..rs");
        assert_eq!(value["store_failures"][0]["stage"], "write");
        // A backslash is a separator only on Windows; on Unix it is part of
        // the file name, and rewriting it would name a different file.
        let expected_bar = if cfg!(windows) {
            serde_json::json!("src/bar.rs")
        } else {
            serde_json::Value::Null
        };
        assert_eq!(value["store_failures"][1]["path"], expected_bar);
        assert_eq!(value["store_failures"][1]["stage"], "encode");
        assert_eq!(value["store_failures_dropped"], 0);
    }

    #[cfg(unix)]
    #[test]
    fn cache_receipt_names_non_utf8_failure_paths_as_unrepresentable() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let mut stats = FileFactCacheStats::zero_work();
        stats.record_store_failure(
            Path::new("src").join(OsStr::from_bytes(b"bad\xff.rs")),
            super::super::seam_cache::FileFactStoreError {
                stage: super::super::seam_cache::FileFactStoreStage::Write,
                message: "lossy failure".to_string(),
            },
        );
        let value = file_fact_cache_receipt(&stats);
        assert_eq!(value["store_errors"], 1);
        assert_eq!(value["store_failures"][0]["path"], serde_json::Value::Null);
    }

    #[test]
    fn only_custom_harness_targets_receive_file_wide_evidence_role() -> Result<(), String> {
        // #3532 review: a registered attribute applies to individual
        // functions, so its target must not enter the file-wide harness
        // evidence set — a mixed production file keeps seeding seams.
        // #3608: a custom_harness target additionally needs its
        // `harness = false` premise confirmed by the parsed Cargo target
        // metadata; an undeclared target keeps nothing.
        let config = crate::config::tests_only_parse(
            r#"[analysis]
[[analysis.test_harnesses]]
registration_id = "mimic"
target = "tests/mimic.rs"
kind = "custom_harness"
adapter = "libtest_mimic_v1"
marker = "libtest_mimic::Trial"

[[analysis.test_harnesses]]
registration_id = "contract"
target = "src/lib.rs"
kind = "registered_attribute"
adapter = "exact_attribute_v1"
marker = "myco::contract_test"

[[analysis.test_harnesses]]
registration_id = "misdeclared"
target = "src/misdeclared.rs"
kind = "custom_harness"
adapter = "libtest_mimic_v1"
marker = "libtest_mimic::Trial"
"#,
        )
        .map_err(|error| format!("fixture config parses: {error}"))?;
        let root = std::env::temp_dir().join(format!(
            "ripr-harness-role-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        // A name-only target reaches metadata's inventory only when its
        // conventional-layout file exists (#3634).
        std::fs::create_dir_all(root.join("tests")).map_err(|error| error.to_string())?;
        std::fs::write(root.join("tests/mimic.rs"), "").map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = 'role-fixture'\nversion = '0.1.0'\n\n[workspace]\n\n[[test]]\nname = 'mimic'\nharness = false\n",
        )
        .map_err(|error| error.to_string())?;
        let targets = harness_targets_from_config(&root, &config);
        assert_eq!(
            targets,
            std::iter::once(PathBuf::from("tests/mimic.rs"))
                .collect::<std::collections::BTreeSet<_>>(),
            "the declared harness = false target keeps the grant; the attribute \
             target never has it and the undeclared custom target loses it"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
    use crate::analysis::rust_index::{
        FileFacts, FunctionFact, RaRustSyntaxAdapter, RustSyntaxAdapter,
    };
    use crate::analysis::seam_cache::files_content_hash;
    use crate::domain::SymbolId;

    fn index_from_files(files: &[(PathBuf, &str)]) -> Result<RustIndex, String> {
        let adapter = RaRustSyntaxAdapter;
        let mut index = RustIndex::default();
        for (path, source) in files {
            let facts = adapter.summarize_file(path, source)?;
            index.files.insert(path.clone(), facts);
            index
                .functions
                .extend(index.files[path].functions.iter().cloned());
        }
        Ok(index)
    }

    /// Seam id, class and the full evidence payload, so a stage that
    /// changed any rendered evidence (related tests, stages, values)
    /// shows up, not only a changed class.
    fn class_by_id(classified: &[ClassifiedSeam]) -> Vec<(String, &'static str, String)> {
        classified
            .iter()
            .map(|entry| {
                (
                    entry.seam.id().as_str().to_string(),
                    entry.class.as_str(),
                    serde_json::to_string(&entry.evidence).unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn staged_scope_evidence_skips_the_rest_only_when_the_first_stage_is_sufficient()
    -> Result<(), String> {
        let prod = PathBuf::from("src/pricing.rs");
        let prod_source = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}

pub fn taxed_total(amount: i32) -> i32 {
    if amount > 100 { amount + 5 } else { amount }
}
"#;
        let test_path = PathBuf::from("tests/pricing.rs");
        let test_source = r#"
#[test]
fn discounted_total_applies_at_threshold() {
    assert_eq!(discounted_total(100, 100), 90);
}

#[test]
fn taxed_total_runs() {
    let _ = taxed_total(200);
}
"#;
        let index = index_from_files(&[(prod.clone(), prod_source), (test_path, test_source)])?;
        let seams = inventory_seams_from_index(&[prod], &index);
        let (unstaged, unstaged_skipped) = classify_scoped_seams(seams.clone(), &index, None)?;
        let first = |seam: &RepoSeam| seam.owner().ends_with("discounted_total");
        let first_ids = unstaged
            .iter()
            .filter(|entry| first(&entry.seam))
            .cloned()
            .collect::<Vec<_>>();
        let first_ids = class_by_id(&first_ids);
        if first_ids.iter().any(|(_, _, evidence)| evidence.is_empty()) {
            return Err("evidence must serialize".to_string());
        }
        if unstaged_skipped != 0 || first_ids.is_empty() || first_ids.len() == unstaged.len() {
            return Err(format!(
                "fixture must split into both stages: {} of {} seams first, {unstaged_skipped} skipped",
                first_ids.len(),
                unstaged.len()
            ));
        }

        let offered = std::cell::Cell::new(0);
        let accept = |classified: &[ClassifiedSeam]| {
            offered.set(classified.len());
            true
        };
        let (staged, skipped) = classify_scoped_seams(
            seams.clone(),
            &index,
            Some(&DiffScopeEvidenceStages {
                first: &first,
                sufficient: &accept,
            }),
        )?;
        assert_eq!(offered.get(), first_ids.len());
        assert_eq!(class_by_id(&staged), first_ids);
        assert_eq!(skipped, unstaged.len() - first_ids.len());

        let decline = |_: &[ClassifiedSeam]| false;
        let (declined, declined_skipped) = classify_scoped_seams(
            seams,
            &index,
            Some(&DiffScopeEvidenceStages {
                first: &first,
                sufficient: &decline,
            }),
        )?;
        assert_eq!(declined_skipped, 0);
        assert_eq!(class_by_id(&declined), class_by_id(&unstaged));
        Ok(())
    }

    #[test]
    fn scoped_evidence_rejects_deadline_after_the_final_seam() -> Result<(), String> {
        use crate::analysis::cancellation::{AnalysisCancellationToken, AnalysisClock, with_token};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let path = PathBuf::from("src/lib.rs");
        let mut index = index_from_files(&[(
            path.clone(),
            r#"
pub fn eligible(value: i32) -> bool { if value >= 10 { true } else { false } }
#[cfg(test)] mod tests {
    #[test] fn equality() { assert_eq!(super::eligible(10), true); }
}
"#,
        )])?;
        index.tests = index
            .files
            .values()
            .flat_map(|facts| facts.tests.iter().cloned())
            .collect();
        let seams = inventory_seams_from_index(&[path], &index);
        if seams.is_empty() || index.tests.is_empty() {
            return Err(
                "late deadline fixture must contain production seams and a test".to_string(),
            );
        }
        let started = Instant::now();
        let observations = Arc::new(AtomicUsize::new(0));
        let clock_observations = Arc::clone(&observations);
        let clock: AnalysisClock = Arc::new(move || {
            clock_observations.fetch_add(1, Ordering::SeqCst);
            started
        });
        let token = AnalysisCancellationToken::with_budget(started, Duration::from_secs(1), clock);
        let complete = with_token(&token, || {
            test_grip_evidence::evidence_for_seams(&seams, &index)
        });
        if complete.len() != seams.len() {
            return Err("unexpired evidence must cover every selected seam".to_string());
        }
        let last_evidence_observation = observations.load(Ordering::SeqCst);
        if last_evidence_observation <= seams.len() {
            return Err("fixture did not enter evidence context construction".to_string());
        }
        let expected = serde_json::to_value(classify_complete_scoped_evidence(
            seams.clone(),
            complete.clone(),
        )?)
        .map_err(|error| format!("serialize complete baseline: {error}"))?;
        let clock_calls = Arc::new(AtomicUsize::new(0));
        let owned_calls = Arc::clone(&clock_calls);
        let deadline = AnalysisCancellationToken::with_budget(
            started,
            Duration::from_secs(1),
            Arc::new(move || {
                let call = owned_calls.fetch_add(1, Ordering::SeqCst);
                if call >= last_evidence_observation {
                    started + Duration::from_secs(1)
                } else {
                    started
                }
            }),
        );
        let result = with_token(&deadline, || {
            let evidence = test_grip_evidence::evidence_for_seams(&seams, &index);
            if clock_calls.load(Ordering::SeqCst) != last_evidence_observation
                || serde_json::to_value(&evidence).map_err(|error| error.to_string())?
                    != serde_json::to_value(&complete).map_err(|error| error.to_string())?
            {
                return Err("deadline fixture expired before the final evidence result".to_string());
            }
            classify_complete_scoped_evidence(seams.clone(), evidence)
        });
        if !result.is_err_and(|error| error.contains("DeadlineExceeded"))
            || deadline.abort_kind() != Some(cancellation::AnalysisAbortKind::DeadlineExceeded)
        {
            return Err("final evidence deadline was classified as complete".to_string());
        }
        // Deadline-free consumers retain identical seam identities/evidence.
        let actual = serde_json::to_value(classify_complete_scoped_evidence(seams, complete)?)
            .map_err(|error| format!("serialize deadline-free evidence: {error}"))?;
        if actual != expected {
            return Err("deadline-free classification changed canonical evidence".to_string());
        }
        Ok(())
    }

    #[test]
    fn given_production_predicate_shape_when_repo_inventory_runs_then_predicate_boundary_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/pricing.rs");
        let source = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams
            .iter()
            .any(|s| s.kind() == SeamKind::PredicateBoundary)
        {
            return Err(format!(
                "expected at least one PredicateBoundary seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        let predicate_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "missing predicate seam".to_string())?;
        if !predicate_seam.owner().contains("discounted_total") {
            return Err(format!(
                "predicate seam owner should contain discounted_total, got {}",
                predicate_seam.owner()
            ));
        }
        Ok(())
    }

    #[test]
    fn given_test_file_predicate_shape_when_repo_inventory_runs_then_no_production_seam_is_emitted()
    -> Result<(), String> {
        let prod = PathBuf::from("src/lib.rs");
        let prod_source = "pub fn dummy() {}\n";
        let test_path = PathBuf::from("tests/some_test.rs");
        let test_source = r#"
#[test]
fn predicate_inside_test() {
    let x = 5;
    if x >= 3 {
        assert!(true);
    }
}
"#;
        let index = index_from_files(&[
            (prod.clone(), prod_source),
            (test_path.clone(), test_source),
        ])?;
        // Caller filters production files exactly the way the role model
        // does: evidence role never enters the production set.
        let context = workspace::SourceRoleContext::empty();
        let production_files: Vec<PathBuf> = [prod, test_path.clone()]
            .into_iter()
            .filter(|p| workspace::classify_with(p, &context).seeds_production_findings())
            .collect();

        if production_files.iter().any(|p| p == &test_path) {
            return Err("test file should not be in production_files".to_string());
        }

        let seams = inventory_seams_from_index(&production_files, &index);
        for seam in &seams {
            let path_str = seam.file().to_string_lossy();
            if path_str.contains("tests/") || path_str.contains("tests\\") {
                return Err(format!(
                    "seam emitted from a test file: {} (kind {})",
                    path_str,
                    seam.kind().as_str()
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn given_same_files_in_different_walk_order_when_repo_inventory_runs_then_seam_ids_are_stable()
    -> Result<(), String> {
        let a = PathBuf::from("src/a.rs");
        let a_src = r#"
pub fn check_a(x: i32) -> bool {
    x > 5
}
"#;
        let b = PathBuf::from("src/b.rs");
        let b_src = r#"
pub fn check_b(x: i32) -> i32 {
    if x < 0 { return -1; }
    x
}
"#;
        let index = index_from_files(&[(a.clone(), a_src), (b.clone(), b_src)])?;

        let forward = inventory_seams_from_index(&[a.clone(), b.clone()], &index);
        let reversed = inventory_seams_from_index(&[b.clone(), a.clone()], &index);

        let forward_ids: Vec<&str> = forward.iter().map(|s| s.id().as_str()).collect();
        let reversed_ids: Vec<&str> = reversed.iter().map(|s| s.id().as_str()).collect();
        if forward_ids != reversed_ids {
            return Err(format!(
                "seam IDs depend on input order:\n  forward:  {forward_ids:?}\n  reversed: {reversed_ids:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn given_error_path_shape_when_repo_inventory_runs_then_error_variant_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/parse.rs");
        let source = r#"
pub fn parse(value: &str) -> Result<i32, String> {
    if value.is_empty() {
        return Err("empty input".to_string());
    }
    value
        .parse::<i32>()
        .map_err(|err| format!("parse failed: {err}"))
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams.iter().any(|s| s.kind() == SeamKind::ErrorVariant) {
            return Err(format!(
                "expected at least one ErrorVariant seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    #[test]
    fn error_variant_discriminator_stores_exact_variant_identity() {
        assert_eq!(
            required_discriminator_for(
                SeamKind::ErrorVariant,
                "return Err(AuthError::RevokedToken);",
            ),
            RequiredDiscriminator::ErrorVariant {
                variant: "AuthError::RevokedToken".to_string(),
            }
        );
    }

    #[test]
    fn unparseable_error_variant_discriminator_stays_fail_closed() {
        let expression = "return Err(format!(\"failed: {reason}\"));";
        assert_eq!(
            required_discriminator_for(SeamKind::ErrorVariant, expression),
            RequiredDiscriminator::ErrorVariant {
                variant: expression.to_string(),
            }
        );
    }

    #[test]
    fn given_field_construction_shape_when_repo_inventory_runs_then_field_construction_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/build.rs");
        let source = r#"
pub struct Quote {
    pub amount: i32,
    pub fee: i32,
}

pub fn build_quote(amount: i32, fee: i32) -> Quote {
    Quote {
        amount: amount,
        fee: fee,
    }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams
            .iter()
            .any(|s| s.kind() == SeamKind::FieldConstruction)
        {
            return Err(format!(
                "expected at least one FieldConstruction seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    #[test]
    fn seam_inventory_omits_seams_with_no_owner_function() -> Result<(), String> {
        let path = PathBuf::from("src/orphan.rs");
        // A bare `if` at module scope has no owner function. The walker
        // must skip it so `RepoSeam.owner` is always meaningful.
        let source = "pub const X: i32 = if true { 1 } else { 0 };\n";
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        for seam in &seams {
            if seam.owner().is_empty() {
                return Err("seam emitted with empty owner".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn seam_inventory_maps_call_sites_to_call_presence_and_side_effect_sink() -> Result<(), String>
    {
        let path = PathBuf::from("src/service.rs");
        let source = r#"
pub fn run(flag: bool) {
    if flag {
        notify();
    }
}

fn notify() {}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);
        let kinds = seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>();
        assert!(
            kinds.contains(&SeamKind::CallPresence.as_str()),
            "expected a CallPresence seam, got {kinds:?}"
        );
        let call_presence = seams
            .iter()
            .find(|s| s.kind() == SeamKind::CallPresence)
            .ok_or("CallPresence seam kind should have a matching seam")?;
        assert!(matches!(
            call_presence.required_discriminator(),
            RequiredDiscriminator::CallSite { .. }
        ));
        assert_eq!(call_presence.expected_sink(), ExpectedSink::SideEffect);
        Ok(())
    }

    #[test]
    fn seam_inventory_skips_inline_test_functions_inside_production_files() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let source = r#"
pub fn production_fn(x: i32) -> bool {
    x > 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn inline_test() {
        assert!(2 > 1);
    }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);
        let owners = seams.iter().map(|s| s.owner()).collect::<Vec<_>>();
        assert!(
            !owners.iter().any(|owner| owner.contains("inline_test")),
            "expected inline #[test] owner to be filtered out, got owners {owners:?}"
        );
        Ok(())
    }

    #[test]
    fn seam_inventory_maps_rich_production_source_to_supported_seam_kinds() -> Result<(), String> {
        let path = PathBuf::from("src/quotes.rs");
        let source = r#"
pub fn classify(amount: i32, service: &mut Service) -> Result<Quote, Error> {
    if amount >= 100 {
        service.publish(
            Event::Discounted,
        );
        return Ok(Quote {
            total: 90,
        });
    }

    match amount {
        0 => Err(Error::Zero),
        _ => Ok(Quote { total: amount }),
    }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);
        let kinds = seams.iter().map(|seam| seam.kind()).collect::<Vec<_>>();

        for required in [
            SeamKind::PredicateBoundary,
            SeamKind::ReturnValue,
            SeamKind::ErrorVariant,
            SeamKind::FieldConstruction,
            SeamKind::SideEffect,
            SeamKind::MatchArm,
            SeamKind::CallPresence,
        ] {
            assert!(
                kinds.contains(&required),
                "expected SeamKind::{required:?} to be inventoried, got {kinds:?}"
            );
        }
        Ok(())
    }

    // -- Cache wiring integration tests -------------------------------
    //
    // These exercise the `inventory_classified_seams_at` -> cache load
    // -> uncached fallback -> cache store loop end-to-end against a
    // real on-disk workspace. They are paired with the unit tests in
    // `analysis::seam_cache::tests` (which characterize the cache
    // module in isolation).

    /// FNV-style unique-ish suffix so tempdir names do not collide
    /// when tests run in parallel.
    fn unique_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{}-{:x}", std::process::id(), nanos)
    }

    fn make_tempdir(label: &str) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join(format!("ripr-inv-{label}-{}", unique_suffix()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
        Ok(dir)
    }

    fn write_file(path: &Path, content: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        }
        std::fs::write(path, content).map_err(|err| format!("write {}: {err}", path.display()))
    }

    /// Rewrite `path` with identical content until the inode change time
    /// advances, so ctime-based assertions hold on filesystems with coarse
    /// timestamp granularity. Bounded so a filesystem that never bumps
    /// ctime fails the test loudly instead of hanging.
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
            std::thread::sleep(Duration::from_millis(2));
        }
        Err("filesystem ctime never advanced; ctime assertions cannot run here".to_string())
    }

    fn cache_dir_under(root: &Path) -> PathBuf {
        root.join("target")
            .join("ripr")
            .join("cache")
            .join("repo-seam-facts")
            .join(super::super::seam_cache::CACHE_SCHEMA_VERSION)
    }

    fn count_cache_dir_under(root: &Path) -> PathBuf {
        root.join("target")
            .join("ripr")
            .join("cache")
            .join("repo-seam-counts")
            .join(crate::analysis::seam_cache::COUNT_CACHE_SCHEMA_VERSION)
    }

    fn compact_cache_dir_under(root: &Path) -> PathBuf {
        root.join("target")
            .join("ripr")
            .join("cache")
            .join("repo-compact-classified-seams")
            .join(super::super::seam_cache::COMPACT_CLASSIFIED_SEAM_CACHE_SCHEMA_VERSION)
    }

    fn list_cache_entries(root: &Path) -> Result<Vec<PathBuf>, String> {
        let dir = cache_dir_under(root);
        list_entries(&dir)
    }

    fn list_count_cache_entries(root: &Path) -> Result<Vec<PathBuf>, String> {
        let dir = count_cache_dir_under(root);
        list_entries(&dir)
    }

    fn list_compact_cache_entries(root: &Path) -> Result<Vec<PathBuf>, String> {
        let dir = compact_cache_dir_under(root);
        list_entries(&dir)
    }

    fn list_entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in
            std::fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))?
        {
            let entry = entry.map_err(|err| format!("read entry: {err}"))?;
            out.push(entry.path());
        }
        out.sort();
        Ok(out)
    }

    #[test]
    fn latency_trace_line_formats_phase_status_and_duration() {
        let line = latency_trace_line("cache_load", "hit", Duration::from_millis(7));
        assert_eq!(
            line,
            "ripr_repo_exposure_latency phase=cache_load status=hit duration_ms=7"
        );
    }

    #[test]
    fn latency_trace_line_can_report_start_input_context() {
        let line = latency_trace_line(
            "file_fact_cache",
            "start_files_42_production_7",
            Duration::ZERO,
        );
        assert_eq!(
            line,
            "ripr_repo_exposure_latency phase=file_fact_cache status=start_files_42_production_7 duration_ms=0"
        );
    }

    #[test]
    fn cache_store_status_label_is_trace_safe() {
        let skip_reason = format!(
            "skipped_large_entry_seams_38124_limit_{}",
            CLASSIFIED_SEAM_CACHE_STORE_LIMIT
        );
        let expected_skip_label = format!(
            "ignored_skipped_large_entry_seams_38124_limit_{}",
            CLASSIFIED_SEAM_CACHE_STORE_LIMIT
        );
        assert_eq!(cache_store_status_label(&skip_reason), expected_skip_label);
        assert_eq!(
            cache_store_status_label("write cache failed: access denied"),
            "ignored_write_cache_failed__access_denied"
        );
    }

    #[test]
    fn repo_exposure_seam_limit_parser_accepts_positive_integer_only() {
        assert_eq!(parse_repo_exposure_seam_limit("8000"), Some(8000));
        assert_eq!(parse_repo_exposure_seam_limit(" 12 "), Some(12));
        assert_eq!(parse_repo_exposure_seam_limit("0"), None);
        assert_eq!(parse_repo_exposure_seam_limit("-1"), None);
        assert_eq!(parse_repo_exposure_seam_limit("not-a-number"), None);
    }

    #[test]
    fn classified_inventory_returns_collect_error_for_non_directory_root() -> Result<(), String> {
        let root = make_tempdir("collect-error")?;
        let file_root = root.join("not-a-directory");
        write_file(&file_root, "not a directory")?;

        let result = inventory_classified_seams_at(&file_root);
        if result.is_ok() {
            return Err("inventory should fail when root is not a directory".to_string());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn uncached_inventory_surfaces_discover_error_for_non_directory_root() -> Result<(), String> {
        let root = make_tempdir("uncached-discover-error")?;
        let file_root = root.join("not-a-directory");
        write_file(&file_root, "not a directory")?;

        let result =
            inventory_classified_seams_uncached_with_config(&file_root, &RiprConfig::default());
        assert!(
            result.is_err(),
            "uncached inventory should surface discover_rust_files error for non-directory root"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn seam_walker_skips_paths_not_present_in_the_index() {
        // The repo walker keeps its production-file list and the index
        // in lockstep, but in tests the two can diverge when a caller
        // passes a synthetic file list. The early-continue at the
        // `index.files.get(path)` lookup is what keeps the walker
        // crash-free in that case.
        let index = RustIndex::default();
        let seams = inventory_seams_from_index(&[PathBuf::from("missing.rs")], &index);
        assert!(
            seams.is_empty(),
            "expected no seams for paths absent from the index, got {}",
            seams.len()
        );
    }

    #[test]
    fn seam_walker_skips_shapes_with_unrecognized_probe_kind() {
        // `seam_kind_from_probe_shape` is the single place where new
        // probe-shape strings become first-class seam kinds. Until a
        // string is mapped explicitly, the walker must drop the shape
        // rather than inventing a fallback seam kind.
        let path = PathBuf::from("src/lib.rs");
        let owner = FunctionFact {
            id: SymbolId(format!("{}::owner", path.display())),
            name: "owner".to_string(),
            file: path.clone(),
            start_line: 1,
            end_line: 5,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let mut index = RustIndex::default();
        index.functions.push(owner.clone());
        index.files.insert(
            path.clone(),
            FileFacts {
                path: path.clone(),
                functions: vec![owner],
                probe_shapes: vec![ProbeShapeFact {
                    start_line: 2,
                    end_line: 2,
                    start_byte: 16,
                    kind: "shape_kind_that_is_not_recognized".to_string(),
                    text: "owner_body".to_string(),
                }],
                ..FileFacts::default()
            },
        );

        let seams = inventory_seams_from_index(&[path], &index);
        let kinds = seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>();
        assert!(
            seams.is_empty(),
            "expected no seams for unrecognized probe-shape kind, got kinds {kinds:?}"
        );
    }

    #[test]
    fn seam_walker_skips_shapes_whose_owner_function_is_marked_test() {
        // Inline `#[test]` modules inside production files share the
        // file with real production code. The walker drops shapes whose
        // owner is itself a test function so the seam inventory stays
        // production-only even when the layout role alone cannot
        // exclude the file outright.
        let path = PathBuf::from("src/lib.rs");
        let test_owner = FunctionFact {
            id: SymbolId(format!("{}::tests::predicate_inside_test", path.display())),
            name: "predicate_inside_test".to_string(),
            file: path.clone(),
            start_line: 10,
            end_line: 14,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::TestAttribute,
            attrs: vec!["#[test]".to_string()],
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let mut index = RustIndex::default();
        index.functions.push(test_owner.clone());
        index.files.insert(
            path.clone(),
            FileFacts {
                path: path.clone(),
                functions: vec![test_owner],
                probe_shapes: vec![ProbeShapeFact {
                    start_line: 11,
                    end_line: 11,
                    start_byte: 120,
                    kind: PROBE_SHAPE_PREDICATE.to_string(),
                    text: "x >= 0".to_string(),
                }],
                ..FileFacts::default()
            },
        );

        let seams = inventory_seams_from_index(&[path], &index);
        let owners = seams
            .iter()
            .map(|s| s.owner().to_string())
            .collect::<Vec<_>>();
        assert!(
            seams.is_empty(),
            "expected no seams when the only owner carries an evidence role, got owners {owners:?}"
        );
    }

    #[test]
    fn compact_seam_class_counts_match_full_classification_for_small_workspace()
    -> Result<(), String> {
        let root = make_tempdir("compact-counts")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;
        write_file(
            &root.join("tests/foo_test.rs"),
            "#[test] fn discount_calls_owner() { assert!(x::discount(100, 100)); }\n",
        )?;

        let full = inventory_classified_seams_uncached_with_config(&root, &RiprConfig::default())?;
        let compact =
            inventory_seam_grip_class_counts_uncached_with_config(&root, &RiprConfig::default())?;
        if compact.analyzed_seams() != full.len() {
            return Err(format!(
                "compact analyzed count {} did not match full classified count {}",
                compact.analyzed_seams(),
                full.len()
            ));
        }
        for class in super::super::seams::SeamGripClass::ALL {
            let full_count = full.iter().filter(|entry| entry.class == class).count();
            let compact_count = compact.count_for(class);
            if compact_count != full_count {
                return Err(format!(
                    "compact count for {} was {}, full count was {}",
                    class.as_str(),
                    compact_count,
                    full_count
                ));
            }
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_cached_seam_class_counts_when_badge_count_runs_then_cached_counts_are_returned()
    -> Result<(), String> {
        let root = make_tempdir("count-cache")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let cold = inventory_seam_grip_class_counts_at_with_config(&root, &RiprConfig::default())?;
        if cold.analyzed_seams() == 0 {
            return Err("cold count path should analyze at least one seam".into());
        }

        let entries = list_count_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 count cache entry, got {}",
                entries.len()
            ));
        }
        let cache_file = &entries[0];
        let bytes = std::fs::read(cache_file)
            .map_err(|err| format!("read {}: {err}", cache_file.display()))?;
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|err| format!("parse count cache: {err}"))?;
        envelope["counts"]["analyzed_seams"] = serde_json::json!(0);
        envelope["counts"]["counts"] = serde_json::json!({});
        let rewritten =
            serde_json::to_vec(&envelope).map_err(|err| format!("encode count cache: {err}"))?;
        std::fs::write(cache_file, rewritten)
            .map_err(|err| format!("rewrite {}: {err}", cache_file.display()))?;

        let warm = inventory_seam_grip_class_counts_at_with_config(&root, &RiprConfig::default())?;
        if warm.analyzed_seams() != 0 {
            return Err(format!(
                "warm count path should return cached analyzed_seams=0, got {}",
                warm.analyzed_seams()
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_corrupt_count_cache_entry_when_badge_count_runs_then_uncached_path_computes_without_failure()
    -> Result<(), String> {
        let root = make_tempdir("count-cache-corrupt")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = count_cache_dir_under(&root);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
        let entry = dir.join(key.filename());
        std::fs::write(&entry, b"{not valid json")
            .map_err(|err| format!("write corrupt count entry: {err}"))?;

        let result =
            inventory_seam_grip_class_counts_at_with_config(&root, &RiprConfig::default())?;
        if result.analyzed_seams() == 0 {
            return Err("count path should compute real seams when count cache is corrupt".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_count_cache_store_fails_when_badge_count_runs_then_analysis_result_is_still_returned()
    -> Result<(), String> {
        let root = make_tempdir("count-cache-storefail")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = count_cache_dir_under(&root);
        std::fs::create_dir_all(dir.join(key.filename()))
            .map_err(|err| format!("mkdir count conflict path: {err}"))?;

        let result =
            inventory_seam_grip_class_counts_at_with_config(&root, &RiprConfig::default())?;
        if result.analyzed_seams() == 0 {
            return Err(
                "count path should return real seams even when count cache write fails".into(),
            );
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_cached_compact_classified_seams_when_badge_projection_runs_then_cached_seams_are_returned()
    -> Result<(), String> {
        let root = make_tempdir("compact-classified-cache")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let cold =
            inventory_compact_classified_seams_at_with_config(&root, &RiprConfig::default())?;
        if cold.is_empty() {
            return Err("cold compact path should classify at least one seam".into());
        }

        let entries = list_compact_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 compact classified cache entry, got {}",
                entries.len()
            ));
        }
        // Seed a valid alternate payload through the actual writer. Integrity is not authenticity:
        // an authorized writer can recompute a checksum; this remains an observable cache-hit control.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache = RepoSeamFactCache::at_compact_classified(&root);
        cache.store_classified_seams_with_limit(&key, &[], None, 10)?;

        let warm =
            inventory_compact_classified_seams_at_with_config(&root, &RiprConfig::default())?;
        if !warm.is_empty() {
            return Err(format!(
                "warm compact path should return cached (empty) seams, got {} seams",
                warm.len()
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_corrupt_compact_classified_cache_entry_when_badge_projection_runs_then_uncached_path_computes_without_failure()
    -> Result<(), String> {
        let root = make_tempdir("compact-classified-cache-corrupt")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = compact_cache_dir_under(&root);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
        let entry = dir.join(key.filename());
        std::fs::write(&entry, b"{not valid json")
            .map_err(|err| format!("write corrupt compact entry: {err}"))?;

        let result =
            inventory_compact_classified_seams_at_with_config(&root, &RiprConfig::default())?;
        if result.is_empty() {
            return Err("compact path should compute real seams when cache is corrupt".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_compact_classified_cache_store_fails_when_badge_projection_runs_then_analysis_result_is_still_returned()
    -> Result<(), String> {
        let root = make_tempdir("compact-classified-cache-storefail")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = compact_cache_dir_under(&root);
        std::fs::create_dir_all(dir.join(key.filename()))
            .map_err(|err| format!("mkdir compact conflict path: {err}"))?;

        let result =
            inventory_compact_classified_seams_at_with_config(&root, &RiprConfig::default())?;
        if result.is_empty() {
            return Err("compact path should return real seams even when cache write fails".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn integrity_invalid_classified_payload_recomputes_full_and_compact_consumers()
    -> Result<(), String> {
        for compact in [false, true] {
            let root = make_tempdir("integrity-consumer")?;
            write_file(
                &root.join("src/foo.rs"),
                "pub fn discount(amount: i32) -> bool { amount >= 5 }\n",
            )?;
            let read = || {
                if compact {
                    inventory_compact_classified_seams_at_with_config(&root, &RiprConfig::default())
                } else {
                    inventory_classified_seams_at(&root)
                }
            };
            let cold = read()?;
            if cold.is_empty() {
                return Err("consumer fixture must contain classified seams".to_owned());
            }
            let entries = if compact {
                list_compact_cache_entries(&root)?
            } else {
                list_cache_entries(&root)?
            };
            if entries.len() != 1 {
                return Err("expected one actual writer entry".to_owned());
            }
            let path = entries.first().ok_or("missing seeded entry")?;
            let original = std::fs::read(path).map_err(|err| err.to_string())?;
            let mut edited: serde_json::Value =
                serde_json::from_slice(&original).map_err(|err| err.to_string())?;
            let seams = edited
                .get_mut("classified_seams")
                .and_then(|value| value.as_array_mut())
                .ok_or("missing seams")?;
            let summary = seams
                .first_mut()
                .and_then(|seam| seam.get_mut("evidence"))
                .and_then(|evidence| evidence.get_mut("reach"))
                .and_then(|reach| reach.get_mut("summary"))
                .ok_or("missing observed summary")?;
            *summary = serde_json::Value::String("fabricated evidence".to_owned());
            std::fs::write(
                path,
                serde_json::to_vec(&edited).map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
            let recovered = read()?;
            let expected = serde_json::to_vec(&cold).map_err(|err| err.to_string())?;
            if serde_json::to_vec(&recovered).map_err(|err| err.to_string())? != expected {
                return Err("integrity recovery must preserve every cold evidence field".to_owned());
            }
            let warm = read()?;
            if serde_json::to_vec(&warm).map_err(|err| err.to_string())? != expected {
                return Err("corrected entry must warm-hit original evidence".to_owned());
            }
            let _ = std::fs::remove_dir_all(&root);
        }
        Ok(())
    }

    #[test]
    fn given_cached_classified_seams_when_inventory_runs_then_cached_seams_are_returned()
    -> Result<(), String> {
        let root = make_tempdir("warm-hit")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        // Cold pass: classifies the predicate seam, writes cache.
        let cold = inventory_classified_seams_at(&root)?;
        if cold.is_empty() {
            return Err("cold path should classify at least one seam from foo.rs".into());
        }

        // Replace the cache file's `classified_seams` with `[]`
        // without changing the key fields. If the warm path returns
        // `[]`, the cache was read; if it returns the cold result,
        // the cache was bypassed.
        let entries = list_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 cache entry, got {}",
                entries.len()
            ));
        }
        // Seed a valid alternate payload through the actual writer. Integrity is not authenticity:
        // an authorized writer can recompute a checksum; this remains an observable cache-hit control.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache = RepoSeamFactCache::at(&root);
        cache.store_classified_seams_with_limit(&key, &[], None, 10)?;

        let warm = inventory_classified_seams_at(&root)?;
        if !warm.is_empty() {
            return Err(format!(
                "warm path should return cached (empty) seams, got {} seams",
                warm.len()
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(not(unix))]
    #[test]
    fn given_preserved_signature_when_content_is_swapped_then_stale_seams_are_not_served()
    -> Result<(), String> {
        // The scenario the issue #2108 fast path got wrong on non-unix
        // platforms, now pinned at its repaired outcome (#3848). Every file
        // keeps its (path, mtime, size) signature across a same-length edit
        // that restores the modification time, so a stat-only signature
        // reproduces exactly and the stored mapping would serve an aggregate
        // hash taken before the edit. There is no field of the non-unix stat
        // tuple that moves on every content write, so `corpus_fingerprint`
        // produces no signature there at all and the warm run must re-read
        // the corpus and classify the content that is actually on disk.
        //
        // The cache entry is doctored to empty first, so serving it is
        // observable: an empty result means the stale entry was served, a
        // non-empty result means the swapped content was read. Until #3848
        // this test asserted the empty result as the intended behavior.
        //
        // On unix the same rewrite invalidates through the inode change time
        // instead — see the unix-gated companion test below.
        let root = make_tempdir("fingerprint-stale-refused")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        // Cold pass: classifies the seam, writes the seam cache entry and
        // the corpus fingerprint mapping.
        let cold = inventory_classified_seams_at(&root)?;
        if cold.is_empty() {
            return Err("cold path should classify at least one seam from foo.rs".into());
        }

        // Doctor the cache entry so a hit is observable: if the warm path
        // returns `[]`, it read the cache; if it recomputes, it returns
        // the real (non-empty) classification.
        let entries = list_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 cache entry, got {}",
                entries.len()
            ));
        }
        // Seed a valid alternate payload through the actual writer. Integrity is not authenticity:
        // an authorized writer can recompute a checksum; this remains an observable cache-hit control.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache = RepoSeamFactCache::at(&root);
        cache.store_classified_seams_with_limit(&key, &[], None, 10)?;

        // Swap the bytes for same-length different content and restore the
        // original mtime, keeping the (path, mtime, size) signature intact.
        let original_mtime = std::fs::metadata(root.join("src/foo.rs"))
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount <= threshold }\n",
        )?;
        let file = std::fs::File::options()
            .write(true)
            .open(root.join("src/foo.rs"))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;

        let warm = inventory_classified_seams_at(&root)?;
        if warm.is_empty() {
            return Err(
                "a same-length edit that restores the modification time must not be answered \
                 from the mapping written before it: with no content-change witness in the \
                 non-unix stat tuple the warm run must re-read the corpus and return the \
                 swapped content's seams, and it returned the doctored (empty) cache entry \
                 instead"
                    .into(),
            );
        }
        if warm.len() != cold.len() {
            return Err(format!(
                "the re-read run should classify the same seam count as a cold run over the \
                 swapped content, got {} against {} cold",
                warm.len(),
                cold.len()
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Unix companion to the test above (codex P2 on #2175): the same
    /// mtime-preserving rewrite bumps the inode change time, so the
    /// fingerprint changes, the doctored cache entry under the old key is
    /// NOT served, and the rerun recomputes from the swapped content. Both
    /// platforms therefore refuse the stale entry; they differ in how, since
    /// unix has a witness to invalidate against and non-unix has none.
    #[cfg(unix)]
    #[test]
    fn given_mtime_preserving_rewrite_when_inventory_reruns_then_ctime_invalidates_mapping()
    -> Result<(), String> {
        let root = make_tempdir("fingerprint-ctime-invalidate")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let cold = inventory_classified_seams_at(&root)?;
        if cold.is_empty() {
            return Err("cold path should classify at least one seam from foo.rs".into());
        }
        let cold_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;

        // Doctor the cache entry so serving it would be observable.
        let entries = list_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 cache entry, got {}",
                entries.len()
            ));
        }
        // Seed a valid alternate payload through the actual writer. Integrity is not authenticity:
        // an authorized writer can recompute a checksum; this remains an observable cache-hit control.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache = RepoSeamFactCache::at(&root);
        cache.store_classified_seams_with_limit(&key, &[], None, 10)?;

        // Same-size rewrite with the mtime explicitly restored — what
        // `rsync -a` / `cp --preserve=timestamps` do. On unix the ctime
        // still bumps, so the fingerprint must invalidate. The tick wait
        // (identical bytes, so content is untouched) guarantees the fs
        // timestamp clock has advanced even on coarse-granularity
        // filesystems.
        let original_mtime = std::fs::metadata(root.join("src/foo.rs"))
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        wait_for_ctime_tick(&root.join("src/foo.rs"))?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount <= threshold }\n",
        )?;
        let file = std::fs::File::options()
            .write(true)
            .open(root.join("src/foo.rs"))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;

        let rerun = inventory_classified_seams_at(&root)?;
        if rerun.is_empty() {
            return Err(
                "mtime-preserving rewrite must invalidate the fingerprint via ctime and recompute"
                    .into(),
            );
        }
        let new_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;
        if new_key == cold_key {
            return Err(
                "mtime-preserving rewrite must produce a new cache key on unix (ctime changed)"
                    .into(),
            );
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Render a classified-seam inventory as comparable bytes. `ClassifiedSeam`
    /// has no `PartialEq`, and the serialized form is what the cache itself
    /// stores, so comparing it compares the thing a warm run would serve.
    fn rendered_inventory(seams: &[ClassifiedSeam]) -> Result<String, String> {
        serde_json::to_string(seams).map_err(|err| format!("encode inventory: {err}"))
    }

    /// The acceptance discriminator for #3848, run end to end on the analysis
    /// result rather than on the cache key.
    ///
    /// A changed cache key and a non-empty rerun are both weaker than the
    /// property that matters. What has to hold is that a warm run over an
    /// edited corpus produces **exactly what a cold run over that same edited
    /// corpus produces** — anything else is stale semantic reuse wearing a
    /// fresh key. The edit is the one that defeats a stat-only signature:
    /// identical byte length, modification time restored afterwards, which is
    /// what `rsync -a` and `cp --preserve=timestamps` leave behind.
    ///
    /// The assertion is deliberately platform-neutral, because the two
    /// platforms reach it by different routes and both routes must land here:
    /// unix invalidates the signature through the inode change time, and a
    /// platform with no content-change witness refuses to sign at all and
    /// recomputes (#3848). This test cannot tell those apart, and does not try
    /// to — it states the consequence they must share. Which mechanism ran on
    /// which platform is `corpus_fingerprint`'s own contract, locked
    /// separately, and native-platform identity belongs to the Windows
    /// evidence packet, not here.
    fn equal_length_timestamp_restored_edit_replays_a_cold_run(
        label: &str,
        relative: &str,
        fixed: &[(&str, &str)],
        before: &str,
        after: &str,
    ) -> Result<(), String> {
        if before.len() != after.len() {
            return Err(format!(
                "{label}: the edit must preserve byte length to be the stale-reuse case ({} vs {})",
                before.len(),
                after.len()
            ));
        }

        // An independent cold run over the edited corpus, in its own root, so
        // the expected value is never produced by the machinery under test.
        let cold_root = make_tempdir(&format!("{label}-cold-after"))?;
        for (path, content) in fixed {
            write_file(&cold_root.join(path), content)?;
        }
        write_file(&cold_root.join(relative), after)?;
        let cold_after = rendered_inventory(&inventory_classified_seams_at(&cold_root)?)?;

        let root = make_tempdir(&format!("{label}-warm"))?;
        for (path, content) in fixed {
            write_file(&root.join(path), content)?;
        }
        write_file(&root.join(relative), before)?;
        let cold_before = rendered_inventory(&inventory_classified_seams_at(&root)?)?;
        if cold_before == cold_after {
            return Err(format!(
                "{label}: the two corpora classify identically, so the equality below \
                 would hold even under stale reuse; this is not a discriminator"
            ));
        }

        // Second run over the unedited corpus: this is what stores the
        // fingerprint -> content-hash mapping the edit then has to defeat.
        let _ = inventory_classified_seams_at(&root)?;

        let edited = root.join(relative);
        let original_mtime = std::fs::metadata(&edited)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("{label}: stat mtime: {err}"))?;
        let original_len = std::fs::metadata(&edited)
            .map(|metadata| metadata.len())
            .map_err(|err| format!("{label}: stat len: {err}"))?;
        // Identical bytes, so content is untouched; this only guarantees the
        // filesystem timestamp clock has advanced before the real edit, which
        // coarse-granularity filesystems otherwise hide.
        #[cfg(unix)]
        wait_for_ctime_tick(&edited)?;
        write_file(&edited, after)?;
        let handle = std::fs::File::options()
            .write(true)
            .open(&edited)
            .map_err(|err| format!("{label}: open for set_modified: {err}"))?;
        handle
            .set_modified(original_mtime)
            .map_err(|err| format!("{label}: set_modified: {err}"))?;
        drop(handle);

        // Assert the observed filesystem state really is the trap, rather than
        // assuming the rewrite and the restore both took effect.
        let observed =
            std::fs::metadata(&edited).map_err(|err| format!("{label}: re-stat: {err}"))?;
        if observed.len() != original_len {
            return Err(format!(
                "{label}: the edit must preserve length on disk ({} vs {original_len})",
                observed.len()
            ));
        }
        if observed
            .modified()
            .map_err(|err| format!("{label}: re-stat mtime: {err}"))?
            != original_mtime
        {
            return Err(format!("{label}: the edit must restore mtime on disk"));
        }

        let warm_after_edit = rendered_inventory(&inventory_classified_seams_at(&root)?)?;
        if warm_after_edit != cold_after {
            return Err(format!(
                "{label}: #3848 stale semantic reuse — a warm run after an \
                 equal-length, mtime-restored edit did not reproduce the cold \
                 run over the same corpus"
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cold_root);
        Ok(())
    }

    #[test]
    fn source_only_equal_length_timestamp_restored_edit_replays_a_cold_run() -> Result<(), String> {
        equal_length_timestamp_restored_edit_replays_a_cold_run(
            "source-only",
            "src/foo.rs",
            &[],
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount <= threshold }\n",
        )
    }

    #[test]
    fn test_only_equal_length_timestamp_restored_edit_replays_a_cold_run() -> Result<(), String> {
        equal_length_timestamp_restored_edit_replays_a_cold_run(
            "test-only",
            "tests/discount_test.rs",
            &[(
                "src/foo.rs",
                "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
            )],
            "#[test]\nfn discounts() { assert!(crate::discount(9, 4)); }\n",
            "#[test]\nfn discounts() { assert!(crate::discount(4, 9)); }\n",
        )
    }

    #[test]
    fn fingerprint_cached_workspace_key_rebuilds_key_from_stored_hash() -> Result<(), String> {
        let root = make_tempdir("fingerprint-key-hit")?;
        let content =
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n";
        let relative = PathBuf::from("src/foo.rs");
        write_file(&root.join(&relative), content)?;
        // The helper only ever receives an already-computed signature, so
        // key rebuilding does not depend on this value. Using a literal
        // keeps the property under test on platforms that produce no
        // signature at all (#3848).
        let fingerprint = "0123456789abcdef";
        let content_hash = files_content_hash(&[(relative.clone(), content.as_bytes().to_vec())]);
        RepoCorpusFingerprintCache::at(&root)
            .store(&root, fingerprint, &content_hash)
            .map_err(|err| format!("store fingerprint mapping: {err}"))?;
        // Change the on-disk bytes after storing the mapping. The helper only
        // receives the already-computed fingerprint, so returning the stored
        // hash proves this fast path does not re-read the corpus.
        let changed_content =
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount <= threshold }\n";
        write_file(&root.join(&relative), changed_content)?;

        let inputs = workspace_key_inputs(&root, &RiprConfig::default());
        let key = fingerprint_cached_workspace_key(&root, &inputs, Some(fingerprint))
            .ok_or("stored fingerprint should rebuild a cache key")?;
        (key.files_content_hash == content_hash)
            .then_some(())
            .ok_or("reconstructed key must reuse the stored content hash")?;
        fingerprint_cached_workspace_key(&root, &inputs, Some("missing-fingerprint"))
            .is_none()
            .then_some(())
            .ok_or("unknown fingerprint mapping must fail closed to the content-read path")?;
        // The production state on a platform with no content-change
        // witness (#3848): no signature at all must fail closed to the
        // content-read path, never to a mapping stored by an older build.
        fingerprint_cached_workspace_key(&root, &inputs, None)
            .is_none()
            .then_some(())
            .ok_or("absent fingerprint must fail closed to the content-read path")?;

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_fallback_reason_vocabulary_is_stable() {
        // The traced `no_impact_fallback_<reason>` statuses are
        // observability contract: pin the exact vocabulary.
        let reasons = [
            (
                NoImpactFallbackReason::ChangedRustSource,
                "changed_rust_source",
            ),
            (
                NoImpactFallbackReason::ChangedTestOrHarness,
                "changed_test_or_harness",
            ),
            (
                NoImpactFallbackReason::ChangedManifestOrConfig,
                "changed_manifest_or_config",
            ),
            (
                NoImpactFallbackReason::ChangedSourceRoleAuthority,
                "changed_source_role_authority",
            ),
            (
                NoImpactFallbackReason::UnsupportedOrUnprovenChangeIdentity,
                "unsupported_or_unproven_change_identity",
            ),
            (
                NoImpactFallbackReason::MissingFingerprintMapping,
                "missing_fingerprint_mapping",
            ),
            (
                NoImpactFallbackReason::IncompatibleWorkspaceIdentity,
                "incompatible_workspace_identity",
            ),
            (NoImpactFallbackReason::CorruptMetadata, "corrupt_metadata"),
            (
                NoImpactFallbackReason::CorpusChangedDuringResolution,
                "corpus_changed_during_resolution",
            ),
        ];
        for (reason, expected) in reasons {
            assert_eq!(reason.as_str(), expected, "fallback reason {reason:?}");
        }
    }

    fn no_impact_layout(root: &Path) -> Result<(), String> {
        write_file(
            &root.join("Cargo.toml"),
            "[package]\nname = 'no-impact-fixture'\nversion = '0.1.0'\n\n[workspace]\n",
        )?;
        write_file(
            &root.join("src/lib.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        )?;
        write_file(&root.join("docs/notes.md"), "# notes\n")?;
        Ok(())
    }

    fn no_impact_eligibility(
        root: &Path,
        config: &RiprConfig,
        changed: &[&str],
    ) -> Result<Option<NoImpactFallbackReason>, String> {
        let (rust_files, _) = scan_corpus_fingerprint(root)?;
        let context =
            production_role_context(root, config, rust_files.iter().map(PathBuf::as_path));
        let changed_files: Vec<PathBuf> = changed.iter().map(PathBuf::from).collect();
        Ok(no_impact_changed_path_fallback(
            &changed_files,
            &rust_files,
            config,
            &context,
        ))
    }

    #[test]
    fn no_impact_docs_only_change_is_eligible() -> Result<(), String> {
        let root = make_tempdir("no-impact-docs")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let eligible = no_impact_eligibility(
            &root,
            &config,
            &[
                "docs/notes.md",
                "README.md",
                ".github/workflows/ci.yml",
                "editors/vscode/package.json",
                "scripts/check.py",
            ],
        )?;
        if eligible.is_some() {
            return Err(format!(
                "documentation and non-Rust changes must be no-impact eligible, got {eligible:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_production_source_change_falls_back() -> Result<(), String> {
        let root = make_tempdir("no-impact-prod")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let reason = no_impact_eligibility(&root, &config, &["src/lib.rs"])?;
        if reason != Some(NoImpactFallbackReason::ChangedRustSource) {
            return Err(format!(
                "production source change must fall back as changed_rust_source, got {reason:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_test_source_change_falls_back() -> Result<(), String> {
        let root = make_tempdir("no-impact-test")?;
        no_impact_layout(&root)?;
        write_file(
            &root.join("tests/contract.rs"),
            "#[test]\nfn reserves() {}\n",
        )?;
        let config = RiprConfig::default();
        let reason = no_impact_eligibility(&root, &config, &["tests/contract.rs"])?;
        if reason != Some(NoImpactFallbackReason::ChangedTestOrHarness) {
            return Err(format!(
                "test source change must fall back as changed_test_or_harness, got {reason:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_manifest_and_config_changes_fall_back() -> Result<(), String> {
        let root = make_tempdir("no-impact-manifest")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        for changed in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "ripr.toml",
            ".ripr/test_intent.toml",
            ".ripr/suppressions.toml",
            ".cargo/config.toml",
        ] {
            let reason = no_impact_eligibility(&root, &config, &[changed])?;
            if reason != Some(NoImpactFallbackReason::ChangedManifestOrConfig) {
                return Err(format!(
                    "{changed} must fall back as changed_manifest_or_config, got {reason:?}"
                ));
            }
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_uppercase_rs_never_slips_past_as_docs() -> Result<(), String> {
        let root = make_tempdir("no-impact-rs-case")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let reason = no_impact_eligibility(&root, &config, &["docs/NOTES.RS"])?;
        if reason != Some(NoImpactFallbackReason::ChangedRustSource) {
            return Err(format!(
                "uppercase .RS must fall back as changed_rust_source, got {reason:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_deleted_declared_test_target_falls_back_with_authority_reason()
    -> Result<(), String> {
        let root = make_tempdir("no-impact-declared")?;
        // The manifest declares the target but the file is absent
        // (deleted): no longer in the corpus, yet its role authority
        // moved.
        write_file(
            &root.join("Cargo.toml"),
            "[package]\nname = 'no-impact-fixture'\nversion = '0.1.0'\n\n[workspace]\n\n[[test]]\nname = 'mimic'\npath = 'tests/mimic.rs'\n",
        )?;
        write_file(
            &root.join("src/lib.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        )?;
        let config = RiprConfig::default();
        let reason = no_impact_eligibility(&root, &config, &["tests/mimic.rs"])?;
        if reason != Some(NoImpactFallbackReason::ChangedSourceRoleAuthority) {
            return Err(format!(
                "deleted declared test target must fall back as changed_source_role_authority, got {reason:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_registered_harness_change_falls_back() -> Result<(), String> {
        let root = make_tempdir("no-impact-harness")?;
        no_impact_layout(&root)?;
        // The registered harness target is deleted: absent from the
        // corpus, but the raw registration still names it a harness.
        let config = crate::config::tests_only_parse(
            r#"[analysis]
[[analysis.test_harnesses]]
registration_id = "mimic"
target = "tests/mimic.rs"
kind = "custom_harness"
adapter = "libtest_mimic_v1"
marker = "libtest_mimic::Trial"
"#,
        )
        .map_err(|error| format!("fixture config parses: {error}"))?;
        let reason = no_impact_eligibility(&root, &config, &["tests/mimic.rs"])?;
        if reason != Some(NoImpactFallbackReason::ChangedTestOrHarness) {
            return Err(format!(
                "registered harness change must fall back as changed_test_or_harness, got {reason:?}"
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_named_owner_falls_back_before_any_filesystem_probe() -> Result<(), String> {
        // Owners are checked before corpus discovery: even an
        // absent root must report the owner reason, never a scan
        // error.
        let root = PathBuf::from("ripr-no-impact-absent-root");
        let config = RiprConfig::default();
        let outcome = try_no_impact_fast_path(&root, &config, &[], &["crate::discount".to_owned()]);
        match outcome {
            Ok(NoImpactOutcome::Declined(NoImpactFallbackReason::ChangedRustSource)) => Ok(()),
            other => Err(format!(
                "named owner must decline as changed_rust_source, got {}",
                match other {
                    Ok(NoImpactOutcome::Fast(_)) => "fast path".to_owned(),
                    Ok(NoImpactOutcome::Declined(reason)) => reason.as_str().to_owned(),
                    Err(error) => format!("error {error}"),
                }
            )),
        }
    }

    #[test]
    fn no_impact_missing_tree_reports_discovery_error() -> Result<(), String> {
        let root = PathBuf::from("ripr-no-impact-absent-root");
        let config = RiprConfig::default();
        match try_no_impact_fast_path(&root, &config, &[], &[]) {
            Err(_) => Ok(()),
            Ok(_) => Err("absent root must propagate the discovery error".to_owned()),
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn no_impact_fast_path_without_change_witness_falls_back() -> Result<(), String> {
        // Issue #3848: no stat field on this platform witnesses every
        // content write, so the fast path can never engage here.
        let root = make_tempdir("no-impact-no-witness")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let outcome =
            try_no_impact_fast_path(&root, &config, &[PathBuf::from("docs/notes.md")], &[]);
        match outcome {
            Ok(NoImpactOutcome::Declined(
                NoImpactFallbackReason::UnsupportedOrUnprovenChangeIdentity,
            )) => Ok(()),
            other => Err(format!(
                "platform without a change witness must decline as unsupported_or_unproven_change_identity, got {}",
                match other {
                    Ok(NoImpactOutcome::Fast(_)) => "fast path".to_owned(),
                    Ok(NoImpactOutcome::Declined(reason)) => reason.as_str().to_owned(),
                    Err(error) => format!("error {error}"),
                }
            )),
        }
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_warm_docs_diff_returns_empty_inventory_with_exact_identity() -> Result<(), String>
    {
        let root = make_tempdir("no-impact-warm")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        // A full-inventory key run populates the fingerprint mapping
        // for the next call (the scoped route itself never stores;
        // the fast path stays purely opportunistic).
        workspace_cache_key_at_with_config(&root, &config)?;
        let full = inventory_diff_scoped_classified_seams_inner(
            &root,
            &config,
            &changed,
            &[],
            false,
            None,
        )?;
        if !full.classified.is_empty() {
            return Err("docs-only diff must classify no seams on the full path".to_owned());
        }
        let outcome = try_no_impact_fast_path(&root, &config, &changed, &[])?;
        let fast = match outcome {
            NoImpactOutcome::Fast(inventory) => inventory,
            NoImpactOutcome::Declined(reason) => {
                return Err(format!(
                    "warm docs-only diff must take the fast path, got fallback {}",
                    reason.as_str()
                ));
            }
        };
        if !fast.classified.is_empty()
            || !fast.scoped_production_files.is_empty()
            || !fast.changed_production_files.is_empty()
            || !fast.immediate_caller_files.is_empty()
        {
            return Err("fast path must return empty seam and file sets".to_owned());
        }
        if fast.workspace_cache_key != full.workspace_cache_key {
            return Err(
                "fast path workspace key must be byte-identical to the full path".to_owned(),
            );
        }
        if fast.total_rust_files != full.total_rust_files
            || fast.total_production_files != full.total_production_files
        {
            return Err(format!(
                "fast path totals ({}/{}) must match the full path ({}/{})",
                fast.total_rust_files,
                fast.total_production_files,
                full.total_rust_files,
                full.total_production_files
            ));
        }
        if fast.file_fact_cache.hits != 0
            || fast.file_fact_cache.misses != 0
            || fast.file_fact_cache.corrupt_ignored != 0
            || fast.file_fact_cache.stores != 0
            || fast.file_fact_cache.store_errors != 0
            || !fast.file_fact_cache.invalidated_files.is_empty()
            || !fast.file_fact_cache.store_failures.is_empty()
            || fast.file_fact_cache.store_failures_dropped != 0
        {
            return Err("fast path must report explicit zero work counters".to_owned());
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_permission_only_change_declines_missing_mapping() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let root = make_tempdir("no-impact-unreadable")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        // Populate the mapping while the corpus is readable (via the
        // full-inventory key route, the mapping's writer).
        workspace_cache_key_at_with_config(&root, &config)?;
        // Revoke all content access to the Rust sources. Manifest and
        // directory reads stay permitted; only file bytes are denied.
        // chmod bumps ctime, so the stat witness no longer matches the
        // stored mapping even though the bytes are identical: the fast
        // path must decline rather than pair the key with a corpus no
        // stored fingerprint witnesses (issue #3848).
        let source = root.join("src/lib.rs");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o000))
            .map_err(|err| format!("revoke source permissions: {err}"))?;
        let outcome = try_no_impact_fast_path(&root, &config, &changed, &[]);
        // Restore before asserting so the temp dir cleans up.
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .map_err(|err| format!("restore source permissions: {err}"))?;
        match outcome {
            Ok(NoImpactOutcome::Declined(NoImpactFallbackReason::MissingFingerprintMapping)) => {
                Ok(())
            }
            Ok(NoImpactOutcome::Declined(reason)) => Err(format!(
                "permission-only change must decline as missing_fingerprint_mapping, got {}",
                reason.as_str()
            )),
            Ok(NoImpactOutcome::Fast(_)) => {
                Err("permission-only change must not take the fast path: ctime moved".to_owned())
            }
            Err(error) => Err(format!(
                "permission-only change must decline, not error: {error}"
            )),
        }?;
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_cold_cache_falls_back_with_mapping_reason() -> Result<(), String> {
        let root = make_tempdir("no-impact-cold")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let outcome =
            try_no_impact_fast_path(&root, &config, &[PathBuf::from("docs/notes.md")], &[])?;
        match outcome {
            NoImpactOutcome::Declined(NoImpactFallbackReason::MissingFingerprintMapping) => Ok(()),
            NoImpactOutcome::Declined(reason) => Err(format!(
                "cold cache must decline as missing_fingerprint_mapping, got {}",
                reason.as_str()
            )),
            NoImpactOutcome::Fast(_) => Err("cold cache must not take the fast path".to_owned()),
        }?;
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    fn no_impact_fingerprint_entries(root: &Path) -> Result<Vec<PathBuf>, String> {
        let dir = root
            .join("target")
            .join("ripr")
            .join("cache")
            .join("repo-corpus-fingerprint")
            .join(crate::analysis::seam_cache::CORPUS_FINGERPRINT_CACHE_SCHEMA_VERSION);
        list_entries(&dir)
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_corrupt_mapping_falls_back() -> Result<(), String> {
        let root = make_tempdir("no-impact-corrupt")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        // Warm the mapping through its writer (the full-inventory key
        // route); the scoped route never stores.
        workspace_cache_key_at_with_config(&root, &config)?;
        let entries = no_impact_fingerprint_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 fingerprint mapping, got {}",
                entries.len()
            ));
        }
        std::fs::write(&entries[0], b"{not valid json")
            .map_err(|err| format!("corrupt mapping: {err}"))?;
        let outcome = try_no_impact_fast_path(&root, &config, &changed, &[])?;
        match outcome {
            NoImpactOutcome::Declined(NoImpactFallbackReason::CorruptMetadata) => Ok(()),
            NoImpactOutcome::Declined(reason) => Err(format!(
                "corrupt mapping must decline as corrupt_metadata, got {}",
                reason.as_str()
            )),
            NoImpactOutcome::Fast(_) => {
                Err("corrupt mapping must not take the fast path".to_owned())
            }
        }?;
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn same_key_fingerprint_payload_edit_declines_and_rehashes_source_truth() -> Result<(), String>
    {
        let root = std::env::temp_dir().join(format!("ripr-inv-integrity-{}", unique_suffix()));
        std::fs::create_dir(&root).map_err(|err| err.to_string())?;
        struct OwnedRoot(PathBuf);
        impl Drop for OwnedRoot {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _owned = OwnedRoot(root.clone());
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        let cold = inventory_diff_scoped_classified_seams_inner(
            &root,
            &config,
            &changed,
            &[],
            false,
            None,
        )?;
        if cold.total_rust_files == 0 || cold.total_production_files == 0 {
            return Err(
                "fingerprint consumer fixture must contain production Rust source".to_owned(),
            );
        }
        // Scoped cold inventory does not write fingerprint mappings; seed its actual producer.
        let produced_key = workspace_cache_key_at_with_config(&root, &config)?;
        if produced_key != cold.workspace_cache_key {
            return Err("actual fingerprint producer must match cold source identity".to_owned());
        }
        if !matches!(
            try_no_impact_fast_path(&root, &config, &changed, &[])?,
            NoImpactOutcome::Fast(_)
        ) {
            return Err(
                "actual-writer mapping must yield a warm canonical hit before tamper".to_owned(),
            );
        }
        let entries = no_impact_fingerprint_entries(&root)?;
        if entries.len() != 1 {
            return Err("expected one actual-writer fingerprint entry".to_owned());
        }
        let entry = entries.first().ok_or("missing mapping")?;
        let mut edited: serde_json::Value =
            serde_json::from_slice(&std::fs::read(entry).map_err(|err| err.to_string())?)
                .map_err(|err| err.to_string())?;
        let original = edited.clone();
        *edited
            .get_mut("files_content_hash")
            .ok_or("missing aggregate hash")? =
            serde_json::Value::String("fabricated-content-hash".to_owned());
        for (key, value) in original.as_object().ok_or("mapping must be object")? {
            if key != "files_content_hash" && edited.get(key) != Some(value) {
                return Err("must preserve mapping identity/digest".to_owned());
            }
        }
        std::fs::write(
            entry,
            serde_json::to_vec(&edited).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        if !matches!(
            try_no_impact_fast_path(&root, &config, &changed, &[])?,
            NoImpactOutcome::Declined(NoImpactFallbackReason::CorruptMetadata)
        ) {
            return Err(
                "valid-JSON mapping corruption must decline canonical fast path".to_owned(),
            );
        }
        let recovered = inventory_diff_scoped_classified_seams_inner(
            &root,
            &config,
            &changed,
            &[],
            true,
            None,
        )?;
        if recovered.workspace_cache_key != cold.workspace_cache_key
            || recovered.total_rust_files != cold.total_rust_files
            || recovered.total_production_files != cold.total_production_files
            || serde_json::to_vec(&recovered.classified).map_err(|err| err.to_string())?
                != serde_json::to_vec(&cold.classified).map_err(|err| err.to_string())?
        {
            return Err(
                "fallback must rehash actual source and preserve complete cold evidence".to_owned(),
            );
        }
        let repaired_key = workspace_cache_key_at_with_config(&root, &config)?;
        if repaired_key != cold.workspace_cache_key
            || !matches!(
                try_no_impact_fast_path(&root, &config, &changed, &[])?,
                NoImpactOutcome::Fast(_)
            )
        {
            return Err(
                "actual producer must repair the mapping for a subsequent warm hit".to_owned(),
            );
        }
        eprintln!(
            "fingerprint consumer stimulus: rust_files={} production_files={} mappings={}",
            cold.total_rust_files,
            cold.total_production_files,
            entries.len()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_foreign_root_mapping_falls_back_incompatible() -> Result<(), String> {
        let root = make_tempdir("no-impact-foreign")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        // Warm the mapping through its writer (the full-inventory key
        // route); the scoped route never stores.
        workspace_cache_key_at_with_config(&root, &config)?;
        let entries = no_impact_fingerprint_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 fingerprint mapping, got {}",
                entries.len()
            ));
        }
        // Retarget the entry at another workspace root: the entry
        // decodes but its root identity no longer matches.
        let bytes = std::fs::read(&entries[0]).map_err(|err| format!("read mapping: {err}"))?;
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|err| format!("parse mapping: {err}"))?;
        envelope["workspace_root_hash"] = serde_json::Value::String("foreign-root".to_owned());
        let rewritten =
            serde_json::to_vec(&envelope).map_err(|err| format!("encode mapping: {err}"))?;
        std::fs::write(&entries[0], rewritten).map_err(|err| format!("rewrite mapping: {err}"))?;
        let outcome = try_no_impact_fast_path(&root, &config, &changed, &[])?;
        match outcome {
            NoImpactOutcome::Declined(NoImpactFallbackReason::IncompatibleWorkspaceIdentity) => {
                Ok(())
            }
            NoImpactOutcome::Declined(reason) => Err(format!(
                "foreign mapping must decline as incompatible_workspace_identity, got {}",
                reason.as_str()
            )),
            NoImpactOutcome::Fast(_) => {
                Err("foreign mapping must not take the fast path".to_owned())
            }
        }?;
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_impact_disabled_route_matches_fast_route_modulo_work_counters() -> Result<(), String> {
        // Removal control (issue #3859, control 12): disabling the
        // shortcut preserves output identity; only the measured work
        // counters may grow.
        let root = make_tempdir("no-impact-disabled")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let changed = vec![PathBuf::from("docs/notes.md")];
        // Warm the mapping through its writer (the full-inventory key
        // route); the scoped route never stores.
        workspace_cache_key_at_with_config(&root, &config)?;
        let fast = inventory_diff_scoped_classified_seams_inner(
            &root,
            &config,
            &changed,
            &[],
            true,
            None,
        )?;
        let full = inventory_diff_scoped_classified_seams_inner(
            &root,
            &config,
            &changed,
            &[],
            false,
            None,
        )?;
        // ClassifiedSeam carries evidence payloads without structural
        // equality; compare canonical seam identities instead.
        let seam_ids = |inventory: &ScopedClassifiedSeamInventory| {
            inventory
                .classified
                .iter()
                .map(|entry| entry.seam.id().as_str().to_owned())
                .collect::<Vec<_>>()
        };
        if seam_ids(&fast) != seam_ids(&full)
            || fast.workspace_cache_key != full.workspace_cache_key
            || fast.total_rust_files != full.total_rust_files
            || fast.total_production_files != full.total_production_files
            || fast.scoped_production_files != full.scoped_production_files
            || fast.changed_production_files != full.changed_production_files
            || fast.immediate_caller_files != full.immediate_caller_files
        {
            return Err(
                "disabled route must preserve output identity modulo work counters".to_owned(),
            );
        }
        if fast.file_fact_cache.hits != 0 || fast.file_fact_cache.misses != 0 {
            return Err("fast route must report zero file-fact work".to_owned());
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn no_impact_production_edit_keeps_full_computation_active() -> Result<(), String> {
        // Issue #3859, control 4: a production edit never shortcuts.
        let root = make_tempdir("no-impact-production")?;
        no_impact_layout(&root)?;
        let config = RiprConfig::default();
        let inventory = inventory_diff_scoped_classified_seams_at_with_config(
            &root,
            &config,
            &[PathBuf::from("src/lib.rs")],
            &[],
        )?;
        if inventory.classified.is_empty() {
            return Err("production edit must keep full seam computation active".to_owned());
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_content_change_with_mtime_bump_when_inventory_reruns_then_recomputes_and_refreshes_mapping()
    -> Result<(), String> {
        // A content change that also bumps the mtime must invalidate the
        // fingerprint mapping: the rerun recomputes (the doctored cache
        // entry under the old key is NOT served) and stores a fresh
        // fingerprint mapping for the new signature.
        let root = make_tempdir("fingerprint-mtime-bump")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let cold = inventory_classified_seams_at(&root)?;
        if cold.is_empty() {
            return Err("cold path should classify at least one seam from foo.rs".into());
        }
        let cold_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;

        // Doctor the old entry so serving it would be observable.
        let entries = list_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 cache entry, got {}",
                entries.len()
            ));
        }
        // Seed a valid alternate payload through the actual writer. Integrity is not authenticity:
        // an authorized writer can recompute a checksum; this remains an observable cache-hit control.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache = RepoSeamFactCache::at(&root);
        cache.store_classified_seams_with_limit(&key, &[], None, 10)?;

        // Change the content and force a distinct mtime so the new
        // signature cannot collide with the old one through mtime
        // granularity.
        let original_mtime = std::fs::metadata(root.join("src/foo.rs"))
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount <= threshold }\n",
        )?;
        let file = std::fs::File::options()
            .write(true)
            .open(root.join("src/foo.rs"))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime + Duration::from_secs(2))
            .map_err(|err| format!("set_modified: {err}"))?;

        let rerun = inventory_classified_seams_at(&root)?;
        if rerun.is_empty() {
            return Err(
                "mtime-bumped content change must invalidate the fingerprint mapping and recompute"
                    .into(),
            );
        }

        let new_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;
        if new_key == cold_key {
            return Err("content change with mtime bump must produce a new cache key".into());
        }
        match corpus_fingerprint(&root, &[PathBuf::from("src/foo.rs")]) {
            Some(new_fingerprint) => {
                let stored = RepoCorpusFingerprintCache::at(&root).lookup(&root, &new_fingerprint);
                if stored.as_deref() != Some(new_key.files_content_hash.as_str()) {
                    return Err(format!(
                        "rerun should store the new fingerprint mapping, got {stored:?}"
                    ));
                }
            }
            // #3848: with no content-change witness no signature is
            // produced, so no mapping is refreshed and the next run reads
            // the corpus. The rerun assertions above already established
            // that this path returns the recomputed result.
            None => {
                if cfg!(unix) {
                    return Err(
                        "unix carries a content-change witness and must sign a stat-able corpus"
                            .into(),
                    );
                }
            }
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_size_change_with_preserved_mtime_when_key_is_resolved_then_key_changes()
    -> Result<(), String> {
        // A size change alone (mtime preserved) must invalidate the
        // fingerprint, so the resolved key reflects the new content.
        let root = make_tempdir("fingerprint-size-change")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;
        let cold_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;

        let original_mtime = std::fs::metadata(root.join("src/foo.rs"))
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("stat mtime: {err}"))?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold || amount < 0 }\n",
        )?;
        let file = std::fs::File::options()
            .write(true)
            .open(root.join("src/foo.rs"))
            .map_err(|err| format!("open for set_modified: {err}"))?;
        file.set_modified(original_mtime)
            .map_err(|err| format!("set_modified: {err}"))?;

        let new_key = workspace_cache_key_at_with_config(&root, &RiprConfig::default())?;
        if new_key == cold_key {
            return Err(
                "size change with preserved mtime must invalidate the fingerprint and change the key"
                    .into(),
            );
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_changed_signature_during_read_when_mapping_store_runs_then_nothing_is_stored()
    -> Result<(), String> {
        // The store guard: if the corpus signature changed between the
        // pre-read fingerprint and the end of the content read, the
        // mapping must NOT be stored — a fingerprint may never be paired
        // with a hash derived from a differently-signed corpus.
        let root = make_tempdir("fingerprint-guard")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;
        let (rust_files, pre_fingerprint) = scan_corpus_fingerprint(&root)?;
        let state = collect_workspace_state_from_files(&root, &RiprConfig::default(), rust_files)?;
        let key = state.cache_key();

        // Simulate a mid-read corpus change: the hashed file itself has a
        // new signature at store time, so the pre-read fingerprint no
        // longer describes the state of the corpus on disk.
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold || amount < 0 }\n",
        )?;

        store_corpus_fingerprint_mapping(&state, pre_fingerprint.clone(), &key);
        let Some(fingerprint) = pre_fingerprint else {
            // #3848: with no content-change witness there is no pre-read
            // signature, so the store is skipped before this guard is
            // reached and nothing can be written either way.
            if cfg!(unix) {
                return Err(
                    "unix carries a content-change witness and must produce a pre-read signature"
                        .into(),
                );
            }
            let _ = std::fs::remove_dir_all(&root);
            return Ok(());
        };
        let stored = RepoCorpusFingerprintCache::at(&root).lookup(&root, &fingerprint);
        if stored.is_some() {
            return Err(format!(
                "mapping must not be stored when the signature changed during the read, got {stored:?}"
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_fingerprint_store_unwritable_when_inventory_runs_then_analysis_still_returns()
    -> Result<(), String> {
        // The fingerprint layer is best-effort: when its directory cannot
        // be created (a file sits at its path), lookup misses and store
        // fails, but analysis must proceed exactly as before.
        let root = make_tempdir("fingerprint-storefail")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;
        let blocker = root
            .join("target")
            .join("ripr")
            .join("cache")
            .join("repo-corpus-fingerprint");
        write_file(&blocker, "not a directory")?;

        let result = inventory_classified_seams_at(&root)?;
        if result.is_empty() {
            return Err(
                "inventory should return real seams even when the fingerprint cache is unwritable"
                    .into(),
            );
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_corrupt_cache_entry_when_inventory_runs_then_uncached_path_computes_without_failure()
    -> Result<(), String> {
        let root = make_tempdir("corrupt-recover")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        // Pre-populate the cache file (under the exact key the
        // inventory will compute) with garbage so the loader returns
        // `CorruptIgnored` and the inventory falls through to compute.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = cache_dir_under(&root);
        std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
        let entry = dir.join(key.filename());
        std::fs::write(&entry, b"{not valid json")
            .map_err(|err| format!("write corrupt entry: {err}"))?;

        // Inventory must still return real classified seams.
        let result = inventory_classified_seams_at(&root)?;
        if result.is_empty() {
            return Err("inventory should compute real seams when cache is corrupt".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_cache_store_fails_when_inventory_runs_then_analysis_result_is_still_returned()
    -> Result<(), String> {
        let root = make_tempdir("storefail")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        // Reserve the path the cache would write to as a *directory*.
        // `std::fs::write` to a path that is a directory fails on
        // both POSIX and Windows; the inventory must still return
        // its in-memory result.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let dir = cache_dir_under(&root);
        std::fs::create_dir_all(dir.join(key.filename()))
            .map_err(|err| format!("mkdir conflict path: {err}"))?;

        let result = inventory_classified_seams_at(&root)?;
        if result.is_empty() {
            return Err("inventory should return real seams even when cache write fails".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_cached_classified_seams_when_related_test_changes_then_inventory_recomputes()
    -> Result<(), String> {
        // Pins the P1 invalidation contract end-to-end: a test-only
        // edit (no production change, no .ripr/* change) must bypass
        // the cache so stale TestGripEvidence cannot leak through.
        // Companion to the seam_cache::tests unit test that pins it
        // at the key derivation level.
        let root = make_tempdir("test-edit-invalidates")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;
        write_file(
            &root.join("tests/foo_test.rs"),
            "#[test] fn smoke() { assert_eq!(1, 1); }\n",
        )?;

        // Cold pass — populates the cache.
        let cold = inventory_classified_seams_at(&root)?;
        if cold.is_empty() {
            return Err("cold path should classify at least one seam".into());
        }

        // Poison the cached envelope's payload. If the next run reads
        // this file (i.e. the test edit did *not* change the key), it
        // will return [] and we'll see it.
        let entries = list_cache_entries(&root)?;
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly 1 cache entry after cold pass, got {}",
                entries.len()
            ));
        }
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        RepoSeamFactCache::at(&root).store_classified_seams_with_limit(&key, &[], None, 10)?;

        // Edit only the test file — production untouched, no .ripr/*
        // files involved. This must change the cache key so the
        // poisoned entry is bypassed.
        write_file(
            &root.join("tests/foo_test.rs"),
            "#[test] fn smoke() { assert!(super::discount(10, 5)); }\n",
        )?;

        let warm = inventory_classified_seams_at(&root)?;
        if warm.is_empty() {
            return Err(
                "test-only edit must invalidate the classified seam cache; got the poisoned \
                 empty entry, meaning stale TestGripEvidence would have leaked through"
                    .into(),
            );
        }

        // Sanity: a second cache file should now exist (under the new
        // key), not just the poisoned one.
        let entries_after = list_cache_entries(&root)?;
        if entries_after.len() < 2 {
            return Err(format!(
                "expected at least 2 cache entries after test-file edit (poisoned + recomputed), \
                 got {}",
                entries_after.len()
            ));
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn given_test_intent_or_suppressions_change_when_inventory_runs_then_cache_key_changes()
    -> Result<(), String> {
        let root = make_tempdir("intentkey")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn discount(amount: i32, threshold: i32) -> bool { amount >= threshold }\n",
        )?;

        let baseline = collect_workspace_state(&root, &RiprConfig::default())?.cache_key();

        // Add a `.ripr/test_intent.toml` and re-derive the key.
        write_file(
            &root.join(".ripr/test_intent.toml"),
            concat!(
                "[[test]]\n",
                "name = \"smoke\"\n",
                "owner = \"src/foo.rs\"\n",
                "intent = \"smoke\"\n",
                "reason = \"bar\"\n"
            ),
        )?;
        let with_intent = collect_workspace_state(&root, &RiprConfig::default())?.cache_key();
        if baseline.test_intent_hash == with_intent.test_intent_hash {
            return Err("adding test_intent.toml should change test_intent_hash".into());
        }
        if baseline.filename() == with_intent.filename() {
            return Err("adding test_intent.toml should change cache filename".into());
        }

        // Add `.ripr/suppressions.toml` and re-derive again.
        write_file(
            &root.join(".ripr/suppressions.toml"),
            concat!(
                "[[suppression]]\n",
                "kind = \"exposure_gap\"\n",
                "owner = \"src/foo.rs\"\n",
                "reason = \"bar\"\n"
            ),
        )?;
        let with_both = collect_workspace_state(&root, &RiprConfig::default())?.cache_key();
        if with_intent.suppressions_hash == with_both.suppressions_hash {
            return Err("adding suppressions.toml should change suppressions_hash".into());
        }
        if with_intent.filename() == with_both.filename() {
            return Err("adding suppressions.toml should change cache filename".into());
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    // ---- seam_limit cache-roundtrip integration test (Slice B) ------------
    //
    // Anti-regression test: a capped cold run stores limit_info; a second run
    // (cache hit, same workspace + same limit) must STILL return
    // seam_limit_applied, NOT complete. This is the entire point of Slice B.

    // The cache-roundtrip integration test is covered by the cli_smoke test
    // `check_repo_exposure_json_cache_roundtrip_preserves_seam_limit_applied`
    // which exercises the full binary pipeline with RIPR_REPO_EXPOSURE_SEAM_LIMIT=1.
    // Here we test the internal store/load round-trip at the cache module level
    // without touching process env (to stay within unsafe_code=forbid).
    #[test]
    fn capped_cold_run_stores_limit_info_and_warm_run_returns_seam_limit_applied()
    -> Result<(), String> {
        use super::super::seam_cache::CLASSIFIED_SEAM_CACHE_STORE_LIMIT;
        use super::super::seam_cache::{CacheLoad, CachedSeamLimitInfo, RepoSeamFactCache};
        let root = make_tempdir("cache-roundtrip-seam-limit")?;
        write_file(
            &root.join("src/foo.rs"),
            "pub fn check_a(x: i32) -> bool { x > 0 }\n\
             pub fn check_b(x: i32) -> bool { x < 0 }\n",
        )?;
        // Collect the workspace state so we can derive the key.
        let state = collect_workspace_state(&root, &RiprConfig::default())?;
        let key = state.cache_key();
        let cache_dir = cache_dir_under(&root);
        let cache = RepoSeamFactCache::at_dir(cache_dir.clone());
        // Simulate what a capped cold run would store: 1 seam analyzed, 2 total.
        let cold_limit = CachedSeamLimitInfo {
            analyzed: 1,
            total: 2,
            source: SeamLimitSource::Configured,
        };
        // We'll store an empty seam list with the limit_info to keep the test
        // lightweight (we're testing the store/load round-trip, not classification).
        cache
            .store_classified_seams_with_limit(
                &key,
                &[],
                Some(&cold_limit),
                CLASSIFIED_SEAM_CACHE_STORE_LIMIT,
            )
            .map_err(|err| format!("store capped cold run: {err}"))?;

        // Warm load (same key) must return the limit_info, NOT None.
        let result = match cache.load_classified_seams(&key) {
            CacheLoad::Hit((_, warm_limit)) => match warm_limit {
                None => Err(
                    "warm cache hit after capped cold run must return Some(limit_info), \
                         not None — a None would incorrectly signal run_status=complete"
                        .to_string(),
                ),
                Some(li) => {
                    if li.analyzed != cold_limit.analyzed {
                        Err(format!(
                            "warm limit_info.analyzed ({}) must match cold ({})",
                            li.analyzed, cold_limit.analyzed
                        ))
                    } else if li.total != cold_limit.total {
                        Err(format!(
                            "warm limit_info.total ({}) must match cold ({})",
                            li.total, cold_limit.total
                        ))
                    } else if li.source.as_str() != cold_limit.source.as_str() {
                        Err(format!(
                            "warm limit_info.source ({}) must match cold ({})",
                            li.source.as_str(),
                            cold_limit.source.as_str()
                        ))
                    } else {
                        Ok(())
                    }
                }
            },
            other => Err(format!("expected Hit on warm cache, got {other:?}")),
        };
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    // ---- seam_limit_source unit tests (Slice B) ----------------------------

    #[test]
    fn seam_limit_source_as_str_returns_correct_strings() {
        assert_eq!(SeamLimitSource::Default.as_str(), "default");
        assert_eq!(SeamLimitSource::Configured.as_str(), "configured");
    }

    #[test]
    fn parse_repo_exposure_seam_limit_zero_returns_none() {
        // "0" is the opt-out value: unbounded.
        assert_eq!(parse_repo_exposure_seam_limit("0"), None);
    }

    #[test]
    fn parse_repo_exposure_seam_limit_positive_returns_some() {
        assert_eq!(parse_repo_exposure_seam_limit("7500"), Some(7500));
    }

    #[test]
    fn apply_repo_exposure_seam_limit_below_cap_returns_none() -> Result<(), String> {
        // Build a tiny seam vec; limit >> vec size → no truncation.
        let path = PathBuf::from("src/a.rs");
        let source = "pub fn check(x: i32) -> bool { x > 0 }\n";
        let index = index_from_files(&[(path.clone(), source)])?;
        let mut seams = inventory_seams_from_index(&[path], &index);
        let result = apply_repo_exposure_seam_limit_for_test(
            &mut seams,
            Some((999_999, SeamLimitSource::Configured)),
        );
        if result.is_some() {
            return Err(format!(
                "below-cap inventory should produce None limit_info, got {result:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn apply_repo_exposure_seam_limit_above_cap_returns_some_with_configured_source()
    -> Result<(), String> {
        // Two seams; cap at 1 → truncation.
        let path = PathBuf::from("src/b.rs");
        let source = r#"
pub fn check_a(x: i32) -> bool { x > 0 }
pub fn check_b(x: i32) -> bool { x < 0 }
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let mut seams = inventory_seams_from_index(&[path], &index);
        if seams.len() < 2 {
            return Ok(()); // can't test truncation without at least 2 seams
        }
        let total_before = seams.len();
        let result = apply_repo_exposure_seam_limit_for_test(
            &mut seams,
            Some((1, SeamLimitSource::Configured)),
        );
        let info = result.ok_or("expected Some(SeamLimitInfo) for above-cap run")?;
        assert_eq!(info.analyzed, 1, "analyzed should be 1");
        assert_eq!(
            info.total, total_before,
            "total should be pre-truncation count"
        );
        assert_eq!(
            info.source,
            SeamLimitSource::Configured,
            "env-set limit should be Configured"
        );
        assert_eq!(seams.len(), 1, "seams should be truncated to 1");
        Ok(())
    }

    #[test]
    fn apply_repo_exposure_seam_limit_above_cap_returns_some_with_default_source()
    -> Result<(), String> {
        // Same truncation test, but with Default source (env var unset).
        let path = PathBuf::from("src/b2.rs");
        let source = r#"
pub fn check_a(x: i32) -> bool { x > 0 }
pub fn check_b(x: i32) -> bool { x < 0 }
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let mut seams = inventory_seams_from_index(&[path], &index);
        if seams.len() < 2 {
            return Ok(());
        }
        let result = apply_repo_exposure_seam_limit_for_test(
            &mut seams,
            Some((1, SeamLimitSource::Default)),
        );
        let info = result.ok_or("expected Some(SeamLimitInfo) for default-cap run")?;
        assert_eq!(
            info.source,
            SeamLimitSource::Default,
            "default limit should be Default"
        );
        Ok(())
    }

    #[test]
    fn apply_repo_exposure_seam_limit_opt_out_none_returns_none_for_any_size() -> Result<(), String>
    {
        // None limit → unbounded opt-out: no truncation.
        let path = PathBuf::from("src/c.rs");
        let source = r#"
pub fn check_a(x: i32) -> bool { x > 0 }
pub fn check_b(x: i32) -> bool { x < 0 }
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let mut seams = inventory_seams_from_index(&[path], &index);
        let result = apply_repo_exposure_seam_limit_for_test(&mut seams, None);
        assert!(
            result.is_none(),
            "None limit (opt-out) should return None (unbounded), got {result:?}"
        );
        Ok(())
    }

    // -- Pilot seam budget tests ---------------------------------------------

    #[test]
    fn pilot_seam_budget_default_constant_is_smaller_than_repo_exposure_cap() {
        // DEFAULT_PILOT_SEAM_BUDGET must be ≤ DEFAULT_REPO_EXPOSURE_SEAM_LIMIT
        // so the pilot budget can never produce a larger artifact than the
        // repo-exposure default.
        // Enforce the compile-time invariant: pilot budget must not exceed
        // the repo-exposure default cap.
        const _: () = assert!(
            DEFAULT_PILOT_SEAM_BUDGET <= DEFAULT_REPO_EXPOSURE_SEAM_LIMIT,
            "pilot budget must not exceed the repo-exposure seam limit"
        );
        assert_eq!(DEFAULT_PILOT_SEAM_BUDGET, 2_000);
    }

    #[test]
    fn pilot_seam_budget_env_zero_parses_as_unbounded() {
        // The same `parse_repo_exposure_seam_limit` helper is shared for
        // opt-out (value "0" → None means no budget applied).
        assert_eq!(parse_repo_exposure_seam_limit("0"), None);
        assert_eq!(parse_repo_exposure_seam_limit("500"), Some(500));
    }

    #[test]
    fn apply_pilot_seam_budget_inner_truncates_when_above_limit() -> Result<(), String> {
        // Use the inner fn to avoid env-var dependency in the test.
        use super::seam_classification::ClassifiedSeam;
        use super::test_grip_evidence::TestGripEvidence;
        use crate::analysis::seams::{
            ExpectedSink, RequiredDiscriminator, SeamGripClass, SeamKind,
        };
        use crate::domain::{Confidence, StageEvidence, StageState};
        use std::path::PathBuf;

        let stage = |state| StageEvidence::new(state, Confidence::Unknown, String::new());

        let make_classified = |byte_offset: usize| -> ClassifiedSeam {
            let seam = RepoSeam::new(
                PathBuf::from("src/lib.rs"),
                format!("owner_{byte_offset}"),
                SeamKind::ReturnValue,
                byte_offset,
                1,
                "x".to_string(),
                RequiredDiscriminator::ReturnValue {
                    description: "x".to_string(),
                },
                ExpectedSink::ReturnValue,
            );
            let seam_id = seam.id().clone();
            ClassifiedSeam {
                seam,
                evidence: TestGripEvidence {
                    seam_id,
                    related_tests: Vec::new(),
                    reach: stage(StageState::Unknown),
                    activate: stage(StageState::Unknown),
                    propagate: stage(StageState::Unknown),
                    observe: stage(StageState::Unknown),
                    discriminate: stage(StageState::Unknown),
                    observed_values: Vec::new(),
                    missing_discriminators: Vec::new(),
                    new_test_target: None,
                },
                class: SeamGripClass::Ungripped,
            }
        };

        let mut classified = vec![make_classified(0), make_classified(10), make_classified(20)];
        let info = apply_pilot_seam_budget_inner(&mut classified, 2, SeamLimitSource::Default);
        let info = info.ok_or("should truncate and return Some when limit < total")?;
        if classified.len() != 2 {
            return Err(format!(
                "expected classified.len() == 2, got {}",
                classified.len()
            ));
        }
        if info.analyzed != 2 || info.total != 3 {
            return Err(format!(
                "expected analyzed=2 total=3, got analyzed={} total={}",
                info.analyzed, info.total
            ));
        }
        if info.source != SeamLimitSource::Default {
            return Err(format!(
                "expected SeamLimitSource::Default, got {:?}",
                info.source
            ));
        }
        Ok(())
    }

    #[test]
    fn apply_pilot_seam_budget_inner_returns_none_when_at_or_below_limit() {
        use super::seam_classification::ClassifiedSeam;
        use super::test_grip_evidence::TestGripEvidence;
        use crate::analysis::seams::{
            ExpectedSink, RequiredDiscriminator, SeamGripClass, SeamKind,
        };
        use crate::domain::{Confidence, StageEvidence, StageState};
        use std::path::PathBuf;

        let stage = |state| StageEvidence::new(state, Confidence::Unknown, String::new());

        let make_classified = |byte_offset: usize| -> ClassifiedSeam {
            let seam = RepoSeam::new(
                PathBuf::from("src/lib.rs"),
                format!("owner_{byte_offset}"),
                SeamKind::ReturnValue,
                byte_offset,
                1,
                "x".to_string(),
                RequiredDiscriminator::ReturnValue {
                    description: "x".to_string(),
                },
                ExpectedSink::ReturnValue,
            );
            let seam_id = seam.id().clone();
            ClassifiedSeam {
                seam,
                evidence: TestGripEvidence {
                    seam_id,
                    related_tests: Vec::new(),
                    reach: stage(StageState::Unknown),
                    activate: stage(StageState::Unknown),
                    propagate: stage(StageState::Unknown),
                    observe: stage(StageState::Unknown),
                    discriminate: stage(StageState::Unknown),
                    observed_values: Vec::new(),
                    missing_discriminators: Vec::new(),
                    new_test_target: None,
                },
                class: SeamGripClass::Ungripped,
            }
        };

        let mut classified = vec![make_classified(0), make_classified(10)];
        let info = apply_pilot_seam_budget_inner(&mut classified, 5, SeamLimitSource::Default);
        assert!(
            info.is_none(),
            "slice smaller than budget must return None, got {info:?}"
        );
        assert_eq!(classified.len(), 2, "slice should be unchanged");
    }

    #[test]
    fn changed_test_inventory_recomputes_only_directly_called_owner_seams() -> Result<(), String> {
        let root = make_tempdir("targeted-test-owner-selection")?;
        write_file(
            &root.join("src/lib.rs"),
            r#"
pub fn discounted_total(amount: i32) -> i32 { if amount >= 100 { amount - 10 } else { amount } }
pub fn unrelated_total(amount: i32) -> i32 { if amount >= 50 { amount - 5 } else { amount } }
"#,
        )?;
        write_file(
            &root.join("tests/pricing.rs"),
            r#"
#[test]
fn discounted_total_case() {
    assert_eq!(discounted_total(100), 90);
}
"#,
        )?;

        let inventory = inventory_changed_test_classified_seams_at_with_config_node(
            &root,
            &RiprConfig::default(),
            Path::new("tests/pricing.rs"),
            None,
        )?;
        if inventory.selected_test_count != 1 {
            return Err(format!(
                "expected one selected test, got {}",
                inventory.selected_test_count
            ));
        }
        if inventory.direct_call_names != ["discounted_total".to_string()] {
            return Err(format!(
                "unexpected owner-call selection: {:?}",
                inventory.direct_call_names
            ));
        }
        if inventory.classified.is_empty() {
            return Err("expected directly called owner seams".to_string());
        }
        if inventory
            .classified
            .iter()
            .any(|entry| entry.seam.owner().ends_with("::unrelated_total"))
        {
            return Err("unrelated owner seams must not be recomputed".to_string());
        }
        if inventory.file_fact_cache.misses == 0 {
            return Err("cold targeted run should record file-fact misses".to_string());
        }

        let warm = inventory_changed_test_classified_seams_at_with_config_node(
            &root,
            &RiprConfig::default(),
            Path::new("tests/pricing.rs"),
            None,
        )?;
        if warm.file_fact_cache.hits == 0 || warm.file_fact_cache.misses != 0 {
            return Err(format!(
                "warm targeted run should reuse all file facts, got {:?}",
                warm.file_fact_cache
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn changed_test_inventory_selects_one_test_node_within_a_file() -> Result<(), String> {
        let root = make_tempdir("targeted-test-node-selection")?;
        write_file(
            &root.join("src/lib.rs"),
            r#"
pub fn discounted_total(amount: i32) -> i32 { if amount >= 100 { amount - 10 } else { amount } }
pub fn surcharge_total(amount: i32) -> i32 { if amount >= 50 { amount + 5 } else { amount } }
"#,
        )?;
        write_file(
            &root.join("tests/pricing.rs"),
            r#"
#[test]
fn discounted_total_case() { assert_eq!(discounted_total(100), 90); }
#[test]
fn surcharge_total_case() { assert_eq!(surcharge_total(50), 55); }
"#,
        )?;

        let inventory = inventory_changed_test_classified_seams_at_with_config_node(
            &root,
            &RiprConfig::default(),
            Path::new("tests/pricing.rs"),
            Some("discounted_total_case"),
        )?;
        if inventory.selected_test_count != 1
            || inventory.direct_call_names != ["discounted_total".to_string()]
            || inventory
                .classified
                .iter()
                .any(|entry| entry.seam.owner().ends_with("::surcharge_total"))
        {
            return Err(format!(
                "test-node selector did not isolate the requested test: count={} calls={:?}",
                inventory.selected_test_count, inventory.direct_call_names
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn changed_test_inventory_rejects_unknown_test_node() -> Result<(), String> {
        let root = make_tempdir("targeted-test-node-missing")?;
        write_file(
            &root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 { amount }",
        )?;
        write_file(
            &root.join("tests/pricing.rs"),
            "#[test] fn discounted_total_case() { assert_eq!(discounted_total(1), 1); }",
        )?;
        let result = inventory_changed_test_classified_seams_at_with_config_node(
            &root,
            &RiprConfig::default(),
            Path::new("tests/pricing.rs"),
            Some("missing_case"),
        );
        let _ = std::fs::remove_dir_all(&root);
        match result {
            Err(TargetedTestInventoryError::Selector {
                kind: CHANGED_TEST_UNRESOLVED,
                message,
                ..
            }) if message.contains("::missing_case") => Ok(()),
            Err(TargetedTestInventoryError::Selector { kind, message, .. }) => Err(format!(
                "unexpected missing-node limitation {kind}: {message}"
            )),
            Err(TargetedTestInventoryError::Analysis(message)) => {
                Err(format!("unexpected missing-node diagnostic: {message}"))
            }
            Ok(_) => Err("unknown test node must fail closed".to_string()),
        }
    }

    #[test]
    fn changed_test_inventory_refuses_ambiguous_direct_production_owner() -> Result<(), String> {
        let root = make_tempdir("targeted-test-ambiguous-owner")?;
        write_file(
            &root.join("src/first.rs"),
            "pub fn same_name(amount: i32) -> i32 { if amount > 0 { amount } else { 0 } }",
        )?;
        write_file(
            &root.join("src/second.rs"),
            "pub fn same_name(amount: i32) -> i32 { if amount >= 0 { amount } else { 0 } }",
        )?;
        write_file(
            &root.join("tests/pricing.rs"),
            "#[test] fn same_name_case() { assert_eq!(same_name(1), 1); }",
        )?;

        let result = inventory_changed_test_classified_seams_at_with_config_node(
            &root,
            &RiprConfig::default(),
            Path::new("tests/pricing.rs"),
            None,
        );
        let _ = std::fs::remove_dir_all(&root);
        match result {
            Err(TargetedTestInventoryError::Selector {
                kind: CHANGED_TEST_OWNER_AMBIGUOUS,
                message,
                selected_test_count: 1,
                ..
            }) if message.contains("ambiguous direct production owner `same_name`") => Ok(()),
            Err(TargetedTestInventoryError::Selector { kind, message, .. }) => {
                Err(format!("unexpected ambiguity limitation {kind}: {message}"))
            }
            Err(TargetedTestInventoryError::Analysis(message)) => {
                Err(format!("unexpected ambiguity diagnostic: {message}"))
            }
            Ok(_) => Err("ambiguous direct owner must fail closed".to_string()),
        }
    }
}
