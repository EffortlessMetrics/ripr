//! Regression controls for the Git source-patch contract (#3850).

use super::{load_diff, load_diff_range, load_worktree_diff, run_git_diff_bytes};
use crate::analysis::diff::parse_unified_diff;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GIT_TIMEOUT: Duration = Duration::from_secs(10);

struct Repo {
    root: PathBuf,
    base: String,
}

impl Repo {
    fn new(name: &str) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        // Own the path before fallible setup so partial fixtures are cleaned.
        let mut repo = Self {
            root: std::env::temp_dir().join(format!(
                "ripr-diff-contract-{name}-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            base: String::new(),
        };
        fs::create_dir_all(repo.root.join("src"))?;
        git(&repo.root, &["init", "--initial-branch=main"])?;
        for (key, value) in [
            ("user.name", "Diff Contract"),
            ("user.email", "diff-contract@example.com"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("color.ui", "never"),
            ("diff.context", "0"),
            ("diff.interHunkContext", "0"),
        ] {
            repo.config(key, value)?;
        }
        fs::write(repo.root.join(".gitattributes"), "src/lib.rs diff=audit\n")?;
        fs::write(repo.root.join("src/lib.rs"), source(false))?;
        git(&repo.root, &["add", "."])?;
        git(&repo.root, &["commit", "--quiet", "-m", "base"])?;
        repo.base = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
        fs::write(repo.root.join("src/lib.rs"), source(true))?;
        git(&repo.root, &["add", "src/lib.rs"])?;
        git(&repo.root, &["commit", "--quiet", "-m", "two source edits"])?;
        Ok(repo)
    }

    fn config(&self, key: &str, value: &str) -> io::Result<()> {
        git(&self.root, &["config", "--local", key, value])?;
        Ok(())
    }

    fn patches(&self) -> io::Result<[String; 3]> {
        Ok([
            load_diff(&self.root, Some(&self.base), None, Some(GIT_TIMEOUT))
                .map_err(io::Error::other)?,
            load_diff_range(&self.root, &self.base, "HEAD").map_err(io::Error::other)?,
            load_worktree_diff(&self.root, Some(&self.base), Some(GIT_TIMEOUT))
                .map_err(io::Error::other)?,
        ])
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Err(error) = clear_readonly_files(&self.root) {
            eprintln!("diff fixture permissions {}: {error}", self.root.display());
        }
        if let Err(error) = fs::remove_dir_all(&self.root)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("diff fixture cleanup {}: {error}", self.root.display());
        }
    }
}

#[cfg(windows)]
fn clear_readonly_files(root: &Path) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            clear_readonly_files(&entry.path())?;
        } else if kind.is_file() {
            let mut permissions = entry.metadata()?.permissions();
            if permissions.readonly() {
                // Clearing FILE_ATTRIBUTE_READONLY is the only way to remove
                // the fixture tree on Windows; the Unix-mode lint does not
                // apply to this cfg-gated helper.
                #[expect(
                    clippy::permissions_set_readonly_false,
                    reason = "clearing the Windows readonly attribute before fixture cleanup"
                )]
                {
                    permissions.set_readonly(false);
                }
                fs::set_permissions(entry.path(), permissions)?;
            }
        }
    }
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> io::Result<String> {
    let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
        root,
        args,
        GIT_TIMEOUT,
        1024 * 1024,
    )
    .map_err(io::Error::other)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "fixture git {args:?}: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

fn source(changed: bool) -> String {
    (1..=2_000)
        .map(|line| {
            let value = if changed && matches!(line, 100 | 1_900) {
                line + 1
            } else {
                line
            };
            format!("pub const VALUE_{line}: u32 = {value};\n")
        })
        .collect()
}

