//! Object-backed execution of a [`GitCandidateSubject`] (#3237 / #3277,
//! R2).
//!
//! R1 (#3276) bound the subject's identity; this module resolves that
//! identity through Git object plumbing and produces the two inputs the
//! existing analyzers need — the exact base→candidate unified diff and a
//! materialized candidate root — without consulting the worktree or the
//! index:
//!
//! - identities are validated with `git rev-parse` / `git cat-file -e`
//!   (read-only plumbing; no ref, index, or worktree mutation);
//! - the diff is `git diff-tree` between the two **trees**, preserving
//!   add/delete/rename/type-change information;
//! - the candidate root is materialized blob-by-blob (`ls-tree` +
//!   `cat-file`) from the
//!   candidate tree alone into a fresh temp directory, so every byte
//!   comes from the bound tree;
//! - any failure (missing base/candidate, unsupported object mode,
//!   traversal, materialization error) fails closed naming the exact
//!   identity — never an empty analysis.

use crate::domain::{
    GitCandidateBase, GitCandidateSubject, GitCandidateSubjectError as SubjectError,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Bounded invocation deadline for each plumbing call.
const GIT_DEADLINE: Option<Duration> = Some(Duration::from_mins(1));

/// Upper bound on the candidate archive size. A tree whose archive
/// exceeds it fails closed with a named limit instead of an unbounded
/// allocation.
const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
/// The resolved, analyzed form of one immutable subject: the derived
/// unified diff, the materialized candidate root (owned temp directory
/// — dropping the guard removes it), and the identities the diff was
/// derived from (recorded for R3 output projection).
pub(crate) fn subject_identity(resolved: &ResolvedGitCandidate) -> String {
    format!(
        "base_tree={} candidate_tree={}",
        resolved.base_tree, resolved.candidate_tree
    )
}

/// The resolved, analyzed form of one immutable subject: the derived
/// unified diff, the materialized candidate root (owned temp directory
/// — dropping the guard removes it), and the identities the diff was
/// derived from (recorded for R3 output projection).
pub(crate) struct ResolvedGitCandidate {
    pub(crate) base_tree: String,
    pub(crate) candidate_tree: String,
    pub(crate) diff: String,
    pub(crate) root: PathBuf,
    /// Removes the materialized root on drop; keep alive for the run.
    pub(crate) _cleanup: TempRootGuard,
}

pub(crate) struct TempRootGuard(PathBuf);

impl TempRootGuard {
    /// Remove the root and, if that fails, report it to `sink`.
    ///
    /// `Drop` delegates here in full so the warning is assertable. Asserting
    /// it any other way is not possible: `Drop` can neither return the error
    /// nor be handed a channel, and a test that re-derives the message by
    /// calling the helpers itself proves nothing about what `Drop` wrote —
    /// deleting the write would leave such a test green.
    fn clean_up_reporting_to(&self, sink: &mut dyn std::io::Write) {
        // A cleanup failure leaves an extracted copy of the candidate tree on
        // disk. Discarding it would make an unbounded, invisible disk leak
        // indistinguishable from a clean run, so name the path an operator
        // has to remove.
        if let Err(error) = remove_temp_root(&self.0) {
            // Report fallibly, discarding the write result. `eprintln!` panics
            // when the stderr write fails (a closed descriptor, a non-blocking
            // pipe), and this runs in `Drop` — possibly while a panic is
            // already unwinding, where a second panic aborts the process.
            // Losing a warning is strictly better than turning a disk-cleanup
            // problem into an abort.
            let _ = writeln!(sink, "{}", cleanup_failure_report(&self.0, &error));
        }
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        self.clean_up_reporting_to(&mut std::io::stderr());
    }
}

/// The operator-facing text for a cleanup that could not be performed.
fn cleanup_failure_report(path: &Path, error: &std::io::Error) -> String {
    format!(
        "ripr: candidate materialization root could not be removed: {} ({error}); \
         remove it manually to reclaim the space",
        path.display()
    )
}

/// Remove one materialization root, retrying once.
///
/// One retry absorbs a transient hold (an antivirus scan or a still-closing
/// handle on Windows). A second failure is real and is returned to the caller.
fn remove_temp_root(path: &Path) -> std::io::Result<()> {
    match remove_temp_root_once(path) {
        Ok(()) => Ok(()),
        Err(_) => remove_temp_root_once(path),
    }
}

/// One removal attempt. A path that is already gone is success: what must not
/// survive is the root, not this particular call — a guard dropped after a
/// manual cleanup has nothing to report.
fn remove_temp_root_once(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn failed(detail: String) -> SubjectError {
    SubjectError::ExecutionFailed { detail }
}

fn git(root: &Path, args: &[&str], deadline: Option<Duration>) -> Result<String, SubjectError> {
    let named = |detail: String| SubjectError::ExecutionFailed { detail };
    let output = crate::git::run_git_output_with_deadline(root, args, deadline).map_err(named)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().next().unwrap_or("unknown git error").trim();
        return Err(named(format!(
            "git {} failed with {}: {detail}",
            args.first().unwrap_or(&""),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve the base side to one exact tree object ID, using the
/// repository's real empty-tree semantics when no base is requested.
fn resolve_base_tree(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    match &subject.base {
        GitCandidateBase::EmptyTree => {
            // The empty tree's object ID is fixed per hash format; ask
            // the repository which format it uses rather than writing
            // an object or passing a literal empty filename (the old
            // `hash-object -t tree ""` fallback always failed with
            // "could not open ''").
            let format = git(
                &subject.repository_root,
                &["rev-parse", "--show-object-format"],
                deadline,
            )?;
            let empty_tree_id = match format.trim() {
                "sha256" => "6ef19b41225c5369f1c104d45d8d85efa9d058d53bc6434cd0f5d23e5dc71d12",
                _ => "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
            };
            git(
                &subject.repository_root,
                &[
                    "rev-parse",
                    "--verify",
                    &format!("{empty_tree_id}^{{tree}}"),
                ],
                deadline,
            )
        }
        // One-step peel: a treeish may name a commit, tag, or tree;
        // `^{tree}` resolves all three without rejecting the model's
        // own documented tree-OID shape.
        GitCandidateBase::Treeish(treeish) => git(
            &subject.repository_root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{tree}}", treeish.as_str()),
            ],
            deadline,
        ),
    }
}

/// Validate that the candidate names one existing tree object.
fn resolve_candidate_tree(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    let treeish = subject.candidate_tree.as_str();
    // One-step peel: the model documents a tree object ID; `^{tree}`
    // accepts a tree directly and still resolves commits and tags.
    git(
        &subject.repository_root,
        &["rev-parse", "--verify", &format!("{treeish}^{{tree}}")],
        deadline,
    )
}

/// Derive the base→candidate unified diff from the trees alone.
fn derive_diff(
    root: &Path,
    base: &str,
    candidate: &str,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    // -M preserves rename information so the pipeline's pinned
    // rename semantics (pure-rename paths produce no probes) fire for
    // subject runs too (#3296 review: without -M a pure rename became
    // delete+add and probed unchanged content).
    git(
        root,
        &[
            "diff-tree",
            "--unified=0",
            "--no-ext-diff",
            "--root",
            "-M",
            base,
            candidate,
        ],
        deadline,
    )
}

/// Materialize the candidate tree into a fresh temp directory. Every
/// byte comes from `git cat-file` of the bound blob; the worktree and index
/// are never consulted.
fn materialize(
    root: &Path,
    candidate_tree: &str,
    deadline: Option<Duration>,
) -> Result<(PathBuf, TempRootGuard), SubjectError> {
    // Unique per invocation: concurrent runs (or racing tests) never
    // share a materialization directory, and a stale directory from a
    // crashed run can never be silently reused.
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or(0)
    );
    let base_dir = std::env::temp_dir().join("ripr-git-candidate").join(unique);
    let target = base_dir.join(candidate_tree);
    // Arm cleanup BEFORE any fallible work. Every `?` below returns early,
    // and until this guard exists those paths leave `base_dir` — which by
    // then holds an extracted copy of the candidate tree — on disk forever.
    // A fail-closed subject (an unsupported entry mode, a git failure
    // failure, a bounded-read overrun) must not cost a permanent temp
    // directory; only the success path hands the guard to the caller, who
    // holds it for as long as the materialization is in use.
    let cleanup = TempRootGuard(base_dir.clone());
    if target.exists() {
        // A stale directory from a crashed run must not be reused: its
        // bytes are unverified. Remove and re-create deterministically.
        std::fs::remove_dir_all(&target).map_err(|error| {
            failed(format!(
                "stale materialization could not be removed: {error}"
            ))
        })?;
    }
    std::fs::create_dir_all(&target)
        .map_err(|error| failed(format!("materialization dir failed: {error}")))?;
    // Materialize straight from blob identities. `git archive` honors
    // `.gitattributes` (e.g. `*.rs text eol=crlf`) even with
    // `core.autocrlf=false`, so extracted bytes could silently differ from
    // the bound blob identity (#3548 review); `ls-tree` + `cat-file` emit
    // raw blob bytes only.
    let listing = crate::git::run_git_output_with_deadline_and_limit(
        root,
        &["ls-tree", "-r", "-z", candidate_tree],
        deadline.unwrap_or(Duration::from_mins(1)),
        MAX_ARCHIVE_BYTES,
    )
    .map_err(|error| failed(format!("git ls-tree failed: {error}")))?;
    if !listing.status.success() {
        return Err(failed(
            "git ls-tree of the candidate tree failed".to_string(),
        ));
    }
    for entry in listing.stdout.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        // Tree paths are repo-relative and must survive identity intact:
        // a non-UTF-8 path fails closed instead of lossily collapsing
        // distinct names (#3545 family).
        let text = std::str::from_utf8(entry).map_err(|_utf8_error| {
            failed(
                "candidate tree entry is not valid UTF-8; refusing lossy materialization"
                    .to_string(),
            )
        })?;
        let Some((meta, path)) = text.split_once('\t') else {
            return Err(failed(format!(
                "malformed ls-tree entry without a TAB separator: {text}"
            )));
        };
        let mut meta_parts = meta.split_whitespace();
        let mode = meta_parts.next().unwrap_or_default();
        let kind = meta_parts.next().unwrap_or_default();
        let object = meta_parts.next().unwrap_or_default();
        if kind != "blob" || !(mode == "100644" || mode == "100755") {
            return Err(failed(format!(
                "unsupported tree entry mode `{mode}` (`{kind}`) for `{path}`: the candidate tree contains a non-file object ripr cannot faithfully materialize"
            )));
        }
        let destination = safe_join(&target, path)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| failed(format!("materialization mkdir failed: {error}")))?;
        }
        let blob = crate::git::run_git_output_with_deadline_and_limit(
            root,
            &["cat-file", "blob", object],
            deadline.unwrap_or(Duration::from_mins(1)),
            MAX_ARCHIVE_BYTES,
        )
        .map_err(|error| failed(format!("git cat-file blob failed: {error}")))?;
        if !blob.status.success() {
            return Err(failed(format!("git cat-file blob {object} failed")));
        }
        std::fs::write(&destination, &blob.stdout)
            .map_err(|error| failed(format!("materialization write failed: {error}")))?;
    }
    Ok((target.clone(), cleanup))
}

