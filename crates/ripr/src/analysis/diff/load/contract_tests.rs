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

// #1627: the retained public raw boundary must run before semantic decode.
// The test-only predecessor used legacy_string.into_bytes here deliberately;
// the same assertions below discriminate that reconstruction from raw intake.
fn canonical_intake_bytes(root: &Path, base: &str, head: &str) -> Result<Vec<u8>, String> {
    crate::analysis::load_canonical_pr_evidence_diff_bytes(root, base, head)
}

fn contains_bytes(bytes: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || bytes.windows(needle.len()).any(|window| window == needle)
}

fn independent_canonical_bytes(repo: &Repo, base: &str, head: &str) -> io::Result<Vec<u8>> {
    run_git_diff_bytes(
        &repo.root,
        &format!("{base}...{head}"),
        &[
            "--relative",
            "--unified=0",
            "--no-ext-diff",
            "--submodule=short",
        ],
        "0",
        Some(GIT_TIMEOUT),
    )
    .map_err(io::Error::other)
}

#[test]
fn raw_canonical_range_preserves_non_utf8_body_before_semantic_decode() -> io::Result<()> {
    let repo = Repo::new("raw-body")?;
    fs::write(repo.root.join("raw.txt"), b"body:\xfe\n")?;
    git(&repo.root, &["add", "raw.txt"])?;
    git(&repo.root, &["commit", "--quiet", "-m", "raw body base"])?;
    let base = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    fs::write(repo.root.join("raw.txt"), b"body:\xff\n")?;
    git(&repo.root, &["add", "raw.txt"])?;
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "distinct raw body"],
    )?;
    let head = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    let expected = independent_canonical_bytes(&repo, &base, &head)?;
    assert!(contains_bytes(&expected, b"-body:\xfe\n"));
    assert!(contains_bytes(&expected, b"+body:\xff\n"));
    let Err(_) = std::str::from_utf8(&expected) else {
        return Err(io::Error::other(
            "fixture must contain non-UTF8 source bytes",
        ));
    };
    let legacy = super::load_canonical_pr_evidence_diff_range(&repo.root, &base, &head)
        .map_err(io::Error::other)?;
    assert_eq!(legacy, String::from_utf8_lossy(&expected));
    assert!(legacy.contains("-body:\u{fffd}\n+body:\u{fffd}\n"));
    assert_ne!(
        legacy.as_bytes(),
        expected.as_slice(),
        "fixture must reject semantic reconstruction"
    );

    let actual = canonical_intake_bytes(&repo.root, &base, &head).map_err(io::Error::other)?;
    assert!(
        contains_bytes(&actual, b"-body:\xfe\n") && contains_bytes(&actual, b"+body:\xff\n"),
        "raw intake lost distinct source bytes before coverage could identify them"
    );
    assert_eq!(actual, expected);
    assert_eq!(String::from_utf8_lossy(&actual), legacy);
    Ok(())
}