/// Commit two repository paths that only canonical side prefixes keep
/// distinct: `b/identity.rs` is literally the token the parser strips from a
/// `+++` marker, so a diff that drops or re-letters the side prefix collapses
/// that real `b/` directory component onto the sibling `identity.rs` (#4086).
fn commit_colliding_identity_paths(repo: &Repo) -> io::Result<()> {
    fs::create_dir_all(repo.root.join("b"))?;
    fs::write(
        repo.root.join("identity.rs"),
        "pub fn outer() -> u32 { 1 }\n",
    )?;
    fs::write(
        repo.root.join("b").join("identity.rs"),
        "pub fn nested() -> u32 { 2 }\n",
    )?;
    git(&repo.root, &["add", "."])?;
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "colliding identity paths"],
    )?;
    fs::write(
        repo.root.join("identity.rs"),
        "pub fn outer() -> u32 { 10 }\n",
    )?;
    fs::write(
        repo.root.join("b").join("identity.rs"),
        "pub fn nested() -> u32 { 20 }\n",
    )?;
    git(&repo.root, &["add", "."])?;
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "edit colliding identity paths"],
    )?;
    Ok(())
}

fn assert_source_patch(patch: &str) {
    assert!(
        !patch.contains('\u{1b}'),
        "analysis patch must not contain color"
    );
    assert_eq!(
        patch.lines().filter(|line| line.starts_with("@@ ")).count(),
        2
    );
    assert!(!patch.lines().any(|line| line.starts_with(' ')));
    let files = parse_unified_diff(patch);
    assert_eq!(files.len(), 1);
    for file in &files {
        assert_eq!(file.path, PathBuf::from("src/lib.rs"));
        let added: Vec<_> = file
            .added_lines
            .iter()
            .map(|line| (line.line, line.text.as_str()))
            .collect();
        let removed: Vec<_> = file
            .removed_lines
            .iter()
            .map(|line| (line.line, line.text.as_str()))
            .collect();
        assert_eq!(
            added,
            vec![
                (100, "pub const VALUE_100: u32 = 101;"),
                (1_900, "pub const VALUE_1900: u32 = 1901;"),
            ]
        );
        assert_eq!(
            removed,
            vec![
                (100, "pub const VALUE_100: u32 = 100;"),
                (1_900, "pub const VALUE_1900: u32 = 1900;"),
            ]
        );
    }
}

#[test]
fn loaders_ignore_external_diff_helper() -> io::Result<()> {
    let repo = Repo::new("external")?;
    let expected = repo.patches()?;
    repo.config("diff.external", "git --version")?;
    // The pre-repair worktree argv actually invokes this helper. Without
    // this control a broken helper fixture could let the regression pass.
    let raw = git(&repo.root, &["diff", "--submodule=short", &repo.base])?;
    assert!(raw.contains("git version"));
    assert!(!raw.contains("VALUE_100"));
    let actual = repo.patches()?;
    assert_eq!(actual, expected);
    for patch in &actual {
        assert_source_patch(patch);
    }
    Ok(())
}

#[test]
fn loaders_ignore_textconv_even_when_external_diff_is_disabled() -> io::Result<()> {
    let repo = Repo::new("textconv")?;
    let expected = repo.patches()?;
    // Git itself is the constant-output helper on Unix and Windows; no
    // shell script, executable permission, or global environment mutation.
    repo.config("diff.audit.textconv", "git --version")?;
    let range = format!("{}...HEAD", repo.base);
    let raw = git(
        &repo.root,
        &["diff", "--no-ext-diff", "--unified=0", &range],
    )?;
    assert!(
        raw.is_empty(),
        "the constant textconv must hide the source edit"
    );
    let actual = repo.patches()?;
    assert_eq!(actual, expected);
    for patch in &actual {
        assert_source_patch(patch);
    }
    Ok(())
}

#[test]
fn loaders_ignore_color_always() -> io::Result<()> {
    let repo = Repo::new("color")?;
    let expected = repo.patches()?;
    repo.config("color.diff", "always")?;
    let raw = git(&repo.root, &["diff", "--no-ext-diff", &repo.base])?;
    assert!(
        raw.contains('\u{1b}'),
        "fixture must enable color in captured output"
    );
    let actual = repo.patches()?;
    assert_eq!(actual, expected);
    for patch in &actual {
        assert_source_patch(patch);
    }
    Ok(())
}

