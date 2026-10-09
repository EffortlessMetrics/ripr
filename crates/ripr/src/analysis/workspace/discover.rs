use crate::analysis::cancellation;
use crate::analysis::committed_source::frozen::fs as frozen_fs;
use crate::analysis::language::{
    LanguageAdapter, LanguageId, RustAdapter, route, unanalyzed_source_language,
};
use crate::analysis_outcome::{
    AnalysisLimitation, AnalysisLimitationKind, AnalysisRecovery, AnalysisRecoveryKind,
    AnalysisStage,
};
use std::path::{Path, PathBuf};

const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    ".ripr",
    ".direnv",
    "fixtures",
    "node_modules",
];

/// Changed source files that are not regular files in the working tree.
///
/// Sparse checkout, local deletes and symlinks drop these from disk walks, so the
/// owner never enters the index. Callers must disclose that as a named
/// limitation instead of classifying the missing owner as
/// `no_static_path` (#4586). Non-source paths (docs, manifests) are
/// ignored: they are not analysis subjects.
pub(crate) fn changed_source_files_absent_from_worktree<'a>(
    root: &Path,
    changed_paths: impl IntoIterator<Item = &'a Path>,
) -> Vec<PathBuf> {
    let mut absent = Vec::new();
    for path in changed_paths {
        if route(path).is_none() {
            continue;
        }
        if worktree_contains_regular_source_file(root, path) {
            continue;
        }
        absent.push(PathBuf::from(super::classify::normalize_path(path)));
    }
    absent.sort();
    absent.dedup();
    absent
}

/// Shared admission disclosure for a changed source path with no regular worktree file.
pub(crate) fn limitations_for_absent_changed_files(
    paths: &[PathBuf],
) -> Result<Vec<AnalysisLimitation>, String> {
    paths
        .iter()
        .map(|path| {
            let display = super::classify::normalize_path(path);
            AnalysisLimitation::new(
                AnalysisLimitationKind::ChangedFileAbsentFromWorktree,
                AnalysisStage::LanguageAdapter,
                AnalysisRecovery::new(
                    AnalysisRecoveryKind::Retry,
                    "Check out the missing file, disable sparse checkout for it, or restore a regular source file under real directories below the selected root, then re-run the analysis.",
                )?,
            )
            .with_path(&display)?
            .with_affected_items(1)?
            .with_detail(
                "changed file is absent from the working tree or is not a discoverable regular source file (sparse checkout, local delete, or symlink); probes for this file were withheld",
            )
        })
        .collect()
}

fn worktree_contains_regular_source_file(root: &Path, relative: &Path) -> bool {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }
    if is_regular_source_below_root(root, relative) {
        return true;
    }
    // Git diffs keep the repository-relative path. `--root` is often a
    // crate subdirectory, so `root.join(diff_path)` misses a file that
    // is on disk as the matching suffix (`src/lib.rs` under
    // `examples/sample` while the diff names
    // `crates/ripr/examples/sample/src/lib.rs`). Accept a suffix only
    // when the stripped prefix is a trailing component sequence of
    // `root`; a sibling crate's `src/lib.rs` must not count.
    let normalized = super::classify::normalize_path(relative);
    let root_norm = super::classify::normalize_path(root);
    for (prefix, suffix) in path_prefix_suffix_pairs(&normalized) {
        // A one-component prefix (`src`) matching only the root
        // basename would map `src/lib.rs` onto a root-level `lib.rs`
        // when `--root` itself is named `src`. Require a multi-segment
        // crate/repo prefix, as in `crates/ripr/examples/sample`.
        if !prefix.contains('/') || !root_ends_with_prefix(&root_norm, prefix) {
            continue;
        }
        if is_regular_source_below_root(root, Path::new(suffix)) {
            return true;
        }
    }
    false
}

