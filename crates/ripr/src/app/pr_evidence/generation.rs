//! Private, inactive whole-subject identity binding.
//!
//! This is data only: it neither establishes complete coverage nor grants
//! execution authority. Requested revisions are resolved once, and callers
//! capture through pinned_options before reobserving the original request.

use super::{PR_EVIDENCE_GIT_DEADLINE, PrEvidenceOptions, command_root_path};
use crate::domain::GitObjectId;
use std::path::{Path, PathBuf};

const IDENTITY_OUTPUT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedWholeSubject {
    pub(super) invocation_repo: PathBuf,
    pub(super) root: PathBuf,
    pub(super) work_tree: PathBuf,
    pub(super) base_sha: GitObjectId,
    pub(super) head_sha: GitObjectId,
    pub(super) base_tree: GitObjectId,
    pub(super) head_tree: GitObjectId,
    pub(super) origin_sha: GitObjectId,
    pub(super) origin_tree: GitObjectId,
    requested: PrEvidenceOptions,
}

impl ResolvedWholeSubject {
    pub(super) fn pinned_options(&self) -> PrEvidenceOptions {
        let mut pinned = self.requested.clone();
        pinned.base = self.base_sha.as_str().to_string();
        pinned.head = self.head_sha.as_str().to_string();
        pinned
    }
}

/// Resolve requested endpoints before any canonical diff or source capture.
/// Tree and merge-base probes use only those resolved commit identities.
pub(super) fn resolve_subject(
    repo: &Path,
    options: &PrEvidenceOptions,
) -> Result<ResolvedWholeSubject, String> {
    let invocation_repo = canonical_directory(repo, "invocation repository")?;
    let root = canonical_directory(&command_root_path(repo, &options.root), "analysis root")?;
    let work_tree = resolve_work_tree(&invocation_repo)?;
    let selected_work_tree = if root == invocation_repo {
        work_tree.clone()
    } else {
        resolve_work_tree(&root)?
    };
    if selected_work_tree != work_tree
        || !invocation_repo.starts_with(&work_tree)
        || !root.starts_with(&work_tree)
    {
        return Err("whole-subject root is outside the invocation Git work tree".to_string());
    }

    let base_sha = resolve_object(&invocation_repo, &options.base, "commit", "requested base")?;
    let head_sha = resolve_object(&invocation_repo, &options.head, "commit", "requested head")?;
    let base_tree = resolve_object(&invocation_repo, base_sha.as_str(), "tree", "base tree")?;
    let head_tree = resolve_object(&invocation_repo, head_sha.as_str(), "tree", "head tree")?;
    let origin = strict_git(
        &invocation_repo,
        &["merge-base", "--all", base_sha.as_str(), head_sha.as_str()],
    )?;
    // Empty or multiple merge bases have no unique three-dot origin.
    let origin_sha = parse_single_oid(&origin, "three-dot origin")?;
    let origin_tree =
        resolve_object(&invocation_repo, origin_sha.as_str(), "tree", "three-dot origin tree")?;

    Ok(ResolvedWholeSubject {
        invocation_repo,
        root,
        work_tree,
        base_sha,
        head_sha,
        base_tree,
        head_tree,
        origin_sha,
        origin_tree,
        requested: options.clone(),
    })
}

/// Reobserve original literals and all identity fields before publication.
/// Equal trees do not excuse movement of either requested commit.
pub(super) fn validate_current(
    repo: &Path,
    options: &PrEvidenceOptions,
    binding: &ResolvedWholeSubject,
) -> Result<(), String> {
    if options != &binding.requested {
        return Err("whole-subject request options changed after pinning".to_string());
    }
    let current = resolve_subject(repo, options)?;
    if current.invocation_repo != binding.invocation_repo
        || current.root != binding.root
        || current.work_tree != binding.work_tree
    {
        return Err("whole-subject repository or root identity changed after pinning".to_string());
    }
    if current.base_sha != binding.base_sha {
        return Err("whole-subject requested base commit changed after pinning".to_string());
    }
    if current.head_sha != binding.head_sha {
        return Err("whole-subject requested head commit changed after pinning".to_string());
    }
    if current.base_tree != binding.base_tree || current.head_tree != binding.head_tree {
        return Err("whole-subject endpoint tree identity changed after pinning".to_string());
    }
    if current.origin_sha != binding.origin_sha || current.origin_tree != binding.origin_tree {
        return Err("whole-subject three-dot origin changed after pinning".to_string());
    }
    Ok(())
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("whole-subject {label} is unavailable: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("whole-subject {label} is not a directory"));
    }
    Ok(canonical)
}

