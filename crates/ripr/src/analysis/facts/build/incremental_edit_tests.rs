//! Small-edit replay below the semantic-cache boundary (#3796).
//!
//! Exercise the real file-fact builder against physical source edits and an
//! independent uncached build. Count actual parser entries, not just cache
//! telemetry. Derived findings are recomputed on both sides: these tests do
//! not claim that semantic classification itself is incrementally cached.

use super::{
    CachedRustIndex, LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RepoFileFactCache, RustIndex,
    RustSyntaxAdapter, build_index, build_index_with_file_fact_cache,
};
use crate::analysis::classifier::classify_probe;
use crate::analysis::facts::FileFacts;
use crate::analysis::probes::probes_for_repo_file;
use crate::analysis::syntax::{SyntaxNodeFact, TextRange};
use crate::domain::Finding;
use std::cell::Cell;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FILE_COUNT: usize = 70;
const OWNER: &str = "src/lib.rs";
const RELATED: &str = "tests/boundary.rs";
const UNRELATED: &str = "tests/elsewhere.rs";
const UNRELATED_SOURCE: &str = "#[test]\nfn spare_case() { assert_eq!(37, 37); }\n";
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

type TestResult<T> = Result<T, Box<dyn Error>>;

#[derive(Default)]
struct CountingSyntaxAdapter {
    parses: AtomicUsize,
}

impl RustSyntaxAdapter for CountingSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        self.parses.fetch_add(1, Ordering::Relaxed);
        RaRustSyntaxAdapter.summarize_file(path, text)
    }

    fn changed_nodes(
        &self,
        facts: crate::analysis::facts::FactSlice<'_, crate::analysis::facts::FunctionFact>,
        ranges: &[TextRange],
    ) -> Vec<SyntaxNodeFact> {
        RaRustSyntaxAdapter.changed_nodes(facts, ranges)
    }
}

struct EditFixture {
    root: PathBuf,
    files: Vec<PathBuf>,
    cache: RepoFileFactCache,
    adapter: CountingSyntaxAdapter,
    inventory_reads: Cell<usize>,
}

