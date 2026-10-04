//! Exact-path recovery must change the analyzed corpus, not just its label.
use super::GeneratedRustSources;
use crate::config::{RiprConfig, tests_only_parse};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-handwritten-{label}-{stamp}"));
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self(root))
    }
    fn write(&self, relative: &str, text: &str) -> Result<(), String> {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, text).map_err(|e| e.to_string())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn handwritten_files_exact_scope_keeps_stronger_exclusions_and_repo_parity() -> Result<(), String> {
    let fixture = Fixture::new("precedence")?;
    let paths = [
        "src/schema.rs",
        "src/generated_other.rs",
        "src/generated_header.rs",
        "vendor/dep/src/generated_lib.rs",
        "src/generated_explicit.rs",
    ];
    for path in paths {
        fixture.write(path, "pub fn value() -> i32 { 3 }\n")?;
    }
    fixture.write(
        "src/generated_header.rs",
        "// @generated\npub fn value() -> i32 { 3 }\n",
    )?;
    fixture.write("vendor/dep/.cargo-checksum.json", "{}")?;
    let config = tests_only_parse(
        r#"[languages.rust]
handwritten_files = ["src/schema.rs", "src/generated_header.rs", "vendor/dep/src/generated_lib.rs", "src/generated_explicit.rs"]
generated_file_patterns = ["src/generated_explicit.rs"]
"#,
    )?;
    let classifier = GeneratedRustSources::for_repo(&fixture.0, &config.languages.rust);
    assert!(!classifier.contains(Path::new(paths[0])));
    for path in &paths[1..] {
        assert!(
            classifier.contains(Path::new(path)),
            "must still exclude {path}"
        );
    }
    let corpus = crate::analysis::generated_rust_corpus::partition_analyzable_rust_corpus(
        &fixture.0,
        paths.iter().map(PathBuf::from).collect(),
        &config,
    );
    assert_eq!(corpus.analyzable, vec![PathBuf::from(paths[0])]);
    assert_eq!(corpus.skipped_generated.len(), 4);
    let seams =
        crate::analysis::seam_inventory::inventory_seams_at_with_config(&fixture.0, &config)?;
    assert!(
        !seams.is_empty(),
        "included source must produce actual inventory subjects"
    );
    assert!(seams.iter().all(|seam| seam.file() == Path::new(paths[0])));

    let default_config = RiprConfig::default();
    let default = GeneratedRustSources::for_repo(&fixture.0, &default_config.languages.rust);
    // A separately bound default avoids letting declarations escape into another root/config.
    assert!(default.contains(Path::new(paths[0])));
    Ok(())
}

#[test]
fn handwritten_files_corpus_identity_tracks_included_bytes_and_restores_exclusion()
-> Result<(), String> {
    let fixture = Fixture::new("fingerprint")?;
    fixture.write("src/lib.rs", "pub fn value() -> i32 { 3 }\n")?;
    fixture.write("src/schema.rs", "pub fn schema() -> i32 { 4 }\n")?;
    let paths = vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/schema.rs")];
    let default = RiprConfig::default();
    let include = tests_only_parse("[languages.rust]\nhandwritten_files = ['src/schema.rs']\n")?;
    let corpus = |config| {
        crate::analysis::generated_rust_corpus::partition_analyzable_rust_corpus(
            &fixture.0,
            paths.clone(),
            config,
        )
    };
    let before = corpus(&default);
    let included = corpus(&include);
    assert_eq!(before.analyzable.len(), 1);
    assert_eq!(included.analyzable.len(), 2);
    let content_key = |corpus: &crate::analysis::generated_rust_corpus::AnalyzableRustCorpus| -> Result<String, String> {
        let files = corpus.analyzable.iter().map(|path| {
            fs::read(fixture.0.join(path)).map(|bytes| (path.clone(), bytes)).map_err(|error| error.to_string())
        }).collect::<Result<Vec<_>, _>>()?;
        Ok(crate::analysis::seam_cache::WorkspaceState {
            workspace_root: &fixture.0, files: &files, cfg_features: None,
            // Hold config text constant to isolate the real selected source bytes.
            config_text: None, test_intent_text: None, suppressions_text: None,
        }.cache_key().filename())
    };
    let before_key = content_key(&before)?;
    let included_key = content_key(&included)?;
    assert_ne!(before_key, included_key);
    #[cfg(unix)]
    {
        assert!(before.fingerprint.is_some());
        assert!(included.fingerprint.is_some());
        assert_ne!(before.fingerprint, included.fingerprint);
    }
    #[cfg(not(unix))]
    {
        assert!(before.fingerprint.is_none());
        assert!(included.fingerprint.is_none());
    }
    fixture.write("src/schema.rs", "pub fn schema() -> i32 { 99 }\n")?;
    assert_ne!(included_key, content_key(&corpus(&include))?);
    assert_eq!(before_key, content_key(&corpus(&default))?);
    #[cfg(unix)]
    {
        assert_ne!(included.fingerprint, corpus(&include).fingerprint);
        assert_eq!(before.fingerprint, corpus(&default).fingerprint);
    }
    Ok(())
}

