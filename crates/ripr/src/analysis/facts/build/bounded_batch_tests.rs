//! Bounded cached-index batches (#5029).
//!
//! The cached builder must admit, parse, store, and insert one
//! `PARSE_BATCH_FILES` slice at a time. These controls drive the real
//! `build_index_with_file_fact_cache` over corpora spanning three batches,
//! compare every result with an independent uncached build, and observe the
//! batch lifetime both through a test-only uninserted-facts high-water mark
//! and through the cache entries already committed when each parse starts.

use super::{
    CachedRustIndex, LexicalRustSyntaxAdapter, PARSE_BATCH_FILES, RaRustSyntaxAdapter,
    RepoFileFactCache, RustSyntaxAdapter, build_index, build_index_with_file_fact_cache,
    uninserted_facts,
};
use crate::analysis::cancellation::{AnalysisAbortKind, AnalysisCancellationToken, with_token};
use crate::analysis::facts::{FactSlice, FileFacts, FunctionFact};
use crate::analysis::syntax::{SyntaxNodeFact, TextRange};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult<T> = Result<T, Box<dyn Error>>;

/// Two full batches plus a partial third.
const FILE_COUNT: usize = PARSE_BATCH_FILES * 2 + 22;
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct BatchFixture {
    root: PathBuf,
    files: Vec<PathBuf>,
    cache: RepoFileFactCache,
}

impl BatchFixture {
    fn new(name: &str) -> TestResult<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ripr-5029-{name}-{}-{stamp}-{ordinal}",
            std::process::id()
        ));
        // Never adopt a pre-existing directory and later remove its contents.
        fs::create_dir(&root)?;
        let mut fixture = Self {
            cache: RepoFileFactCache::at_dir(root.join("owned-cache")),
            root,
            files: Vec::new(),
        };
        fs::create_dir_all(fixture.root.join("src"))?;
        fs::write(
            fixture.root.join("Cargo.toml"),
            "[package]\nname='bounded_batches'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        for ordinal in 0..FILE_COUNT {
            let relative = PathBuf::from(format!("src/f{ordinal:03}.rs"));
            fixture.files.push(relative);
            fixture.write(ordinal, ordinal)?;
        }
        Ok(fixture)
    }

    fn write(&self, ordinal: usize, value: usize) -> TestResult<()> {
        fs::write(
            self.root.join(&self.files[ordinal]),
            format!(
                "pub fn fn_f{ordinal:03}() -> usize {{ {value} }}\n\n#[test]\nfn test_f{ordinal:03}() {{ assert_eq!(fn_f{ordinal:03}(), {value}); }}\n"
            ),
        )?;
        Ok(())
    }

    fn loaded(&self) -> TestResult<Vec<(PathBuf, Vec<u8>)>> {
        Ok(self
            .files
            .iter()
            .map(|path| Ok((path.clone(), fs::read(self.root.join(path))?)))
            .collect::<Result<Vec<_>, std::io::Error>>()?)
    }

    fn stored_paths(&self) -> BTreeSet<PathBuf> {
        self.cache.known_file_paths().into_iter().collect()
    }

    /// Build through the production cached builder and return the result,
    /// the uninserted-facts high-water mark, and the inventory read count.
    fn build(
        &self,
        adapter: &(dyn RustSyntaxAdapter + Send + Sync),
        fallback: &(dyn RustSyntaxAdapter + Send + Sync),
    ) -> TestResult<(Result<CachedRustIndex, String>, usize, usize)> {
        let loaded = self.loaded()?;
        let inventory_reads = Cell::new(0);
        let (result, high_water) = uninserted_facts::observe(|| {
            build_index_with_file_fact_cache(
                &self.root,
                &loaded,
                adapter,
                fallback,
                &self.cache,
                || {
                    inventory_reads.set(inventory_reads.get() + 1);
                    self.cache.known_file_paths()
                },
            )
        });
        Ok((result, high_water, inventory_reads.get()))
    }

    /// Cached result must equal an independent uncached parse of the files.
    fn assert_matches_uncached(&self, cached: &CachedRustIndex) -> TestResult<()> {
        let uncached = build_index(&self.root, &self.files)?;
        assert_eq!(cached.index.files(), uncached.files());
        assert_eq!(cached.index.functions(), uncached.functions());
        assert_eq!(cached.index.tests(), uncached.tests());
        assert_eq!(cached.index.package_names, uncached.package_names);
        assert_eq!(cached.index.non_utf8_sources, uncached.non_utf8_sources);
        let names = cached
            .index
            .tests()
            .iter()
            .map(|test| test.name.clone())
            .collect::<Vec<_>>();
        let expected = (0..FILE_COUNT)
            .map(|ordinal| format!("test_f{ordinal:03}"))
            .collect::<Vec<_>>();
        assert_eq!(names, expected, "input order must survive batching");
        Ok(())
    }
}

