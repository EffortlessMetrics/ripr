//! Analyzable Rust corpus for repo seam inventory.
//!
//! `ripr check` already drops generated Rust through
//! [`GeneratedRustSources`]. Seam inventory used to walk the
//! raw `discover_rust_files` set, so the same files became seams in
//! repo-exposure. This module is the single owner for the analyzable file
//! set, the skipped generated paths, and the fingerprint of that same set.

use super::language::GeneratedRustSources;
use super::seam_cache::corpus_fingerprint;
use super::workspace;
use crate::config::RiprConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Discovered Rust files after the generated-source predicate `ripr check`
/// uses, plus the skipped paths and the fingerprint of the analyzable set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnalyzableRustCorpus {
    pub(crate) analyzable: Vec<PathBuf>,
    pub(crate) skipped_generated: Vec<PathBuf>,
    pub(crate) naming_only_skips: Vec<PathBuf>,
    pub(crate) fingerprint: Option<String>,
}

/// Size of admitted workspace and owner-attribution inputs. File sizes use
/// metadata; the shared generated-source classifier may read bounded headers
/// and vendor markers, but this census never loads or indexes the corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CorpusPayloadSize {
    pub(crate) file_count: usize,
    pub(crate) total_bytes: u64,
}

/// Use the inventory's discovery/generated-file predicate, without computing
/// its cache fingerprint. Add changed owner inputs because attribution also
/// reads files outside that canonical corpus. Count each present path once;
/// absent changed paths remain available for the downstream absence warning.
pub(crate) fn analyzable_corpus_payload_size(
    root: &Path,
    config: &RiprConfig,
    owner_files: &[PathBuf],
) -> Result<CorpusPayloadSize, String> {
    let generated_sources = GeneratedRustSources::for_repo(root, &config.languages().rust);
    let mut analyzable = Vec::new();
    for path in workspace::discover_rust_files(root)? {
        super::cancellation::checkpoint()?;
        if !generated_sources.contains(&path) {
            analyzable.push(path);
        }
    }
    corpus_payload_size_for_paths(root, analyzable, owner_files)
}

fn corpus_payload_size_for_paths(
    root: &Path,
    analyzable: Vec<PathBuf>,
    owner_files: &[PathBuf],
) -> Result<CorpusPayloadSize, String> {
    let mut files = analyzable
        .into_iter()
        .map(|path| (path, true))
        .collect::<BTreeMap<_, _>>();
    for path in owner_files {
        files.entry(path.clone()).or_insert(false);
    }
    let mut total_bytes = 0u64;
    let mut file_count = 0usize;
    for (path, discovered) in &files {
        super::cancellation::checkpoint()?;
        let metadata = match std::fs::metadata(root.join(path)) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound && !discovered => {
                continue;
            }
            Err(err) => return Err(format!("stat {} failed: {err}", path.display())),
        };
        file_count += 1;
        total_bytes = total_bytes.saturating_add(metadata.len());
    }
    Ok(CorpusPayloadSize {
        file_count,
        total_bytes,
    })
}

/// Walk `root` and keep the generated-Rust files `ripr check` skips out of
/// the inventory corpus. The fingerprint covers only analyzable files so a
/// generated-file edit does not bust the seam cache.
pub(crate) fn discover_analyzable_rust_corpus(
    root: &Path,
    config: &RiprConfig,
) -> Result<AnalyzableRustCorpus, String> {
    let discovered = workspace::discover_rust_files(root)?;
    Ok(partition_analyzable_rust_corpus(root, discovered, config))
}

pub(crate) fn partition_analyzable_rust_corpus(
    root: &Path,
    discovered: Vec<PathBuf>,
    config: &RiprConfig,
) -> AnalyzableRustCorpus {
    let generated_sources = GeneratedRustSources::for_repo(root, &config.languages().rust);
    let mut analyzable = Vec::new();
    let mut skipped_generated = Vec::new();
    let mut naming_only_skips = Vec::new();
    for path in discovered {
        if generated_sources.contains(&path) {
            if generated_sources.is_convention_only_exclusion(&path) {
                naming_only_skips.push(path.clone());
            }
            skipped_generated.push(path);
        } else {
            analyzable.push(path);
        }
    }
    let fingerprint = corpus_fingerprint(root, &analyzable);
    AnalyzableRustCorpus {
        analyzable,
        skipped_generated,
        naming_only_skips,
        fingerprint,
    }
}

