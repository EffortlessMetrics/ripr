#[derive(Debug)]
struct MaterializedTree {
    source_repo: PathBuf,
    parent: PathBuf,
    root: PathBuf,
    commit: String,
    cleaned: bool,
}

#[derive(Debug, Default)]
struct CleanupResult {
    worktree_remove_succeeded: bool,
    materialization_directory_removed: bool,
    worktree_residue_observed: bool,
    failure_reason: Option<String>,
}

impl MaterializedTree {
    fn create(options: &Options) -> Result<Self, String> {
        let parent =
            create_exclusive_temp_dir("ripr-resolved-tree-validation", &options.reviewed_tree)?;
        let root = parent.join("tree");

        let commit = git(
            &options.repo,
            &[
                "commit-tree",
                options.reviewed_tree.as_str(),
                "-p",
                options.source_parent.as_str(),
                "-m",
                "ripr resolved-tree validation materialization",
            ],
            &[
                ("GIT_AUTHOR_NAME", "ripr resolved-tree validator"),
                ("GIT_AUTHOR_EMAIL", "ripr-validator@invalid"),
                ("GIT_AUTHOR_DATE", "1970-01-01T00:00:00Z"),
                ("GIT_COMMITTER_NAME", "ripr resolved-tree validator"),
                ("GIT_COMMITTER_EMAIL", "ripr-validator@invalid"),
                ("GIT_COMMITTER_DATE", "1970-01-01T00:00:00Z"),
            ],
        )?;
        let commit = commit.trim().to_string();
        validate_exact_hex("disposable materialization commit", &commit, 40)?;

        let root_text = root.to_string_lossy().into_owned();
        if let Err(reason) = git(
            &options.repo,
            &["worktree", "add", "--detach", &root_text, &commit],
            &[],
        ) {
            let _ = git(
                &options.repo,
                &["worktree", "remove", "--force", &root_text],
                &[],
            );
            let mut cleanup_failures = Vec::new();
            if let Err(error) = fs::remove_dir_all(&parent)
                && parent.exists()
            {
                cleanup_failures.push(format!(
                    "failed to remove partial materialization directory: {error}"
                ));
            }
            match snapshot_worktrees(&options.repo) {
                Ok(worktrees)
                    if worktree_listing_contains_path(&worktrees, &normalize_path(&root)) =>
                {
                    cleanup_failures.push(
                        "partial materialization remains registered as a worktree".to_string(),
                    );
                }
                Ok(_) => {}
                Err(error) => cleanup_failures.push(error),
            }
            if cleanup_failures.is_empty() {
                return Err(reason);
            }
            return Err(format!(
                "{reason}; failed to clean partial materialization: {}",
                cleanup_failures.join("; ")
            ));
        }

        Ok(Self {
            source_repo: options.repo.clone(),
            parent,
            root,
            commit,
            cleaned: false,
        })
    }

    fn cleanup(&mut self) -> CleanupResult {
        if self.cleaned {
            return CleanupResult {
                worktree_remove_succeeded: true,
                materialization_directory_removed: true,
                worktree_residue_observed: false,
                failure_reason: None,
            };
        }

        let mut result = CleanupResult::default();
        let root_text = self.root.to_string_lossy().into_owned();
        let normalized_root_text = normalize_path(&self.root);
        let canonical_root_text = self
            .root
            .canonicalize()
            .ok()
            .map(|path| normalize_path(&path));
        result.worktree_remove_succeeded = git(
            &self.source_repo,
            &["worktree", "remove", "--force", &root_text],
            &[],
        )
        .is_ok();

        let worktrees = match snapshot_worktrees(&self.source_repo) {
            Ok(worktrees) => worktrees,
            Err(error) => {
                result.failure_reason = Some(error);
                String::new()
            }
        };
        result.worktree_residue_observed =
            worktree_listing_contains_path(&worktrees, &normalized_root_text)
                || canonical_root_text
                    .as_deref()
                    .is_some_and(|canonical| worktree_listing_contains_path(&worktrees, canonical));

        result.materialization_directory_removed = if self.parent.exists() {
            fs::remove_dir_all(&self.parent).is_ok()
        } else {
            true
        };

        let mut failures = Vec::new();
        if !result.worktree_remove_succeeded {
            failures.push("exact validator-owned worktree removal failed".to_string());
        }
        if result.worktree_residue_observed {
            failures.push("disposable worktree remains registered after cleanup".to_string());
        }
        if !result.materialization_directory_removed {
            failures.push("disposable materialization directory remains on disk".to_string());
        }
        if !failures.is_empty() {
            let joined = failures.join("; ");
            result.failure_reason = match result.failure_reason.take() {
                Some(existing) => Some(format!("{existing}; {joined}")),
                None => Some(joined),
            };
        }
        self.cleaned = result.failure_reason.is_none();
        result
    }
}