#[test]
fn raw_canonical_range_keeps_metadata_and_same_head_empty() -> io::Result<()> {
    let repo = Repo::new("raw-metadata")?;
    repo.config("core.filemode", "false")?;
    repo.config("diff.renames", "true")?;
    repo.config("diff.submodule", "log")?;
    let old_gitlink = repo.base.clone();
    let new_gitlink = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    assert_ne!(old_gitlink, new_gitlink);
    fs::write(repo.root.join("deleted.txt"), "deleted source\n")?;
    fs::write(
        repo.root.join("rename_from.txt"),
        "unique exact rename payload\n",
    )?;
    fs::write(repo.root.join("binary.dat"), b"\0old\0")?;
    fs::write(repo.root.join("mode.txt"), "unchanged mode payload\n")?;
    git(&repo.root, &["add", "."])?;
    git(
        &repo.root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{old_gitlink},nested"),
        ],
    )?;
    git(&repo.root, &["commit", "--quiet", "-m", "metadata base"])?;
    let base = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();

    fs::remove_file(repo.root.join("deleted.txt"))?;
    fs::rename(
        repo.root.join("rename_from.txt"),
        repo.root.join("rename_to.txt"),
    )?;
    fs::write(repo.root.join("binary.dat"), b"\0new\0")?;
    fs::write(repo.root.join("empty_added.txt"), b"")?;
    git(&repo.root, &["add", "."])?;
    git(&repo.root, &["update-index", "--chmod=+x", "mode.txt"])?;
    git(
        &repo.root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{new_gitlink},nested"),
        ],
    )?;
    git(&repo.root, &["commit", "--quiet", "-m", "metadata head"])?;
    let head = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();

    let expected = independent_canonical_bytes(&repo, &base, &head)?;
    let actual = canonical_intake_bytes(&repo.root, &base, &head).map_err(io::Error::other)?;
    assert_eq!(actual, expected);
    let text = String::from_utf8(actual).map_err(io::Error::other)?;
    for required in [
        "deleted file mode 100644",
        "+++ /dev/null",
        "similarity index 100%",
        "rename from rename_from.txt",
        "rename to rename_to.txt",
        "diff --git a/empty_added.txt b/empty_added.txt",
        "new file mode 100644",
        "old mode 100644\nnew mode 100755",
        "Binary files a/binary.dat and b/binary.dat differ",
        &format!("-Subproject commit {old_gitlink}"),
        &format!("+Subproject commit {new_gitlink}"),
    ] {
        assert!(
            text.contains(required),
            "missing real Git metadata: {required}"
        );
    }
    assert!(!text.contains("GIT binary patch"));
    assert!(!text.contains("Submodule nested"));
    let legacy = super::load_canonical_pr_evidence_diff_range(&repo.root, &base, &head)
        .map_err(io::Error::other)?;
    assert_eq!(legacy, text);
    let project = |input: &str| {
        parse_unified_diff(input)
            .into_iter()
            .map(|file| (file.path, file.added_lines, file.removed_lines))
            .collect::<Vec<_>>()
    };
    assert_eq!(project(&legacy), project(&text));
    assert!(
        canonical_intake_bytes(&repo.root, &head, &head)
            .map_err(io::Error::other)?
            .is_empty()
    );
    assert!(
        canonical_intake_bytes(&repo.root, &head, &base)
            .map_err(io::Error::other)?
            .is_empty()
    );
    Ok(())
}