impl EditFixture {
    fn new() -> TestResult<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ripr-small-edit-{}-{stamp}-{ordinal}",
            std::process::id()
        ));
        // Never adopt a pre-existing directory and later remove its contents.
        fs::create_dir(&root)?;
        let mut fixture = Self {
            cache: RepoFileFactCache::at_dir(root.join("owned-cache")),
            root,
            files: Vec::new(),
            adapter: CountingSyntaxAdapter::default(),
            inventory_reads: Cell::new(0),
        };
        fixture.write_manifest("cache_edit_fixture")?;
        let mut owner = "pub fn threshold(value: i32) -> bool {\n    value >= 10\n}\n".to_string();
        for ordinal in 0..FILE_COUNT - 3 {
            owner.push_str(&format!("mod filler{ordinal:03};\n"));
        }
        fixture.add(OWNER, &owner)?;
        fixture.add(RELATED, &test_source("boundary_case", 10))?;
        fixture.add(UNRELATED, UNRELATED_SOURCE)?;
        for ordinal in 0..FILE_COUNT - 3 {
            fixture.add(
                &format!("src/filler{ordinal:03}.rs"),
                &format!("pub fn filler{ordinal:03}() -> usize {{ {ordinal} }}\n"),
            )?;
        }
        Ok(fixture)
    }

    fn write_manifest(&self, name: &str) -> TestResult<()> {
        fs::write(
            self.root.join("Cargo.toml"),
            format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n"),
        )?;
        Ok(())
    }

    fn add(&mut self, path: &str, source: &str) -> TestResult<()> {
        let relative = PathBuf::from(path);
        assert!(!self.files.contains(&relative), "duplicate fixture path");
        if let Some(parent) = self.root.join(path).parent() {
            fs::create_dir_all(parent)?;
        }
        self.write(path, source)?;
        self.files.push(relative);
        Ok(())
    }

    fn write(&self, path: &str, source: &str) -> TestResult<()> {
        fs::write(self.root.join(path), source)?;
        Ok(())
    }

    fn remove(&mut self, path: &str) -> TestResult<()> {
        fs::remove_file(self.root.join(path))?;
        self.files.retain(|candidate| candidate != Path::new(path));
        Ok(())
    }

    fn replay(
        &self,
        expected_hits: usize,
        expected_parses: usize,
        invalidated: &[&str],
    ) -> TestResult<CachedRustIndex> {
        let loaded = self
            .files
            .iter()
            .map(|path| Ok((path.clone(), fs::read(self.root.join(path))?)))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        self.adapter.parses.store(0, Ordering::Relaxed);
        self.inventory_reads.set(0);
        let cached = build_index_with_file_fact_cache(
            &self.root,
            &loaded,
            &self.adapter,
            &LexicalRustSyntaxAdapter,
            &self.cache,
            || {
                self.inventory_reads.set(self.inventory_reads.get() + 1);
                self.cache.known_file_paths()
            },
        )?;
        assert_eq!(cached.file_fact_cache.hits, expected_hits);
        assert_eq!(cached.file_fact_cache.misses, expected_parses);
        assert_eq!(cached.file_fact_cache.stores, expected_parses);
        assert_eq!(cached.file_fact_cache.corrupt_ignored, 0);
        assert_eq!(cached.file_fact_cache.store_errors, 0);
        assert_eq!(
            self.adapter.parses.load(Ordering::Relaxed),
            expected_parses,
            "unchanged files must not enter the syntax adapter"
        );
        assert_eq!(
            self.inventory_reads.get(),
            usize::from(expected_parses != 0),
            "real misses still need one historical inventory; this is not semantic reuse"
        );
        assert_eq!(
            cached.file_fact_cache.invalidated_files,
            invalidated
                .iter()
                .map(|path| PathBuf::from(*path))
                .collect()
        );
        assert_eq!(cached.index.files().len(), self.files.len());
        assert!(
            cached
                .index
                .files()
                .values()
                .all(|facts| !facts.used_lexical_fallback),
            "the parser-count controls must exercise the primary syntax adapter"
        );

        // This reads physical source files and parses all of them without any
        // cache. Do not use a second warm build as the correctness oracle.
        let uncached = build_index(&self.root, &self.files)?;
        assert_eq!(cached.index.files(), uncached.files());
        assert_eq!(cached.index.functions(), uncached.functions());
        assert_eq!(cached.index.tests(), uncached.tests());
        assert_eq!(cached.index.package_names, uncached.package_names);
        assert_eq!(
            serde_json::to_value(owner_findings(&cached.index))?,
            serde_json::to_value(owner_findings(&uncached))?,
            "file-fact reuse must preserve the derived finding payload"
        );
        Ok(cached)
    }
}

impl Drop for EditFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn test_source(name: &str, value: usize) -> String {
    format!(
        "use cache_edit_fixture::threshold;\n#[test]\nfn {name}() {{ assert!(threshold({value})); }}\n"
    )
}

fn owner_findings(index: &RustIndex) -> Vec<Finding> {
    // Relative locations match the relative facts used by the classifier;
    // probe extraction reads the supplied index, not the current directory.
    let probes = probes_for_repo_file(Path::new(""), Path::new(OWNER), index);
    assert!(
        !probes.is_empty(),
        "the fixture must emit actual owner probes"
    );
    probes
        .iter()
        .map(|probe| classify_probe(probe, index, true, None))
        .collect()
}

fn relates(index: &RustIndex, name: &str) -> bool {
    owner_findings(index)
        .iter()
        .any(|finding| finding.related_tests.iter().any(|test| test.name == name))
}