#[test]
fn loaders_do_not_expand_context_or_fuse_distant_hunks() -> io::Result<()> {
    let repo = Repo::new("context")?;
    let expected = repo.patches()?;
    for patch in &expected {
        assert_source_patch(patch);
    }
    repo.config("diff.context", "10000")?;
    let expanded = git(&repo.root, &["diff", "--submodule=short", &repo.base])?;
    assert!(expanded.lines().any(|line| line.starts_with(' ')));
    assert_eq!(repo.patches()?, expected);

    repo.config("diff.context", "0")?;
    repo.config("diff.interHunkContext", "10000")?;
    let fused = git(&repo.root, &["diff", "--unified=0", &repo.base])?;
    assert_eq!(
        fused.lines().filter(|line| line.starts_with("@@ ")).count(),
        1
    );
    let actual = repo.patches()?;
    assert_eq!(actual, expected);
    for patch in &actual {
        assert_source_patch(patch);
    }
    Ok(())
}

#[test]
fn worktree_loader_keeps_staged_and_unstaged_source_edits() -> io::Result<()> {
    let repo = Repo::new("worktree")?;
    let committed = repo.patches()?;
    let staged = source(true).replace("VALUE_500: u32 = 500", "VALUE_500: u32 = 501");
    fs::write(repo.root.join("src/lib.rs"), &staged)?;
    git(&repo.root, &["add", "src/lib.rs"])?;
    fs::write(
        repo.root.join("src/lib.rs"),
        staged.replace("VALUE_1500: u32 = 1500", "VALUE_1500: u32 = 1501"),
    )?;
    let [committed_after, range_after, worktree] = repo.patches()?;
    let [committed_before, range_before, _] = committed;
    assert_eq!(committed_after, committed_before);
    assert_eq!(range_after, range_before);
    let files = parse_unified_diff(&worktree);
    assert_eq!(files.len(), 1);
    for file in &files {
        assert_eq!(file.path, PathBuf::from("src/lib.rs"));
        assert_eq!(
            file.added_lines
                .iter()
                .map(|line| line.line)
                .collect::<Vec<_>>(),
            vec![100, 500, 1_500, 1_900]
        );
        assert_eq!(
            file.removed_lines
                .iter()
                .map(|line| line.line)
                .collect::<Vec<_>>(),
            vec![100, 500, 1_500, 1_900]
        );
    }
    Ok(())
}