#[test]
fn raw_canonical_range_uses_full_literal_three_dot_subject_and_refusals() -> io::Result<()> {
    let repo = Repo::new("raw-range")?;
    let head = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    let forward =
        canonical_intake_bytes(&repo.root, &repo.base, &head).map_err(io::Error::other)?;
    assert_eq!(
        forward,
        independent_canonical_bytes(&repo, &repo.base, &head)?
    );
    assert!(contains_bytes(
        &forward,
        b"+pub const VALUE_100: u32 = 101;"
    ));
    assert!(contains_bytes(
        &forward,
        b"+pub const VALUE_1900: u32 = 1901;"
    ));

    git(
        &repo.root,
        &["checkout", "--quiet", "-b", "side", &repo.base],
    )?;
    fs::write(
        repo.root.join("src/lib.rs"),
        source(false).replace("VALUE_1: u32 = 1;", "VALUE_1: u32 = 777;"),
    )?;
    git(&repo.root, &["add", "src/lib.rs"])?;
    git(&repo.root, &["commit", "--quiet", "-m", "divergent side"])?;
    let side = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    let side_diff = canonical_intake_bytes(&repo.root, &head, &side).map_err(io::Error::other)?;
    assert_eq!(side_diff, independent_canonical_bytes(&repo, &head, &side)?);
    assert!(contains_bytes(
        &side_diff,
        b"+pub const VALUE_1: u32 = 777;"
    ));
    assert!(!contains_bytes(&side_diff, b"VALUE_100"));
    assert!(!contains_bytes(&side_diff, b"VALUE_1900"));
    for (base, head, category) in [
        (
            repo.base.as_str(),
            "refs/heads/absent-intake-head",
            "does not resolve to a commit",
        ),
        (
            "-invalid-intake-base",
            side.as_str(),
            "a revision range cannot start with",
        ),
    ] {
        let raw_error = canonical_intake_bytes(&repo.root, base, head)
            .err()
            .ok_or_else(|| io::Error::other("raw loader accepted an invalid range"))?;
        let legacy_error = super::load_canonical_pr_evidence_diff_range(&repo.root, base, head)
            .err()
            .ok_or_else(|| io::Error::other("legacy loader accepted an invalid range"))?;
        assert!(raw_error.contains(category), "{raw_error}");
        assert_eq!(raw_error, legacy_error);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn raw_canonical_range_keeps_c_quoted_invalid_path_and_literal_octal_distinct() -> io::Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let repo = Repo::new("raw-path")?;
    let invalid = PathBuf::from(OsString::from_vec(b"raw_\xff.rs".to_vec()));
    let literal = PathBuf::from(r"raw_\377.rs");
    assert_ne!(invalid, literal);
    fs::write(repo.root.join(&invalid), "fn invalid() -> u32 { 1 }\n")?;
    fs::write(repo.root.join(&literal), "fn literal() -> u32 { 2 }\n")?;
    git(&repo.root, &["add", "."])?;
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "distinct byte paths"],
    )?;
    let base = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    fs::write(repo.root.join(&invalid), "fn invalid() -> u32 { 11 }\n")?;
    fs::write(repo.root.join(&literal), "fn literal() -> u32 { 22 }\n")?;
    git(&repo.root, &["add", "."])?;
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "edit both byte paths"],
    )?;
    let head = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    repo.config("core.quotePath", "false")?;
    repo.config("diff.noprefix", "true")?;
    repo.config("diff.context", "1000")?;

    let expected = independent_canonical_bytes(&repo, &base, &head)?;
    let raw = canonical_intake_bytes(&repo.root, &base, &head).map_err(io::Error::other)?;
    assert_eq!(raw, expected);
    assert!(contains_bytes(&raw, br#"+++ "b/raw_\377.rs""#));
    assert!(contains_bytes(&raw, br#"+++ "b/raw_\\377.rs""#));
    let text = String::from_utf8(raw).map_err(io::Error::other)?;
    assert!(!text.lines().any(|line| line.starts_with(' ')));
    let mut paths = parse_unified_diff(&text)
        .into_iter()
        .map(|file| file.path)
        .collect::<Vec<_>>();
    paths.sort();
    let mut expected_paths = vec![invalid, literal];
    expected_paths.sort();
    assert_eq!(paths, expected_paths);
    Ok(())
}

#[test]
fn raw_canonical_range_retains_cooperative_deadline_refusal() -> io::Result<()> {
    let repo = Repo::new("raw-deadline")?;
    let head = git(&repo.root, &["rev-parse", "HEAD"])?.trim().to_string();
    let raw_error = super::load_diff_range_bytes_with_deadline_core(
        &repo.root,
        &repo.base,
        &head,
        Some(Duration::ZERO),
    )
    .err()
    .ok_or_else(|| io::Error::other("raw loader accepted an expired deadline"))?;
    let semantic_error = super::load_diff_range_with_deadline_core(
        &repo.root,
        &repo.base,
        &head,
        Some(Duration::ZERO),
    )
    .err()
    .ok_or_else(|| io::Error::other("semantic loader accepted an expired deadline"))?;
    assert!(raw_error.is_git_invocation_timeout(), "{raw_error}");
    assert!(
        semantic_error.is_git_invocation_timeout(),
        "{semantic_error}"
    );
    assert_eq!(raw_error.to_string(), semantic_error.to_string());
    Ok(())
}