#[test]
fn production_edit_reuses_other_files_and_revert_reuses_old_generation() -> TestResult<()> {
    let fixture = EditFixture::new()?;
    let first = fixture.replay(0, FILE_COUNT, &[])?;
    fixture.replay(FILE_COUNT, 0, &[])?;
    let original = fs::read_to_string(fixture.root.join(OWNER))?;
    let edited = original.replace("value >= 10", "value >= 11");
    assert_ne!(original, edited);
    assert_eq!(original.len(), edited.len(), "same-size content edit");
    fixture.write(OWNER, &edited)?;
    let changed = fixture.replay(FILE_COUNT - 1, 1, &[OWNER])?;
    assert_ne!(first.index.files(), changed.index.files());
    assert_ne!(
        serde_json::to_value(owner_findings(&first.index))?,
        serde_json::to_value(owner_findings(&changed.index))?
    );
    fixture.write(OWNER, &original)?;
    let restored = fixture.replay(FILE_COUNT, 0, &[])?;
    assert_eq!(first.index.files(), restored.index.files());
    assert_eq!(
        serde_json::to_value(owner_findings(&first.index))?,
        serde_json::to_value(owner_findings(&restored.index))?
    );
    Ok(())
}

#[test]
fn related_test_edit_refreshes_assertions_without_reparsing_owner() -> TestResult<()> {
    let fixture = EditFixture::new()?;
    let first = fixture.replay(0, FILE_COUNT, &[])?;
    assert!(relates(&first.index, "boundary_case"));
    fixture.write(RELATED, &test_source("boundary_case", 9))?;
    let changed = fixture.replay(FILE_COUNT - 1, 1, &[RELATED])?;
    assert_eq!(
        first.index.files().get(Path::new(OWNER)),
        changed.index.files().get(Path::new(OWNER))
    );
    assert_ne!(first.index.tests(), changed.index.tests());
    assert!(relates(&changed.index, "boundary_case"));
    let test = changed
        .index
        .tests()
        .iter()
        .find(|test| test.name == "boundary_case")
        .ok_or("updated test fact missing")?;
    assert!(test.body.contains("threshold(9)"));
    assert!(!test.body.contains("threshold(10)"));
    Ok(())
}

#[test]
fn previously_unrelated_test_enters_and_leaves_the_relation_set() -> TestResult<()> {
    let fixture = EditFixture::new()?;
    let first = fixture.replay(0, FILE_COUNT, &[])?;
    assert!(!relates(&first.index, "spare_case"));
    fixture.write(UNRELATED, &test_source("spare_case", 10))?;
    let now_related = fixture.replay(FILE_COUNT - 1, 1, &[UNRELATED])?;
    assert!(relates(&now_related.index, "spare_case"));
    assert_eq!(
        first.index.files().get(Path::new(OWNER)),
        now_related.index.files().get(Path::new(OWNER))
    );
    fixture.write(UNRELATED, UNRELATED_SOURCE)?;
    let no_longer_related = fixture.replay(FILE_COUNT, 0, &[])?;
    assert!(!relates(&no_longer_related.index, "spare_case"));
    Ok(())
}

#[test]
fn added_and_deleted_test_refreshes_relations_without_false_invalidation() -> TestResult<()> {
    let mut fixture = EditFixture::new()?;
    fixture.replay(0, FILE_COUNT, &[])?;
    fixture.add("tests/later.rs", &test_source("later_case", 10))?;
    let added = fixture.replay(FILE_COUNT, 1, &[])?;
    assert!(relates(&added.index, "later_case"));
    fixture.remove("tests/later.rs")?;
    let deleted = fixture.replay(FILE_COUNT, 0, &[])?;
    assert!(!relates(&deleted.index, "later_case"));
    assert!(
        !deleted
            .index
            .files()
            .contains_key(Path::new("tests/later.rs"))
    );
    Ok(())
}

#[test]
fn manifest_edit_refreshes_package_authority_without_source_reparse() -> TestResult<()> {
    let fixture = EditFixture::new()?;
    let first = fixture.replay(0, FILE_COUNT, &[])?;
    assert!(first.index.package_names.contains("cache_edit_fixture"));
    fixture.write_manifest("renamed_fixture")?;
    let changed = fixture.replay(FILE_COUNT, 0, &[])?;
    assert!(changed.index.package_names.contains("renamed_fixture"));
    assert!(!changed.index.package_names.contains("cache_edit_fixture"));
    assert_eq!(first.index.files(), changed.index.files());
    Ok(())
}

