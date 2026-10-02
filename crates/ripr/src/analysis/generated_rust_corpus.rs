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