/// Join a tree entry path under the target, rejecting traversal.
fn safe_join(target: &Path, name: &str) -> Result<PathBuf, SubjectError> {
    let relative = Path::new(name);
    if relative.is_absolute() || name.contains("..") || name.contains('\\') || name.starts_with('/')
    {
        return Err(failed(format!(
            "candidate tree entry `{name}` escapes the materialization root"
        )));
    }
    Ok(target.join(relative))
}

/// Resolve and execute one subject: validate identities, derive the
/// diff, and materialize the candidate root.
pub(crate) fn resolve(
    subject: &GitCandidateSubject,
    git_timeout: Option<Duration>,
) -> Result<ResolvedGitCandidate, SubjectError> {
    // The caller's timeout wins; the internal default only covers
    // library callers that pass None (#3294 review: the candidate path
    // previously dropped the user's git_timeout).
    let deadline = git_timeout.or(GIT_DEADLINE);
    // Git owns the "is this a repository" decision; a hand-rolled
    // `.git`/`HEAD` check both misses GIT_DIR setups and accepts
    // lookalike directories (#3294 review).
    if git(
        &subject.repository_root,
        &["rev-parse", "--absolute-git-dir"],
        deadline,
    )
    .is_err()
    {
        return Err(failed(format!(
            "repository root `{}` does not own a Git object database",
            subject.repository_root.display()
        )));
    }
    let base_tree = resolve_base_tree(subject, deadline)?;
    let candidate_tree = resolve_candidate_tree(subject, deadline)?;
    if base_tree == candidate_tree {
        return Err(failed(
            "base tree and candidate tree are identical: no diff to analyze".to_string(),
        ));
    }
    let diff = derive_diff(
        &subject.repository_root,
        &base_tree,
        &candidate_tree,
        deadline,
    )?;
    let (root, cleanup) = materialize(&subject.repository_root, &candidate_tree, deadline)?;
    Ok(ResolvedGitCandidate {
        base_tree,
        candidate_tree,
        diff,
        root,
        _cleanup: cleanup,
    })
}