#[test]
fn observer_predicate_facts_recompute_after_build_miss_and_match_warm() -> TestResult<()> {
    use crate::analysis::seam_cache::{CacheLoad, RepoFileFactCacheKey};
    use crate::domain::{OracleKind, OracleStrength};

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ripr-4916-cache-{}-{stamp}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir(&root)?;
    let result = (|| -> TestResult<()> {
        let path = PathBuf::from("src/lib.rs");
        let text =
            "#[test]\nfn published_count() {\n    assert!(plan.published_payload_bytes > 0);\n}\n";
        let bytes = text.as_bytes().to_vec();
        let cache = RepoFileFactCache::at_dir(root.join("owned-cache"));
        let key = RepoFileFactCacheKey::new(&path, &bytes);
        let prior_key = key.with_test_analyzer_identity(format!(
            "{}+4916-prior-classifier",
            crate::build_identity::cache_identity()
        ));
        let mut prior_facts = RaRustSyntaxAdapter.summarize_file(&path, text)?;
        let prior_test = prior_facts
            .tests
            .first_mut()
            .ok_or("seed requires one real parsed test")?;
        if prior_test.assertions.len() != 1 {
            return Err("seed requires one real parsed assertion".into());
        }
        let prior_assertion = prior_test
            .assertions
            .first_mut()
            .ok_or("seed assertion absent")?;
        // Retained old producer observation: published-count comparison was
        // MockExpectation/Medium. Only the cache fixture models that old kind.
        prior_assertion.kind = OracleKind::MockExpectation;
        prior_assertion.strength = OracleStrength::Medium;
        cache
            .store_file_facts(&prior_key, &prior_facts)
            .map_err(|error| format!("store prior-build file facts: {error:?}"))?;
        if !matches!(cache.load_file_facts(&prior_key), CacheLoad::Hit(ref facts) if facts == &prior_facts)
        {
            return Err("prior-build seed did not read its exact real facts".into());
        }
        if !matches!(cache.load_file_facts(&key), CacheLoad::Miss) {
            return Err("new build reused prior classifier facts".into());
        }
        let files = [(path.clone(), bytes)];
        let adapter = CountingSyntaxAdapter::default();
        let cold = build_index_with_file_fact_cache(
            &root,
            &files,
            &adapter,
            &LexicalRustSyntaxAdapter,
            &cache,
            || cache.known_file_paths(),
        )?;
        if cold.file_fact_cache.misses != 1
            || cold.file_fact_cache.hits != 0
            || cold.file_fact_cache.stores != 1
            || adapter.parses.load(Ordering::Relaxed) != 1
        {
            return Err(format!(
                "cold recomputation did not reach real parser: {:?}",
                cold.file_fact_cache
            )
            .into());
        }
        let cold_facts = cold
            .index
            .files()
            .get(&path)
            .ok_or("cold file facts absent")?;
        if cold_facts.used_lexical_fallback || cold_facts.source != text {
            return Err("cold facts lost parsed source identity".into());
        }
        let assertions = &cold_facts
            .tests
            .first()
            .ok_or("cold real test absent")?
            .assertions;
        if assertions.len() != 1 {
            return Err("cold real assertion absent or duplicated".into());
        }
        let assertion = assertions.first().ok_or("cold assertion absent")?;
        if assertion.kind != OracleKind::RelationalCheck
            || assertion.strength != OracleStrength::Weak
            || assertion.line != 3
            || !assertion
                .observed_tokens
                .iter()
                .any(|token| token == "published_payload_bytes")
        {
            return Err(
                format!("cold producer did not recompute weak predicate: {assertion:?}").into(),
            );
        }
        let warm = build_index_with_file_fact_cache(
            &root,
            &files,
            &adapter,
            &LexicalRustSyntaxAdapter,
            &cache,
            || cache.known_file_paths(),
        )?;
        if warm.file_fact_cache.hits != 1
            || warm.file_fact_cache.misses != 0
            || adapter.parses.load(Ordering::Relaxed) != 1
        {
            return Err(format!(
                "warm reuse was not one real hit without parsing: {:?}",
                warm.file_fact_cache
            )
            .into());
        }
        if warm.index.files().get(&path) != Some(cold_facts) {
            return Err("warm whole facts differ from cold recomputation".into());
        }
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&root);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}
