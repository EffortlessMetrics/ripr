//! Hermetic fixture workspace for the gate evaluation matrix.
//!
//! The gate tests resolve corpus inputs (`fixtures/...`) against an
//! explicit root. Rooting those evaluations at the live repository couples
//! outputs to ambient generated state: the default-path causal loads read
//! `<root>/target/ripr/pr/canonical-delta.json`, so a producer artifact
//! left in the live tree silently changes decisions and receipt fields
//! (#3742 class (b)). This helper materializes a private copy containing
//! exactly the committed corpus trees the matrix reads, preserving layout
//! so relative input strings stay byte-identical. Expected-content reads
//! and documented re-bless writes intentionally stay repo-rooted (see
//! `assert_repo_fixture`); only evaluation roots move here.

use std::path::{Path, PathBuf};

/// Committed corpus trees the gate matrix reads, relative to the workspace
/// root. Keep this list closed: every `fixtures/...` input the matrix
/// resolves must be covered, and nothing else is copied.
const GATE_CORPUS_TREES: &[&str] = &[
    "fixtures/boundary_gap/expected",
    "fixtures/gate_baseline_fallback_disclosure",
];

/// Process-shared hermetic corpus root. Materialized once per test process
/// and read-only thereafter: no test may write into it (planting tests use
/// their own temp roots). Sharing keeps one ~2M footprint with no per-test
/// residue; content is fixed at init from committed bytes.
static HERMETIC_ROOT: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();

/// Returns the hermetic gate corpus root, materializing it on first use.
/// The copy carries committed bytes only — never `target/` generated
/// state — so evaluations rooted here cannot observe ambient artifacts.
/// Materialization failures are memoized and reported on every call.
pub(crate) fn hermetic_gate_fixture_root() -> Result<PathBuf, String> {
    HERMETIC_ROOT.get_or_init(materialize_hermetic_root).clone()
}

fn materialize_hermetic_root() -> Result<PathBuf, String> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "hermetic fixture root needs a workspace root".to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system time before unix epoch: {err}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-gate-corpus-{stamp}"));
    for tree in GATE_CORPUS_TREES {
        copy_tree(&workspace_root.join(tree), &root.join(tree))?;
    }
    Ok(root)
}

fn copy_tree(source: &Path, dest: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(source)
        .map_err(|err| format!("read {} failed: {err}", source.display()))?;
    std::fs::create_dir_all(dest)
        .map_err(|err| format!("create {} failed: {err}", dest.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read entry failed: {err}"))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|err| format!("copy {} failed: {err}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermetic_copy_carries_corpus_without_generated_state() -> Result<(), String> {
        let root = hermetic_gate_fixture_root()?;
        for tree in GATE_CORPUS_TREES {
            assert!(
                root.join(tree).is_dir(),
                "corpus tree missing from hermetic copy: {tree}"
            );
        }
        assert!(
            root.join("fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json")
                .is_file(),
            "matrix guidance input missing from hermetic copy"
        );
        assert!(
            !root.join("target").exists(),
            "hermetic copy must not carry generated state"
        );
        // Shared process copy: intentionally not removed (one ~2M dir per
        // suite run, no per-test residue).
        Ok(())
    }
}