fn resolve_work_tree(repo: &Path) -> Result<PathBuf, String> {
    let output = strict_git(repo, &["rev-parse", "--show-toplevel"])?;
    let text = single_line(&output, "Git work-tree identity")?;
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err("whole-subject Git work-tree identity is not absolute".to_string());
    }
    canonical_directory(path, "Git work tree")
}

fn resolve_object(
    repo: &Path,
    revision: &str,
    kind: &str,
    label: &str,
) -> Result<GitObjectId, String> {
    let expression = format!("{revision}^{{{kind}}}");
    let output = strict_git(
        repo,
        &["rev-parse", "--verify", "--end-of-options", &expression],
    )?;
    parse_single_oid(&output, label)
}

fn strict_git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = crate::git::run_git_output_with_deadline_and_limit_strict(
        repo,
        args,
        PR_EVIDENCE_GIT_DEADLINE,
        IDENTITY_OUTPUT_BYTES,
    )
    .map_err(|error| format!("whole-subject Git probe {args:?} failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "whole-subject Git probe {args:?} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

fn single_line<'a>(bytes: &'a [u8], label: &str) -> Result<&'a str, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("whole-subject {label} is not UTF-8: {error}"))?;
    let text = match text.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => text,
    };
    if text.is_empty() || text.chars().any(|character| matches!(character, '\n' | '\r' | '\0')) {
        return Err(format!(
            "whole-subject {label} must contain exactly one nonempty line"
        ));
    }
    Ok(text)
}

