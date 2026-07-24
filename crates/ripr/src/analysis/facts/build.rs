use super::super::syntax::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter};
use super::model::RustIndex;
use crate::analysis::cancellation;
use crate::analysis::seam_cache::{
    CacheLoad, FileFactCacheStats, RepoFileFactCache, RepoFileFactCacheKey,
};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Files parsed per parallel batch. Cancellation is cooperative and
/// thread-local: `cancellation::with_token` installs the token on the
/// calling thread only, so rayon workers cannot observe `checkpoint()`.
/// Checking the token on the calling thread between batches keeps
/// cancellation effective with latency bounded by one batch of parses
/// while the per-file reads/parses still run on the pool.
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
    let known_cached_file_paths: HashSet<PathBuf> = cache.known_file_paths();
    let mut stats = FileFactCacheStats::default();

    // Phase 1 (sequential, cheap): cache lookups decide which files need a
    // fresh parse. Hit/miss/invalidation stats and the corrupt-entry stderr
    // ordering stay byte-identical to the sequential loop.
    enum Pending {
        Ready(super::FileFacts),
        Parse { key: RepoFileFactCacheKey },
    }
    let mut pending: Vec<Pending> = Vec::with_capacity(files.len());
    for (file, bytes) in files {
        cancellation::checkpoint()?;
        let key = RepoFileFactCacheKey::new(file, bytes);
        match cache.load_file_facts(&key) {
            CacheLoad::Hit(facts) => {
                stats.hits += 1;
                pending.push(Pending::Ready(facts));
            }
            CacheLoad::Miss => {
                stats.misses += 1;
                if known_cached_file_paths.contains(file) {
                    stats.invalidated_files.insert(file.clone());
                }
                pending.push(Pending::Parse { key });
            }
            CacheLoad::CorruptIgnored { reason } => {
                stats.corrupt_ignored += 1;
                eprintln!("ripr: repo file fact cache entry ignored ({reason})");
                pending.push(Pending::Parse { key });
            }
        }
    }

    // Phase 2 (parallel): parse cache misses on the rayon pool. Each parse
    // is independent; collecting an indexed parallel iterator preserves
    // input order, so results map back to their file positions.
    let mut parsed: Vec<Option<Result<super::FileFacts, String>>> = Vec::new();
    parsed.resize_with(files.len(), || None);
    let parse_positions: Vec<usize> = pending
        .iter()
        .enumerate()
        .filter_map(|(position, entry)| matches!(entry, Pending::Parse { .. }).then_some(position))
        .collect();
    for batch in parse_positions.chunks(PARSE_BATCH_FILES) {
        cancellation::checkpoint()?;
        let results: Vec<(usize, Result<super::FileFacts, String>)> = batch
            .par_iter()
            .map(|&position| {
                let (file, bytes) = &files[position];
                (
                    position,
                    summarize_loaded_file(root, file, bytes, adapter, fallback),
                )
            })
            .collect();
        for (position, result) in results {
            parsed[position] = Some(result);
        }
    }

    // Phase 3 (sequential, input order): store fresh facts, then insert.
    // Mirroring the sequential loop keeps cache stats identical, lets the
    // first error in input order win, and keeps the per-iteration
    // checkpoint ordering (error first, then checkpoint) unchanged.
    let mut index = RustIndex::default();
    for (position, entry) in pending.into_iter().enumerate() {
        let (file, _) = &files[position];
        let summary = match entry {
            Pending::Ready(facts) => facts,
            Pending::Parse { key } => {
                let facts = match parsed[position].take() {
                    Some(result) => result?,
                    None => {
                        return Err(format!(
                            "missing parse result for {}",
                            root.join(file).display()
                        ));
                    }
                };
                match cache.store_file_facts(&key, &facts) {
                    Ok(()) => stats.stores += 1,
                    Err(_) => stats.store_errors += 1,
                }
                facts
            }
        };
        insert_file_summary(&mut index, file.clone(), summary);
        cancellation::checkpoint()?;
    }
    Ok(CachedRustIndex {
        index,
        file_fact_cache: stats,
    })
}

