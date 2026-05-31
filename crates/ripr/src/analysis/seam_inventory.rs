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
//!    `workspace::is_production_rust_path`).
//!
//! Both contracts are pinned by tests in this file.

use super::rust_index::{
    self, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, RustIndex,
};
#[cfg(test)]
use super::seam_cache::RepoSeamCountCache;
use super::seam_cache::{
    CLASSIFIED_SEAM_CACHE_STORE_LIMIT, CacheLoad, RepoSeamFactCache, WorkspaceState,
    compact_classified_seam_cache_store_limit,
};
#[cfg(test)]
use super::seam_classification::SeamGripClassCounts;
use super::seam_classification::{self, ClassifiedSeam};
use super::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use super::test_grip_evidence;
use super::workspace;
use crate::config::RiprConfig;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const REPO_EXPOSURE_SEAM_LIMIT_ENV: &str = "RIPR_REPO_EXPOSURE_SEAM_LIMIT";

const LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";

/// Walk production Rust files at `root` and emit the raw seam inventory.
/// Used by the `repo-seams-*` formats; the classified inventory used by
/// `repo-exposure-*` formats lives in [`inventory_classified_seams_at`].
pub(crate) fn inventory_seams_at(root: &Path) -> Result<Vec<RepoSeam>, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::is_production_rust_path(p))
        .cloned()
        .collect();

    // Index the full set so `find_owner_function` can resolve owners
    // even when the seam appears in a file the production filter
    // includes but tests reference.
    let index = rust_index::build_index(root, &rust_files)?;
    Ok(inventory_seams_from_index(&production_files, &index))
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
}

