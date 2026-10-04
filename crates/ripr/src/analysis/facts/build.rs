#[cfg(test)]
mod bounded_batch_tests;
#[cfg(test)]
mod incremental_edit_tests;

use super::super::syntax::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter};
use super::model::{RustIndex, WorkspaceRootAuthority};
use crate::analysis::cancellation;
use crate::analysis::seam_cache::{
    CacheLoad, FileFactCacheStats, RepoFileFactCache, RepoFileFactCacheKey,
};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Files parsed per parallel batch. Each worker installs the owning request's
/// cancellation token and checks before and after its file, so queued work can
/// stop without parsing the rest of a batch. One parser call remains cooperative,
/// not preemptible; callers must still enforce file/closure admission limits.
const PARSE_BATCH_FILES: usize = 64;

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<RustIndex, String> {
    build_index_with_adapters(root, files, &RaRustSyntaxAdapter, &LexicalRustSyntaxAdapter)
}

pub(crate) struct CachedRustIndex {
    pub(crate) index: RustIndex,
    pub(crate) file_fact_cache: FileFactCacheStats,
}

pub(crate) fn build_index_from_loaded_files_with_cache(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
) -> Result<CachedRustIndex, String> {
    build_index_from_loaded_files_with_cache_and_adapters(
        root,
        files,
        &RaRustSyntaxAdapter,
        &LexicalRustSyntaxAdapter,
    )
}

fn build_index_from_loaded_files_with_cache_and_adapters(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    adapter: &(dyn RustSyntaxAdapter + Send + Sync),
    fallback: &(dyn RustSyntaxAdapter + Send + Sync),
) -> Result<CachedRustIndex, String> {
    let cache = RepoFileFactCache::at(root);
    build_index_with_file_fact_cache(root, files, adapter, fallback, &cache, || {
        cache.known_file_paths()
    })
}

fn build_index_with_file_fact_cache(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    adapter: &(dyn RustSyntaxAdapter + Send + Sync),
    fallback: &(dyn RustSyntaxAdapter + Send + Sync),
    cache: &RepoFileFactCache,
    mut load_known_file_paths: impl FnMut() -> HashSet<PathBuf>,
) -> Result<CachedRustIndex, String> {
    let mut accounting = CacheAccounting::default();
    let batched = insert_cached_file_batches(
        root,
        files,
        adapter,
        fallback,
        cache,
        &mut load_known_file_paths,
        &mut accounting,
    );
    let CacheAccounting {
        stats,
        first_corrupt_reason,
    } = accounting;
    // Emitted once on every exit after lookup began, before finalization.
    // A complete build reports the whole-corpus count. An aborted build
    // reports the entries found in the batches it admitted: later batches
    // are never looked up, so their entries are not claimed either way.
    emit_corrupt_entries_warning(stats.corrupt_ignored, first_corrupt_reason.as_deref());
    let mut index = batched?;
    cancellation::checkpoint()?;
    super::includes::resolve_repository_local_includes(root, &mut index);
    cancellation::checkpoint()?;
    index.workspace_authority = Some(WorkspaceRootAuthority::from_sources(
        root,
        index
            .files()
            .iter()
            .map(|(path, facts)| (path, facts.data().source.as_str())),
    ));
    cancellation::checkpoint()?;
    index.package_names = manifest_package_names(root);
    cancellation::checkpoint()?;
    Ok(CachedRustIndex {
        index,
        file_fact_cache: stats,
    })
}

/// Cache statistics plus the first corrupt-entry reason, owned by the caller
/// so they survive an `Err` from the batch loop.
#[derive(Default)]
struct CacheAccounting {
    stats: FileFactCacheStats,
    // One stderr line per build, not one per file: an unusable cache
    // directory makes every lookup fail the same way, and a line per file
    // buried the analysis output under hundreds of repeats (#4888).
    first_corrupt_reason: Option<String>,
}

/// Admit, parse, store, and insert one `PARSE_BATCH_FILES` slice at a time
/// (#5029). Cache-hit facts and parsed misses live outside the canonical
/// index only for the current batch; every batch temporary is dropped before
/// the next batch is looked up. The returned index is a prefix-free whole:
/// any error or cancellation returns `Err`, never a partial index.
///
/// Earlier batches' fresh facts may already be stored when a later batch
/// fails. That matches the per-file, non-transactional cache contract: each
/// entry is keyed by its own content and remains a valid future hit.
fn insert_cached_file_batches(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    adapter: &(dyn RustSyntaxAdapter + Send + Sync),
    fallback: &(dyn RustSyntaxAdapter + Send + Sync),
    cache: &RepoFileFactCache,
    load_known_file_paths: &mut impl FnMut() -> HashSet<PathBuf>,
    accounting: &mut CacheAccounting,
) -> Result<RustIndex, String> {
    let CacheAccounting {
        stats,
        first_corrupt_reason,
    } = accounting;
    enum Pending {
        Ready(super::FileFacts),
        Parse { key: RepoFileFactCacheKey },
    }
    // Only genuine misses need historical paths for invalidation attribution.
    // All-hit, corrupt-only, empty, and already-cancelled builds must not walk
    // and decode the entire cache directory just to discard the inventory.
    // Initialized once, at the first miss.
    let mut known_cached_file_paths: Option<HashSet<PathBuf>> = None;
    let mut index = RustIndex::default();
    let token = cancellation::current_token();
    for batch in files.chunks(PARSE_BATCH_FILES) {
        // Phase 1 (sequential): this batch's cache lookups, in input order.
        let mut pending: Vec<Pending> = Vec::with_capacity(batch.len());
        for (file, bytes) in batch {
            cancellation::checkpoint()?;
            let key = RepoFileFactCacheKey::new(file, bytes);
            match cache.load_file_facts(&key) {
                CacheLoad::Hit(facts) => {
                    stats.hits += 1;
                    pending.push(Pending::Ready(facts));
                }
                CacheLoad::Miss => {
                    stats.misses += 1;
                    if known_cached_file_paths
                        .get_or_insert_with(&mut *load_known_file_paths)
                        .contains(file)
                    {
                        stats.invalidated_files.insert(file.clone());
                    }
                    pending.push(Pending::Parse { key });
                }
                CacheLoad::CorruptIgnored { reason } => {
                    stats.corrupt_ignored += 1;
                    first_corrupt_reason.get_or_insert(reason);
                    pending.push(Pending::Parse { key });
                }
            }
        }

        // Phase 2 (parallel): parse only this batch's misses. Each worker
        // installs the owning request's token. Indexed collection preserves
        // input order, and an observed source/worker failure wins in input
        // order before anything from this batch is stored or inserted.
        let parse_positions: Vec<usize> = pending
            .iter()
            .enumerate()
            .filter_map(|(position, entry)| {
                matches!(entry, Pending::Parse { .. }).then_some(position)
            })
            .collect();
        let mut parsed: Vec<Option<super::FileFacts>> = Vec::new();
        parsed.resize_with(batch.len(), || None);
        if !parse_positions.is_empty() {
            cancellation::checkpoint()?;
            let results: Vec<(usize, Result<super::FileFacts, String>)> = parse_positions
                .par_iter()
                .map(|&position| {
                    let result = cancellation::with_optional_token(token.as_ref(), || {
                        cancellation::checkpoint()?;
                        let (file, bytes) = &batch[position];
                        let facts = summarize_loaded_file(file, bytes, adapter, fallback)?;
                        cancellation::checkpoint()?;
                        Ok(facts)
                    });
                    (position, result)
                })
                .collect();
            // Do not replace an observed failure with a deadline noticed
            // only after joining.
            if let Some(error) = results.iter().find_map(|(_, result)| result.as_ref().err()) {
                return Err(error.clone());
            }
            cancellation::checkpoint()?;
            for (position, result) in results {
                parsed[position] = Some(result?);
            }
        }
        #[cfg(test)]
        uninserted_facts::record(
            pending
                .iter()
                .filter(|entry| matches!(entry, Pending::Ready(_)))
                .count()
                + parsed.iter().filter(|facts| facts.is_some()).count(),
        );

        // Phase 3 (sequential, input order): store fresh facts, then insert.
        // `pending` and `parsed` are consumed here and dropped at the end of
        // this iteration, before the next batch is admitted.
        for (position, entry) in pending.into_iter().enumerate() {
            let (file, bytes) = &batch[position];
            // Recorded on cache hits too, so a warm run discloses the same file.
            if rust_source_text(bytes).not_utf8 {
                index.non_utf8_sources.insert(file.clone());
            }
            let summary = match entry {
                Pending::Ready(facts) => facts,
                Pending::Parse { key } => {
                    let Some(facts) = parsed[position].take() else {
                        return Err(format!(
                            "missing parse result for {}",
                            root.join(file).display()
                        ));
                    };
                    cancellation::checkpoint()?;
                    match cache.store_file_facts(&key, &facts) {
                        Ok(()) => stats.stores += 1,
                        Err(error) => stats.record_store_failure(file.clone(), error),
                    }
                    facts
                }
            };
            cancellation::checkpoint()?;
            insert_file_summary(&mut index, file.clone(), summary);
            cancellation::checkpoint()?;
        }
    }
    Ok(index)
}

