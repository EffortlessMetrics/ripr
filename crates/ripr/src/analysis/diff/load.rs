use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[cfg(test)]
mod contract_tests;

/// A loaded diff together with the base ref that produced it (#3940).
///
/// `effective_base` is the explicit base when one was given, the resolved
/// default base when the loader chose one, and `None` when the text came
/// from a diff file or stdin (where `base` is ignored by contract).
///
/// A reported base is always one that resolved, because the field only exists
/// on a successful load: `resolve_effective_base` rejects an unresolvable
/// explicit `--base` before the diff runs, and a base that slipped past that
/// probe still fails in `run_git_diff`.
pub struct LoadedDiff {
    pub text: String,
    pub effective_base: Option<String>,
}

pub fn load_diff(
    root: &Path,
    base: Option<&str>,
    diff_file: Option<&PathBuf>,
    git_timeout: Option<Duration>,
) -> Result<String, String> {
    load_diff_with_effective_base(root, base, diff_file, git_timeout).map(|loaded| loaded.text)
}

pub fn load_diff_with_effective_base(
    root: &Path,
    base: Option<&str>,
    diff_file: Option<&PathBuf>,
    git_timeout: Option<Duration>,
) -> Result<LoadedDiff, String> {
    if let Some(diff_file) = diff_file {
        if diff_file == std::path::Path::new("-") {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| format!("failed to read diff from stdin: {err}"))?;
            return Ok(LoadedDiff {
                text: buffer,
                effective_base: None,
            });
        }
        let text = std::fs::read_to_string(diff_file)
            .map_err(|err| format!("failed to read diff file {}: {err}", diff_file.display()))?;
        return Ok(LoadedDiff {
            text,
            effective_base: None,
        });
    }

    // #3952: a missing root must fail as a missing root, not as an
    // unresolvable base. Default-base resolution would otherwise turn a
    // typo'd --root into "could not resolve a default base", misdirecting
    // the caller toward --base.
    if !root.is_dir() {
        return Err(format!(
            "repository root {} does not exist or is not a directory",
            root.display()
        ));
    }

    warn_if_git_operation_in_progress(root, git_timeout);

    let base = resolve_effective_base(root, base, git_timeout)?;

    let text = run_git_diff(
        root,
        &format!("{base}...HEAD"),
        &["--no-ext-diff", "--submodule=short", "--unified=0"],
        git_timeout,
    )?;
    Ok(LoadedDiff {
        text,
        effective_base: Some(base),
    })
}

/// Load the diff from `base` to the live working tree.
///
/// This is the explicit `ripr check --worktree` path: it includes committed
/// changes since `base` plus staged and unstaged tracked edits. Untracked files
/// are intentionally not included by plain `git diff <base>`.
pub fn load_worktree_diff(
    root: &Path,
    base: Option<&str>,
    git_timeout: Option<Duration>,
) -> Result<String, String> {
    load_worktree_diff_with_effective_base(root, base, git_timeout).map(|loaded| loaded.text)
}

pub fn load_worktree_diff_with_effective_base(
    root: &Path,
    base: Option<&str>,
    git_timeout: Option<Duration>,
) -> Result<LoadedDiff, String> {
    warn_if_git_operation_in_progress(root, git_timeout);

    let base = resolve_effective_base(root, base, git_timeout)?;

    let text = run_git_diff(root, &base, &["--submodule=short"], git_timeout)?;
    Ok(LoadedDiff {
        text,
        effective_base: Some(base),
    })
}

/// Resolve the base ref the diff will actually run against, which is also
/// the value reported as [`LoadedDiff::effective_base`].
///
/// RIPR-SPEC-0084: an explicit `--base` is never substituted. It is only
/// verified, so an unresolvable ref fails in ripr's own voice — naming the ref
/// the user chose and saying the analysis did not run — instead of reaching
/// `git diff` and surfacing git's `ambiguous argument` usage advice, which
/// recommends `--` path separation for a mistake the user did not make. The
/// zero-config path (no `--base`) still resolves the repository's real default
/// branch below.
///
/// Both of those failures ask [`not_a_work_tree`] first, because neither
/// names the right thing when the root is not a repository: no ref resolves
/// there, so blaming the chosen ref or the default-base search sends the user
/// to a repair that cannot work. When that probe answers, its message replaces
/// theirs; otherwise they stand.
///
/// The probe is evidence, not an assumption: only a `rev-parse` that actually
/// ran and reported the ref absent produces the named failure above. When the
/// probe cannot complete at all — the spawn fails, or it exceeds `git_timeout`
/// — this returns the base unverified and `run_git_diff` decides, exactly as
/// before this check existed.
///
/// That fallback is deliberately unconditional about *why* the probe failed,
/// so it also carries the case where the ref really is absent but nothing
/// could establish it. Git's raw `ambiguous argument` advice can therefore
/// still reach the user on that path; the trade is that a probe which never
/// ran is never allowed to assert a bad ref, and an unusable root keeps
/// producing the `failed to run git diff: ...` text that the `context` and
/// `explain` invalid-root contract pins.
fn resolve_effective_base(
    root: &Path,
    base: Option<&str>,
    git_timeout: Option<Duration>,
) -> Result<String, String> {
    let Some(explicit) = base else {
        return resolve_default_base(root, git_timeout)
            .map_err(|err| not_a_work_tree(root, git_timeout).unwrap_or(err));
    };

    let commit = format!("{explicit}^{{commit}}");
    match git_ref_output(root, &commit, git_timeout) {
        Some(output) if !output.status.success() => Err(not_a_work_tree(root, git_timeout)
            .unwrap_or_else(|| {
                format!(
                    "the base `{explicit}` does not resolve to a commit (the analysis did not \
                     run). Fetch the ref (for example `git fetch origin`) or pass `--base <ref>` \
                     for a ref this repository has."
                )
            })),
        _ => Ok(explicit.to_string()),
    }
}

/// The accurate failure when no base could resolve because `root` is not a Git
/// work tree, or `None` when it is one.
///
/// Every base failure above reads as a ref problem and sends the user to
/// `git fetch` or to a different `--base`. Outside a repository neither repair
/// applies: no ref can resolve there, so `git fetch origin` fails for the same
/// reason the base did. Only this probe tells the two apart, and it runs on the
/// failure path alone, so the ordinary run still costs one `rev-parse`.
///
/// It is evidence on the same terms as the base probe: `None` when the command
/// could not run at all, because a probe that never ran may not assert that a
/// directory is not a repository any more than it may assert a ref is absent.
/// `--is-inside-work-tree` prints `true` only inside a work tree, so a run that
/// printed anything else — or failed, which is what it does outside a
/// repository — is the case this names.
fn not_a_work_tree(root: &Path, git_timeout: Option<Duration>) -> Option<String> {
    let output = crate::git::run_git_output_with_deadline(
        root,
        &["rev-parse", "--is-inside-work-tree"],
        git_timeout,
    )
    .ok()?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
        return None;
    }
    Some(format!(
        "`{}` is not inside a Git work tree (the analysis did not run). `ripr check` diffs \
         committed history, so run it from inside your repository, or pass `--root <path>` \
         pointing at one. For a repository-free scan of the current sources, use \
         `ripr check --root . --format repo-exposure-md`.",
        root.display()
    ))
}