fn build_index_with_adapters(
    root: &Path,
    files: &[PathBuf],
    adapter: &(dyn RustSyntaxAdapter + Send + Sync),
    fallback: &(dyn RustSyntaxAdapter + Send + Sync),
) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
    for batch in files.chunks(PARSE_BATCH_FILES) {
        cancellation::checkpoint()?;
        // Read + parse run on rayon workers; every file is independent.
        // `collect` on an indexed parallel iterator preserves input order,
        // so `results[i]` corresponds to `batch[i]`.
        let results: Vec<Result<(PathBuf, super::FileFacts), String>> = batch
            .par_iter()
            .map(|file| {
                let full = root.join(file);
                let text = std::fs::read_to_string(&full)
                    .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
                let summary = summarize_file_with_adapters(file, &text, adapter, fallback)?;
                Ok((file.clone(), summary))
            })
            .collect();
        // Insert sequentially in original input order: `RustIndex.tests`
        // and `RustIndex.functions` are extended per file, so this drain
        // reproduces the sequential loop byte-for-byte — the first error
        // in input order wins and the per-iteration checkpoint ordering
        // (error first, then checkpoint) is unchanged.
        for result in results {
            let (file, summary) = result?;
            insert_file_summary(&mut index, file, summary);
            cancellation::checkpoint()?;
        }
    }
    Ok(index)
}

fn summarize_loaded_file(
    root: &Path,
    file: &Path,
    bytes: &[u8],
    adapter: &dyn RustSyntaxAdapter,
    fallback: &dyn RustSyntaxAdapter,
) -> Result<super::FileFacts, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|err| format!("failed to read {}: {err}", root.join(file).display()))?;
    summarize_file_with_adapters(file, text, adapter, fallback)
}

fn summarize_file_with_adapters(
    file: &Path,
    text: &str,
    adapter: &dyn RustSyntaxAdapter,
    fallback: &dyn RustSyntaxAdapter,
) -> Result<super::FileFacts, String> {
    adapter
        .summarize_file(file, text)
        .or_else(|_| fallback.summarize_file(file, text))
}

fn insert_file_summary(index: &mut RustIndex, file: PathBuf, summary: super::FileFacts) {
    index.tests.extend(summary.tests.clone());
    index.functions.extend(summary.functions.clone());
    index.files.insert(file, summary);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::syntax::{SyntaxNodeFact, TextRange};
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
        assert!(!index.functions.is_empty());
        assert!(!index.tests.is_empty());
        assert!(index.files.contains_key(&PathBuf::from("src/lib.rs")));
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
        let file_facts = index.files.get(&PathBuf::from("src/lib.rs"));
        assert!(file_facts.is_some());
        assert!(file_facts.is_some_and(|facts| !facts.calls.is_empty()));
        assert!(
            index
                .files
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
                .files
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
            _facts: &super::super::FileFacts,
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
            _facts: &super::super::FileFacts,
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
                .files
                .get(&PathBuf::from("src/lib.rs"))
                .map_or("", |facts| facts.source.as_str()),
            "pub fn fallback() {}\n"
        );
        assert!(
            FailingSyntaxAdapter
                .changed_nodes(&super::super::FileFacts::default(), &[])
                .is_empty()
        );
        assert!(
            StubSyntaxAdapter
                .changed_nodes(&super::super::FileFacts::default(), &[])
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
        assert!(cold.index.files.contains_key(&file));
        assert!(!cold.index.functions.is_empty());

        let warm = build_index_from_loaded_files_with_cache(&root, &files)?;
        assert_eq!(warm.file_fact_cache.hits, 1);
        assert_eq!(warm.file_fact_cache.misses, 0);
        assert_eq!(warm.file_fact_cache.stores, 0);
        assert_eq!(warm.index.files.get(&file), cold.index.files.get(&file));
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
                .files
                .get(&file)
                .is_some_and(|facts| facts.source.contains("{ 2 }"))
        );
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
            .functions
            .iter()
            .filter(|function| !function.is_test)
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
        // per-file extension order of `index.functions` / `index.tests`.
        let mut files = Vec::new();
        for ordinal in 0..70 {
            files.push(write_named_fn_file(&root, &format!("f{ordinal:03}"))?);
        }

        let index = build_index(&root, &files)?;
        let expected: Vec<String> = (0..70).map(|ordinal| format!("fn_f{ordinal:03}")).collect();
        assert_eq!(non_test_function_names(&index), expected);
        let test_names: Vec<&str> = index.tests.iter().map(|test| test.name.as_str()).collect();
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
            assert_eq!(baseline.tests, rerun.tests);
            assert_eq!(baseline.functions, rerun.functions);
            assert_eq!(baseline.files, rerun.files);
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

        let files = [
            (PathBuf::from("src/z_invalid.rs"), vec![0xff, 0xfe]),
            (PathBuf::from("src/a_invalid.rs"), vec![0xff, 0xfe]),
        ];
        let result = build_index_from_loaded_files_with_cache(&root, &files);
        let Err(error) = result else {
            return Err("expected invalid UTF-8 inputs to fail the cached build".into());
        };
        assert!(
            error.contains("z_invalid.rs"),
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

        assert_eq!(cached.index.tests, uncached.tests);
        assert_eq!(cached.index.functions, uncached.functions);
        assert_eq!(cached.index.files.len(), uncached.files.len());
        Ok(())
    }
}
