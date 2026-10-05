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
                let failure_reason = required_command_failure_reason(root, command, index, &output);
                command_receipt(
                    command,
                    "failed",
                    exit_code,
                    Some(&evidence),
                    Some(&failure_reason),
                )
            }
        }
        Err(reason) => command_receipt(command, "unavailable", None, None, Some(&reason)),
    }
}

// Retain policy failure context before the disposable checkout is removed.
// This only enriches an already-failed receipt; it cannot earn a passed state.
fn required_command_failure_reason(
    root: &Path,
    command: &str,
    index: usize,
    output: &TimedBoundedOutput,
) -> String {
    const CONTEXT_LIMIT: usize = 16 * 1024;
    if command == "check-command-catalog" {
        let report = bounded_command_catalog_failure_report(root);
        let (stderr, stderr_truncated) = bounded_failure_text(output.stderr.as_bytes(), 4 * 1024);
        let (stdout, stdout_truncated) = bounded_failure_text(output.stdout.as_bytes(), 2 * 1024);
        let context = format!(
            "command={command} subject_role=source_parent_trusted_checker_self_health\n\
             command_catalog_report: {report}\n\
             command_stderr path=commands/{:02}-{command}.stderr.log capture_truncated={} \
             excerpt_truncated={stderr_truncated} text={stderr:?}\n\
             command_stdout path=commands/{:02}-{command}.stdout.log capture_truncated={} \
             excerpt_truncated={stdout_truncated} text={stdout:?}",
            index + 1,
            output.stderr_truncated,
            index + 1,
            output.stdout_truncated,
        );
        let (context, truncated) = bounded_failure_text(context.as_bytes(), CONTEXT_LIMIT);
        return format!(
            "command exited non-zero; bounded_failure_context bytes_limit={CONTEXT_LIMIT} \
             truncated={truncated}\n{context}"
        );
    }
    if command != "check-workflows" {
        return "command exited non-zero".to_string();
    }
    let report = bounded_workflow_failure_report(root);
    let (stderr, stderr_truncated) = bounded_failure_text(output.stderr.as_bytes(), 4 * 1024);
    let (stdout, stdout_truncated) = bounded_failure_text(output.stdout.as_bytes(), 2 * 1024);
    let (cwd, cwd_truncated) = bounded_failure_text(root.to_string_lossy().as_bytes(), 512);
    let context = format!(
        "command=check-workflows cwd={cwd:?} cwd_truncated={cwd_truncated}\n\
         workflow_report: {report}\n\
         command_stderr path=commands/{:02}-check-workflows.stderr.log \
         capture_truncated={} excerpt_truncated={stderr_truncated} text={stderr:?}\n\
         command_stdout path=commands/{:02}-check-workflows.stdout.log \
         capture_truncated={} excerpt_truncated={stdout_truncated} text={stdout:?}",
        index + 1,
        output.stderr_truncated,
        index + 1,
        output.stdout_truncated,
    );
    let (context, truncated) = bounded_failure_text(context.as_bytes(), CONTEXT_LIMIT);
    format!(
        "command exited non-zero; bounded_failure_context bytes_limit={CONTEXT_LIMIT} \
         truncated={truncated}\n{context}"
    )
}

fn bounded_failure_text(bytes: &[u8], limit: usize) -> (String, bool) {
    let prefix = &bytes[..bytes.len().min(limit)];
    let mut text = String::from_utf8_lossy(prefix).into_owned();
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let truncated = bytes.len() > limit || end < text.len();
    text.truncate(end);
    (text, truncated)
}