pub(crate) fn inventory_classified_seams_at_with_config(
    root: &Path,
    config: &RiprConfig,
) -> Result<Vec<ClassifiedSeam>, String> {
    let total_started = Instant::now();
    let cache = RepoSeamFactCache::at(root);
    let collect_started = Instant::now();
    let state = match collect_workspace_state(root, config) {
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
    if repo_exposure_seam_limit().is_some() {
        trace_latency_phase("cache_load", "skipped_due_seam_limit", Duration::ZERO);
        let classified = inventory_classified_seams_from_state_with_config(&state, config)?;
        trace_latency_phase("total", "sampled_computed", total_started.elapsed());
        return Ok(classified);
    }
    let cache_started = Instant::now();
    trace_latency_phase(
        "cache_load",
        &format!("start_files_{}", state.files.len()),
        Duration::ZERO,
    );
    match cache.load_classified_seams(&key) {
        CacheLoad::Hit(cached) => {
            trace_latency_phase("cache_load", "hit", cache_started.elapsed());
            trace_latency_phase("total", "cache_hit", total_started.elapsed());
            return Ok(cached);
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
    let classified = match inventory_classified_seams_from_state_with_config(&state, config) {
        Ok(classified) => {
            trace_latency_phase("cold_compute", "ok", compute_started.elapsed());
            classified
        }
        Err(err) => {
            trace_latency_phase("cold_compute", "error", compute_started.elapsed());
            trace_latency_phase("total", "error", total_started.elapsed());
            return Err(err);
        }
    };
    // Best-effort write: a write failure does not fail analysis. The
    // result is already in memory; the next run just sees a miss again.
    let store_started = Instant::now();
    trace_latency_phase(
        "cache_store",
        &format!(
            "start_classified_{}_limit_{}",
            classified.len(),
            CLASSIFIED_SEAM_CACHE_STORE_LIMIT
        ),
        Duration::ZERO,
    );
    let store_status = match cache.store_classified_seams(&key, &classified) {
        Ok(()) => "ok".to_string(),
        Err(reason) => {
            eprintln!("ripr: repo seam cache store ignored ({reason})");
            cache_store_status_label(&reason)
        }
    };
    trace_latency_phase("cache_store", &store_status, store_started.elapsed());
    trace_latency_phase("total", "computed", total_started.elapsed());
    Ok(classified)
}

fn trace_latency_phase(phase: &str, status: &str, duration: Duration) {
    if std::env::var_os(LATENCY_TRACE_ENV).is_some() {
        eprintln!("{}", latency_trace_line(phase, status, duration));
    }
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
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::is_production_rust_path(p))
        .cloned()
        .collect();
    trace_latency_phase("filter_production_files", "ok", filter_started.elapsed());

    let index_started = Instant::now();
    let mut index = match rust_index::build_index(root, &rust_files) {
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
    let state = collect_workspace_state(root, config)?;
    let key = state.cache_key();
    let cache_started = Instant::now();
    match cache.load_classified_seams(&key) {
        CacheLoad::Hit(cached) => {
            trace_latency_phase("compact_cache_load", "hit", cache_started.elapsed());
            trace_latency_phase("total", "compact_cache_hit", total_started.elapsed());
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

    let classified = inventory_compact_classified_seams_from_state_with_config(&state, config)?;
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
    let store_status =
        match cache.store_compact_classified_seams_with_limit(&key, &classified, store_limit) {
            Ok(()) => "ok".to_string(),
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
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::is_production_rust_path(p))
        .cloned()
        .collect();

    let mut index = rust_index::build_index(root, &rust_files)?;
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
) -> Result<Vec<ClassifiedSeam>, String> {
    let production_files = production_files_from_state(state);
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
    let mut cached =
        rust_index::build_index_from_loaded_files_with_cache(&state.workspace_root, &state.files)?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    let seams = inventory_seams_from_index(&production_files, &cached.index);
    let context = test_grip_evidence::CompactGripContext::new(&cached.index);
    let mut classified = Vec::with_capacity(seams.len());
    for seam in seams {
        let evidence = test_grip_evidence::compact_evidence_for_seam(&seam, &context);
        let class = seam_classification::classify_seam(&seam, &evidence);
        classified.push(ClassifiedSeam {
            evidence,
            seam,
            class,
        });
    }
    Ok(classified)
}

fn inventory_classified_seams_from_state_with_config(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Result<Vec<ClassifiedSeam>, String> {
    let production_files = production_files_from_state(state);
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
    let mut cached =
        rust_index::build_index_from_loaded_files_with_cache(&state.workspace_root, &state.files)?;
    trace_latency_phase(
        "file_fact_cache",
        &cached.file_fact_cache.status_label(),
        build_started.elapsed(),
    );
    let policy_started = Instant::now();
    rust_index::apply_oracle_policy(&mut cached.index, config.oracles());
    trace_latency_phase("apply_oracle_policy", "ok", policy_started.elapsed());
    let seams_started = Instant::now();
    let mut seams = inventory_seams_from_index(&production_files, &cached.index);
    trace_latency_phase("inventory_seams", "ok", seams_started.elapsed());
    apply_repo_exposure_seam_limit(&mut seams);
    let evidence_started = Instant::now();
    trace_latency_phase(
        "evidence_for_seams",
        &format!("start_seams_{}", seams.len()),
        Duration::ZERO,
    );
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &cached.index);
    trace_latency_phase("evidence_for_seams", "ok", evidence_started.elapsed());
    let classify_started = Instant::now();
    let classified = seam_classification::classify_seams_owned(seams, evidence);
    trace_latency_phase("classify_seams", "ok", classify_started.elapsed());
    Ok(classified)
}

fn repo_exposure_seam_limit() -> Option<usize> {
    std::env::var(REPO_EXPOSURE_SEAM_LIMIT_ENV)
        .ok()
        .and_then(|value| parse_repo_exposure_seam_limit(&value))
}

fn parse_repo_exposure_seam_limit(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|limit| *limit > 0)
}

fn apply_repo_exposure_seam_limit(seams: &mut Vec<RepoSeam>) {
    let Some(limit) = repo_exposure_seam_limit() else {
        return;
    };
    let total = seams.len();
    if total <= limit {
        return;
    }
    seams.truncate(limit);
    trace_latency_phase(
        "repo_exposure_seam_limit",
        &format!("limit_{}_of_{total}", seams.len()),
        Duration::ZERO,
    );
}

#[cfg(test)]
fn inventory_seam_grip_class_counts_from_state_with_config(
    state: &OwnedWorkspaceState,
    config: &RiprConfig,
) -> Result<SeamGripClassCounts, String> {
    let production_files = production_files_from_state(state);
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
    let mut cached =
        rust_index::build_index_from_loaded_files_with_cache(&state.workspace_root, &state.files)?;
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

fn production_files_from_state(state: &OwnedWorkspaceState) -> Vec<PathBuf> {
    state
        .files
        .iter()
        .map(|(path, _)| path)
        .filter(|path| workspace::is_production_rust_path(path))
        .cloned()
        .collect()
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
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(rust_files.len());
    for path in rust_files {
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
        WorkspaceState {
            workspace_root: &self.workspace_root,
            files: &self.files,
            cfg_features: None,
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
    // `is_production_rust_path` already excludes physical test files;
    // this catches inline test modules.
    if owner_fact.is_test {
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
            variant: expression.to_string(),
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
    use crate::analysis::rust_index::{
        FileFacts, FunctionFact, RaRustSyntaxAdapter, RustSyntaxAdapter,
    };
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
        // Caller filters production files exactly the way `inventory_seams_at`
        // does: `is_production_rust_path` excludes anything whose path
        // contains a `tests` segment.
        let production_files: Vec<PathBuf> = [prod, test_path.clone()]
            .into_iter()
            .filter(|p| workspace::is_production_rust_path(p))
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
            .join("0.1")
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
            is_test: false,
            attrs: Vec::new(),
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
        // production-only even when `is_production_rust_path` cannot
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
            is_test: true,
            attrs: vec!["#[test]".to_string()],
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
            "expected no seams when the only owner is `is_test = true`, got owners {owners:?}"
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
        let cache_file = &entries[0];
        let bytes = std::fs::read(cache_file)
            .map_err(|err| format!("read {}: {err}", cache_file.display()))?;
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|err| format!("parse compact cache: {err}"))?;
        envelope["classified_seams"] = serde_json::Value::Array(Vec::new());
        let rewritten =
            serde_json::to_vec(&envelope).map_err(|err| format!("encode compact cache: {err}"))?;
        std::fs::write(cache_file, rewritten)
            .map_err(|err| format!("rewrite {}: {err}", cache_file.display()))?;

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
        let cache_file = &entries[0];
        let bytes = std::fs::read(cache_file)
            .map_err(|err| format!("read {}: {err}", cache_file.display()))?;
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|err| format!("parse cache: {err}"))?;
        envelope["classified_seams"] = serde_json::Value::Array(Vec::new());
        let rewritten =
            serde_json::to_vec(&envelope).map_err(|err| format!("encode cache: {err}"))?;
        std::fs::write(cache_file, rewritten)
            .map_err(|err| format!("rewrite {}: {err}", cache_file.display()))?;

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
        let cache_file = &entries[0];
        let bytes = std::fs::read(cache_file)
            .map_err(|err| format!("read {}: {err}", cache_file.display()))?;
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|err| format!("parse cache: {err}"))?;
        envelope["classified_seams"] = serde_json::Value::Array(Vec::new());
        let rewritten =
            serde_json::to_vec(&envelope).map_err(|err| format!("encode cache: {err}"))?;
        std::fs::write(cache_file, rewritten)
            .map_err(|err| format!("rewrite {}: {err}", cache_file.display()))?;

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
}