fn parse_single_oid(bytes: &[u8], label: &str) -> Result<GitObjectId, String> {
    let oid = single_line(bytes, label)?;
    GitObjectId::parse(oid).map_err(|error| format!("whole-subject {label} is malformed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture_git::{FIXTURE_GIT_DEADLINE, fixture_git_ok, remove_fixture_tree};
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        repo: PathBuf,
        base_sha: String,
        head_sha: String,
        base_tree: String,
        head_tree: String,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = remove_fixture_tree(&self.repo);
        }
    }

    impl Fixture {
        fn new() -> Result<Self, String> {
            let temporary = std::env::temp_dir();
            std::fs::create_dir_all(&temporary).map_err(|error| error.to_string())?;
            let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let repo = temporary.join(format!(
                "ripr-whole-subject-{}-{stamp}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&repo).map_err(|error| error.to_string())?;
            let mut fixture = Self {
                repo,
                base_sha: String::new(),
                head_sha: String::new(),
                base_tree: String::new(),
                head_tree: String::new(),
            };
            fixture.git(&["-c", "init.templateDir=", "init", "--quiet", "-b", "subject"])?;
            fixture.git(&["config", "--local", "user.name", "RIPR subject fixture"])?;
            fixture.git(&[
                "config",
                "--local",
                "user.email",
                "subject-fixture@example.invalid",
            ])?;
            fixture.git(&["config", "--local", "commit.gpgsign", "false"])?;
            fixture.base_tree = fixture.write_tree(1)?;
            fixture.base_sha = fixture.commit(&fixture.base_tree, &[], "base")?;
            fixture.head_tree = fixture.write_tree(2)?;
            fixture.head_sha = fixture.commit(&fixture.head_tree, &[&fixture.base_sha], "head")?;
            fixture.git(&["update-ref", "refs/heads/base", &fixture.base_sha])?;
            fixture.git(&["update-ref", "refs/heads/subject", &fixture.head_sha])?;
            fixture.git(&["symbolic-ref", "HEAD", "refs/heads/subject"])?;
            Ok(fixture)
        }

        fn git(&self, args: &[&str]) -> Result<(), String> {
            fixture_git_ok(&self.repo, args)
        }

        fn git_output(&self, args: &[&str]) -> Result<String, String> {
            let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
                &self.repo,
                args,
                FIXTURE_GIT_DEADLINE,
                IDENTITY_OUTPUT_BYTES,
            )
            .map_err(|error| format!("fixture Git invocation {args:?} failed: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "fixture Git invocation {args:?} returned {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            String::from_utf8(output.stdout).map_err(|error| error.to_string())
        }

        fn write_tree(&self, value: u8) -> Result<String, String> {
            std::fs::write(
                self.repo.join("source.rs"),
                format!("pub const VALUE: u8 = {value};\n"),
            )
            .map_err(|error| error.to_string())?;
            self.git(&["add", "--", "source.rs"])?;
            let tree = self.git_output(&["write-tree"])?;
            Ok(parse_single_oid(tree.as_bytes(), "fixture tree")?.as_str().to_string())
        }

        fn commit(&self, tree: &str, parents: &[&str], message: &str) -> Result<String, String> {
            let mut args = vec!["commit-tree", tree, "-m", message];
            for parent in parents {
                args.extend(["-p", *parent]);
            }
            let commit = self.git_output(&args)?;
            Ok(parse_single_oid(commit.as_bytes(), "fixture commit")?.as_str().to_string())
        }

        fn options(&self) -> PrEvidenceOptions {
            PrEvidenceOptions {
                base: "refs/heads/base".to_string(),
                base_explicit: true,
                ..PrEvidenceOptions::default()
            }
        }
    }

    #[test]
    fn binding_uses_actual_three_dot_origin_and_pins_capture_options() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let divergent_tree = fixture.write_tree(3)?;
        let divergent_base = fixture.commit(&divergent_tree, &[&fixture.base_sha], "base fork")?;
        fixture.git(&["update-ref", "refs/heads/base", &divergent_base])?;
        let mut options = fixture.options();
        options.check = true;
        let binding = resolve_subject(&fixture.repo, &options)?;
        assert_eq!(binding.base_sha.as_str(), divergent_base);
        assert_eq!(binding.head_sha.as_str(), fixture.head_sha);
        assert_eq!(binding.origin_sha.as_str(), fixture.base_sha);
        assert_eq!(binding.origin_tree.as_str(), fixture.base_tree);
        assert_ne!(binding.base_tree, binding.origin_tree);
        let pinned = binding.pinned_options();
        assert_eq!(pinned.base, divergent_base);
        assert_eq!(pinned.head, fixture.head_sha);
        assert_eq!(pinned.root, options.root);
        assert_eq!(pinned.base_explicit, options.base_explicit);
        assert_eq!(pinned.check, options.check);
        let diff = crate::analysis::load_canonical_pr_evidence_diff_range(
            &fixture.repo,
            &pinned.base,
            &pinned.head,
        )?;
        assert!(diff.contains("-pub const VALUE: u8 = 1;"), "{diff}");
        assert!(diff.contains("+pub const VALUE: u8 = 2;"), "{diff}");
        assert!(!diff.contains("-pub const VALUE: u8 = 3;"), "{diff}");
        validate_current(&fixture.repo, &options, &binding)?;
        Ok(())
    }

    #[test]
    fn same_tree_head_movement_refuses_and_keeps_pinned_head() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let options = fixture.options();
        let binding = resolve_subject(&fixture.repo, &options)?;
        let moved = fixture.commit(&fixture.head_tree, &[&fixture.head_sha], "same-tree head")?;
        assert_ne!(moved, fixture.head_sha);
        fixture.git(&["update-ref", "refs/heads/subject", &moved])?;
        let current = resolve_subject(&fixture.repo, &options)?;
        assert_eq!(current.head_tree, binding.head_tree);
        let error = validate_current(&fixture.repo, &options, &binding)
            .err()
            .ok_or("same-tree head movement was accepted")?;
        assert!(error.contains("head commit changed"), "{error}");
        assert_eq!(binding.pinned_options().head, fixture.head_sha);
        Ok(())
    }

    #[test]
    fn same_tree_base_movement_refuses_and_keeps_pinned_base() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let options = fixture.options();
        let binding = resolve_subject(&fixture.repo, &options)?;
        let moved = fixture.commit(&fixture.base_tree, &[&fixture.base_sha], "same-tree base")?;
        assert_ne!(moved, fixture.base_sha);
        fixture.git(&["update-ref", "refs/heads/base", &moved])?;
        let current = resolve_subject(&fixture.repo, &options)?;
        assert_eq!(current.base_tree, binding.base_tree);
        assert_eq!(current.origin_sha, binding.origin_sha);
        let error = validate_current(&fixture.repo, &options, &binding)
            .err()
            .ok_or("same-tree base movement was accepted")?;
        assert!(error.contains("base commit changed"), "{error}");
        assert_eq!(binding.pinned_options().base, fixture.base_sha);
        Ok(())
    }

    #[test]
    fn missing_requested_endpoint_and_unavailable_git_probe_refuse() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let options = fixture.options();
        let binding = resolve_subject(&fixture.repo, &options)?;
        fixture.git(&["update-ref", "-d", "refs/heads/subject"])?;
        let missing = validate_current(&fixture.repo, &options, &binding)
            .err()
            .ok_or("missing requested head was accepted")?;
        assert!(missing.contains("whole-subject Git probe"), "{missing}");
        assert!(missing.contains("HEAD^{commit}"), "{missing}");
        fixture.git(&["update-ref", "refs/heads/subject", &fixture.head_sha])?;
        validate_current(&fixture.repo, &options, &binding)?;
        let git_dir = fixture.repo.join(".git");
        let held_git_dir = fixture.repo.join(".git-held");
        std::fs::rename(&git_dir, &held_git_dir).map_err(|error| error.to_string())?;
        // A broken gitfile forces a real probe error even when temp_dir is
        // nested inside the owning workspace's Git work tree.
        std::fs::write(&git_dir, b"gitdir: missing-binding-fixture-dir\n")
            .map_err(|error| error.to_string())?;
        let unavailable = validate_current(&fixture.repo, &options, &binding);
        std::fs::remove_file(&git_dir).map_err(|error| error.to_string())?;
        std::fs::rename(&held_git_dir, &git_dir).map_err(|error| error.to_string())?;
        let unavailable = unavailable
            .err()
            .ok_or("unavailable Git identity became success")?;
        assert!(unavailable.contains("whole-subject Git probe"), "{unavailable}");
        assert!(unavailable.contains("--show-toplevel"), "{unavailable}");
        validate_current(&fixture.repo, &options, &binding)?;
        Ok(())
    }

    #[test]
    fn empty_and_multiple_actual_merge_bases_refuse() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let unrelated = fixture.commit(&fixture.head_tree, &[], "unrelated")?;
        let mut options = fixture.options();
        options.head = unrelated;
        let empty = resolve_subject(&fixture.repo, &options)
            .err()
            .ok_or("missing actual merge base was accepted")?;
        assert!(empty.contains("whole-subject Git probe"), "{empty}");
        assert!(empty.contains("merge-base"), "{empty}");

        let left = fixture.commit(&fixture.base_tree, &[&fixture.base_sha], "left")?;
        let right = fixture.commit(&fixture.head_tree, &[&fixture.base_sha], "right")?;
        let first_merge = fixture.commit(&fixture.head_tree, &[&left, &right], "first merge")?;
        let second_merge = fixture.commit(&fixture.head_tree, &[&right, &left], "second merge")?;
        let actual = fixture.git_output(&["merge-base", "--all", &first_merge, &second_merge])?;
        assert_eq!(actual.lines().count(), 2, "fixture must have two actual merge bases");
        options.base = first_merge;
        options.head = second_merge;
        let multiple = resolve_subject(&fixture.repo, &options)
            .err()
            .ok_or("multiple actual merge bases were accepted")?;
        assert_eq!(
            multiple,
            "whole-subject three-dot origin must contain exactly one nonempty line"
        );
        Ok(())
    }

    #[test]
    fn changed_request_flags_and_literal_aliases_refuse() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let options = fixture.options();
        let binding = resolve_subject(&fixture.repo, &options)?;
        let mut alias = options.clone();
        alias.head = fixture.head_sha.clone();
        let alias_error = validate_current(&fixture.repo, &alias, &binding)
            .err()
            .ok_or("changed requested head literal was accepted")?;
        assert_eq!(
            alias_error,
            "whole-subject request options changed after pinning"
        );
        let mut flags = options.clone();
        flags.check = !flags.check;
        let check_error = validate_current(&fixture.repo, &flags, &binding)
            .err()
            .ok_or("changed check flag was accepted")?;
        assert_eq!(
            check_error,
            "whole-subject request options changed after pinning"
        );
        flags = options.clone();
        flags.base_explicit = !flags.base_explicit;
        let base_error = validate_current(&fixture.repo, &flags, &binding)
            .err()
            .ok_or("changed explicit-base flag was accepted")?;
        assert_eq!(
            base_error,
            "whole-subject request options changed after pinning"
        );
        Ok(())
    }

    #[test]
    fn identity_output_requires_one_valid_oid_and_strict_utf8() -> Result<(), String> {
        let oid = "0123456789012345678901234567890123456789";
        assert_eq!(parse_single_oid(format!("{oid}\n").as_bytes(), "test")?.as_str(), oid);
        assert_eq!(parse_single_oid(format!("{oid}\r\n").as_bytes(), "test")?.as_str(), oid);
        for bytes in [
            Vec::new(),
            b"not-an-object\n".to_vec(),
            format!("{oid}\n{oid}\n").into_bytes(),
            format!(" {oid}\n").into_bytes(),
            vec![0xff],
        ] {
            assert!(parse_single_oid(&bytes, "test").is_err(), "{bytes:?}");
        }
        Ok(())
    }
}