fn bounded_workflow_failure_report(root: &Path) -> String {
    const REPORT_LIMIT: usize = 8 * 1024;
    const REPORT_PATH: &str = "target/ripr/reports/workflows.md";
    let read_report = || -> Result<Vec<u8>, String> {
        let components = ["target", "ripr", "reports", "workflows.md"];
        let mut path = root.to_path_buf();
        for (index, component) in components.iter().enumerate() {
            path.push(component);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("report path metadata: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!("report component {component} is a symlink"));
            }
            if (index + 1 == components.len() && !metadata.is_file())
                || (index + 1 < components.len() && !metadata.is_dir())
            {
                return Err(format!(
                    "report component {component} has an unexpected type"
                ));
            }
        }
        let file = File::open(&path).map_err(|error| format!("open report: {error}"))?;
        let mut bytes = Vec::with_capacity(REPORT_LIMIT + 1);
        file.take((REPORT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read report: {error}"))?;
        Ok(bytes)
    };
    match read_report() {
        Ok(bytes) => {
            let raw_prefix = &bytes[..bytes.len().min(REPORT_LIMIT)];
            let (text, truncated) = bounded_failure_text(&bytes, REPORT_LIMIT);
            format!(
                "path={REPORT_PATH} read_bytes={} bytes_limit={REPORT_LIMIT} \
                 truncated={truncated} prefix_sha256={} text={text:?}",
                bytes.len(),
                digest_bytes(raw_prefix),
            )
        }
        Err(error) => {
            let (error, truncated) = bounded_failure_text(error.as_bytes(), 512);
            format!(
                "path={REPORT_PATH} bytes_limit={REPORT_LIMIT} \
                 read_error_truncated={truncated} read_error={error:?}"
            )
        }
    }
}

// Optional diagnostic sibling: never a validation packet member or an admission predicate.
fn retain_command_catalog_context(
    options: &Options,
    state: &ValidationState,
    root: &Path,
    evidence_root: &Path,
    receipt: &Value,
) -> Result<(), String> {
    let final_out = if options.out.is_absolute() {
        options.out.clone()
    } else {
        std::env::current_dir()
            .map_err(|_error| "diagnostic output root unavailable")?
            .join(&options.out)
    };
    let expected_parent = options.repo.join(".git/source-promotion-admission-fixture");
    let parent = evidence_root
        .parent()
        .ok_or("diagnostic output parent unavailable")?;
    if final_out.file_name().and_then(|value| value.to_str()) != Some("validation-packet")
        || final_out
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("diagnostic capture outside exact owned fixture output".into());
    }
    for directory in [
        options.repo.as_path(),
        options.repo.join(".git").as_path(),
        expected_parent.as_path(),
        evidence_root,
        parent,
    ] {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|_error| "owned fixture diagnostic directory unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("unexpected owned fixture diagnostic directory type".into());
        }
    }
    let expected = expected_parent
        .canonicalize()
        .map_err(|_error| "owned fixture diagnostic parent unavailable")?;
    let final_parent = final_out
        .parent()
        .ok_or("final diagnostic output parent unavailable")?;
    if parent
        .canonicalize()
        .map_err(|_error| "diagnostic parent unavailable")?
        != expected
        || final_parent
            .canonicalize()
            .map_err(|_error| "final diagnostic output parent unavailable")?
            != expected
    {
        return Err("diagnostic capture outside exact owned fixture output".into());
    }
    let source = state
        .checker_source_sha
        .as_deref()
        .ok_or("diagnostic checker source unavailable")?;
    let checker = state
        .checker_executable_sha256
        .as_deref()
        .ok_or("diagnostic checker digest unavailable")?;
    if source != options.source_parent
        || receipt["command"] != "check-command-catalog"
        || receipt["subject_role"] != "source_parent_trusted_checker_self_health"
        || receipt["evidence_present"] != true
    {
        return Err("diagnostic source/catalog binding unavailable".into());
    }
    let report = bounded_command_catalog_failure_report(root);
    let (text, truncated) = bounded_failure_text(report.as_bytes(), 9 * 1024);
    let context = format!("catalog_snapshot bytes_limit=9216 truncated={truncated}\n{text}\n");
    let value = serde_json::json!({
        "diagnostic_kind":"bounded_command_catalog_context_v1",
        "diagnostic_only":true,
        "tool_version":env!("CARGO_PKG_VERSION"),
        "source_parent":options.source_parent,
        "swarm_parent":options.swarm_parent,
        "reviewed_tree":options.reviewed_tree,
        "checker_sha256":checker,
        "command_receipt":receipt,
        "snapshot_bytes":context.len(),
        "snapshot_sha256":digest_bytes(context.as_bytes()),
        "snapshot":context,
    });
    let bytes =
        serde_json::to_vec(&value).map_err(|_error| "diagnostic serialization unavailable")?;
    if bytes.len() > 64 * 1024 {
        return Err("catalog diagnostic sidecar byte ceiling exceeded".into());
    }
    write_new_file(
        &parent.join("validation-packet.command-catalog-context.json"),
        &bytes,
    )
}