/// Resolve the best available base ref for `ripr check` when none was
/// explicitly given by the caller.
///
/// Tries in order, verifying each candidate with `git rev-parse --verify`:
/// 1. `git symbolic-ref --quiet refs/remotes/origin/HEAD` — the remote's own
///    default branch pointer (works for `master`-default, renamed, etc.).
/// 2. `origin/main` — common explicit remote-tracking fallback.
/// 3. `origin/master` — common explicit remote-tracking fallback.
/// 4. `main` — local branch fallback.
/// 5. `master` — local branch fallback.
///
/// Returns `Err` with a named, actionable message when none of the above
/// resolves (e.g. bare repo, no commits, no remote, detached with no
/// branches). The message does NOT claim an empty analysis result — it
/// explicitly says "could not resolve a base (the analysis did not run)".
fn resolve_default_base(root: &Path, git_timeout: Option<Duration>) -> Result<String, String> {
    // Step 1: ask the remote itself what its default branch is.
    if let Some(remote_head) = git_symbolic_ref_quiet(root, "refs/remotes/origin/HEAD", git_timeout)
    {
        // symbolic-ref returns e.g. "refs/remotes/origin/master"; convert to
        // the tracking ref form "origin/master".
        if let Some(stripped) = remote_head.strip_prefix("refs/remotes/") {
            let candidate = stripped.to_string();
            if git_ref_exists(root, &candidate, git_timeout) {
                return Ok(candidate);
            }
        }
    }

    // Steps 2-5: explicit fallbacks verified with rev-parse.
    for candidate in &["origin/main", "origin/master", "main", "master"] {
        if git_ref_exists(root, candidate, git_timeout) {
            return Ok((*candidate).to_string());
        }
    }

    // Fail closed: nothing resolves — emit a named, actionable message.
    // This is distinct from "analyzed and found nothing": the analysis did
    // not run because there was no base to diff against.
    Err(
        "could not resolve a default base (no origin/main, origin/master, or local main/master \
         found). Pass `--base <ref>` to diff against a specific ref, or \
         `--root . --format repo-exposure-md` for a full-repo scan."
            .to_string(),
    )
}

/// Resolve the diff loader's default base for `root` together with the
/// commit it currently points at, in one authority call.
///
/// This is the export for consumers that need the loader's default-base
/// decision *and* its commit identity without re-running the candidate
/// search — the LSP refresh Git-input record (#2261, RIPR-SPEC-0142
/// amendment) resolves the default base once per refresh through this helper
/// so default-base workspaces dedup on the same commit authority as the
/// explicit-base path. Returns `(effective base ref, resolved commit)`, or
/// the same named, actionable error as the default-base candidate search
/// when nothing resolves; an unresolvable workspace is never fabricated
/// into an empty commit identity.
pub fn resolve_default_base_commit(
    root: &Path,
    git_timeout: Option<Duration>,
) -> Result<(String, String), String> {
    let base = resolve_default_base(root, git_timeout)?;
    let commit = resolve_base_commit(root, Some(&base), git_timeout).ok_or_else(|| {
        format!(
            "could not resolve a commit for the default base {base} (the analysis did not run). \
             Pass `--base <ref>` to diff against a specific ref."
        )
    })?;
    Ok((base, commit))
}

/// Run `git symbolic-ref --quiet <refname>` and return the target on success.
/// Returns `None` when the ref does not exist or is not symbolic (exit ≠ 0),
/// and — fail-closed (#2303) — when the invocation cannot complete within
/// `git_timeout` (a timed-out probe is never mistaken for a resolved ref).
fn git_symbolic_ref_quiet(
    root: &Path,
    refname: &str,
    git_timeout: Option<Duration>,
) -> Option<String> {
    let output = crate::git::run_git_output_with_deadline(
        root,
        &["symbolic-ref", "--quiet", refname],
        git_timeout,
    )
    .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Return `true` when `git rev-parse --verify --quiet <refname>` succeeds,
/// meaning the ref genuinely exists in the repository.
fn git_ref_exists(root: &Path, refname: &str, git_timeout: Option<Duration>) -> bool {
    git_ref_output(root, refname, git_timeout).is_some_and(|out| out.status.success())
}

fn git_ref_output(
    root: &Path,
    refname: &str,
    git_timeout: Option<Duration>,
) -> Option<std::process::Output> {
    crate::git::run_git_output_with_deadline(
        root,
        &["rev-parse", "--verify", "--quiet", refname],
        git_timeout,
    )
    .ok()
}

/// Resolve a requested base to the commit that the next analysis will use.
///
/// Tracking the commit rather than only the ref name keeps moving refs such as
/// `origin/main` from being mistaken for the same analysis input after they
/// advance. An unresolved ref remains `None`; the analysis path will report
/// the named base failure instead of manufacturing a commit identity.
pub fn resolve_base_commit(
    root: &Path,
    base: Option<&str>,
    git_timeout: Option<Duration>,
) -> Option<String> {
    let base = base?;
    let commit = format!("{base}^{{commit}}");
    let output = git_ref_output(root, &commit, git_timeout)?;
    if !output.status.success() {
        return None;
    }
    let commit = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!commit.is_empty()).then_some(commit)
}

pub fn load_diff_range(root: &Path, base: &str, head: &str) -> Result<String, String> {
    // CLI-only range path (#1921 migration scope note): no deadline is
    // threaded here yet, so the invocation stays unbounded like the
    // pre-#2303 behavior.
    run_git_diff(
        root,
        &format!("{base}...{head}"),
        &["--unified=0", "--no-ext-diff", "--submodule=short"],
        None,
    )
}

/// PR-evidence range path (issue #3930): the same pinned presentation as
/// the analysis loaders, with `--binary` as the caller extra (the packet
/// artifact keeps binary hunks) and three context lines (the pre-#3930
/// `PR_DIFF` presentation was Git's three-line default). No
/// `--submodule=short`: submodule rendering stays exactly as the
/// PR-evidence path produced it, so ordinary repositories see
/// byte-identical `PR_DIFF`. The packet artifact records evidence, so the
/// decode stays strict like the pre-#3930 helper: non-UTF-8 stdout is a
/// named error, never silently recorded with replacement characters. Like
/// `load_diff_range`, no deadline is threaded.
pub fn load_pr_evidence_diff_range(root: &Path, base: &str, head: &str) -> Result<String, String> {
    let bytes = run_git_diff_bytes(root, &format!("{base}...{head}"), &["--binary"], "3", None)?;
    String::from_utf8(bytes).map_err(|err| format!("packet diff is not valid UTF-8: {err}"))
}