impl Drop for MaterializedTree {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn run_required_command(
    checker: &Path,
    root: &Path,
    command: &str,
    index: usize,
    logs_dir: &Path,
    source_parent: &str,
) -> Value {
    let args = vec![command.to_string()];
    let output = capture_output_in_dir_with_timeout_bounded(
        checker,
        &args,
        &[
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA", source_parent),
            ("RIPR_SOURCE_PROMOTION_VALIDATION", "1"),
        ],
        root,
        COMMAND_TIMEOUT,
        MAX_STREAM_BYTES,
        &format!("source-trusted governance command {command}"),
    );

    match output {
        Ok(output) => {
            let _ = output.duration;
            let evidence = match write_command_logs(logs_dir, command, index, &output) {
                Ok(evidence) => evidence,
                Err(reason) => {
                    return command_receipt(
                        command,
                        "unavailable",
                        output.status.as_ref().and_then(|status| status.code()),
                        None,
                        Some(&reason),
                    );
                }
            };
            let exit_code = output.status.as_ref().and_then(|status| status.code());
            if output.timed_out {
                command_receipt(
                    command,
                    "failed",
                    exit_code,
                    Some(&evidence),
                    Some(
                        "command exceeded the 180 second bound and its process tree was terminated",
                    ),
                )
            } else if output
                .status
                .as_ref()
                .is_some_and(|status| status.success())
            {
                command_receipt(command, "passed", exit_code, Some(&evidence), None)
            } else {
                command_receipt(
                    command,
                    "failed",
                    exit_code,
                    Some(&evidence),
                    Some("command exited non-zero"),
                )
            }
        }
        Err(reason) => command_receipt(command, "unavailable", None, None, Some(&reason)),
    }
}

fn write_command_logs(
    logs_dir: &Path,
    command: &str,
    index: usize,
    output: &TimedBoundedOutput,
) -> Result<CommandEvidence, String> {
    let stdout_relative = format!("commands/{:02}-{command}.stdout.log", index + 1);
    let stderr_relative = format!("commands/{:02}-{command}.stderr.log", index + 1);
    let stdout_bytes = output.stdout.as_bytes();
    let stderr_bytes = output.stderr.as_bytes();
    write_new_file(
        &logs_dir.join(format!("{:02}-{command}.stdout.log", index + 1)),
        stdout_bytes,
    )?;
    write_new_file(
        &logs_dir.join(format!("{:02}-{command}.stderr.log", index + 1)),
        stderr_bytes,
    )?;
    Ok(CommandEvidence {
        stdout_path: stdout_relative,
        stdout_bytes: stdout_bytes.len(),
        stdout_sha256: digest_bytes(stdout_bytes),
        stdout_truncated: output.stdout_truncated,
        stderr_path: stderr_relative,
        stderr_bytes: stderr_bytes.len(),
        stderr_sha256: digest_bytes(stderr_bytes),
        stderr_truncated: output.stderr_truncated,
    })
}

fn command_subject_role(command: &str) -> &'static str {
    if command == "check-command-catalog" {
        "source_parent_trusted_checker_self_health"
    } else {
        "reviewed_tree_source_governance_contract"
    }
}