/// Test-only lifetime instrument for #5029: the most `FileFacts` held
/// outside the canonical index (cache hits plus parsed misses) at the point
/// just before insertion. A builder that stages the whole corpus before
/// inserting reports the corpus size; the bounded builder reports at most
/// one batch.
#[cfg(test)]
pub(super) mod uninserted_facts {
    use std::cell::Cell;

    thread_local! {
        static HIGH_WATER: Cell<usize> = const { Cell::new(0) };
    }

    pub(in crate::analysis::facts) fn record(live: usize) {
        HIGH_WATER.with(|high| high.set(high.get().max(live)));
    }

    /// Reset, run `work` on this thread, and return its result with the
    /// observed high-water mark.
    pub(in crate::analysis::facts) fn observe<T>(work: impl FnOnce() -> T) -> (T, usize) {
        HIGH_WATER.with(|high| high.set(0));
        let result = work();
        (result, HIGH_WATER.with(Cell::get))
    }
}

fn emit_corrupt_entries_warning(count: usize, first_reason: Option<&str>) {
    if let Some(reason) = first_reason {
        eprintln!("{}", corrupt_entries_warning(count, reason));
    }
}

/// The build's single corrupt-cache-entry warning. A lone entry keeps the
/// message it always had; several name the count and the first reason.
fn corrupt_entries_warning(count: usize, first_reason: &str) -> String {
    if count == 1 {
        format!("ripr: repo file fact cache entry ignored ({first_reason})")
    } else {
        format!("ripr: {count} repo file fact cache entries ignored; first: ({first_reason})")
    }
}

fn build_index_with_adapters(
    root: &Path,
    files: &[PathBuf],
    adapter: &(dyn RustSyntaxAdapter + Send + Sync),
    fallback: &(dyn RustSyntaxAdapter + Send + Sync),
) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
    let token = cancellation::current_token();
    for batch in files.chunks(PARSE_BATCH_FILES) {
        cancellation::checkpoint()?;
        // Read + parse run on rayon workers; every file is independent.
        // `collect` on an indexed parallel iterator preserves input order,
        // so `results[i]` corresponds to `batch[i]`.
        let results: Vec<Result<(PathBuf, super::FileFacts, bool), String>> = batch
            .par_iter()
            .map(|file| {
                cancellation::with_optional_token(token.as_ref(), || {
                    cancellation::checkpoint()?;
                    let full = root.join(file);
                    let bytes = std::fs::read(&full)
                        .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
                    cancellation::checkpoint()?;
                    let summary = summarize_loaded_file(file, &bytes, adapter, fallback)?;
                    cancellation::checkpoint()?;
                    Ok((file.clone(), summary, rust_source_text(&bytes).not_utf8))
                })
            })
            .collect();
        if let Some(error) = results.iter().find_map(|result| result.as_ref().err()) {
            return Err(error.clone());
        }
        cancellation::checkpoint()?;
        // Insert sequentially in original input order: `RustIndex.tests`
        // and `RustIndex.functions` are extended per file, so this drain
        // reproduces the sequential loop byte-for-byte — the first error
        // in input order wins and the per-iteration checkpoint ordering
        // (error first, then checkpoint) is unchanged.
        for result in results {
            let (file, summary, not_utf8) = result?;
            if not_utf8 {
                index.non_utf8_sources.insert(file.clone());
            }
            cancellation::checkpoint()?;
            insert_file_summary(&mut index, file, summary);
            cancellation::checkpoint()?;
        }
    }
    cancellation::checkpoint()?;
    super::includes::resolve_repository_local_includes(root, &mut index);
    cancellation::checkpoint()?;
    index.workspace_authority = Some(WorkspaceRootAuthority::from_sources(
        root,
        index
            .files()
            .iter()
            .map(|(path, facts)| (path, facts.data().source.as_str())),
    ));
    cancellation::checkpoint()?;
    index.package_names = manifest_package_names(root);
    cancellation::checkpoint()?;
    Ok(index)
}

/// The crate names of the analyzed root manifest: the `[package] name`
/// plus the `[lib] name` target when the manifest declares one. Root
/// manifest only: member manifests are not resolved here, so multi-crate
/// workspaces leave member-crate names unlisted and the same-name-import
/// gate treats member imports as foreign (fail-closed under-credit; see
/// `RustIndex.package_names`). Each name is stored in BOTH spellings —
/// raw and crate-identifier form (#3731 review F23: hyphens normalize to
/// underscores in crate identifiers, so a package named `foo-bar` is
/// imported as `foo_bar`, and integration tests import the `[lib]`
/// target, whose name may differ from the package name) — so the
/// import gate admits an import through any of the crate's own
/// identifiers. A missing or unparseable manifest yields an empty set,
/// never an error: the gate only ever under-credits.
fn manifest_package_names(root: &Path) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return names;
    };
    // A manifest is a TOML document (a table), not a standalone value:
    // `toml::Value::from_str` would reject the `[package]` header.
    let Ok(value) = text.parse::<toml::Table>() else {
        return names;
    };
    let mut insert = |name: &str| {
        names.insert(name.to_string());
        names.insert(name.replace('-', "_"));
    };
    if let Some(name) = value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
    {
        insert(name);
    }
    if let Some(name) = value
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(toml::Value::as_str)
    {
        insert(name);
    }
    names
}