fn is_regular_source_below_root(root: &Path, relative: &Path) -> bool {
    // The selected root may itself be an alias. Below that boundary, match
    // discovery's no-follow semantics for both directory and source entries.
    // Inspecting only the leaf would still follow a symlinked parent directory.
    let mut components = relative
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .peekable();
    let mut path = root.to_path_buf();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(name) = component else {
            return false;
        };
        path.push(name);
        let Ok(metadata) = frozen_fs::symlink_metadata(&path) else {
            return false;
        };
        if components.peek().is_none() {
            return metadata.is_file();
        }
        if !metadata.is_dir() {
            return false;
        }
    }
    false
}

fn path_prefix_suffix_pairs(normalized: &str) -> impl Iterator<Item = (&str, &str)> {
    let bytes = normalized.as_bytes();
    (0..normalized.len()).filter_map(move |idx| {
        if idx == 0 || bytes.get(idx) != Some(&b'/') {
            return None;
        }
        let prefix = normalized.get(..idx)?;
        let suffix = normalized.get(idx + 1..)?;
        if prefix.is_empty() || suffix.is_empty() {
            return None;
        }
        Some((prefix, suffix))
    })
}

fn root_ends_with_prefix(root_norm: &str, prefix: &str) -> bool {
    root_norm == prefix || root_norm.ends_with(&format!("/{prefix}"))
}

pub fn discover_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let adapter = RustAdapter;
    visit(root, root, &adapter, &mut out)?;
    out.sort();
    Ok(out)
}

/// Discover production files in the workspace that route to a preview-language
/// adapter (TypeScript/JavaScript, Python, or Perl), by path extension only.
///
/// Routing is `analysis::language::route`, the same predicate adapter dispatch
/// uses. This does not require the adapter to be enabled; it is used so the
/// repo pipeline can disclose preview-language files in scope even when the
/// adapter is not enabled (RIPR-SPEC-0082, #1111). Test files and excluded
/// directories are skipped the same way as Rust discovery.
pub(crate) fn discover_preview_language_files(root: &Path) -> Vec<(LanguageId, PathBuf)> {
    let mut out: Vec<(LanguageId, PathBuf)> = Vec::new();
    visit_classified(root, root, &mut out, &|path| {
        route(path).filter(|language| {
            matches!(
                language,
                LanguageId::TypeScript
                    | LanguageId::JavaScript
                    | LanguageId::Python
                    | LanguageId::Perl
            )
        })
    });
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Python test files a Rust diff cannot be linked to (#6340): the count and one
/// repository-relative example. `at_least` is set when the bounded walk hit its
/// entry cap, so the count is a lower bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnlinkedPythonTests {
    pub(crate) count: usize,
    pub(crate) example: String,
    pub(crate) at_least: bool,
}

/// Directories holding installed third-party Python packages or tool caches,
/// whose `tests/` folders are not the repository's own tests. Skipped only by
/// [`discover_python_test_files`].
const PYTHON_VENDOR_DIRS: &[&str] = &[
    ".venv",
    "venv",
    ".tox",
    ".nox",
    "site-packages",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
];

/// Upper bound on directory entries inspected by [`discover_python_test_files`].
const PYTHON_TEST_WALK_ENTRY_CAP: usize = 50_000;

/// Whether a repository-relative path is a Python test file: `test_*.py` or
/// `*_test.py`, or any `.py` below a `tests/` or `test/` directory.
pub(crate) fn is_python_test_path(relative: &Path) -> bool {
    let Some(name) = relative.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(stem) = name.strip_suffix(".py") else {
        return false;
    };
    if stem.starts_with("test_") || stem.ends_with("_test") {
        return true;
    }
    relative
        .parent()
        .into_iter()
        .flat_map(|parent| parent.components())
        .any(|component| {
            matches!(component, std::path::Component::Normal(dir)
                if dir == "tests" || dir == "test")
        })
}

fn is_named_python_test(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.starts_with("test_") || stem.ends_with("_test"))
}