/// Return `true` when the working tree at `root` has uncommitted changes to
/// tracked source files (staged or unstaged).
///
/// Runs `git status --porcelain -- .` and treats any non-untracked output line
/// as a change. The pathspec keeps parent-repo changes outside `root` from
/// leaking into nested workspace checks. Untracked files are intentionally
/// excluded because `git diff <base>` does not include them unless the user
/// stages them.
/// Fail-closed: if git cannot be run or the directory is not a git repo,
/// returns `false` (does NOT fabricate a disclosure) — but the probe failure
/// is named on stderr so a broken git install is not mistaken for a clean
/// tree (#2074).
///
/// RIPR-SPEC-0112: used to disclose when `--base` analyzed committed history
/// while uncommitted working-tree changes were silently excluded.
pub fn working_tree_has_tracked_changes(root: &Path) -> bool {
    match working_tree_probe(root) {
        WorkingTreeProbe::Dirty => true,
        WorkingTreeProbe::Clean => false,
        // Fail-closed on disclosure (unchanged): a probe error never
        // fabricates a positive "uncommitted changes exist" signal. But the
        // error is no longer silent (#2074): name the probe failure so a
        // broken git install is not mistaken for a clean tree.
        WorkingTreeProbe::Error(reason) => {
            eprintln!("{}", working_tree_probe_error_warning(&reason));
            false
        }
    }
}

/// The stderr warning for a failed working-tree probe (#2074). Pure so the
/// exact phrasings ("git could not be run", "git status exited with") are
/// unit-testable without capturing stderr.
fn working_tree_probe_error_warning(reason: &str) -> String {
    format!(
        "ripr: working-tree change probe failed ({reason}); treating the tree as \
         unchanged — the uncommitted-changes disclosure may be incomplete"
    )
}

/// Outcome of the working-tree change probe (#2074): a clean tree is
/// distinguishable from a failed probe.
#[derive(Debug)]
enum WorkingTreeProbe {
    Dirty,
    Clean,
    Error(String),
}

fn working_tree_probe(root: &Path) -> WorkingTreeProbe {
    let result = Command::new("git")
        .args(["status", "--porcelain", "--", "."])
        .current_dir(root)
        .output();
    match result {
        Ok(out) if out.status.success() => {
            if String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|line| !line.starts_with("??"))
            {
                WorkingTreeProbe::Dirty
            } else {
                WorkingTreeProbe::Clean
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let detail = stderr.lines().next().unwrap_or("unknown git error");
            WorkingTreeProbe::Error(format!("git status exited with {}: {detail}", out.status))
        }
        Err(err) => WorkingTreeProbe::Error(format!("git could not be run: {err}")),
    }
}

/// Disclose repository operation state before a live git diff is analyzed.
///
/// During an interrupted rebase, merge, or cherry-pick, the worktree can
/// contain conflict markers; a rebase may also leave `HEAD` at an ephemeral
/// replay commit. The diff may still be loadable, but presenting its findings
/// without this context would make the result look more authoritative than
/// the input warrants. The probe is fail-closed: an unavailable git invocation
/// does not fabricate an in-progress operation.
fn warn_if_git_operation_in_progress(root: &Path, git_timeout: Option<Duration>) {
    if let Some(operation) = git_operation_in_progress(root, git_timeout) {
        eprintln!("{}", git_operation_warning(operation));
    }
}