impl Drop for BatchFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Primary adapter that records, at each parse, how many cache entries were
/// already committed, and fails on chosen files.
struct ObservingAdapter<'a> {
    cache: &'a RepoFileFactCache,
    fail: BTreeSet<PathBuf>,
    cancel_at: Option<(PathBuf, AnalysisCancellationToken)>,
    stored_at_parse: Mutex<Vec<(PathBuf, usize)>>,
}

impl<'a> ObservingAdapter<'a> {
    fn new(cache: &'a RepoFileFactCache) -> Self {
        Self {
            cache,
            fail: BTreeSet::new(),
            cancel_at: None,
            stored_at_parse: Mutex::new(Vec::new()),
        }
    }

    fn observations(&self) -> TestResult<Vec<(PathBuf, usize)>> {
        let mut observed = self
            .stored_at_parse
            .lock()
            .map_err(|error| error.to_string())?
            .clone();
        observed.sort();
        Ok(observed)
    }
}

impl RustSyntaxAdapter for ObservingAdapter<'_> {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        if self.fail.contains(path) {
            return Err(format!("forced parse failure for {}", path.display()));
        }
        if let Some((target, token)) = &self.cancel_at
            && target == path
        {
            token.cancel(AnalysisAbortKind::Cancelled);
        }
        let stored = self.cache.known_file_paths().len();
        self.stored_at_parse
            .lock()
            .map_err(|error| error.to_string())?
            .push((path.to_path_buf(), stored));
        RaRustSyntaxAdapter.summarize_file(path, text)
    }

    fn changed_nodes(
        &self,
        functions: FactSlice<'_, FunctionFact>,
        ranges: &[TextRange],
    ) -> Vec<SyntaxNodeFact> {
        RaRustSyntaxAdapter.changed_nodes(functions, ranges)
    }
}

/// Fallback that refuses the same files, so a forced failure is terminal.
struct RefusingFallback<'a>(&'a BTreeSet<PathBuf>);

impl RustSyntaxAdapter for RefusingFallback<'_> {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        if self.0.contains(path) {
            return Err(format!("forced fallback failure for {}", path.display()));
        }
        LexicalRustSyntaxAdapter.summarize_file(path, text)
    }

    fn changed_nodes(
        &self,
        functions: FactSlice<'_, FunctionFact>,
        ranges: &[TextRange],
    ) -> Vec<SyntaxNodeFact> {
        LexicalRustSyntaxAdapter.changed_nodes(functions, ranges)
    }
}