/// Count Python test files in the workspace with a bounded, no-follow walk that
/// skips the same directories as the other discovery walks. `Ok(None)` when no
/// Python test was seen, including when the entry cap stopped the walk before
/// one was found: the note only names tests ripr saw, so an absent note is not
/// a claim that none exist. A directory or entry the walk could not read makes
/// the count a lower bound, like the cap. A cancelled walk is an error, not a
/// partial count.
/// Used only to name, in human output, that a Rust change may be covered by
/// tests ripr does not link to Rust.
pub(crate) fn discover_python_test_files(
    root: &Path,
) -> Result<Option<UnlinkedPythonTests>, String> {
    let mut count = 0usize;
    let mut example: Option<PathBuf> = None;
    let mut visited = 0usize;
    let mut stack = vec![root.to_path_buf()];
    let mut capped = false;
    'walk: while let Some(dir) = stack.pop() {
        cancellation::checkpoint()?;
        let Ok(entries) = frozen_fs::read_dir(&dir) else {
            // An unreadable directory means part of the tree was not inspected,
            // so a count found elsewhere is only a lower bound.
            capped = true;
            continue;
        };
        for entry in entries {
            cancellation::checkpoint()?;
            let Ok(entry) = entry else {
                capped = true;
                continue;
            };
            visited += 1;
            if visited > PYTHON_TEST_WALK_ENTRY_CAP {
                capped = true;
                break 'walk;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                capped = true;
                continue;
            };
            if kind.is_dir() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !DEFAULT_IGNORED_DIRS.contains(&name) && !PYTHON_VENDOR_DIRS.contains(&name) {
                    stack.push(path);
                }
            } else if kind.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                if is_python_test_path(relative) {
                    count += 1;
                    // Prefer a `test_*.py` / `*_test.py` example over a helper
                    // such as `tests/__init__.py`, then the smallest path.
                    let better = example.as_ref().is_none_or(|current| {
                        (!is_named_python_test(relative), relative)
                            < (!is_named_python_test(current), current.as_path())
                    });
                    if better {
                        example = Some(relative.to_path_buf());
                    }
                }
            }
        }
    }
    let Some(example) = example else {
        return Ok(None);
    };
    Ok(Some(UnlinkedPythonTests {
        count,
        example: example.to_string_lossy().replace('\\', "/"),
        at_least: capped,
    }))
}