// The catalog is trusted-checker self-health. This fixed file can enrich diagnostics only.
fn bounded_command_catalog_failure_report(root: &Path) -> String {
    const PATH: &str = "target/ripr/reports/command-catalog.md";
    const LIMIT: usize = 8 * 1024;
    let read = || -> Result<(u64, String, Vec<u8>), String> {
        let mut path = root.to_path_buf();
        for (index, part) in ["target", "ripr", "reports", "command-catalog.md"]
            .iter()
            .enumerate()
        {
            path.push(part);
            let metadata =
                fs::symlink_metadata(&path).map_err(|_error| "catalog report unavailable")?;
            if metadata.file_type().is_symlink()
                || (index < 3 && !metadata.is_dir())
                || (index == 3 && !metadata.is_file())
            {
                return Err("unexpected catalog report path type".into());
            }
        }
        let owner = root
            .canonicalize()
            .map_err(|_error| "catalog owner unavailable")?;
        if !path
            .canonicalize()
            .map_err(|_error| "catalog path unavailable")?
            .starts_with(owner)
        {
            return Err("catalog report escaped owner".into());
        }
        let mut file = File::open(&path).map_err(|_error| "catalog open failed")?;
        let before = file
            .metadata()
            .map_err(|_error| "catalog metadata unavailable")?;
        let extent = before.len();
        if extent > MAX_STREAM_BYTES as u64 {
            return Err("catalog report extent exceeds existing 2MiB evidence ceiling".into());
        }
        let mut hasher = Sha256::new();
        let mut prefix = Vec::with_capacity(LIMIT);
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        while total < extent {
            let remaining = usize::try_from((extent - total).min(buffer.len() as u64))
                .map_err(|_error| "catalog remaining extent overflow")?;
            let count = file
                .read(&mut buffer[..remaining])
                .map_err(|_error| "catalog read failed")?;
            if count == 0 {
                return Err("catalog ended before its extent".into());
            }
            hasher.update(&buffer[..count]);
            let keep = count.min(LIMIT - prefix.len());
            prefix.extend_from_slice(&buffer[..keep]);
            total += count as u64;
        }
        let mut growth = [0u8; 1];
        let after = file
            .metadata()
            .map_err(|_error| "catalog final metadata unavailable")?;
        if file
            .read(&mut growth)
            .map_err(|_error| "catalog growth read failed")?
            != 0
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err("catalog report changed during read".into());
        }
        Ok((extent, format!("{:x}", hasher.finalize()), prefix))
    };
    match read() {
        Ok((extent, digest, bytes)) => {
            let (text, text_truncated) = bounded_failure_text(&bytes, LIMIT);
            format!(
                "path={PATH} file_bytes={extent} sha256={digest} excerpt_bytes={} \
                bytes_limit={LIMIT} truncated={} text={text:?}",
                bytes.len(),
                extent > bytes.len() as u64 || text_truncated
            )
        }
        Err(reason) => format!("path={PATH} state=unavailable reason={reason}"),
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
