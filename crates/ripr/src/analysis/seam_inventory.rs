//! Voice B repo seam inventory walker per RIPR-SPEC-0005.
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
use super::seam_cache::{CacheLoad, RepoSeamFactCache, WorkspaceState};
use super::seam_classification::{self, ClassifiedSeam};
use super::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use super::test_grip_evidence;
use super::workspace;
use std::path::{Path, PathBuf};

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
pub(crate) fn inventory_classified_seams_at(root: &Path) -> Result<Vec<ClassifiedSeam>, String> {
    let cache = RepoSeamFactCache::at(root);
    let state = collect_workspace_state(root)?;
    let key = state.cache_key();
    match cache.load_classified_seams(&key) {
        CacheLoad::Hit(cached) => return Ok(cached),
        CacheLoad::Miss => {}
        CacheLoad::CorruptIgnored { reason } => {
            // Advisory: surface the reason so operators can see why a
            // warm path degraded to cold. Never fail analysis.
            eprintln!("ripr: repo seam cache entry ignored ({reason})");
        }
    }
    let classified = inventory_classified_seams_uncached(root)?;
    // Best-effort write: a write failure does not fail analysis. The
    // result is already in memory; the next run just sees a miss again.
    let _ = cache.store_classified_seams(&key, &classified);
    Ok(classified)
}

/// Cold-path inventory + classify with no cache. Used by the cached
/// entry point on miss and by tests that want to drive the pipeline
/// directly. Stays crate-private; the public entry is the cached
/// function above.
pub(crate) fn inventory_classified_seams_uncached(
    root: &Path,
) -> Result<Vec<ClassifiedSeam>, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::is_production_rust_path(p))
        .cloned()
        .collect();

    let index = rust_index::build_index(root, &rust_files)?;
    let seams = inventory_seams_from_index(&production_files, &index);
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &index);
    Ok(seam_classification::classify_seams(&seams, &evidence))
}

/// Collect the per-file content + intent + suppressions inputs the
/// cache key derives from. Reads files once; the build_index path
/// reads them again, but the cost is minor compared to parsing. A
/// future optimization can share the file contents.
///
/// Hashes the **same Rust file set fed to `build_index`** — production
/// seam sources *and* test evidence sources. `ClassifiedSeam` carries
/// `TestGripEvidence` derived from test files, so a test-only edit must
/// invalidate the cache; filtering to production-only here would let
/// stale grip evidence survive a test rewrite.
fn collect_workspace_state(root: &Path) -> Result<OwnedWorkspaceState, String> {
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
        test_intent_text: read_optional(&root.join(".ripr").join("test_intent.toml")),
        suppressions_text: read_optional(&root.join(".ripr").join("suppressions.toml")),
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
    test_intent_text: Option<String>,
    suppressions_text: Option<String>,
}

impl OwnedWorkspaceState {
    fn cache_key(&self) -> super::seam_cache::RepoSeamCacheKey {
        WorkspaceState {
            workspace_root: &self.workspace_root,
            files: &self.files,
            cfg_features: None,
            config_text: None,
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
        // syntax of a call site. In repo (Voice B) scope the same shape
        // is the seam asking "are tests verifying this call happens at
        // all?" — i.e. `SeamKind::CallPresence`.
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
    use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};

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
            .join("0.1")
    }

    fn list_cache_entries(root: &Path) -> Result<Vec<PathBuf>, String> {
        let dir = cache_dir_under(root);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in
            std::fs::read_dir(&dir).map_err(|err| format!("read {}: {err}", dir.display()))?
        {
            let entry = entry.map_err(|err| format!("read entry: {err}"))?;
            out.push(entry.path());
        }
        out.sort();
        Ok(out)
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
        let state = collect_workspace_state(&root)?;
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
        let state = collect_workspace_state(&root)?;
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

        let baseline = collect_workspace_state(&root)?.cache_key();

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
        let with_intent = collect_workspace_state(&root)?.cache_key();
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
        let with_both = collect_workspace_state(&root)?.cache_key();
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