#[test]
fn handwritten_files_root_vendor_marker_retains_precedence() -> Result<(), String> {
    let fixture = Fixture::new("root-vendor")?;
    fixture.write("src/schema.rs", "pub fn value() -> i32 { 3 }\n")?;
    fixture.write("src/lib.rs", "pub fn ordinary() -> i32 { 4 }\n")?;
    fixture.write(".cargo-checksum.json", "{}")?;
    let default = RiprConfig::default();
    let config = tests_only_parse("[languages.rust]\nhandwritten_files = ['src/schema.rs']\n")?;
    for policy in [&default, &config] {
        assert!(
            GeneratedRustSources::for_repo(&fixture.0, &policy.languages.rust)
                .contains(Path::new("src/schema.rs")),
            "a vendor marker at the selected root must win over handwritten inclusion"
        );
        assert!(
            GeneratedRustSources::for_repo(&fixture.0, &policy.languages.rust)
                .contains(Path::new("src/lib.rs")),
            "ordinary source in a root-level vendored crate stays excluded too"
        );
        let corpus = crate::analysis::generated_rust_corpus::partition_analyzable_rust_corpus(
            &fixture.0,
            vec![PathBuf::from("src/schema.rs"), PathBuf::from("src/lib.rs")],
            policy,
        );
        assert!(corpus.analyzable.is_empty());
        assert_eq!(
            corpus.skipped_generated,
            vec![PathBuf::from("src/schema.rs"), PathBuf::from("src/lib.rs")]
        );
    }
    Ok(())
}

#[test]
fn handwritten_files_root_vendor_marker_in_diff_retains_precedence() -> Result<(), String> {
    let fixture = Fixture::new("changed-root-vendor")?;
    fixture.write("src/schema.rs", "pub fn value() -> i32 { 3 }\n")?;
    let config = tests_only_parse("[languages.rust]\nhandwritten_files = ['src/schema.rs']\n")?;
    // Changed checksum metadata remains authoritative even if its file is
    // unavailable in the worktree. Deleted-only paths are not ChangedFile
    // subjects, so do not manufacture that production-path claim here.
    let changed = crate::analysis::diff::parse_unified_diff(
        r#"diff --git a/.cargo-checksum.json b/.cargo-checksum.json
--- a/.cargo-checksum.json
+++ b/.cargo-checksum.json
@@ -1 +1 @@
-{"files":{}}
+{"files":{},"package":null}
"#,
    );
    assert_eq!(
        changed.len(),
        1,
        "the actual changed-marker diff must parse"
    );
    assert_eq!(changed[0].path, Path::new(".cargo-checksum.json"));
    assert_eq!(changed[0].removed_lines.len(), 1);
    assert_eq!(changed[0].added_lines.len(), 1);
    assert!(
        !GeneratedRustSources::for_repo(&fixture.0, &config.languages.rust)
            .contains(Path::new("src/schema.rs")),
        "marker-free exact inclusion is the positive control"
    );
    assert!(
        GeneratedRustSources::for_diff(&fixture.0, &config.languages.rust, &changed)
            .contains(Path::new("src/schema.rs")),
        "a root-level vendor marker named by the diff must govern the selected source"
    );
    Ok(())
}