/// Source text of one Rust file as the index reads it.
///
/// rustc drops a leading UTF-8 byte-order mark before lexing, so the index
/// does too: left in place, the stray `U+FEFF` was a parse error, the whole
/// file fell to lexical fallback, and an item on line 1 lost its owner. The
/// mark has no newline, so line numbers are unchanged.
///
/// Bytes that are not UTF-8 do not abort the run: rustc rejects such a file,
/// so one stray fixture used to fail every `ripr check` in the workspace. The
/// text is decoded lossily and `not_utf8` is set so the caller keeps it on
/// lexical fallback and discloses `rust_source_not_utf8`.
pub(crate) struct RustSourceText<'a> {
    pub(crate) text: std::borrow::Cow<'a, str>,
    pub(crate) not_utf8: bool,
}

pub(crate) fn rust_source_text(bytes: &[u8]) -> RustSourceText<'_> {
    let bytes = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let not_utf8 = matches!(text, std::borrow::Cow::Owned(_));
    RustSourceText { text, not_utf8 }
}

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Typed static-limit reason for a Rust source that is not UTF-8.
pub(crate) const RUST_SOURCE_NOT_UTF8_REASON: &str = "static limit rust_source_not_utf8: the file is not valid UTF-8, which rustc rejects; indexed from a lossy decode on lexical fallback";

fn summarize_loaded_file(
    file: &Path,
    bytes: &[u8],
    adapter: &dyn RustSyntaxAdapter,
    fallback: &dyn RustSyntaxAdapter,
) -> Result<super::FileFacts, String> {
    let source = rust_source_text(bytes);
    if source.not_utf8 {
        let mut facts = fallback.summarize_file(file, &source.text)?;
        facts.used_lexical_fallback = true;
        return Ok(facts);
    }
    summarize_file_with_adapters(file, &source.text, adapter, fallback)
}

fn summarize_file_with_adapters(
    file: &Path,
    text: &str,
    adapter: &dyn RustSyntaxAdapter,
    fallback: &dyn RustSyntaxAdapter,
) -> Result<super::FileFacts, String> {
    match adapter.summarize_file(file, text) {
        Ok(facts) => Ok(facts),
        Err(_) => {
            let mut facts = fallback.summarize_file(file, text)?;
            // Belt-and-suspenders: the lexical adapter already sets this flag
            // internally (#2698), so this overwrite is redundant but harmless.
            facts.used_lexical_fallback = true;
            Ok(facts)
        }
    }
}

fn insert_file_summary(index: &mut RustIndex, file: PathBuf, summary: super::FileFacts) {
    index.insert_file(file, summary, true);
}

#[cfg(test)]
mod tests {
    #[test]
    fn corrupt_entries_collapse_into_one_warning_line() {
        let one = super::corrupt_entries_warning(1, "read failed");
        assert_eq!(
            one,
            "ripr: repo file fact cache entry ignored (read failed)"
        );
        let many = super::corrupt_entries_warning(723, "read failed: Not a directory");
        assert_eq!(many.lines().count(), 1);
        assert!(
            many.starts_with("ripr: 723 repo file fact cache entries ignored;"),
            "{many}"
        );
        assert!(
            many.ends_with("first: (read failed: Not a directory)"),
            "{many}"
        );
    }

    use super::*;
    use crate::analysis::syntax::{SyntaxNodeFact, TextRange};
    use std::cell::Cell;
    use std::error::Error;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn write_manifest(root: &Path) -> Result<(), Box<dyn Error>> {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        Ok(())
    }

    #[test]
    fn over_deep_rust_file_falls_back_with_typed_nesting_reason() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_nesting_budget")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        let deep = format!(
            "pub fn deep(x: i32) -> i32 {{ {}x{} }}\n",
            "(".repeat(5_000),
            ")".repeat(5_000)
        );
        fs::write(root.join("src/deep.rs"), deep)?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn shallow(x: i32) -> i32 { x }\n",
        )?;
        let files = vec![PathBuf::from("src/deep.rs"), PathBuf::from("src/lib.rs")];

        // The production index path must not abort; the deep file keeps its
        // lexical facts and the shallow file stays parser-backed.
        let index = build_index(&root, &files)?;
        let deep_facts = index
            .files()
            .get(Path::new("src/deep.rs"))
            .ok_or("deep file missing from index")?;
        assert!(deep_facts.used_lexical_fallback);
        assert!(index.functions().iter().any(|f| f.name == "deep"));
        let lib_facts = index
            .files()
            .get(Path::new("src/lib.rs"))
            .ok_or("lib file missing from index")?;
        assert!(!lib_facts.used_lexical_fallback);

        let cold = crate::analysis::rust_index::lexical_fallback_disclosure(&index)
            .ok_or("missing lexical fallback disclosure")?;
        assert!(
            cold.contains("ripr: src/deep.rs: static limit rust_nesting_budget:"),
            "{cold}"
        );
        assert!(!cold.contains("src/lib.rs"), "{cold}");
        let warm = crate::analysis::rust_index::lexical_fallback_disclosure_at(
            &root,
            &crate::analysis::rust_index::lexical_fallback_files(&index),
        );
        assert_eq!(warm.as_deref(), Some(cold.as_str()));
        fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn build_index_collects_functions_and_tests_from_workspace_files() -> Result<(), Box<dyn Error>>
    {
        let root = temp_dir("index_functions")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}
"#,
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert!(!index.functions().is_empty());
        assert!(!index.tests().is_empty());
        assert!(index.files().contains_key(&PathBuf::from("src/lib.rs")));
        Ok(())
    }

    /// F23 (#3731 review): the analyzed crate's own names include the
    /// `[lib]` target when declared, and hyphenated names are stored in
    /// their underscore crate-identifier form too — an integration test
    /// imports the lib target, not the package name. The lib target name
    /// is DISTINCT from the normalized package name (#3731 review G5: a
    /// `[lib] name` equal to the underscore package spelling would satisfy
    /// both assertions even if lib-target insertion regressed), so both
    /// spellings of each name are asserted.
    #[test]
    fn manifest_package_names_include_lib_target_and_underscore_forms() -> Result<(), Box<dyn Error>>
    {
        let root = temp_dir("index_package_names")?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn existing() {}\n")?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='foo-bar'\nversion='0.1.0'\nedition='2024'\n\n[lib]\nname='foo-core'\n",
        )?;
        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert!(
            index.package_names.contains("foo-bar"),
            "the raw package name is listed: {:?}",
            index.package_names
        );
        assert!(
            index.package_names.contains("foo_bar"),
            "the package's underscore crate identifier is listed: {:?}",
            index.package_names
        );
        assert!(
            index.package_names.contains("foo-core"),
            "the raw [lib] target name is listed: {:?}",
            index.package_names
        );
        assert!(
            index.package_names.contains("foo_core"),
            "the [lib] target's underscore crate identifier is listed: {:?}",
            index.package_names
        );
        assert_eq!(
            index.package_names.len(),
            4,
            "each name in both spellings, nothing else: {:?}",
            index.package_names
        );
        Ok(())
    }

    /// F23: without an explicit `[lib] name`, a hyphenated package's
    /// default lib target is the underscore form, so that spelling is
    /// admitted too; a plain name stays itself.
    #[test]
    fn manifest_package_names_normalize_hyphenated_defaults() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_package_names_default_lib")?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn existing() {}\n")?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='baz-qux'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert_eq!(
            index.package_names,
            ["baz-qux", "baz_qux"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            "both spellings of the hyphenated package name are listed"
        );
        Ok(())
    }

    #[test]
    fn build_index_collects_calls_returns_literals() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_facts")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn process() -> Result<i32, String> {
    let value = some_fn();
    Ok(42)
}