/// Return the first active Git operation marker, if one can be resolved.
///
/// `git rev-parse --git-path` is intentional instead of `root/.git/<marker>`:
/// linked worktrees may use a `.git` file and Git can place operation state in
/// a worktree-specific git directory.
fn git_operation_in_progress(root: &Path, git_timeout: Option<Duration>) -> Option<GitOperation> {
    let output = crate::git::run_git_output_with_deadline(
        root,
        &[
            "rev-parse",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
            "--git-path",
            "MERGE_HEAD",
            "--git-path",
            "CHERRY_PICK_HEAD",
        ],
        git_timeout,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }

    let operations = [
        GitOperation::RebaseMerge,
        GitOperation::RebaseApply,
        GitOperation::Merge,
        GitOperation::CherryPick,
    ];
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .zip(operations)
        .find_map(|(path, operation)| {
            let path = path.trim();
            (!path.is_empty() && root.join(path).exists()).then_some(operation)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitOperation {
    RebaseMerge,
    RebaseApply,
    Merge,
    CherryPick,
}

fn git_operation_warning(operation: GitOperation) -> String {
    let (operation, context) = match operation {
        GitOperation::RebaseMerge | GitOperation::RebaseApply => (
            "rebase",
            "HEAD may identify an ephemeral replay commit and the working tree may contain conflict markers",
        ),
        GitOperation::Merge => ("merge", "the working tree may contain conflict markers"),
        GitOperation::CherryPick => (
            "cherry-pick",
            "the working tree may contain conflict markers",
        ),
    };
    format!("ripr: git repository is mid-{operation}; {context}. Results may be distorted.")
}

fn run_git_diff(
    root: &Path,
    range: &str,
    extra_args: &[&str],
    git_timeout: Option<Duration>,
) -> Result<String, String> {
    // Analysis loaders consume source-coordinate patches: zero context
    // lines stay the assembly default.
    run_git_diff_with_unified(root, range, extra_args, "0", git_timeout)
}

fn run_git_diff_with_unified(
    root: &Path,
    range: &str,
    extra_args: &[&str],
    unified: &str,
    git_timeout: Option<Duration>,
) -> Result<String, String> {
    // Analysis decodes lossy (unchanged): coordinates come from the
    // C-quoted path contract above, and hunk bodies are parsed, not
    // recorded as evidence.
    Ok(String::from_utf8_lossy(&run_git_diff_bytes(
        root,
        range,
        extra_args,
        unified,
        git_timeout,
    )?)
    .into_owned())
}

fn run_git_diff_bytes(
    root: &Path,
    range: &str,
    extra_args: &[&str],
    unified: &str,
    git_timeout: Option<Duration>,
) -> Result<Vec<u8>, String> {
    // Delegate the spawn to the shared git authority (#1921, #2303), which
    // spawns with `current_dir(root)`. A missing root never reaches the
    // spawn: `load_diff_with_effective_base` rejects non-directories up
    // front (#3952), so the wrap arm below only reproduces the
    // `failed to run git diff: ...` text for other invocation failures.
    // The named timeout and cancellation errors pass through
    // unwrapped so the LSP refresh path can match them; the non-zero-exit
    // text below stays byte-identical.
    // `core.quotePath=true` pins the diff input contract (#3601): with the
    // user's `quotePath=false`, non-UTF-8 path bytes reached the lossy
    // stdout decode below and distinct file names collapsed onto one
    // U+FFFD string before any analysis saw them. Git's C-quoting keeps
    // those paths ASCII (`"src/pricing_\376.rs"`), so they survive the
    // decode distinct and the C-quoted parser form applies. ASCII-only
    // paths are unaffected, so existing fixtures and goldens see no
    // change.
    let mut args: Vec<&str> = vec!["-c", "core.quotePath=true", "diff"];
    args.extend_from_slice(extra_args);
    // Analysis consumes source-coordinate patches, not human diff views.
    // Pin every caller, including worktree mode: helpers can suppress real
    // changes, textconv can invent source coordinates, and ambient context
    // can expand a one-line edit into a full-file payload (#3850). Git's
    // source/destination prefixes are also parser identity syntax: ambient
    // diff.noprefix can remove them, so a real path such as b/identity.rs
    // can collapse onto identity.rs after the parser strips the expected b/
    // side prefix (#4086). (diff.mnemonicPrefix only rewrites worktree/index
    // comparisons, never the <base>...HEAD range form issued here.) Append canonical
    // side prefixes after caller extras so neither repository config nor a
    // conflicting presentation extra can change file identity.
    // The context-line count is the one caller-selected presentation knob
    // (the appended pin wins over any same-named extra): analysis takes zero,
    // the PR-evidence packet takes three.
    let unified_arg = format!("--unified={unified}");
    args.extend_from_slice(&[
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        unified_arg.as_str(),
        "--inter-hunk-context=0",
    ]);
    args.push(range);
    let output = match crate::git::run_git_output_with_deadline(root, &args, git_timeout) {
        Ok(output) => output,
        Err(err)
            if crate::git::is_git_invocation_timeout(&err)
                || crate::analysis::cancellation::is_cancellation_error(&err) =>
        {
            return Err(err);
        }
        Err(err) => return Err(format!("failed to run git diff: {err}")),
    };
    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(output.stdout)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Test asserts an expected error variant via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Best-effort temp-dir teardown. The `io::Result` is matched with `if let`
    /// so a `#[must_use]` cleanup failure is an explicit ignore.
    fn ignore_remove_dir_all(path: &Path) {
        if let Ok(()) = fs::remove_dir_all(path) {}
    }

    #[test]
    fn load_diff_from_file_returns_content() -> std::io::Result<()> {
        let dir = unique_fixture_root("load-diff-test")?;
        ignore_remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        let diff_file = dir.join("test.diff");
        fs::write(&diff_file, "test content")?;

        let result = load_diff(&dir, None, Some(&diff_file), None);
        assert_eq!(result.as_deref(), Ok("test content"));

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn given_ambient_diff_prefix_settings_when_range_loaded_then_repository_paths_keep_identity()
    -> std::io::Result<()> {
        // #4086: the parser strips Git's ordinary a/ and b/ side prefixes.
        // Without explicit loader pins, diff.noprefix=true turns b/identity.rs
        // into a marker that is indistinguishable from identity.rs after that
        // strip. diff.mnemonicPrefix is deliberately not in this matrix: it
        // only rewrites worktree/index comparisons, so a <base>...HEAD range
        // diff keeps canonical prefixes and the leg could never discriminate.
        let dir = unique_fixture_root("diff-side-prefix-identity")?;
        init_git_repo(&dir, "main")?;
        fs::create_dir_all(dir.join("b"))?;
        fs::write(dir.join("identity.rs"), "pub fn outer() -> u32 { 1 }\n")?;
        fs::write(
            dir.join("b").join("identity.rs"),
            "pub fn nested() -> u32 { 2 }\n",
        )?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "base paths", "--quiet"])?;

        fs::write(dir.join("identity.rs"), "pub fn outer() -> u32 { 10 }\n")?;
        fs::write(
            dir.join("b").join("identity.rs"),
            "pub fn nested() -> u32 { 20 }\n",
        )?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "change paths", "--quiet"])?;

        {
            let (setting, value) = ("diff.noprefix", "true");
            run_git_checked(&dir, &["config", setting, value])?;

            let raw = Command::new("git")
                .args(["diff", "HEAD~1...HEAD"])
                .current_dir(&dir)
                .output()?;
            assert!(
                raw.status.success(),
                "{setting} raw-control git diff failed: {}",
                String::from_utf8_lossy(&raw.stderr)
            );
            let raw = String::from_utf8_lossy(&raw.stdout);
            assert!(
                raw.contains("identity.rs") && raw.contains("b/identity.rs"),
                "{setting} raw control did not contain both fixture paths:\n{raw}"
            );
            assert!(
                !raw.contains("diff --git a/identity.rs b/identity.rs"),
                "{setting} raw control retained canonical prefixes, so the fixture does not discriminate:\n{raw}"
            );

            let diff = load_diff_range(&dir, "HEAD~1", "HEAD").map_err(std::io::Error::other)?;

            assert!(
                diff.contains("diff --git a/identity.rs b/identity.rs"),
                "{setting} must not change the canonical outer-file boundary:\n{diff}"
            );
            assert!(
                diff.contains("diff --git a/b/identity.rs b/b/identity.rs"),
                "{setting} must not change the canonical nested-file boundary:\n{diff}"
            );

            let parsed = super::super::parse::parse_unified_diff(&diff);
            let mut paths: Vec<PathBuf> = parsed.iter().map(|file| file.path.clone()).collect();
            paths.sort();
            assert_eq!(
                paths,
                vec![PathBuf::from("b/identity.rs"), PathBuf::from("identity.rs")],
                "{setting} must not collapse distinct repository paths"
            );

            run_git_checked(&dir, &["config", "--unset", setting])?;
        }

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn shared_diff_authority_overrides_conflicting_prefix_extras() -> std::io::Result<()> {
        // The shared authority appends its identity pins after caller extras.
        // A future caller adding its own presentation flags must not be able to
        // move the parser onto a different side-prefix dialect.
        let dir = unique_fixture_root("diff-side-prefix-extra-precedence")?;
        init_git_repo(&dir, "main")?;
        fs::create_dir_all(dir.join("src"))?;
        fs::write(
            dir.join("src").join("lib.rs"),
            "pub fn value() -> u32 { 1 }\n",
        )?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "base source", "--quiet"])?;

        fs::write(
            dir.join("src").join("lib.rs"),
            "pub fn value() -> u32 { 2 }\n",
        )?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "change source", "--quiet"])?;

        let bytes = run_git_diff_bytes(
            &dir,
            "HEAD~1...HEAD",
            &["--src-prefix=old/", "--dst-prefix=new/"],
            "0",
            None,
        )
        .map_err(std::io::Error::other)?;
        let diff = String::from_utf8(bytes).map_err(std::io::Error::other)?;

        assert!(
            diff.contains("diff --git a/src/lib.rs b/src/lib.rs"),
            "{diff}"
        );
        assert!(diff.contains("--- a/src/lib.rs"), "{diff}");
        assert!(diff.contains("+++ b/src/lib.rs"), "{diff}");
        assert!(!diff.contains("old/src/lib.rs"), "{diff}");
        assert!(!diff.contains("new/src/lib.rs"), "{diff}");

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn given_quote_path_disabled_repo_when_diff_loaded_then_non_ascii_paths_keep_identity()
    -> std::io::Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // #3601: with the repo's `core.quotePath=false`, two distinct
        // non-UTF-8 file names reached the loader's lossy UTF-8 decode as
        // one U+FFFD-collapsed string and their changes merged. The
        // loader pins `core.quotePath=true`, so git's own C-quoting
        // carries all non-ASCII names through the decode: valid UTF-8
        // names reconstruct their on-disk identity, and distinct
        // invalid-byte names stay distinct.
        let dir = unique_fixture_root("diff-quote-path-distinct")?;
        init_git_repo(&dir, "main")?;
        run_git_checked(&dir, &["config", "core.quotePath", "false"])?;
        fs::write(dir.join("base.rs"), "pub fn base() -> u32 { 0 }\n")?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "base", "--quiet"])?;
        let base = {
            let output =
                crate::git::run_git_output_with_deadline(&dir, &["rev-parse", "HEAD"], None)
                    .map_err(std::io::Error::other)?;
            if !output.status.success() {
                return Err(std::io::Error::other("git rev-parse HEAD failed"));
            }
            String::from_utf8(output.stdout)
                .map_err(|err| std::io::Error::other(err.to_string()))?
                .trim()
                .to_string()
        };

        let first = dir.join(OsStr::from_bytes(b"pricing_\xff.rs"));
        let second = dir.join(OsStr::from_bytes(b"pricing_\xfe.rs"));
        fs::write(&first, "pub fn first() -> u32 { 1 }\n")?;
        fs::write(&second, "pub fn second() -> u32 { 2 }\n")?;
        fs::write(dir.join("café.rs"), "pub fn third() -> u32 { 3 }\n")?;
        run_git_checked(&dir, &["add", "-A"])?;
        run_git_checked(&dir, &["commit", "-m", "non-ascii filenames", "--quiet"])?;

        let diff = load_diff_range(&dir, &base, "HEAD").map_err(std::io::Error::other)?;
        let files = super::super::parse::parse_unified_diff(&diff);
        let mut paths: Vec<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
        paths.sort();

        assert_eq!(
            paths.len(),
            3,
            "distinct non-ASCII names must not collapse at decode: {paths:?} from:\n{diff}"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_os_str().as_bytes() == "café.rs".as_bytes()),
            "valid UTF-8 names must decode to their on-disk identity: {paths:?}"
        );
        let invalid: Vec<&PathBuf> = paths
            .iter()
            .filter(|path| {
                path.as_os_str().as_bytes().starts_with(b"pricing_")
                    && path.as_os_str().as_bytes().ends_with(b".rs")
            })
            .collect();
        assert_eq!(
            invalid.len(),
            2,
            "distinct invalid-byte names must stay distinct: {paths:?}"
        );
        assert!(
            invalid
                .iter()
                .any(|path| path.as_os_str().as_bytes() == b"pricing_\xff.rs")
                && invalid
                    .iter()
                    .any(|path| path.as_os_str().as_bytes() == b"pricing_\xfe.rs"),
            "raw invalid bytes must remain distinct in parsed paths: {paths:?}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn given_quote_path_disabled_repo_when_diff_loaded_then_raw_byte_and_mimic_names_stay_distinct()
    -> std::io::Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // #3609: an invalid-byte file name and a valid-UTF-8 name that
        // literally spells the octal escape text (`pricing_\377.rs`) are two
        // distinct files. A string-contract decoder renders both as the same
        // octal residue, so their changes merge in the parser's path-keyed
        // map. Decoding through the path type keeps the raw byte native on
        // Unix, so both identities stay byte-distinct end to end, and valid
        // UTF-8 names keep their on-disk identity.
        let dir = unique_fixture_root("diff-raw-byte-vs-mimic-distinct")?;
        init_git_repo(&dir, "main")?;
        run_git_checked(&dir, &["config", "core.quotePath", "false"])?;
        fs::write(dir.join("base.rs"), "pub fn base() -> u32 { 0 }\n")?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "base", "--quiet"])?;
        let base = {
            let output =
                crate::git::run_git_output_with_deadline(&dir, &["rev-parse", "HEAD"], None)
                    .map_err(std::io::Error::other)?;
            if !output.status.success() {
                return Err(std::io::Error::other("git rev-parse HEAD failed"));
            }
            String::from_utf8(output.stdout)
                .map_err(|err| std::io::Error::other(err.to_string()))?
                .trim()
                .to_string()
        };

        let raw_byte = dir.join(OsStr::from_bytes(b"pricing_\xff.rs"));
        // The mimic's name carries the literal backslash characters, not the
        // escaped byte: `pricing_\377.rs` spelled with ordinary ASCII.
        let mimic = dir.join(OsStr::from_bytes(b"pricing_\\377.rs"));
        fs::write(&raw_byte, "pub fn raw_byte() -> u32 { 1 }\n")?;
        fs::write(&mimic, "pub fn mimic() -> u32 { 2 }\n")?;
        fs::write(dir.join("café.rs"), "pub fn third() -> u32 { 3 }\n")?;
        run_git_checked(&dir, &["add", "-A"])?;
        run_git_checked(
            &dir,
            &[
                "commit",
                "-m",
                "raw byte and residue-mimic filenames",
                "--quiet",
            ],
        )?;

        let diff = load_diff_range(&dir, &base, "HEAD").map_err(std::io::Error::other)?;
        let files = super::super::parse::parse_unified_diff(&diff);
        let mut paths: Vec<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
        paths.sort();

        assert_eq!(
            paths.len(),
            3,
            "the raw-byte name and its residue mimic must stay distinct: {paths:?} from:\n{diff}"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_os_str().as_bytes() == b"pricing_\xff.rs"),
            "the raw-byte name must keep byte 0xFF in its decoded identity: {paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_os_str().as_bytes() == b"pricing_\\377.rs"),
            "the literal mimic name must keep its own text identity: {paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.as_os_str().as_bytes() == "café.rs".as_bytes()),
            "valid UTF-8 names must keep their on-disk identity: {paths:?}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn load_diff_with_missing_file_returns_error() -> std::io::Result<()> {
        let result = load_diff(
            &std::env::current_dir()?,
            None,
            Some(&PathBuf::from("/nonexistent/path/to/file")),
            None,
        );
        result.expect_err("expected diff load to fail for missing file");
        Ok(())
    }

    // RIPR-SPEC-0084: resolution tests using real temp git repos.

    /// Helper: initialise a git repo, create an initial commit, and return the
    /// repo root. Uses `--initial-branch` if available; falls back to renaming
    /// the default branch via `git symbolic-ref`.
    /// A unique fixture root, so two concurrent or overlapping suite runs cannot
    /// share a git repo.
    ///
    /// Fixed names are unsafe here beyond the obvious collision: the cleanup
    /// ignores a failed `fs::remove_dir_all`, and on Windows that cannot delete
    /// a git object store whose files are read-only, so a half-deleted repo
    /// would be silently reused by the next run.
    fn unique_fixture_root(name: &str) -> std::io::Result<PathBuf> {
        let dir = unique_fixture_path(name);
        ignore_remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// A unique fixture path that is **not** created, for fixtures that need the
    /// path to be something other than a directory.
    fn unique_fixture_path(name: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ripr-{name}-{}-{stamp}-{counter}",
            std::process::id()
        ))
    }

    /// Run one git command and fail with its stderr if it does not succeed.
    ///
    /// `Command::output()?` only propagates a *spawn* failure. A git command
    /// that runs and exits nonzero must not be ignored: the fixture would return
    /// `Ok(())` having produced a repo with no commit and no refs, and every
    /// assertion downstream would then fail for a reason unrelated to what it
    /// tests.
    fn run_git_checked(dir: &Path, args: &[&str]) -> std::io::Result<()> {
        let output = Command::new("git").args(args).current_dir(dir).output()?;
        if output.status.success() {
            return Ok(());
        }
        Err(std::io::Error::other(format!(
            "git {args:?} in {} failed with {:?}: {}{}",
            dir.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )))
    }

    fn init_git_repo(dir: &Path, branch: &str) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        // Try --initial-branch first (git >= 2.28); fall back to symbolic-ref.
        if run_git_checked(dir, &["init", "--initial-branch", branch]).is_err() {
            run_git_checked(dir, &["init"])?;
            run_git_checked(
                dir,
                &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
            )?;
        }
        run_git_checked(dir, &["config", "user.email", "test@example.com"])?;
        run_git_checked(dir, &["config", "user.name", "Test"])?;
        // Signing would make the fixture depend on host gpg configuration.
        run_git_checked(dir, &["config", "commit.gpgsign", "false"])?;
        // Create an initial commit so rev-parse works.
        fs::write(dir.join("README"), "init")?;
        run_git_checked(dir, &["add", "."])?;
        run_git_checked(dir, &["commit", "-m", "init"])?;
        Ok(())
    }

    #[test]
    fn explicit_base_resolves_to_exact_commit_and_unknown_refs_fail_closed() -> std::io::Result<()>
    {
        let dir = unique_fixture_root("resolve-exact-base")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let expected = String::from_utf8(
            git_ref_output(&dir, "HEAD", None)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "git HEAD was not resolved")
                })?
                .stdout,
        )
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
        .trim()
        .to_string();

        assert_eq!(
            resolve_base_commit(&dir, Some("HEAD"), None).as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(resolve_base_commit(&dir, Some("missing-base"), None), None);

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolve_default_base_uses_origin_master_when_symbolic_ref_points_there()
    -> std::io::Result<()> {
        // Simulates a repo whose remote default branch is "master" (not "main").
        // We create a local repo, then set refs/remotes/origin/HEAD to point at
        // refs/remotes/origin/master, and create that ref.
        let dir = unique_fixture_root("resolve-base-origin-master")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "master")?;
        // Create the remote-tracking ref manually (simulates a fetched remote).
        Command::new("git")
            .args(["update-ref", "refs/remotes/origin/master", "HEAD"])
            .current_dir(&dir)
            .output()?;
        Command::new("git")
            .args([
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/master",
            ])
            .current_dir(&dir)
            .output()?;

        let result = resolve_default_base(&dir, None);
        assert_eq!(
            result.as_deref(),
            Ok("origin/master"),
            "expected origin/master resolution via symbolic-ref"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn load_diff_with_effective_base_reports_the_base_the_loader_used() -> std::io::Result<()> {
        // #3940: the loader is the single authority for which base produced
        // the diff — explicit, resolved default, or none for diff files.
        let dir = unique_fixture_root("effective-base-reporting")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "master")?;
        run_git_checked(&dir, &["update-ref", "refs/remotes/origin/master", "HEAD"])?;
        run_git_checked(
            &dir,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/master",
            ],
        )?;

        let resolved =
            load_diff_with_effective_base(&dir, None, None, None).map_err(std::io::Error::other)?;
        assert_eq!(
            resolved.effective_base.as_deref(),
            Some("origin/master"),
            "a scope-less load must report the resolved default base"
        );
        let explicit = load_diff_with_effective_base(&dir, Some("master"), None, None)
            .map_err(std::io::Error::other)?;
        assert_eq!(
            explicit.effective_base.as_deref(),
            Some("master"),
            "an explicit base must be reported as-is"
        );

        let diff_file = dir.join("change.diff");
        fs::write(&diff_file, "diff --git a/x b/x\n")?;
        let from_file = load_diff_with_effective_base(&dir, Some("master"), Some(&diff_file), None)
            .map_err(std::io::Error::other)?;
        assert_eq!(
            from_file.effective_base, None,
            "a diff file ignores base, so none is reported"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolve_default_base_uses_local_main_when_no_remote() -> std::io::Result<()> {
        // Simulates a fresh git init with no remote; local branch is "main".
        let dir = unique_fixture_root("resolve-base-local-main")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;
        // Confirm no remote refs exist.
        let refs_remote = dir.join(".git").join("refs").join("remotes");
        ignore_remove_dir_all(&refs_remote);
        let result = resolve_default_base(&dir, None);
        assert_eq!(
            result.as_deref(),
            Ok("main"),
            "expected local main fallback when no remote"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolve_default_base_returns_named_error_when_nothing_resolves() -> std::io::Result<()> {
        // Simulates a bare repo with no commits and no remote refs. We create
        // a temp dir, run git init, but do NOT create any commits or refs.
        let dir = unique_fixture_root("resolve-base-no-base")?;
        ignore_remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Command::new("git").arg("init").current_dir(&dir).output()?;
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&dir)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&dir)
            .output()?;
        // No commit, no remote refs, no branches — nothing resolves.

        let result = resolve_default_base(&dir, None);
        let err = result.expect_err("expected a named error when no base resolves");
        assert!(
            err.contains("could not resolve a default base"),
            "expected named actionable message, got: {err}"
        );
        assert!(
            err.contains("--base <ref>"),
            "expected --base guidance in message, got: {err}"
        );
        assert!(
            err.contains("--format repo-exposure-md"),
            "expected --format repo-exposure-md guidance in message, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn explicit_base_is_used_as_is_without_resolution() -> std::io::Result<()> {
        // When an explicit base is given, load_diff does not attempt resolution.
        // A nonexistent explicit base must fail naming the ref the user chose,
        // never the auto-resolve message (that would mean we silently
        // substituted it).
        let dir = unique_fixture_root("explicit-base-no-subst")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let result = load_diff(&dir, Some("nonexistent-branch-xyz"), None, None);
        let err = result.expect_err("expected error for nonexistent explicit base");
        assert!(
            !err.contains("could not resolve a default base"),
            "explicit base must not trigger auto-resolve fallback; got: {err}"
        );
        assert!(
            err.contains("nonexistent-branch-xyz") || err.contains("git diff failed"),
            "expected error naming the chosen ref, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn unresolvable_explicit_base_reports_the_ref_instead_of_git_usage_advice()
    -> std::io::Result<()> {
        // The user-facing defect: `ripr check --base origin/main` in a repo with
        // no `origin` used to print git's raw `ambiguous argument` text, whose
        // remedy ("use `--` to separate paths from revisions") addresses a
        // mistake the user did not make. The failure now names the ref, says the
        // analysis did not run, and gives the two real next actions.
        let dir = unique_fixture_root("explicit-base-named-failure")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let err = load_diff(&dir, Some("origin/main"), None, None)
            .expect_err("expected error for a base with no origin remote");

        assert!(
            err.contains("`origin/main`"),
            "expected the chosen ref to be named, got: {err}"
        );
        assert!(
            err.contains("does not resolve to a commit"),
            "expected the named non-resolution state, got: {err}"
        );
        assert!(
            err.contains("the analysis did not run"),
            "expected an explicit did-not-run boundary so an unresolvable base \
             is never read as an empty result, got: {err}"
        );
        assert!(
            err.contains("--base <ref>"),
            "expected the next action, got: {err}"
        );
        // Discriminator: git's usage advice for a different mistake must be gone.
        assert!(
            !err.contains("ambiguous argument") && !err.contains("separate paths from revisions"),
            "raw git usage advice must not reach the user, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolvable_explicit_base_still_loads_its_diff() -> std::io::Result<()> {
        // Negative control for the preflight above: a base that does resolve is
        // analyzed exactly as before, so the new check cannot pass by rejecting
        // every explicit base.
        let dir = unique_fixture_root("explicit-base-resolvable")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        fs::write(dir.join("src.rs"), "pub fn added() -> i32 { 1 }\n")?;
        run_git_checked(&dir, &["add", "."])?;
        run_git_checked(&dir, &["commit", "-m", "add src"])?;

        let loaded = load_diff(&dir, Some("HEAD~1"), None, None);
        assert!(
            loaded.as_ref().is_ok_and(|diff| diff.contains("src.rs")),
            "expected a resolvable explicit base to analyze the changed file, got: {loaded:?}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn worktree_load_reports_an_unresolvable_explicit_base_by_name() -> std::io::Result<()> {
        // `--worktree` shares the same base authority, so it shares the fix.
        let dir = unique_fixture_root("worktree-base-named-failure")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let err = load_worktree_diff(&dir, Some("origin/main"), None)
            .expect_err("expected error for a worktree base with no origin remote");
        assert!(
            err.contains("`origin/main`") && err.contains("does not resolve to a commit"),
            "expected the named non-resolution state, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn outside_a_work_tree_names_the_missing_repository_not_a_missing_ref() -> std::io::Result<()> {
        // A root that is not a usable work tree fails every base, and the ref
        // messages send the user to `git fetch origin` or to a different
        // `--base`. Neither repair applies there: `git fetch` fails for the
        // same reason the base did. Reported against a plain directory, where
        // `--base origin/main` printed git's whole `--no-index` usage — 129
        // lines of it — with `ripr:` in front.
        //
        // The two fixtures are the two ways the probe establishes it, and
        // neither depends on where the temp directory happens to live: a bare
        // repository makes `--is-inside-work-tree` print `false`, and an
        // invalid gitfile makes it exit nonzero. A plain directory outside any
        // checkout takes the second path, so it is the second fixture's case.
        let bare = unique_fixture_root("no-work-tree-bare")?;
        run_git_checked(&bare, &["init", "--bare", "--quiet", "."])?;
        let gitfile = unique_fixture_root("no-work-tree-gitfile")?;
        fs::write(gitfile.join(".git"), "not a gitfile\n")?;

        for dir in [&bare, &gitfile] {
            // Assert the fixture before reading anything into the message: a
            // root that is a work tree would make this pass for another reason.
            let probe = crate::git::run_git_output_with_deadline(
                dir,
                &["rev-parse", "--is-inside-work-tree"],
                None,
            )
            .map_err(std::io::Error::other)?;
            assert!(
                !(probe.status.success()
                    && String::from_utf8_lossy(&probe.stdout).trim() == "true"),
                "fixture {} is a work tree, so this test proves nothing",
                dir.display()
            );

            for base in [Some("origin/main"), None] {
                let err = load_diff(dir, base, None, None)
                    .expect_err("expected an error outside a work tree");
                assert!(
                    err.contains("not inside a Git work tree"),
                    "expected the repository state to be named for {base:?} in {}, got: {err}",
                    dir.display()
                );
                assert!(
                    err.contains("the analysis did not run"),
                    "expected the did-not-run boundary for {base:?} in {}, got: {err}",
                    dir.display()
                );
                assert!(
                    err.contains("--root <path>"),
                    "expected the next action for {base:?} in {}, got: {err}",
                    dir.display()
                );
                // Discriminators. The ref advice is wrong here, and so is git's
                // own text; neither may reach the user in this state.
                assert!(
                    !err.contains("does not resolve to a commit") && !err.contains("git fetch"),
                    "ref-repair advice must not be given for {base:?} in {}, got: {err}",
                    dir.display()
                );
                assert!(
                    !err.contains("could not resolve a default base"),
                    "default-base search advice must not be given for {base:?} in {}, got: {err}",
                    dir.display()
                );
                assert!(
                    !err.contains("--no-index") && !err.contains("invalid gitfile"),
                    "raw git output must not reach the user for {base:?} in {}, got: {err}",
                    dir.display()
                );
            }
        }

        ignore_remove_dir_all(&bare);
        ignore_remove_dir_all(&gitfile);
        Ok(())
    }

    #[test]
    fn resolve_default_base_commit_returns_the_ref_and_its_commit() -> std::io::Result<()> {
        // #2261 (RIPR-SPEC-0142 amendment): the combined export reports the
        // same base the candidate search picks and the exact commit the
        // analysis will diff against.
        let dir = unique_fixture_root("resolve-default-base-commit")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let expected = String::from_utf8(
            git_ref_output(&dir, "HEAD", None)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "git HEAD was not resolved")
                })?
                .stdout,
        )
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
        .trim()
        .to_string();

        let (base, commit) =
            resolve_default_base_commit(&dir, None).map_err(std::io::Error::other)?;
        assert_eq!(base, "main", "expected the local main fallback");
        assert_eq!(commit, expected, "expected HEAD's exact commit");

        // A workspace with no resolvable default base fails closed with the
        // named error instead of fabricating a commit identity. A repo whose
        // only branch is neither main nor master has no candidate in the
        // loader's default-base search order.
        let bare = unique_fixture_root("resolve-default-base-commit-empty")?;
        ignore_remove_dir_all(&bare);
        init_git_repo(&bare, "trunk")?;
        let err = resolve_default_base_commit(&bare, None)
            .expect_err("expected a named error when no default base resolves");
        assert!(
            err.contains("could not resolve a default base"),
            "expected the candidate-search error, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        ignore_remove_dir_all(&bare);
        Ok(())
    }

    #[test]
    fn working_tree_probe_error_warning_names_both_failure_arms() {
        // #2074 review: the contracted phrasings are unit-testable without
        // capturing stderr.
        let spawn_err = working_tree_probe_error_warning("git could not be run: not a directory");
        assert!(spawn_err.contains("git could not be run"));
        assert!(spawn_err.contains("disclosure may be incomplete"));
        let exit_err =
            working_tree_probe_error_warning("git status exited with exit code: 128: fatal");
        assert!(exit_err.contains("git status exited with"));
    }

    #[test]
    fn git_operation_probe_detects_rebase_merge_and_cherry_pick_markers() -> std::io::Result<()> {
        let dir = unique_fixture_root("git-operation-markers")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        for (marker, expected) in [
            ("rebase-merge", GitOperation::RebaseMerge),
            ("rebase-apply", GitOperation::RebaseApply),
            ("MERGE_HEAD", GitOperation::Merge),
            ("CHERRY_PICK_HEAD", GitOperation::CherryPick),
        ] {
            let marker_path = git_marker_path(&dir, marker)?;
            if marker.ends_with("-merge") || marker.ends_with("-apply") {
                fs::create_dir_all(&marker_path)?;
            } else {
                fs::write(&marker_path, "marker\n")?;
            }

            assert_eq!(
                git_operation_in_progress(&dir, None),
                Some(expected),
                "expected Git marker {marker} to be detected at {}",
                marker_path.display()
            );

            if marker_path.is_dir() {
                fs::remove_dir_all(&marker_path)?;
            } else {
                fs::remove_file(&marker_path)?;
            }
            assert_eq!(git_operation_in_progress(&dir, None), None);
        }

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn git_operation_warning_qualifies_head_and_conflict_markers() {
        let rebase_warning = git_operation_warning(GitOperation::RebaseMerge);
        assert!(rebase_warning.contains("mid-rebase"));
        assert!(rebase_warning.contains("HEAD may identify an ephemeral replay commit"));
        assert!(rebase_warning.contains("working tree may contain conflict markers"));
        assert!(rebase_warning.contains("Results may be distorted"));

        let merge_warning = git_operation_warning(GitOperation::Merge);
        assert!(merge_warning.contains("mid-merge"));
        assert!(!merge_warning.contains("ephemeral replay commit"));

        let cherry_pick_warning = git_operation_warning(GitOperation::CherryPick);
        assert!(cherry_pick_warning.contains("mid-cherry-pick"));
        assert!(!cherry_pick_warning.contains("ephemeral replay commit"));
    }

    fn git_marker_path(dir: &Path, marker: &str) -> std::io::Result<PathBuf> {
        let output = crate::git::run_git_output_with_deadline(
            dir,
            &["rev-parse", "--git-path", marker],
            None,
        )
        .map_err(std::io::Error::other)?;
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "git rev-parse --git-path {marker} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(dir.join(path))
    }

    #[test]
    fn working_tree_probe_distinguishes_error_from_clean() -> std::io::Result<()> {
        // A file (not a directory) as the probe root fails deterministically
        // on every host: spawn errors with "not a directory" (#2074). A plain
        // non-repo temp dir is NOT a portable error case — git walks up to a
        // parent repo when one exists.
        let file = unique_fixture_path("wt-probe-notdir");
        fs::write(&file, "not a directory\n")?;

        match working_tree_probe(&file) {
            WorkingTreeProbe::Error(reason) => {
                // The spawn-failure arm carries the contracted phrasing.
                assert!(
                    reason.contains("git could not be run"),
                    "unexpected probe error: {reason}"
                );
            }
            other => {
                return Err(std::io::Error::other(format!(
                    "expected probe error for a file-as-root, got {other:?}"
                )));
            }
        }
        // The public fn still fails closed to false on a probe error.
        if working_tree_has_tracked_changes(&file) {
            return Err(std::io::Error::other(
                "a failed probe must not report tracked changes",
            ));
        }

        if let Ok(()) = fs::remove_file(&file) {}
        Ok(())
    }

    #[test]
    fn tracked_change_detector_ignores_untracked_only_files() -> std::io::Result<()> {
        let dir = unique_fixture_root("tracked-change-untracked-only")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;
        fs::write(dir.join("scratch.rs"), "fn scratch() {}\n")?;

        if working_tree_has_tracked_changes(&dir) {
            return Err(std::io::Error::other(
                "untracked-only files must not trigger tracked worktree disclosure",
            ));
        }

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn tracked_change_detector_detects_tracked_edit() -> std::io::Result<()> {
        let dir = unique_fixture_root("tracked-change-edit")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;
        fs::write(dir.join("README"), "changed\n")?;

        if !working_tree_has_tracked_changes(&dir) {
            return Err(std::io::Error::other(
                "tracked edits must trigger tracked worktree disclosure",
            ));
        }

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn tracked_change_detector_ignores_parent_repo_changes_outside_root() -> std::io::Result<()> {
        let dir = unique_fixture_root("tracked-change-parent-dirty")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;
        let nested = dir.join("nested-workspace");
        fs::create_dir_all(&nested)?;
        fs::write(dir.join("README"), "changed outside nested root\n")?;

        if working_tree_has_tracked_changes(&nested) {
            return Err(std::io::Error::other(
                "tracked edits outside the requested root must not trigger tracked worktree disclosure",
            ));
        }

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn zero_deadline_diff_load_fails_with_the_named_timeout_error() -> std::io::Result<()> {
        // #2303: a deadline that cannot be met fails before spawning with the
        // named, matchable `git_invocation_timeout` error — the string the
        // LSP refresh path converts into a committed limited snapshot.
        let dir = unique_fixture_root("load-diff-zero-deadline")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let result = load_diff(&dir, Some("HEAD"), None, Some(Duration::ZERO));
        let err = result.expect_err("a zero deadline must fail the diff load");
        assert!(
            crate::git::is_git_invocation_timeout(&err),
            "expected the named git_invocation_timeout error, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn zero_deadline_default_base_diff_load_fails_closed() -> std::io::Result<()> {
        // #2613: the CLI's default-base path must pass its deadline through
        // candidate resolution instead of silently falling back to an
        // unbounded Git probe or fabricating a base.
        let dir = unique_fixture_root("load-diff-default-base-zero-deadline")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let result = load_diff(&dir, None, None, Some(Duration::ZERO));
        let err = result.expect_err("a zero deadline must fail default-base resolution");
        assert!(
            err.contains("could not resolve a default base"),
            "expected fail-closed default-base error, got: {err}"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn zero_deadline_base_probes_fail_closed_instead_of_hanging() -> std::io::Result<()> {
        // #2303: probe-path timeouts degrade to the same fail-closed states
        // as an unresolvable ref — never to a fabricated base or commit.
        let dir = unique_fixture_root("probe-zero-deadline")?;
        ignore_remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        assert_eq!(
            resolve_base_commit(&dir, Some("HEAD"), Some(Duration::ZERO)),
            None
        );
        let err = resolve_default_base_commit(&dir, Some(Duration::ZERO))
            .expect_err("a zero-deadline default-base search must fail closed");
        assert!(
            err.contains("could not resolve a default base"),
            "expected the candidate-search error, got: {err}"
        );

        // The same workspace resolves normally without a deadline, proving
        // the failure above is the deadline, not the fixture.
        assert!(
            resolve_base_commit(&dir, Some("HEAD"), None).is_some(),
            "unbounded probe must resolve HEAD in the same repo"
        );

        ignore_remove_dir_all(&dir);
        Ok(())
    }
}