#[test]
fn cold_warm_and_edited_builds_hold_at_most_one_batch_outside_the_index() -> TestResult<()> {
    let fixture = BatchFixture::new("parity")?;
    let adapter = ObservingAdapter::new(&fixture.cache);

    let (cold, high_water, inventory_reads) = fixture.build(&adapter, &LexicalRustSyntaxAdapter)?;
    let cold = cold?;
    assert_eq!(cold.file_fact_cache.misses, FILE_COUNT);
    assert_eq!(cold.file_fact_cache.stores, FILE_COUNT);
    assert_eq!(cold.file_fact_cache.hits, 0);
    assert!(cold.file_fact_cache.invalidated_files.is_empty());
    assert_eq!(inventory_reads, 1);
    assert_eq!(
        high_water, PARSE_BATCH_FILES,
        "cold parsed misses must be inserted batch by batch, not staged"
    );
    fixture.assert_matches_uncached(&cold)?;

    let (warm, high_water, inventory_reads) = fixture.build(&adapter, &LexicalRustSyntaxAdapter)?;
    let warm = warm?;
    assert_eq!(warm.file_fact_cache.hits, FILE_COUNT);
    assert_eq!(warm.file_fact_cache.misses, 0);
    assert_eq!(warm.file_fact_cache.stores, 0);
    assert_eq!(inventory_reads, 0, "all-hit builds must not read inventory");
    assert_eq!(
        high_water, PARSE_BATCH_FILES,
        "cache-hit facts must not be retained across later batches"
    );
    fixture.assert_matches_uncached(&warm)?;
    assert_eq!(warm.index.files(), cold.index.files());

    // One same-size edit in each batch: three invalidated misses, one
    // inventory read, and still one batch of uninserted facts at most.
    let edited = [5, PARSE_BATCH_FILES + 6, PARSE_BATCH_FILES * 2 + 7];
    for ordinal in edited {
        fixture.write(ordinal, ordinal + 1000)?;
    }
    let (mixed, high_water, inventory_reads) =
        fixture.build(&adapter, &LexicalRustSyntaxAdapter)?;
    let mixed = mixed?;
    assert_eq!(mixed.file_fact_cache.hits, FILE_COUNT - edited.len());
    assert_eq!(mixed.file_fact_cache.misses, edited.len());
    assert_eq!(mixed.file_fact_cache.stores, edited.len());
    assert_eq!(
        mixed.file_fact_cache.invalidated_files,
        edited
            .iter()
            .map(|&ordinal| fixture.files[ordinal].clone())
            .collect()
    );
    assert_eq!(inventory_reads, 1);
    assert_eq!(high_water, PARSE_BATCH_FILES);
    fixture.assert_matches_uncached(&mixed)?;
    assert_ne!(mixed.index.files(), cold.index.files());
    Ok(())
}

#[test]
fn each_batch_is_stored_before_the_next_batch_parses() -> TestResult<()> {
    let fixture = BatchFixture::new("store_order")?;
    let adapter = ObservingAdapter::new(&fixture.cache);
    let (cold, _, _) = fixture.build(&adapter, &LexicalRustSyntaxAdapter)?;
    fixture.assert_matches_uncached(&cold?)?;
    let observed = adapter.observations()?;
    assert_eq!(observed.len(), FILE_COUNT, "every cold file must parse");
    for (ordinal, (path, stored)) in observed.iter().enumerate() {
        assert_eq!(path, &fixture.files[ordinal]);
        assert_eq!(
            *stored,
            (ordinal / PARSE_BATCH_FILES) * PARSE_BATCH_FILES,
            "{} parsed with the wrong number of earlier batches committed",
            path.display()
        );
    }
    Ok(())
}

#[test]
fn late_batch_parse_failure_returns_first_input_order_error_and_no_index() -> TestResult<()> {
    let fixture = BatchFixture::new("late_failure")?;
    let failing = [PARSE_BATCH_FILES * 2 + 9, PARSE_BATCH_FILES * 2 + 3];
    let fail = failing
        .iter()
        .map(|&ordinal| fixture.files[ordinal].clone())
        .collect::<BTreeSet<_>>();
    let mut adapter = ObservingAdapter::new(&fixture.cache);
    adapter.fail = fail.clone();
    let (result, _, _) = fixture.build(&adapter, &RefusingFallback(&fail))?;
    let Err(error) = result else {
        return Err("a failed late batch must not return an index".into());
    };
    assert_eq!(
        error,
        format!(
            "forced fallback failure for {}",
            fixture.files[PARSE_BATCH_FILES * 2 + 3].display()
        ),
        "the first failure in input order wins"
    );
    // Completed earlier batches keep their ordinary per-file entries; nothing
    // from the failing batch is committed, including files before the error.
    let expected = fixture.files[..PARSE_BATCH_FILES * 2]
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(fixture.stored_paths(), expected);
    Ok(())
}

#[test]
fn cancellation_in_a_later_batch_returns_no_index_and_commits_no_part_of_it() -> TestResult<()> {
    let fixture = BatchFixture::new("late_cancel")?;
    let token = AnalysisCancellationToken::new();
    let mut adapter = ObservingAdapter::new(&fixture.cache);
    adapter.cancel_at = Some((fixture.files[PARSE_BATCH_FILES + 10].clone(), token.clone()));
    let (result, _, _) = with_token(&token, || {
        fixture.build(&adapter, &LexicalRustSyntaxAdapter)
    })?;
    assert_eq!(
        result.err().as_deref(),
        Some("analysis cancelled: Cancelled")
    );
    let expected = fixture.files[..PARSE_BATCH_FILES]
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(fixture.stored_paths(), expected);
    Ok(())
}