fn some_fn() -> i32 {
    100
}
"#,
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        let file_facts = index.files().get(&PathBuf::from("src/lib.rs"));
        assert!(file_facts.is_some());
        assert!(file_facts.is_some_and(|facts| !facts.calls.is_empty()));
        assert!(
            index
                .files()
                .get(&PathBuf::from("src/lib.rs"))
                .is_some_and(|facts| !facts.returns.is_empty())
        );
        Ok(())
    }

    #[test]
    fn build_index_collects_parser_probe_shapes_for_valid_source() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_probes")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn check(x: i32) -> bool {
    if x > 0 {
        true
    } else {
        false
    }
}
"#,
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert!(
            index
                .files()
                .get(&PathBuf::from("src/lib.rs"))
                .is_some_and(|facts| !facts.probe_shapes.is_empty())
        );
        Ok(())
    }

    #[test]
    fn build_index_returns_read_error_for_missing_file() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_missing")?;
        fs::create_dir_all(root.join("src"))?;

        let result = build_index(&root, &[PathBuf::from("src/nonexistent.rs")]);
        assert!(matches!(result, Err(ref err) if err.contains("failed to read")));
        Ok(())
    }

    #[derive(Clone, Debug, Default)]
    struct FailingSyntaxAdapter;

    impl RustSyntaxAdapter for FailingSyntaxAdapter {
        fn summarize_file(
            &self,
            _path: &Path,
            _text: &str,
        ) -> Result<super::super::FileFacts, String> {
            Err("synthetic parser failure".to_string())
        }

        fn changed_nodes(
            &self,
            _facts: crate::analysis::facts::FactSlice<'_, crate::analysis::facts::FunctionFact>,
            _ranges: &[TextRange],
        ) -> Vec<SyntaxNodeFact> {
            Vec::new()
        }
    }

    #[derive(Clone, Debug, Default)]
    struct StubSyntaxAdapter;

    impl RustSyntaxAdapter for StubSyntaxAdapter {
        fn summarize_file(
            &self,
            path: &Path,
            text: &str,
        ) -> Result<super::super::FileFacts, String> {
            Ok(super::super::FileFacts {
                path: path.to_path_buf(),
                source: text.to_string(),
                ..super::super::FileFacts::default()
            })
        }

        fn changed_nodes(
            &self,
            _facts: crate::analysis::facts::FactSlice<'_, crate::analysis::facts::FunctionFact>,
            _ranges: &[TextRange],
        ) -> Vec<SyntaxNodeFact> {
            Vec::new()
        }
    }

    #[test]
    fn build_index_falls_back_when_primary_adapter_errors() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_fallback")?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn fallback() {}\n")?;

        let index = build_index_with_adapters(
            &root,
            &[PathBuf::from("src/lib.rs")],
            &FailingSyntaxAdapter,
            &StubSyntaxAdapter,
        )?;
        assert_eq!(
            index
                .files()
                .get(&PathBuf::from("src/lib.rs"))
                .map_or("", |facts| facts.data().source.as_str()),
            "pub fn fallback() {}\n"
        );
        assert!(
            index
                .files()
                .get(&PathBuf::from("src/lib.rs"))
                .is_some_and(|facts| facts.used_lexical_fallback)
        );
        assert!(
            FailingSyntaxAdapter
                .changed_nodes(crate::analysis::facts::FactSlice::from_slice(&[]), &[])
                .is_empty()
        );
        assert!(
            StubSyntaxAdapter
                .changed_nodes(crate::analysis::facts::FactSlice::from_slice(&[]), &[])
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn build_index_from_loaded_files_reuses_warm_file_facts() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_file_fact_cache")?;
        fs::create_dir_all(root.join("src"))?;
        let file = PathBuf::from("src/lib.rs");
        let bytes = b"pub fn cached(value: i32) -> bool { value >= 10 }\n".to_vec();
        let files = [(file.clone(), bytes.clone())];

        let cold = build_index_from_loaded_files_with_cache(&root, &files)?;
        assert_eq!(cold.file_fact_cache.hits, 0);
        assert_eq!(cold.file_fact_cache.misses, 1);
        assert_eq!(cold.file_fact_cache.stores, 1);
        assert!(cold.index.files().contains_key(&file));
        assert!(!cold.index.functions().is_empty());

        let warm = build_index_from_loaded_files_with_cache(&root, &files)?;
        assert_eq!(warm.file_fact_cache.hits, 1);
        assert_eq!(warm.file_fact_cache.misses, 0);
        assert_eq!(warm.file_fact_cache.stores, 0);
        assert_eq!(warm.index.files().get(&file), cold.index.files().get(&file));
        Ok(())
    }

    #[test]
    fn build_index_from_loaded_files_misses_when_content_changes() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_file_fact_cache_invalidate")?;
        fs::create_dir_all(root.join("src"))?;
        let file = PathBuf::from("src/lib.rs");
        let first = [(file.clone(), b"pub fn cached() -> i32 { 1 }\n".to_vec())];
        let second = [(file.clone(), b"pub fn cached() -> i32 { 2 }\n".to_vec())];

        let _ = build_index_from_loaded_files_with_cache(&root, &first)?;
        let changed = build_index_from_loaded_files_with_cache(&root, &second)?;

        assert_eq!(changed.file_fact_cache.hits, 0);
        assert_eq!(changed.file_fact_cache.misses, 1);
        assert_eq!(changed.file_fact_cache.stores, 1);
        assert!(changed.file_fact_cache.invalidated_files.contains(&file));
        assert!(
            changed
                .index
                .files()
                .get(&file)
                .is_some_and(|facts| facts.source.contains("{ 2 }"))
        );
        Ok(())
    }

    struct CacheInventoryFixture {
        root: PathBuf,
        cache: RepoFileFactCache,
    }

    impl CacheInventoryFixture {
        fn new(name: &str) -> Result<Self, Box<dyn Error>> {
            let root = temp_dir(name)?;
            let fixture = Self {
                cache: RepoFileFactCache::at_dir(root.join("cache")),
                root,
            };
            fs::create_dir_all(fixture.root.join("src"))?;
            write_manifest(&fixture.root)?;
            Ok(fixture)
        }
    }

    impl Drop for CacheInventoryFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn build_loaded(
        fixture: &CacheInventoryFixture,
        files: &[(PathBuf, Vec<u8>)],
    ) -> Result<CachedRustIndex, String> {
        build_index_with_file_fact_cache(
            &fixture.root,
            files,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || fixture.cache.known_file_paths(),
        )
    }

    #[test]
    fn leading_bom_is_dropped_so_a_line_one_item_stays_parser_backed() -> Result<(), Box<dyn Error>>
    {
        let fixture = CacheInventoryFixture::new("index_bom")?;
        let file = PathBuf::from("src/lib.rs");
        let source = b"pub fn b(x: u32) -> u32 { if x >= 5 { 1 } else { 0 } }\n";
        let with_bom = [(file.clone(), [UTF8_BOM, source.as_slice()].concat())];
        fs::write(fixture.root.join(&file), &with_bom[0].1)?;

        let index = build_loaded(&fixture, &with_bom)?.index;
        let facts = index
            .files()
            .get(&file)
            .ok_or("bom file missing from index")?;
        assert!(!facts.used_lexical_fallback);
        assert!(!facts.source.starts_with('\u{feff}'));
        let owner = index
            .functions()
            .iter()
            .find(|function| function.name == "b")
            .ok_or("line-1 owner missing")?;
        assert_eq!(owner.start_line, 1);
        assert!(!facts.probe_shapes.is_empty());
        assert!(index.non_utf8_sources.is_empty());

        // The currentness authority re-reads the on-disk bytes (BOM included)
        // through the same decode, so the file's evidence stays admissible.
        let authority = index
            .workspace_authority
            .as_ref()
            .ok_or("missing workspace authority")?;
        assert!(authority.validates_target(&file, &file, &facts.source));

        // The same bytes without the mark index to the same facts.
        let fixture = CacheInventoryFixture::new("index_no_bom")?;
        let plain = build_loaded(&fixture, &[(file.clone(), source.to_vec())])?.index;
        assert_eq!(plain.functions(), index.functions());
        Ok(())
    }

    #[test]
    fn non_utf8_source_is_indexed_on_fallback_with_typed_reason() -> Result<(), Box<dyn Error>> {
        let fixture = CacheInventoryFixture::new("index_non_utf8")?;
        let latin = PathBuf::from("tests/data/latin.rs");
        let lib = PathBuf::from("src/lib.rs");
        let files = [
            (
                latin.clone(),
                b"pub fn l(x: u32) -> u32 {\n    // caf\xe9\n    if x > 5 { 1 } else { 0 }\n}\n"
                    .to_vec(),
            ),
            (lib.clone(), b"pub fn ok(x: u32) -> u32 { x }\n".to_vec()),
        ];

        // Before: the whole build failed with "failed to read ...".
        let cold = build_loaded(&fixture, &files)?;
        let latin_facts = cold.index.files().get(&latin).ok_or("latin file missing")?;
        assert!(latin_facts.used_lexical_fallback);
        assert!(cold.index.functions().iter().any(|f| f.name == "l"));
        assert!(
            !cold
                .index
                .files()
                .get(&lib)
                .ok_or("lib missing")?
                .used_lexical_fallback
        );
        assert_eq!(
            cold.index.non_utf8_sources.iter().collect::<Vec<_>>(),
            vec![&latin]
        );
        let cold_disclosure = crate::analysis::rust_index::lexical_fallback_disclosure(&cold.index)
            .ok_or("missing fallback disclosure")?;
        assert!(
            cold_disclosure.contains(&format!(
                "ripr: tests/data/latin.rs: {RUST_SOURCE_NOT_UTF8_REASON}"
            )),
            "{cold_disclosure}"
        );
        assert!(!cold_disclosure.contains("src/lib.rs"), "{cold_disclosure}");

        // A warm file-fact hit still records the file.
        let warm = build_loaded(&fixture, &files)?;
        assert_eq!(warm.file_fact_cache.hits, 2);
        assert_eq!(warm.index.non_utf8_sources, cold.index.non_utf8_sources);

        // The classified-seam warm route re-reads the file from disk.
        fs::create_dir_all(fixture.root.join("tests/data"))?;
        fs::write(fixture.root.join(&latin), &files[0].1)?;
        let reread = crate::analysis::rust_index::lexical_fallback_disclosure_at(
            &fixture.root,
            &crate::analysis::rust_index::lexical_fallback_files(&cold.index),
        );
        assert_eq!(reread.as_deref(), Some(cold_disclosure.as_str()));
        Ok(())
    }

    #[test]
    fn rust_source_text_strips_only_a_leading_bom() {
        let decoded = rust_source_text(b"\xEF\xBB\xBFfn a() {}");
        assert_eq!(decoded.text, "fn a() {}");
        assert!(!decoded.not_utf8);
        let inner = rust_source_text(b"fn a() {}\n// \xEF\xBB\xBF\n");
        assert_eq!(inner.text, "fn a() {}\n// \u{feff}\n");
        let invalid = rust_source_text(b"// \xFF\n");
        assert!(invalid.not_utf8);
        assert_eq!(invalid.text, "// \u{fffd}\n");
    }

    #[test]
    fn cache_inventory_skips_warm_build_and_preserves_index() -> Result<(), Box<dyn Error>> {
        let fixture = CacheInventoryFixture::new("cache_inventory_warm")?;
        let file = PathBuf::from("src/lib.rs");
        let first = [(file.clone(), b"pub fn value() -> i32 { 1 }\n".to_vec())];
        let current = [(file, b"pub fn value() -> i32 { 2 }\n".to_vec())];
        let inventory_reads = Cell::new(0);
        let inventory = || {
            inventory_reads.set(inventory_reads.get() + 1);
            fixture.cache.known_file_paths()
        };
        let _ = build_index_with_file_fact_cache(
            &fixture.root,
            &first,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            inventory,
        )?;
        let cold = build_index_with_file_fact_cache(
            &fixture.root,
            &current,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            inventory,
        )?;
        assert_eq!(cold.file_fact_cache.misses, 1);
        assert_eq!(cold.file_fact_cache.stores, 1);
        let entries = fs::read_dir(fixture.root.join("cache"))?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 2);

        inventory_reads.set(0);
        let warm = build_index_with_file_fact_cache(
            &fixture.root,
            &current,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            inventory,
        )?;
        assert_eq!(
            inventory_reads.get(),
            0,
            "all-hit builds must not scan history"
        );
        assert_eq!(warm.file_fact_cache.hits, 1);
        assert_eq!(warm.file_fact_cache.misses, 0);
        assert_eq!(warm.file_fact_cache.corrupt_ignored, 0);
        assert_eq!(warm.file_fact_cache.stores, 0);
        assert_eq!(warm.file_fact_cache.store_errors, 0);
        assert!(warm.file_fact_cache.invalidated_files.is_empty());
        assert_eq!(warm.index.files(), cold.index.files());
        assert_eq!(warm.index.functions(), cold.index.functions());
        assert_eq!(warm.index.tests(), cold.index.tests());
        Ok(())
    }

    #[test]
    fn literal_path_module_declarations_store_and_reload_from_file_fact_cache()
    -> Result<(), Box<dyn Error>> {
        // #4171: an internally tagged `ModulePathTarget::Literal(String)`
        // failed to encode, so every file declaring `#[path = "..."] mod`
        // missed the cache on every run and counted a store error.
        let fixture = CacheInventoryFixture::new("cache_literal_path_module")?;
        let files = [(
            PathBuf::from("src/lib.rs"),
            b"#[path = \"other.rs\"]\nmod inner;\npub fn value() -> i32 { 1 }\n".to_vec(),
        )];
        let build = || {
            build_index_with_file_fact_cache(
                &fixture.root,
                &files,
                &RaRustSyntaxAdapter,
                &LexicalRustSyntaxAdapter,
                &fixture.cache,
                || fixture.cache.known_file_paths(),
            )
        };
        let cold = build()?;
        let declarations = cold
            .index
            .files()
            .values()
            .flat_map(|facts| facts.data().module_declarations.iter())
            .map(|declaration| declaration.path_target.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            declarations,
            vec![super::super::ModulePathTarget::Literal(
                "other.rs".to_string()
            )],
            "fixture must parse the literal #[path] declaration it exists to cache"
        );
        assert_eq!(cold.file_fact_cache.store_failures, Vec::new());
        assert_eq!(cold.file_fact_cache.store_errors, 0);
        assert_eq!(cold.file_fact_cache.stores, 1);

        let warm = build()?;
        assert_eq!(warm.file_fact_cache.hits, 1);
        assert_eq!(warm.file_fact_cache.misses, 0);
        assert_eq!(warm.file_fact_cache.corrupt_ignored, 0);
        assert_eq!(warm.index.files(), cold.index.files());
        Ok(())
    }

    #[test]
    fn cache_inventory_takes_one_pre_store_snapshot_for_misses() -> Result<(), Box<dyn Error>> {
        let fixture = CacheInventoryFixture::new("cache_inventory_misses")?;
        let a = PathBuf::from("src/a.rs");
        let b = PathBuf::from("src/b.rs");
        let unchanged = PathBuf::from("src/unchanged.rs");
        let new_file = PathBuf::from("src/new.rs");
        let first = [
            (a.clone(), b"pub fn a() -> i32 { 1 }\n".to_vec()),
            (b.clone(), b"pub fn b() -> i32 { 1 }\n".to_vec()),
            (unchanged.clone(), b"pub fn unchanged() {}\n".to_vec()),
        ];
        let _ = build_index_with_file_fact_cache(
            &fixture.root,
            &first,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || fixture.cache.known_file_paths(),
        )?;
        let current = [
            (new_file.clone(), b"pub fn new_file() {}\n".to_vec()),
            (a.clone(), b"pub fn a() -> i32 { 2 }\n".to_vec()),
            (b.clone(), b"pub fn b() -> i32 { 2 }\n".to_vec()),
            (unchanged, b"pub fn unchanged() {}\n".to_vec()),
        ];
        let inventory_reads = Cell::new(0);
        let changed = build_index_with_file_fact_cache(
            &fixture.root,
            &current,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || {
                inventory_reads.set(inventory_reads.get() + 1);
                let paths = fixture.cache.known_file_paths();
                assert!(
                    !paths.contains(&new_file),
                    "the snapshot must precede stores from this build"
                );
                paths
            },
        )?;
        assert_eq!(inventory_reads.get(), 1, "misses must share one snapshot");
        assert_eq!(changed.file_fact_cache.hits, 1);
        assert_eq!(changed.file_fact_cache.misses, 3);
        assert_eq!(changed.file_fact_cache.stores, 3);
        assert_eq!(changed.file_fact_cache.corrupt_ignored, 0);
        assert_eq!(changed.file_fact_cache.store_errors, 0);
        assert_eq!(
            changed.file_fact_cache.invalidated_files,
            [a, b].into_iter().collect()
        );
        assert!(
            !changed
                .file_fact_cache
                .invalidated_files
                .contains(&new_file)
        );
        assert_eq!(changed.index.files().len(), 4);
        Ok(())
    }

    #[test]
    fn cache_inventory_skips_corrupt_only_build() -> Result<(), Box<dyn Error>> {
        let fixture = CacheInventoryFixture::new("cache_inventory_corrupt")?;
        let file = PathBuf::from("src/lib.rs");
        let bytes = b"pub fn cached() -> i32 { 1 }\n".to_vec();
        let key = RepoFileFactCacheKey::new(&file, &bytes);
        let files = [(file, bytes)];
        let cold = build_index_with_file_fact_cache(
            &fixture.root,
            &files,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || fixture.cache.known_file_paths(),
        )?;
        let entry = fs::read_dir(fixture.root.join("cache"))?
            .next()
            .ok_or("missing seeded cache entry")??
            .path();
        fs::write(&entry, b"not a valid cache envelope")?;
        let CacheLoad::CorruptIgnored { reason } = fixture.cache.load_file_facts(&key) else {
            return Err("corrupt entry must load as CorruptIgnored".into());
        };
        // The warning names the entry so it can be found and removed (#4383).
        assert!(
            reason.starts_with(&format!("{}: ", entry.display())),
            "corrupt-entry reason must name the cache file: {reason}"
        );
        let inventory_reads = Cell::new(0);
        let recovered = build_index_with_file_fact_cache(
            &fixture.root,
            &files,
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || {
                inventory_reads.set(inventory_reads.get() + 1);
                fixture.cache.known_file_paths()
            },
        )?;
        assert_eq!(inventory_reads.get(), 0);
        assert_eq!(recovered.file_fact_cache.hits, 0);
        assert_eq!(recovered.file_fact_cache.misses, 0);
        assert_eq!(recovered.file_fact_cache.corrupt_ignored, 1);
        assert_eq!(recovered.file_fact_cache.stores, 1);
        assert_eq!(recovered.file_fact_cache.store_errors, 0);
        assert!(recovered.file_fact_cache.invalidated_files.is_empty());
        assert_eq!(recovered.index.files(), cold.index.files());
        assert_eq!(recovered.index.functions(), cold.index.functions());
        assert_eq!(recovered.index.tests(), cold.index.tests());
        assert!(matches!(
            fixture.cache.load_file_facts(&key),
            CacheLoad::Hit(_)
        ));
        Ok(())
    }

    #[test]
    fn cache_inventory_skips_empty_build() -> Result<(), Box<dyn Error>> {
        let fixture = CacheInventoryFixture::new("cache_inventory_empty")?;
        let inventory_reads = Cell::new(0);
        let empty = build_index_with_file_fact_cache(
            &fixture.root,
            &[],
            &RaRustSyntaxAdapter,
            &LexicalRustSyntaxAdapter,
            &fixture.cache,
            || {
                inventory_reads.set(inventory_reads.get() + 1);
                fixture.cache.known_file_paths()
            },
        )?;
        assert_eq!(inventory_reads.get(), 0);
        assert!(empty.index.files().is_empty());
        assert_eq!(empty.file_fact_cache.hits, 0);
        assert_eq!(empty.file_fact_cache.misses, 0);
        assert_eq!(empty.file_fact_cache.stores, 0);
        assert!(empty.file_fact_cache.invalidated_files.is_empty());
        Ok(())
    }

    #[test]
    fn same_key_valid_json_payload_edit_recomputes_actual_index() -> Result<(), Box<dyn Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-integrity-index-{stamp}"));
        fs::create_dir(&root)?;
        let fixture = CacheInventoryFixture {
            cache: RepoFileFactCache::at_dir(root.join("cache")),
            root,
        };
        fs::create_dir(fixture.root.join("src"))?;
        write_manifest(&fixture.root)?;
        let files = [(
            PathBuf::from("src/lib.rs"),
            b"pub fn value() -> i32 { 1 }\n#[test]\nfn actual_test() { let _ = value(); }\n"
                .to_vec(),
        )];
        let build = || {
            build_index_with_file_fact_cache(
                &fixture.root,
                &files,
                &RaRustSyntaxAdapter,
                &LexicalRustSyntaxAdapter,
                &fixture.cache,
                || fixture.cache.known_file_paths(),
            )
        };
        let cold = build()?;
        if cold.index.functions().is_empty() || cold.index.tests().is_empty() {
            return Err("integrity fixture must produce functions and tests".into());
        }
        let entries = fs::read_dir(fixture.root.join("cache"))?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() != 1 {
            return Err("expected exactly one seeded entry".into());
        }
        let entry = entries.first().ok_or("missing seeded entry")?.path();
        let original = fs::read(&entry)?;
        let mut edited: serde_json::Value = serde_json::from_slice(&original)?;
        fs::write(&entry, serde_json::to_vec(&edited)?)?;
        let reformatted = build()?;
        if reformatted.file_fact_cache.hits != 1 || reformatted.index.tests() != cold.index.tests()
        {
            return Err("semantic-preserving JSON formatting must remain a warm hit".into());
        }
        let changed_source_key = RepoFileFactCacheKey::new(
            &PathBuf::from("src/lib.rs"),
            b"pub fn changed_source() {}\n",
        );
        if !matches!(
            fixture.cache.load_file_facts(&changed_source_key),
            CacheLoad::Miss
        ) {
            return Err("ordinary changed-source identity must remain a miss".into());
        }
        let original_keys = edited
            .as_object()
            .ok_or("envelope must be object")?
            .iter()
            .filter(|(key, _)| key.as_str() != "file_facts")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let facts = edited.get_mut("file_facts").ok_or("missing facts")?;
        let encoded_facts = serde_json::to_string(facts)?;
        let changed_facts = encoded_facts.replace("actual_test", "forged_test");
        if encoded_facts == changed_facts {
            return Err("test-name edit must change real facts".into());
        }
        *facts = serde_json::from_str(&changed_facts)?;
        for (key, value) in original_keys {
            if edited.get(&key) != Some(&value) {
                return Err("identity or digest changed".into());
            }
        }
        let altered = serde_json::to_vec(&edited)?;
        if altered == original {
            return Err("payload edit must change bytes".into());
        }
        fs::write(&entry, altered)?;
        let recovered = build()?;
        if recovered.file_fact_cache.hits != 0
            || recovered.file_fact_cache.corrupt_ignored != 1
            || recovered.file_fact_cache.stores != 1
            || recovered.file_fact_cache.misses != 0
        {
            return Err(format!(
                "same-key semantic corruption must reparse: {:?}",
                recovered.file_fact_cache
            )
            .into());
        }
        if recovered.index.files() != cold.index.files()
            || recovered.index.functions() != cold.index.functions()
            || recovered.index.tests() != cold.index.tests()
        {
            return Err("recovered complete index must equal cold source truth".into());
        }
        let warm = build()?;
        if warm.file_fact_cache.hits != 1 || warm.index.tests() != cold.index.tests() {
            return Err("corrected entry must warm-hit original evidence".into());
        }
        Ok(())
    }

    #[test]
    fn cache_inventory_observes_cancellation_before_directory_scan() -> Result<(), Box<dyn Error>> {
        use crate::analysis::cancellation::{
            AnalysisAbortKind, AnalysisCancellationToken, with_token,
        };

        let fixture = CacheInventoryFixture::new("cache_inventory_cancelled")?;
        let files = [(
            PathBuf::from("src/lib.rs"),
            b"pub fn cancelled() {}\n".to_vec(),
        )];
        let inventory_reads = Cell::new(0);
        let token = AnalysisCancellationToken::new();
        token.cancel(AnalysisAbortKind::Cancelled);
        let result = with_token(&token, || {
            build_index_with_file_fact_cache(
                &fixture.root,
                &files,
                &RaRustSyntaxAdapter,
                &LexicalRustSyntaxAdapter,
                &fixture.cache,
                || {
                    inventory_reads.set(inventory_reads.get() + 1);
                    fixture.cache.known_file_paths()
                },
            )
        });
        assert!(matches!(result, Err(ref error) if error.contains("analysis cancelled")));
        assert_eq!(inventory_reads.get(), 0);
        Ok(())
    }

    fn write_named_fn_file(root: &Path, name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let relative = PathBuf::from(format!("src/{name}.rs"));
        fs::write(
            root.join(&relative),
            format!(
                "pub fn fn_{name}() -> i32 {{ 1 }}\n\n#[test]\nfn test_{name}() {{ assert_eq!(fn_{name}(), 1); }}\n"
            ),
        )?;
        Ok(relative)
    }

    fn non_test_function_names(index: &RustIndex) -> Vec<&str> {
        index
            .functions()
            .iter()
            .filter(|function| !function.source_role.is_evidence_role())
            .map(|function| function.name.as_str())
            .collect()
    }

    #[test]
    fn build_index_preserves_input_order_across_parallel_batches() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_parallel_order")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        // Spanning two parse batches (batch size 64) proves the ordered
        // collect-then-insert drain reproduces the sequential loop's
        // per-file extension order of `index.functions()` / `index.tests()`.
        let mut files = Vec::new();
        for ordinal in 0..70 {
            files.push(write_named_fn_file(&root, &format!("f{ordinal:03}"))?);
        }

        let index = build_index(&root, &files)?;
        let expected: Vec<String> = (0..70).map(|ordinal| format!("fn_f{ordinal:03}")).collect();
        assert_eq!(non_test_function_names(&index), expected);
        let test_names: Vec<&str> = index
            .tests()
            .iter()
            .map(|test| test.name.as_str())
            .collect();
        let expected_tests: Vec<String> = (0..70)
            .map(|ordinal| format!("test_f{ordinal:03}"))
            .collect();
        assert_eq!(test_names, expected_tests);
        Ok(())
    }

    #[test]
    fn build_index_is_byte_identical_across_repeated_parallel_runs() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_parallel_determinism")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        let mut files = Vec::new();
        for ordinal in 0..70 {
            files.push(write_named_fn_file(&root, &format!("f{ordinal:03}"))?);
        }

        let baseline = build_index(&root, &files)?;
        for _ in 0..3 {
            let rerun = build_index(&root, &files)?;
            assert_eq!(baseline.tests(), rerun.tests());
            assert_eq!(baseline.functions(), rerun.functions());
            assert_eq!(baseline.files(), rerun.files());
        }
        Ok(())
    }

    #[test]
    fn build_index_reports_first_error_in_input_order() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_parallel_error_order")?;
        fs::create_dir_all(root.join("src"))?;

        let files = [
            PathBuf::from("src/z_missing.rs"),
            PathBuf::from("src/a_missing.rs"),
        ];
        let result = build_index(&root, &files);
        assert!(
            matches!(result, Err(ref err) if err.contains("z_missing.rs")),
            "first error in input order must win: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn cached_parse_workers_observe_request_cancellation_before_cache_store()
    -> Result<(), Box<dyn Error>> {
        use crate::analysis::cancellation::{
            AnalysisAbortKind, AnalysisCancellationToken, with_token,
        };
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        struct CancellingAdapter {
            token: AnalysisCancellationToken,
            missing_context: AtomicBool,
            calls: AtomicUsize,
            entered: std::sync::Barrier,
        }
        impl RustSyntaxAdapter for CancellingAdapter {
            fn summarize_file(
                &self,
                path: &Path,
                text: &str,
            ) -> Result<super::super::FileFacts, String> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.entered.wait();
                self.token.cancel(AnalysisAbortKind::Cancelled);
                if cancellation::checkpoint().err().as_deref()
                    != Some("analysis cancelled: Cancelled")
                {
                    self.missing_context.store(true, Ordering::SeqCst);
                }
                Ok(super::super::FileFacts {
                    path: path.to_path_buf(),
                    source: text.to_string(),
                    ..super::super::FileFacts::default()
                })
            }
            fn changed_nodes(
                &self,
                _: super::super::FactSlice<'_, super::super::FunctionFact>,
                _: &[TextRange],
            ) -> Vec<SyntaxNodeFact> {
                Vec::new()
            }
        }
        let fixture = CacheInventoryFixture::new("worker_cancellation")?;
        let files = [
            (PathBuf::from("src/a.rs"), b"pub fn value_a() {}".to_vec()),
            (PathBuf::from("src/b.rs"), b"pub fn value_b() {}".to_vec()),
        ];
        let token = AnalysisCancellationToken::new();
        let adapter = CancellingAdapter {
            token: token.clone(),
            missing_context: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            entered: std::sync::Barrier::new(2),
        };
        let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;
        let result = pool.install(|| {
            with_token(&token, || {
                build_index_with_file_fact_cache(
                    &fixture.root,
                    &files,
                    &adapter,
                    &StubSyntaxAdapter,
                    &fixture.cache,
                    HashSet::new,
                )
            })
        });
        assert_eq!(
            result.err().as_deref(),
            Some("analysis cancelled: Cancelled")
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
        assert!(
            !adapter.missing_context.load(Ordering::SeqCst),
            "worker must inherit the owning cancellation context"
        );
        let key = RepoFileFactCacheKey::new(&files[0].0, &files[0].1);
        assert!(
            matches!(fixture.cache.load_file_facts(&key), CacheLoad::Miss),
            "cancelled parse must not commit cache facts"
        );
        Ok(())
    }

    #[test]
    fn build_index_observes_cancelled_token_before_parsing() -> Result<(), Box<dyn Error>> {
        use crate::analysis::cancellation::{
            AnalysisAbortKind, AnalysisCancellationToken, with_token,
        };

        let root = temp_dir("index_parallel_cancellation")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        let file = write_named_fn_file(&root, "cancelled")?;

        let token = AnalysisCancellationToken::new();
        token.cancel(AnalysisAbortKind::Cancelled);
        let result = with_token(&token, || build_index(&root, &[file]));
        assert!(
            matches!(result, Err(ref err) if err.contains("analysis cancelled")),
            "cancelled token must abort the build: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn cached_build_reports_first_error_in_input_order() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_cached_error_order")?;
        fs::create_dir_all(root.join("src"))?;

        // Non-UTF-8 bytes no longer fail the build (#4582), so both adapters
        // fail here and name the file they were given.
        struct PathNamingFailure;
        impl RustSyntaxAdapter for PathNamingFailure {
            fn summarize_file(
                &self,
                path: &Path,
                _text: &str,
            ) -> Result<super::super::FileFacts, String> {
                Err(format!("synthetic failure for {}", path.display()))
            }

            fn changed_nodes(
                &self,
                _facts: crate::analysis::facts::FactSlice<'_, crate::analysis::facts::FunctionFact>,
                _ranges: &[TextRange],
            ) -> Vec<SyntaxNodeFact> {
                Vec::new()
            }
        }

        let files = [
            (PathBuf::from("src/z_failing.rs"), b"fn z() {}".to_vec()),
            (PathBuf::from("src/a_failing.rs"), b"fn a() {}".to_vec()),
        ];
        let result = build_index_from_loaded_files_with_cache_and_adapters(
            &root,
            &files,
            &PathNamingFailure,
            &PathNamingFailure,
        );
        let Err(error) = result else {
            return Err("expected failing adapters to fail the cached build".into());
        };
        assert!(
            error.contains("z_failing.rs"),
            "first error in input order must win: {error}"
        );
        Ok(())
    }

    #[test]
    fn cached_build_matches_uncached_index_for_same_sources() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_cached_parity")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        let mut files = Vec::new();
        for ordinal in 0..70 {
            files.push(write_named_fn_file(&root, &format!("f{ordinal:03}"))?);
        }

        let uncached = build_index(&root, &files)?;
        let loaded: Vec<(PathBuf, Vec<u8>)> = files
            .iter()
            .map(|file| Ok((file.clone(), fs::read(root.join(file))?)))
            .collect::<Result<_, Box<dyn Error>>>()?;
        let cached = build_index_from_loaded_files_with_cache(&root, &loaded)?;

        assert_eq!(cached.index.tests(), uncached.tests());
        assert_eq!(cached.index.functions(), uncached.functions());
        assert_eq!(cached.index.files().len(), uncached.files().len());
        Ok(())
    }

    fn source_failure_precedes_join_deadline(cached: bool) -> Result<(), Box<dyn Error>> {
        use crate::analysis::cancellation::{AnalysisCancellationToken, with_token};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use std::time::{Duration, Instant};

        struct FailingBeforeDeadline {
            expired: Arc<AtomicBool>,
        }
        impl RustSyntaxAdapter for FailingBeforeDeadline {
            fn summarize_file(
                &self,
                path: &Path,
                text: &str,
            ) -> Result<super::super::FileFacts, String> {
                if path.ends_with("first.rs") {
                    // Produce an ordinary source failure, then make the clock
                    // expire before the collecting thread can inspect it.
                    let error = "controlled first-file source failure".to_string();
                    self.expired.store(true, Ordering::SeqCst);
                    return Err(error);
                }
                Ok(super::super::FileFacts {
                    path: path.to_path_buf(),
                    source: text.to_string(),
                    ..super::super::FileFacts::default()
                })
            }
            fn changed_nodes(
                &self,
                _: super::super::FactSlice<'_, super::super::FunctionFact>,
                _: &[TextRange],
            ) -> Vec<SyntaxNodeFact> {
                Vec::new()
            }
        }
        let fixture = CacheInventoryFixture::new(if cached {
            "cached_error_deadline"
        } else {
            "uncached_error_deadline"
        })?;
        let files = [
            (PathBuf::from("src/first.rs"), b"fn first() {}".to_vec()),
            (PathBuf::from("src/second.rs"), b"fn second() {}".to_vec()),
        ];
        for (path, bytes) in &files {
            fs::write(fixture.root.join(path), bytes)?;
        }
        let expired = Arc::new(AtomicBool::new(false));
        let clock_expired = Arc::clone(&expired);
        let started = Instant::now();
        let token = AnalysisCancellationToken::with_budget(
            started,
            Duration::from_secs(1),
            Arc::new(move || {
                if clock_expired.load(Ordering::SeqCst) {
                    started + Duration::from_secs(1)
                } else {
                    started
                }
            }),
        );
        let adapter = FailingBeforeDeadline {
            expired: Arc::clone(&expired),
        };
        let paths = files
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;
        let error = pool.install(|| {
            with_token(&token, || {
                if cached {
                    build_index_with_file_fact_cache(
                        &fixture.root,
                        &files,
                        &adapter,
                        &adapter,
                        &fixture.cache,
                        HashSet::new,
                    )
                    .err()
                } else {
                    build_index_with_adapters(&fixture.root, &paths, &adapter, &adapter).err()
                }
            })
        });
        assert!(
            expired.load(Ordering::SeqCst),
            "fixture must cross the deadline"
        );
        assert_eq!(
            error.as_deref(),
            Some("controlled first-file source failure")
        );
        assert_eq!(
            with_token(&token, cancellation::checkpoint)
                .err()
                .as_deref(),
            Some("analysis cancelled: DeadlineExceeded")
        );
        for (path, bytes) in &files {
            assert!(
                matches!(
                    fixture
                        .cache
                        .load_file_facts(&RepoFileFactCacheKey::new(path, bytes)),
                    CacheLoad::Miss
                ),
                "failed batch must not store successful sibling facts"
            );
        }
        Ok(())
    }

    #[test]
    fn cached_source_failure_precedes_join_deadline() -> Result<(), Box<dyn Error>> {
        source_failure_precedes_join_deadline(true)
    }

    #[test]
    fn uncached_source_failure_precedes_join_deadline() -> Result<(), Box<dyn Error>> {
        source_failure_precedes_join_deadline(false)
    }
}