/// Discover source files in languages no ripr adapter reads (Go, Java, C,
/// shell, ...), with their language names, skipping the same directories as
/// the other discovery walks. Pilot uses it so a repository written in such a
/// language gets a named non-claim instead of an empty "complete" ranking.
pub(crate) fn discover_unanalyzed_source_files(root: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut out: Vec<(&'static str, PathBuf)> = Vec::new();
    visit_classified(root, root, &mut out, &unanalyzed_source_language);
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

fn visit_classified<T>(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(T, PathBuf)>,
    classify: &dyn Fn(&Path) -> Option<T>,
) {
    let Ok(entries) = frozen_fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        // Cooperative cancellation (#1972): the preview walk cannot
        // propagate an error, so a cancelled refresh stops the traversal
        // early instead of walking the whole tree. No-op without a token.
        if cancellation::checkpoint().is_err() {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if DEFAULT_IGNORED_DIRS.contains(&name) {
                continue;
            }
            visit_classified(root, &path, out, classify);
        } else if let Some(class) = classify(&path) {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push((class, relative));
        }
    }
}

fn visit(
    root: &Path,
    dir: &Path,
    adapter: &RustAdapter,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries =
        frozen_fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        cancellation::checkpoint()?;
        let entry = entry.map_err(|err| format!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        // `file_type` does not follow links. A committed `src/x.rs` symlink
        // can name `/dev/zero`, a FIFO or a file outside the checkout, so only
        // regular files are sources, as in the Python and TypeScript reads.
        let file_type = entry.file_type().ok();
        if file_type.is_some_and(|kind| kind.is_dir()) {
            if DEFAULT_IGNORED_DIRS.contains(&name) {
                continue;
            }
            visit(root, &path, adapter, out)?;
        } else if file_type.is_some_and(|kind| kind.is_file()) && adapter.accepts_path(&path) {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn actual_preview_discovery_refuses_deleted_snapshot_children() -> Result<(), Box<dyn std::error::Error>> {
        use crate::analysis::committed_source::frozen;
        let fixture = frozen::tests::Fixture::new(&[
            ("src/a.ts", b"export const value = 1;\n"),
            ("src/b.py", b"value = 1\n"),
        ])?;
        frozen::with_context(Some(fixture.authority.clone()), || -> Result<(), Box<dyn std::error::Error>> {
            let before = super::discover_preview_language_files(&fixture.logical);
            assert_eq!(before.len(), 2, "the intact admitted preview files must be discovered");
            fixture.authority.ensure_clean()?;
            std::fs::remove_file(fixture.physical.join("src/a.ts"))?;
            let _after = super::discover_preview_language_files(&fixture.logical);
            let failure = fixture.authority.ensure_clean().err()
                .ok_or("actual preview discovery silently omitted an admitted snapshot child")?;
            assert!(failure.to_string().contains("admitted snapshot child is missing"), "{failure}");
            Ok(())
        })?;
        Ok(())
    }

    use super::*;
    use std::fs;

    #[test]
    fn python_test_path_predicate_is_discriminating() {
        for yes in [
            "tests/test_x.py",
            "pkg/test_y.py",
            "pkg/y_test.py",
            "tests/helpers/conftest.py",
            "test/util.py",
        ] {
            assert!(is_python_test_path(Path::new(yes)), "{yes}");
        }
        for no in [
            "src/lib.rs",
            "python/pkg/mod.py",
            "tests/test_x.rs",
            "contest/mod.py",
            "tests.py",
            "pkg/latest.py",
        ] {
            assert!(!is_python_test_path(Path::new(no)), "{no}");
        }
    }

    #[test]
    fn discover_python_test_files_reports_cancellation_instead_of_a_partial_count()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-pytest-cancel-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tests"))?;
        fs::write(dir.join("tests/test_a.py"), "")?;
        let token = cancellation::AnalysisCancellationToken::new();
        assert!(token.cancel(cancellation::AnalysisAbortKind::Superseded));
        let result = cancellation::with_token(&token, || discover_python_test_files(&dir));
        let _ = fs::remove_dir_all(&dir);
        let error = result.err().ok_or("a cancelled walk must be an error")?;
        assert!(cancellation::is_cancellation_error(&error), "{error}");
        Ok(())
    }

    #[test]
    fn discover_python_test_files_counts_and_skips_ignored_dirs()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-pytest-discover-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tests"))?;
        fs::create_dir_all(dir.join("src"))?;
        fs::create_dir_all(dir.join("target"))?;
        fs::create_dir_all(dir.join(".venv/lib/site-packages/pkg/tests"))?;
        fs::write(
            dir.join(".venv/lib/site-packages/pkg/tests/test_vendor.py"),
            "",
        )?;
        fs::write(dir.join("src/lib.rs"), "")?;
        fs::write(dir.join("tests/test_b.py"), "")?;
        fs::write(dir.join("tests/test_a.py"), "")?;
        fs::write(dir.join("tests/__init__.py"), "")?;
        fs::write(dir.join("target/test_ignored.py"), "")?;
        let found = discover_python_test_files(&dir)?;
        assert_eq!(
            found,
            Some(UnlinkedPythonTests {
                count: 3,
                example: "tests/test_a.py".to_string(),
                at_least: false,
            })
        );
        fs::remove_file(dir.join("tests/test_a.py"))?;
        fs::remove_file(dir.join("tests/test_b.py"))?;
        fs::remove_file(dir.join("tests/__init__.py"))?;
        assert_eq!(discover_python_test_files(&dir)?, None);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn discover_rust_files_is_callable() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-test-{:?}",
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        fs::create_dir(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "")?;

        let result = discover_rust_files(&dir)?;
        assert!(result.iter().any(|p| p.ends_with("src/lib.rs")));

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    /// A cloned repository can commit `src/zero.rs -> /dev/zero`; reading it
    /// as source exhausted memory. Symlinked `.rs` entries are not sources.
    #[cfg(unix)]
    #[test]
    fn discover_rust_files_skips_symlinked_sources() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-symlink-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "pub fn one() -> i32 { 1 }")?;
        std::os::unix::fs::symlink("/dev/zero", dir.join("src/zero.rs"))?;
        std::os::unix::fs::symlink(dir.join("src/lib.rs"), dir.join("src/alias.rs"))?;
        let result = discover_rust_files(&dir);
        let _ = fs::remove_dir_all(&dir);
        let result = result?;
        assert!(result.iter().any(|p| p.ends_with("src/lib.rs")));
        assert!(
            !result
                .iter()
                .any(|p| p.ends_with("src/zero.rs") || p.ends_with("src/alias.rs")),
            "symlinked sources must not be discovered: {result:?}"
        );
        Ok(())
    }

    /// #3235: the related-test package guard depends on file identities
    /// staying repo-relative from discovery through classification. Discovery
    /// is the authority that strips the workspace root (the RustAdapter
    /// selects from these paths and passes them into the index unchanged), so
    /// pin it here: every discovered identity is relative on every host.
    #[test]
    fn discover_rust_files_returns_repo_relative_identities()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-relative-{:?}",
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "pub fn one() -> i32 { 1 }")?;
        fs::create_dir_all(dir.join("tests"))?;
        fs::write(
            dir.join("tests/it.rs"),
            "#[test] fn t() { assert_eq!(1, 1); }",
        )?;

        let result = discover_rust_files(&dir)?;
        assert!(
            result.iter().any(|p| p.ends_with("src/lib.rs")),
            "fixture must be discovered"
        );
        for path in &result {
            let text = path.to_string_lossy();
            assert!(
                !path.is_absolute(),
                "discovered identity must stay repo-relative: {text}"
            );
            assert!(
                !text.starts_with('/'),
                "discovered identity leaked a Unix root: {text}"
            );
            assert!(
                text.as_bytes().get(1) != Some(&b':'),
                "discovered identity leaked a drive prefix: {text}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn discover_skips_default_excluded_directories() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-default-exclusions-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "")?;

        for ignored in DEFAULT_IGNORED_DIRS {
            let ignored_src = dir.join(ignored).join("src");
            fs::create_dir_all(&ignored_src)?;
            fs::write(ignored_src.join("lib.rs"), "")?;
        }

        let result = discover_rust_files(&dir)?;
        assert_eq!(result, vec![PathBuf::from("src/lib.rs")]);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn discover_preview_language_files_includes_perl_without_adapter()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir =
            std::env::temp_dir().join(format!("ripr-discover-perl-preview-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("lib/My"))?;
        fs::write(dir.join("lib/My/App.pm"), "sub value { return 1 }\n")?;

        let result = discover_preview_language_files(&dir);

        assert_eq!(
            result,
            vec![(LanguageId::Perl, PathBuf::from("lib/My/App.pm"))]
        );
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #4586: a changed source path that is not a regular file on disk
    /// is named; docs and a present sibling stay out of the list.
    #[test]
    fn absent_worktree_source_is_named_and_present_or_nonsource_paths_are_not()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("ripr-absent-worktree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "pub fn one() -> i32 { 1 }\n")?;
        fs::write(dir.join("README.md"), "docs\n")?;

        let absent = changed_source_files_absent_from_worktree(
            &dir,
            [
                Path::new("src/lib.rs"),
                Path::new("src/missing.rs"),
                Path::new("README.md"),
            ],
        );
        assert_eq!(absent, vec![PathBuf::from("src/missing.rs")]);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #4586: a repo-relative diff path against a crate subdirectory
    /// root is the same on-disk file, not an absent worktree file.
    /// A sibling crate prefix must not borrow this crate's `src/lib.rs`.
    #[test]
    fn repo_relative_diff_path_under_crate_root_is_present()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-absent-worktree-prefix-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let crate_root = dir.join("crates/ripr/examples/sample");
        fs::create_dir_all(crate_root.join("src"))?;
        fs::write(crate_root.join("src/lib.rs"), "pub fn one() -> i32 { 1 }\n")?;

        let absent = changed_source_files_absent_from_worktree(
            &crate_root,
            [
                Path::new("crates/ripr/examples/sample/src/lib.rs"),
                Path::new("crates/other/src/lib.rs"),
                Path::new("crates/ripr/examples/sample/src/missing.rs"),
            ],
        );
        assert_eq!(
            absent,
            vec![
                PathBuf::from("crates/other/src/lib.rs"),
                PathBuf::from("crates/ripr/examples/sample/src/missing.rs"),
            ]
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Both direct and validated repository-prefix paths use discovery's
    /// no-follow boundary, while the selected root itself may be an alias.
    #[cfg(unix)]
    #[test]
    fn source_admission_refuses_links_below_root_and_preserves_root_alias()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-source-admission-links-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let root = dir.join("crates/ripr/examples/sample");
        fs::create_dir_all(root.join("src"))?;
        let source = root.join("src/lib.rs");
        fs::write(&source, "pub fn one() -> i32 { 1 }\n")?;
        let alias = dir.join("alias/crates/ripr/examples/sample");
        fs::create_dir_all(alias.parent().ok_or("alias parent missing")?)?;
        std::os::unix::fs::symlink(&root, &alias)?;
        let paths = [
            Path::new("src/lib.rs"),
            Path::new("crates/ripr/examples/sample/src/lib.rs"),
        ];
        for selected in [&root, &alias] {
            assert!(changed_source_files_absent_from_worktree(selected, paths).is_empty());
        }

        let held_file = root.join("held-source.txt");
        fs::rename(&source, &held_file)?;
        std::os::unix::fs::symlink(&held_file, &source)?;
        for selected in [&root, &alias] {
            assert_eq!(
                changed_source_files_absent_from_worktree(selected, paths).len(),
                2
            );
        }
        fs::remove_file(&source)?;
        fs::rename(&held_file, &source)?;

        let held_dir = root.join("target/held-src");
        fs::create_dir_all(root.join("target"))?;
        fs::rename(root.join("src"), &held_dir)?;
        std::os::unix::fs::symlink(&held_dir, root.join("src"))?;
        for selected in [&root, &alias] {
            assert_eq!(
                changed_source_files_absent_from_worktree(selected, paths).len(),
                2
            );
        }
        fs::remove_file(root.join("src"))?;
        fs::rename(&held_dir, root.join("src"))?;
        for selected in [&root, &alias] {
            assert!(changed_source_files_absent_from_worktree(selected, paths).is_empty());
        }
        fs::remove_file(&alias)?;
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// #4586: a `--root` whose last component is `src` must not treat a
    /// root-level `lib.rs` as the missing nested `src/lib.rs`.
    #[test]
    fn single_component_root_basename_does_not_mask_nested_absent_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-absent-worktree-src-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let crate_root = dir.join("src");
        fs::create_dir_all(&crate_root)?;
        fs::write(crate_root.join("lib.rs"), "pub fn one() -> i32 { 1 }\n")?;

        let absent =
            changed_source_files_absent_from_worktree(&crate_root, [Path::new("src/lib.rs")]);
        assert_eq!(absent, vec![PathBuf::from("src/lib.rs")]);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