/// Producer-owned, character-bounded recovery shared by diff and repo output.
/// Only naming-only exclusions receive the handwritten opt-in action.
pub(crate) fn generated_rust_recovery(skipped: &[PathBuf], naming_only: &[PathBuf]) -> String {
    let (paths, naming) = if naming_only.is_empty() {
        (skipped, false)
    } else {
        (naming_only, true)
    };
    let mut listed = paths
        .iter()
        .take(3)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > 3 {
        listed.push_str(&format!(" and {} more", paths.len() - 3));
    }
    if listed.chars().count() > 160 {
        listed = format!("{}…", listed.chars().take(159).collect::<String>());
    }
    if naming {
        format!(
            "Files skipped only by naming conventions: {listed}. If these are hand-written, declare exact repository-relative paths in `[languages.rust] handwritten_files`. Explicit generated_file_patterns, generator headers and vendor markers remain excluded."
        )
    } else {
        format!(
            "Generated or vendored files excluded: {listed}. These match explicit generated_file_patterns, generator headers or vendor markers. handwritten_files cannot override those signals; correct only an inaccurate exclusion or analyze the original hand-written source."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::partition_analyzable_rust_corpus;
    use crate::config::RiprConfig;
    use std::path::{Path, PathBuf};

    fn paths(values: &[&str]) -> Vec<PathBuf> {
        values.iter().map(PathBuf::from).collect()
    }

    fn portable(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn payload_census_distinguishes_absent_owner_from_disappeared_corpus() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-census-absent-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos()
        ));
        assert!(
            !root.exists(),
            "missing-file control must start with an absent fixture"
        );
        let missing = PathBuf::from("missing.rs");
        let absent_owner = super::corpus_payload_size_for_paths(
            &root,
            Vec::new(),
            std::slice::from_ref(&missing),
        )?;
        assert_eq!(absent_owner.file_count, 0);
        assert_eq!(absent_owner.total_bytes, 0);
        // Even when the diff names it, a file already observed by discovery
        // cannot vanish silently from the admitted workspace denominator.
        let error = super::corpus_payload_size_for_paths(&root, vec![missing.clone()], &[missing])
            .err()
            .ok_or("a disappeared discovered file must fail closed")?;
        assert!(error.starts_with("stat missing.rs failed:"));
        Ok(())
    }

    #[test]
    fn payload_census_observes_cancellation_before_metadata() -> Result<(), String> {
        let token = crate::analysis::cancellation::AnalysisCancellationToken::new();
        assert!(token.cancel(crate::analysis::cancellation::AnalysisAbortKind::Cancelled));
        let error = crate::analysis::cancellation::with_token(&token, || {
            super::corpus_payload_size_for_paths(
                Path::new("unused-cancelled-root"),
                vec![PathBuf::from("missing.rs")],
                &[],
            )
        })
        .err()
        .ok_or("cancelled census must not complete")?;
        assert!(
            crate::analysis::cancellation::is_cancellation_error(&error),
            "metadata failure must not replace cancellation: {error}"
        );
        Ok(())
    }

    #[test]
    fn payload_census_matches_handwritten_and_stronger_exclusion_policy() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-census-handwritten-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos()
        ));
        for (path, text) in [
            ("src/plain.rs", "pub fn plain() -> i32 { 1 }\n"),
            ("src/schema.rs", "pub fn handwritten() -> i32 { 2 }\n"),
            (
                "src/header.rs",
                "// @generated\npub fn generated() -> i32 { 3 }\n",
            ),
            ("src/explicit.rs", "pub fn explicit() -> i32 { 4 }\n"),
            ("deps/external/lib.rs", "pub fn vendored() -> i32 { 5 }\n"),
            ("deps/external/.cargo-checksum.json", "{}"),
        ] {
            let path = root.join(path);
            let parent = path.parent().ok_or("fixture source lacks a parent")?;
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            std::fs::write(path, text).map_err(|err| err.to_string())?;
        }
        let check = |config: &RiprConfig, expected: &[&str]| -> Result<(), String> {
            let corpus = super::discover_analyzable_rust_corpus(&root, config)?;
            assert_eq!(corpus.analyzable, paths(expected));
            let census = super::analyzable_corpus_payload_size(&root, config, &[])?;
            assert_eq!(census.file_count, expected.len());
            let expected_bytes = expected.iter().try_fold(0u64, |sum, path| {
                std::fs::metadata(root.join(path))
                    .map(|metadata| sum + metadata.len())
                    .map_err(|err| err.to_string())
            })?;
            assert_eq!(census.total_bytes, expected_bytes);
            Ok(())
        };
        let explicit = "[languages.rust]\ngenerated_file_patterns = ['src/explicit.rs']\n";
        std::fs::write(root.join("ripr.toml"), explicit).map_err(|err| err.to_string())?;
        let default = crate::config::load_for_root(&root)?;
        check(&default, &["src/plain.rs"])?;
        std::fs::write(root.join("ripr.toml"), format!("{explicit}handwritten_files = ['src/schema.rs', 'src/header.rs', 'src/explicit.rs', 'deps/external/lib.rs']\n")).map_err(|err| err.to_string())?;
        let handwritten = crate::config::load_for_root(&root)?;
        check(&handwritten, &["src/plain.rs", "src/schema.rs"])?;
        // Reloading/removing an opt-in changes the census as it changes the
        // canonical corpus; stronger header, pattern and vendor signals win.
        check(&default, &["src/plain.rs"])?;
        std::fs::write(
            root.join("src/schema.rs"),
            "pub fn handwritten() -> i32 { 123456 }\n",
        )
        .map_err(|err| err.to_string())?;
        check(&handwritten, &["src/plain.rs", "src/schema.rs"])?;
        std::fs::write(root.join(".cargo-checksum.json"), "{}").map_err(|err| err.to_string())?;
        check(&handwritten, &[])?;
        // Owner attribution still consumes changed excluded paths. They are
        // counted by admission even when the canonical inventory is empty.
        let owners =
            super::analyzable_corpus_payload_size(&root, &handwritten, &paths(&["src/header.rs"]))?;
        assert_eq!(owners.file_count, 1);
        assert_eq!(
            owners.total_bytes,
            std::fs::metadata(root.join("src/header.rs"))
                .map_err(|err| err.to_string())?
                .len()
        );
        std::fs::remove_dir_all(root).map_err(|err| err.to_string())?;
        Ok(())
    }

    #[test]
    fn partition_drops_conventional_generated_rust_and_keeps_near_misses() {
        let corpus = partition_analyzable_rust_corpus(
            Path::new("/tmp/unused-root"),
            paths(&[
                "src/lib.rs",
                "src/bindings.rs",
                "src/schema.rs",
                "src/bind.rs",
                "src/ffi.rs",
                "src/gen/model.rs",
                "src/out_of_band.rs",
            ]),
            &RiprConfig::default(),
        );
        let analyzable: Vec<String> = corpus.analyzable.iter().map(|p| portable(p)).collect();
        let skipped: Vec<String> = corpus
            .skipped_generated
            .iter()
            .map(|p| portable(p))
            .collect();
        assert_eq!(
            analyzable,
            vec![
                "src/lib.rs".to_string(),
                "src/bind.rs".to_string(),
                "src/ffi.rs".to_string(),
                "src/out_of_band.rs".to_string()
            ]
        );
        assert_eq!(
            skipped,
            vec![
                "src/bindings.rs".to_string(),
                "src/schema.rs".to_string(),
                "src/gen/model.rs".to_string()
            ]
        );
    }

    #[test]
    fn partition_applies_configured_generated_file_patterns() -> Result<(), String> {
        let config = crate::config::tests_only_parse(
            "[languages.rust]\ngenerated_file_patterns = [\"src/ffi.rs\"]\n",
        )
        .map_err(|error| format!("fixture config parses: {error}"))?;
        let corpus = partition_analyzable_rust_corpus(
            Path::new("/tmp/unused-root"),
            paths(&["src/lib.rs", "src/ffi.rs"]),
            &config,
        );
        assert_eq!(
            corpus
                .analyzable
                .iter()
                .map(|p| portable(p))
                .collect::<Vec<_>>(),
            vec!["src/lib.rs".to_string()]
        );
        assert_eq!(
            corpus
                .skipped_generated
                .iter()
                .map(|p| portable(p))
                .collect::<Vec<_>>(),
            vec!["src/ffi.rs".to_string()]
        );
        Ok(())
    }
}