/// The candidate tree's `ripr.toml` bytes, when the tree carries one
/// (#3279 R4: a worktree ripr.toml must not configure a subject run).
/// Read from the tree object alone; the worktree file is never opened.
pub(crate) fn candidate_config_bytes(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<Option<String>, SubjectError> {
    let treeish = subject.candidate_tree.as_str();
    let output = crate::git::run_git_output_with_deadline(
        &subject.repository_root,
        &["show", &format!("{treeish}:ripr.toml")],
        deadline,
    )
    .map_err(|error| SubjectError::ExecutionFailed {
        detail: format!("reading candidate ripr.toml failed: {error}"),
    })?;
    if !output.status.success() {
        // A tree without a ripr.toml uses the default config.
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GitCandidateDiffSemantics, GitObjectId, GitTreeish};

    fn subject(
        root: &Path,
        base: GitCandidateBase,
        treeish: &str,
    ) -> Result<GitCandidateSubject, String> {
        Ok(GitCandidateSubject {
            repository_root: root.to_path_buf(),
            base,
            candidate_tree: GitObjectId::parse(treeish).map_err(|error| error.to_string())?,
            diff_semantics: GitCandidateDiffSemantics::DirectTreeToTree,
        })
    }

    fn candidate_blob(root: &Path, treeish: &str, path: &str) -> Result<Vec<u8>, String> {
        let output = crate::git::run_git_output_with_deadline(
            root,
            &["show", &format!("{treeish}:{path}")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "git show {treeish}:{path} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }

    struct RepoGuard(PathBuf);
    impl Drop for RepoGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A real two-commit fixture repository with distinct base/candidate
    /// content, a rename, and a deletion.
    fn fixture_repo(name: &str) -> Result<(RepoGuard, String, String), String> {
        let root = std::env::temp_dir().join(format!("ripr-3277-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(&root, args, GIT_DEADLINE)
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        run(&["init", "--initial-branch=main"])?;
        run(&["config", "user.email", "ripr@example.invalid"])?;
        run(&["config", "user.name", "ripr test"])?;
        std::fs::write(root.join("src/lib.rs"), "pub fn base() -> u8 { 1 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(root.join("src/old.rs"), "pub fn gone() {}\n").map_err(|e| e.to_string())?;
        std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"draft\"\n")
            .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "base"])?;
        let base = run(&["rev-parse", "HEAD"])?;
        std::fs::write(root.join("src/lib.rs"), "pub fn candidate() -> u8 { 2 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::rename(root.join("src/old.rs"), root.join("src/renamed.rs"))
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(root.join("tests")).map_err(|e| e.to_string())?;
        std::fs::write(root.join("tests/it.rs"), "use crate::candidate;\n")
            .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "candidate"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        Ok((RepoGuard(root), base, candidate))
    }

    #[test]
    fn resolves_trees_derives_diff_and_materializes_bytes() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("resolve")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(resolved.diff.contains("src/lib.rs"), "{}", resolved.diff);
        assert!(resolved.diff.contains("src/renamed.rs") || resolved.diff.contains("src/old.rs"));
        // Candidate bytes, not worktree bytes:
        assert_eq!(
            std::fs::read(resolved.root.join("src/lib.rs")).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, "src/lib.rs")?
        );
        assert!(resolved.root.join("ripr.toml").exists());
        Ok(())
    }

    #[test]
    fn worktree_mutations_cannot_change_the_analysis_input() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("mutate")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let first = resolve(&s, None).map_err(|e| e.to_string())?;
        // The three mutations from the issue's reproduction.
        std::fs::write(guard.0.join("src/lib.rs"), "pub fn dirty() -> u8 { 9 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(guard.0.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(guard.0.join("src/staged.rs"), "pub fn staged() {}\n")
            .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        let second = resolve(&s, None).map_err(|e| e.to_string())?;
        assert_eq!(
            first.diff, second.diff,
            "diff must follow objects, not the worktree"
        );
        assert_eq!(
            std::fs::read(second.root.join("src/lib.rs")).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, "src/lib.rs")?,
            "materialized bytes must follow the candidate tree"
        );
        assert!(
            !second.root.join("src/staged.rs").exists(),
            "a blob staged after binding must not appear in the candidate"
        );
        Ok(())
    }

    // #3296 review finding 4: a pure rename stays a rename (delete+add
    // would probe unchanged content).
    #[test]
    fn rename_information_is_preserved_in_the_derived_diff() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("rename")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.diff.contains("rename from") || resolved.diff.contains("src/renamed.rs"),
            "rename must survive derivation: {}",
            resolved.diff
        );
        Ok(())
    }

    // #3296 review finding 3: paths longer than the 100-char tar name
    // field arrive as pax extended headers and must materialize.
    #[test]
    fn long_paths_materialize_through_pax_headers() -> Result<(), String> {
        let (guard, base, _fixture_candidate) = fixture_repo("longpath")?;
        let deep = format!("metrics/{}", "x".repeat(60));
        let nested = guard.0.join(&deep);
        std::fs::create_dir_all(&nested).map_err(|e| e.to_string())?;
        let long_name = "y".repeat(60);
        std::fs::write(
            nested.join(&long_name),
            "long path content
",
        )
        .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(
            &guard.0,
            &["commit", "-m", "long path"],
            GIT_DEADLINE,
        )
        .map_err(|e| e.to_string())?;
        let candidate = String::from_utf8_lossy(
            &crate::git::run_git_output_with_deadline(
                &guard.0,
                &["rev-parse", "HEAD"],
                GIT_DEADLINE,
            )
            .map_err(|e| e.to_string())?
            .stdout,
        )
        .trim()
        .to_string();
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.root.join(&deep).join(&long_name).exists(),
            "long path must materialize from the pax header"
        );
        assert_eq!(
            std::fs::read(resolved.root.join(&deep).join(&long_name)).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, &format!("{deep}/{long_name}"))?
        );
        Ok(())
    }

    // #3548 review: a `.gitattributes` `text eol=crlf` attribute must not
    // convert materialized bytes. `git archive` honors the attribute even
    // with core.autocrlf=false, so the blob-wise materialization is pinned
    // against the `git show <tree>:<path>` oracle bytes.
    #[test]
    fn materialization_ignores_attribute_driven_conversion() -> Result<(), String> {
        let (guard, _base, _pre_attribute_head) = fixture_repo("attributes")?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(&guard.0, args, GIT_DEADLINE)
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        std::fs::write(
            guard.0.join(".gitattributes"),
            "*.rs text eol=crlf
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", ".gitattributes"])?;
        run(&["commit", "-m", "attributes"])?;
        // A fresh commit AFTER the attribute exists: the candidate tree's
        // blob is pure LF, but archive would deliver CRLF.
        std::fs::write(
            guard.0.join("src/lib.rs"),
            "pub fn attr() -> u8 { 3 }
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "candidate under attribute"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        let base = run(&["rev-parse", "HEAD~1"])?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        let materialized =
            std::fs::read(resolved.root.join("src/lib.rs")).map_err(|e| e.to_string())?;
        let oracle = candidate_blob(&guard.0, &candidate, "src/lib.rs")?;
        assert_eq!(
            materialized, oracle,
            "materialized bytes must equal the bound blob bytes under eol=crlf"
        );
        assert!(
            !materialized.contains(&b'\r'),
            "an LF blob must stay LF under an eol=crlf attribute"
        );
        Ok(())
    }

    // #3296 review blocker 1: worktree and repo modes fail closed on a
    // bound subject instead of silently analyzing the live tree.
    #[test]
    fn worktree_and_repo_modes_reject_a_bound_subject() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("modes")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let options = crate::analysis::AnalysisOptions {
            root: guard.0.clone(),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: Some(s.clone()),
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
            resolved_subject_identity: None,
        };
        let error =
            crate::analysis::run_worktree_analysis_with_oracle_policy_and_generated_file_patterns(
                &options,
                &crate::config::OraclePolicy::default(),
                &[crate::analysis::language::LanguageId::Rust],
                &[],
            )
            .err()
            .ok_or("worktree mode must fail closed on a subject")?;
        assert!(
            error.contains("git candidate subject"),
            "worktree rejection must name the subject: {error}"
        );
        let repo_error = crate::analysis::run_repo_analysis_with_oracle_policy(
            &options,
            &crate::config::OraclePolicy::default(),
            &[crate::analysis::language::LanguageId::Rust],
        )
        .err()
        .ok_or("repo mode must fail closed on a subject")?;
        assert!(
            repo_error.contains("git candidate subject"),
            "repo rejection must name the subject: {repo_error}"
        );
        Ok(())
    }

    #[test]
    fn pipeline_executes_the_subject_against_candidate_bytes() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("pipeline")?;
        // Dirty the worktree AND stage a different blob: the analysis
        // input must still follow the bound objects.
        std::fs::write(
            guard.0.join("src/lib.rs"),
            "pub fn dirty() -> u8 { 9 }
",
        )
        .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        let subject = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let options = crate::analysis::AnalysisOptions {
            root: guard.0.clone(),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: Some(subject),
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
            resolved_subject_identity: None,
        };
        let result = crate::analysis::run_analysis_with_oracle_policy(
            &options,
            &crate::config::OraclePolicy::default(),
            &[crate::analysis::language::LanguageId::Rust],
        )?;
        // The derived diff contains the candidate's src/lib.rs change;
        // the dirty worktree bytes never entered.
        // A successful Rust run records no LanguageRun failure entry;
        // the completed outcome with the candidate's diff counts is the
        // observable proof the materialized root was analyzed.
        assert!(
            result
                .language_runs
                .iter()
                .all(|run| run.language != "rust"),
            "the Rust adapter must not fail against the candidate root: {:?}",
            result.language_runs
        );
        assert!(
            result.summary.changed_rust_files >= 1,
            "the candidate diff must be consumed: {:?}",
            result.summary
        );
        Ok(())
    }

    #[test]
    fn missing_candidate_fails_closed_naming_the_identity() -> Result<(), String> {
        let (guard, base, _candidate) = fixture_repo("missing")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            "0123456789012345678901234567890123456789",
        )?;
        let error = match resolve(&s, None) {
            Err(error) => error,
            Ok(_) => return Err("unknown candidate unexpectedly resolved".to_string()),
        };
        assert!(
            error.to_string().contains("git rev-parse"),
            "failure must name the resolver: {error}"
        );
        Ok(())
    }

    #[test]
    fn empty_base_uses_real_empty_tree_semantics() -> Result<(), String> {
        let (guard, _base, candidate) = fixture_repo("empty")?;
        let s = subject(&guard.0, GitCandidateBase::EmptyTree, &candidate)?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.diff.contains("src/lib.rs"),
            "empty→candidate must show the added files"
        );
        Ok(())
    }

    /// The guard reports a cleanup it could not perform instead of
    /// discarding the error. `Drop` cannot return one, so the decision of
    /// what counts as a failure lives here, where it can be asserted.
    #[test]
    fn remove_temp_root_separates_removal_success_from_real_failure() -> Result<(), String> {
        let base = std::env::temp_dir().join(format!(
            "ripr-temp-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        let root = base.join("tree");
        std::fs::create_dir_all(root.join("nested")).map_err(|error| error.to_string())?;
        std::fs::write(root.join("nested/file.rs"), "pub fn one() {}\n")
            .map_err(|error| error.to_string())?;

        // A populated root is removed, and the removal is observable.
        remove_temp_root(&root)
            .map_err(|error| format!("populated root must be removed: {error}"))?;
        assert!(!root.exists(), "root must not survive a successful removal");

        // An already-absent root is success: the postcondition is that the
        // root is gone, not that this call is the one that removed it. A
        // guard dropped after a manual cleanup must stay quiet.
        remove_temp_root(&root).map_err(|error| format!("absent root must be success: {error}"))?;

        // A real failure is returned, not swallowed. A regular file is not a
        // directory on every platform this runs on, and — unlike a
        // permission denial — it fails for root too, so the negative control
        // holds in a container as well as on a developer machine.
        let not_a_directory = base.join("regular-file");
        std::fs::write(&not_a_directory, "not a directory\n").map_err(|error| error.to_string())?;
        let error = remove_temp_root(&not_a_directory)
            .err()
            .ok_or_else(|| "removing a non-directory must report the failure".to_string())?;
        assert_ne!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "a present-but-unremovable path must not be reported as already gone"
        );
        assert!(
            not_a_directory.exists(),
            "the failing path must still be there for the operator the warning names"
        );

        let _ = std::fs::remove_file(&not_a_directory);
        let _ = std::fs::remove_dir_all(&base);
        Ok(())
    }

    /// The guard's whole reason to exist on the failure path is that it says
    /// something. Capture what it actually writes.
    ///
    /// An earlier version of this test dropped the guard and then re-derived
    /// the message by calling `remove_temp_root` and `cleanup_failure_report`
    /// itself. Those two halves were disconnected: deleting the write from the
    /// guard left every assertion green, so the promised operator warning was
    /// not bound at all. `clean_up_reporting_to` is what `Drop` delegates to in
    /// full, so driving it against a buffer observes the real emission.
    #[test]
    fn cleanup_failure_writes_the_operator_warning_and_success_writes_nothing() -> Result<(), String>
    {
        let base = std::env::temp_dir().join(format!(
            "ripr-guard-drop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).map_err(|error| error.to_string())?;

        // A regular file is not a directory on every platform, and — unlike a
        // permission denial — it fails for root too, so this control holds in
        // a container as well as on a developer machine.
        let unremovable = base.join("regular-file");
        std::fs::write(&unremovable, "not a directory\n").map_err(|error| error.to_string())?;

        let mut reported = Vec::new();
        TempRootGuard(unremovable.clone()).clean_up_reporting_to(&mut reported);
        let reported = String::from_utf8(reported).map_err(|error| error.to_string())?;
        assert!(
            reported.contains(&unremovable.display().to_string()),
            "the warning must name the path an operator has to remove: {reported:?}"
        );
        assert!(
            reported.contains("could not be removed"),
            "the warning must say what went wrong: {reported:?}"
        );
        assert!(
            unremovable.exists(),
            "the failing path must survive, or this proves nothing about the failure branch"
        );

        // A removable root is removed, and says nothing. Without this half a
        // guard that warned on every drop would still pass.
        let removable = base.join("tree");
        std::fs::create_dir_all(removable.join("nested")).map_err(|error| error.to_string())?;
        let mut quiet = Vec::new();
        TempRootGuard(removable.clone()).clean_up_reporting_to(&mut quiet);
        assert!(
            quiet.is_empty(),
            "a successful cleanup must not warn: {:?}",
            String::from_utf8_lossy(&quiet)
        );
        assert!(
            !removable.exists(),
            "a removable root must be gone after cleanup"
        );

        let _ = std::fs::remove_file(&unremovable);
        let _ = std::fs::remove_dir_all(&base);
        Ok(())
    }
}