#[test]
fn loaders_pin_canonical_side_prefixes_against_ambient_diff_config() -> io::Result<()> {
    // #4086 acceptance: `diff.mnemonicPrefix` (and its `diff.noprefix`
    // sibling) must not change parsed file identity. The parser strips a
    // fixed `a/`/`b/` side prefix, so either setting rewrites identity:
    // `diff.noprefix` drops the prefixes, after which the parser's `b/` strip
    // eats a real `b/` directory component and `b/identity.rs` collapses onto
    // `identity.rs`; `diff.mnemonicPrefix` re-labels the sides `c/`/`w/`,
    // which that strip does not recognise, so the side prefix leaks into the
    // parsed path. Both are demonstrated on every loader.
    //
    // `diff.mnemonicPrefix` only rewrites the base-tree-to-worktree
    // comparison that `load_worktree_diff` issues. It does not touch the
    // `<base>...HEAD` range form the other two loaders use, so the worktree
    // leg is the only one it can discriminate.
    let repo = Repo::new("side-prefix")?;
    commit_colliding_identity_paths(&repo)?;
    let canonical_paths = vec![
        PathBuf::from("b/identity.rs"),
        PathBuf::from("identity.rs"),
        PathBuf::from("src/lib.rs"),
    ];

    for (setting, value, mutated_boundary) in [
        (
            "diff.noprefix",
            "true",
            "diff --git identity.rs identity.rs",
        ),
        (
            "diff.mnemonicPrefix",
            "true",
            "diff --git c/identity.rs w/identity.rs",
        ),
    ] {
        repo.config(setting, value)?;

        // The unpinned control is the exact argv shape the worktree loader
        // issues. Assert the divergence directly, by parsing the control with
        // the same production parser and requiring it to lose identity: a
        // control that merely mentions both paths, or that survives intact,
        // would leave every assertion below vacuous.
        let raw = git(&repo.root, &["diff", &repo.base])?;
        assert!(
            raw.contains(mutated_boundary),
            "{setting} raw control did not present its mutated boundary {mutated_boundary:?}, so the fixture does not discriminate:\n{raw}"
        );
        assert!(
            !raw.contains("diff --git a/identity.rs b/identity.rs"),
            "{setting} raw control kept canonical prefixes, so the fixture does not discriminate:\n{raw}"
        );
        let mut raw_paths: Vec<PathBuf> = parse_unified_diff(&raw)
            .iter()
            .map(|file| file.path.clone())
            .collect();
        raw_paths.sort();
        assert_ne!(
            raw_paths, canonical_paths,
            "{setting} raw control kept parsed identity, so the fixture does not discriminate: {raw_paths:?}"
        );

        let [committed, range, worktree] = repo.patches()?;
        for (loader, patch) in [
            ("load_diff", committed),
            ("load_diff_range", range),
            ("load_worktree_diff", worktree),
        ] {
            assert!(
                patch.contains("diff --git a/identity.rs b/identity.rs"),
                "{loader} let {setting} change the canonical outer-file boundary:\n{patch}"
            );
            assert!(
                patch.contains("diff --git a/b/identity.rs b/b/identity.rs"),
                "{loader} let {setting} change the canonical nested-file boundary:\n{patch}"
            );
            let mut paths: Vec<PathBuf> = parse_unified_diff(&patch)
                .iter()
                .map(|file| file.path.clone())
                .collect();
            paths.sort();
            assert_eq!(
                paths, canonical_paths,
                "{loader} let {setting} change parsed identity"
            );
        }

        // Each leg owns the hostile config for its own assertions only. The
        // pins already defeat any ambient prefix setting — git's
        // `--default-prefix` documents that it "overrides configuration
        // variables such as `diff.noprefix`, `diff.srcPrefix`, `diff.dstPrefix`,
        // and `diff.mnemonicPrefix`", and the production pins set that same
        // `options->prefix` — so a leak could not make a loader assertion pass
        // quietly. The unset is here so each leg measures its own setting
        // rather than whatever the previous leg happened to leave behind.
        git(&repo.root, &["config", "--unset", setting])?;
    }

    Ok(())
}

#[test]
fn shared_diff_authority_overrides_conflicting_prefix_extras() -> io::Result<()> {
    // The shared authority appends its identity pins after caller extras, so a
    // future caller adding its own presentation flags cannot move the parser
    // onto a different side-prefix dialect.
    let repo = Repo::new("side-prefix-extra-precedence")?;
    let bytes = run_git_diff_bytes(
        &repo.root,
        &format!("{}...HEAD", repo.base),
        &["--src-prefix=old/", "--dst-prefix=new/"],
        "0",
        Some(GIT_TIMEOUT),
    )
    .map_err(io::Error::other)?;
    let diff = String::from_utf8(bytes).map_err(io::Error::other)?;

    assert!(
        diff.contains("diff --git a/src/lib.rs b/src/lib.rs"),
        "{diff}"
    );
    assert!(diff.contains("--- a/src/lib.rs"), "{diff}");
    assert!(diff.contains("+++ b/src/lib.rs"), "{diff}");
    assert!(!diff.contains("old/src/lib.rs"), "{diff}");
    assert!(!diff.contains("new/src/lib.rs"), "{diff}");

    Ok(())
}

#[test]
fn explicit_diff_file_remains_verbatim_and_needs_no_git_base() -> io::Result<()> {
    let repo = Repo::new("file")?;
    repo.config("diff.external", "git --version")?;
    repo.config("color.diff", "always")?;
    let path = repo.root.join("supplied.diff");
    let supplied = "--- a/input.rs\n+++ b/input.rs\n@@ -1,2 +1,2 @@\n-old\n+new\n context\n";
    fs::write(&path, supplied)?;
    let actual = load_diff(
        &repo.root,
        Some("nonexistent-base"),
        Some(&path),
        Some(Duration::ZERO),
    )
    .map_err(io::Error::other)?;
    assert_eq!(actual, supplied);
    Ok(())
}
