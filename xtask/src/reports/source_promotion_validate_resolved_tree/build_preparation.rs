// Setup only: each materialization still runs the unchanged governed checker.
// This is trusted repository Cargo, with observed known-path storage growth,
// not a sandbox or a hard aggregate writable-byte quota.
// Keep this total setup budget separate from the 180-second governed command.
// The existing 90-minute CI job deadline also bounds both materializations and
// their focused/full-suite replay; individual limits do not guarantee completion.
const MATERIALIZED_PREPARATION_TIMEOUT: Duration = Duration::from_mins(10);
const PREPARATION_SETTLEMENT_RESERVE: Duration = Duration::from_secs(30);
const PREPARATION_STORAGE_GROWTH: u64 = 4 * 1024 * 1024 * 1024;

fn preparation_remaining(elapsed: Duration) -> Result<Duration, String> {
    MATERIALIZED_PREPARATION_TIMEOUT
        .checked_sub(elapsed)
        .and_then(|remaining| remaining.checked_sub(PREPARATION_SETTLEMENT_RESERVE))
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| "materialized preparation total setup deadline exhausted".to_string())
}

fn preparation_output_failure(
    output: &TimedBoundedOutput,
    supervision_failure: Option<&str>,
) -> Option<String> {
    if let Some(reason) = supervision_failure {
        return Some(reason.to_string());
    }
    if output.timed_out {
        return Some("materialized preparation deadline exceeded".into());
    }
    if output.stdout_truncated || output.stderr_truncated {
        return Some("materialized preparation output overflow or incomplete drain".into());
    }
    if !output
        .status
        .as_ref()
        .is_some_and(|status| status.success())
    {
        return Some("materialized preparation did not exit successfully".into());
    }
    None
}

fn prepare_materialized_file_policy(
    options: &Options,
    state: &ValidationState,
    checker: &Path,
    root: &Path,
    evidence_root: &Path,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let logs = evidence_root.join("build-preparation");
    fs::create_dir(&logs).map_err(|error| format!("preparation evidence directory: {error}"))?;
    let mut rows = Vec::<Value>::new();
    let mut storage = None;
    let outcome = (|| {
        let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
        let before_head = git_with_timeout(
            root,
            &["rev-parse", "HEAD"],
            &[],
            preparation_remaining(started.elapsed())?.min(GIT_TIMEOUT),
        )?;
        if state.disposable_commit.as_deref() != Some(before_head.trim()) {
            return Err("materialized preparation disposable commit mismatch".into());
        }
        let plan = crate::policy::materialized_build_preparation_plan(
            &root.join("policy/non-rust-allowlist.toml"),
            crate::FilePolicyHost::current()?,
        )?;
        let canonical_evidence = evidence_root
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let mut build_roots = vec![canonical_root.join("target")];
        let mut known_roots = vec![build_roots[0].clone(), canonical_evidence.clone()];
        for name in ["CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"] {
            if let Some(value) = std::env::var_os(name) {
                if value.is_empty() {
                    return Err(format!("empty known preparation storage path {name}"));
                }
                let path = PathBuf::from(value);
                reject_parent_components(&path, "known preparation storage")?;
                let path = if path.is_absolute() {
                    path
                } else {
                    canonical_root.join(path)
                };
                build_roots.push(path.clone());
                known_roots.push(path);
            }
        }
        crate::run::TrustedStorageMonitor::establish_roots(
            &build_roots,
            &[canonical_root.clone(), canonical_evidence],
        )?;
        storage = Some(crate::run::TrustedStorageMonitor::new(
            known_roots,
            PREPARATION_STORAGE_GROWTH,
        )?);
        let monitor = storage
            .as_mut()
            .ok_or("preparation storage observation unavailable")?;
        for (index, args) in plan.iter().enumerate() {
            let timeout = preparation_remaining(started.elapsed())?;
            let class_started = std::time::Instant::now();
            let setup_elapsed_before = started.elapsed();
            let growth_before = monitor.peak_growth();
            eprintln!(
                "materialized preparation class={} total_setup_seconds_limit={} remaining_seconds={} argv={args:?}",
                index + 1,
                MATERIALIZED_PREPARATION_TIMEOUT.as_secs(),
                timeout.as_secs()
            );
            let captured = crate::run::capture_trusted_build_preparation(
                args,
                root,
                timeout,
                MAX_STREAM_BYTES,
                monitor,
            );
            match captured {
                Ok(captured) => {
                    let stdout = format!("{:02}.stdout.log", index + 1);
                    let stderr = format!("{:02}.stderr.log", index + 1);
                    // Try both streams and record the class even if retaining
                    // one fails; a retention failure cannot erase its outcome.
                    let stdout_retention_failure =
                        write_new_file(&logs.join(&stdout), captured.output.stdout.as_bytes())
                            .err();
                    let stderr_retention_failure =
                        write_new_file(&logs.join(&stderr), captured.output.stderr.as_bytes())
                            .err();
                    let failure = preparation_output_failure(
                        &captured.output,
                        captured.failure_reason.as_deref(),
                    );
                    let retention_failures = [
                        stdout_retention_failure.as_deref(),
                        stderr_retention_failure.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                    let failure = match (failure, retention_failures.is_empty()) {
                        (failure, true) => failure,
                        (Some(reason), false) => Some(format!(
                            "{reason}; preparation stream retention failed: {}",
                            retention_failures.join("; ")
                        )),
                        (None, false) => Some(format!(
                            "preparation stream retention failed: {}",
                            retention_failures.join("; ")
                        )),
                    };
                    rows.push(serde_json::json!({
                        "program": "cargo", "argv": args, "setup_only": true,
                        "work_budget_ms": timeout.as_millis(),
                        "setup_elapsed_before_ms": setup_elapsed_before.as_millis(),
                        "observed_class_wall_ms": class_started.elapsed().as_millis(),
                        "observed_peak_growth_before_bytes": growth_before,
                        "observed_peak_growth_after_bytes": monitor.peak_growth(),
                        "capture_failure_reason": captured.failure_reason,
                        "stdout_retention_failure": stdout_retention_failure,
                        "stderr_retention_failure": stderr_retention_failure,
                        // A terminal status is returned only after the owned
                        // group/Job wait settles. Missing status stays unknown.
                        "owned_settlement": if captured.output.status.is_some() { "confirmed_by_capture_owner" } else { "unavailable" },
                        "exit_code": captured.output.status.as_ref().and_then(|status| status.code()),
                        "duration_ms": captured.output.duration.as_millis(),
                        "timed_out": captured.output.timed_out,
                        "stdout": {"path": format!("build-preparation/{stdout}"), "bytes": captured.output.stdout.len(), "sha256": digest_bytes(captured.output.stdout.as_bytes()), "truncated": captured.output.stdout_truncated},
                        "stderr": {"path": format!("build-preparation/{stderr}"), "bytes": captured.output.stderr.len(), "sha256": digest_bytes(captured.output.stderr.as_bytes()), "truncated": captured.output.stderr_truncated},
                        "failure_reason": failure,
                    }));
                    if let Some(reason) = failure {
                        return Err(reason);
                    }
                }
                Err(reason) => {
                    rows.push(serde_json::json!({
                        "program": "cargo", "argv": args, "setup_only": true,
                        "work_budget_ms": timeout.as_millis(),
                        "setup_elapsed_before_ms": setup_elapsed_before.as_millis(),
                        "observed_class_wall_ms": class_started.elapsed().as_millis(),
                        "observed_peak_growth_before_bytes": growth_before,
                        "observed_peak_growth_after_bytes": monitor.peak_growth(),
                        "capture_unavailable": true, "owned_settlement": "unavailable",
                        "failure_reason": reason,
                    }));
                    return Err(reason);
                }
            }
        }
        // Fresh exact identity and tracked cleanliness, never a cache receipt.
        if root.canonicalize().map_err(|error| error.to_string())? != canonical_root {
            return Err("materialized preparation canonical root changed".into());
        }
        for (args, expected) in [
            (vec!["rev-parse", "HEAD"], before_head.trim()),
            (
                vec!["rev-parse", "HEAD^{tree}"],
                options.reviewed_tree.as_str(),
            ),
            (vec!["status", "--porcelain=v1", "--untracked-files=no"], ""),
        ] {
            if git_with_timeout(
                root,
                &args,
                &[],
                preparation_remaining(started.elapsed())?.min(GIT_TIMEOUT),
            )?
            .trim()
                != expected
            {
                return Err(format!(
                    "materialized preparation changed source identity or tracked files: {args:?}"
                ));
            }
        }
        if state.checker_executable_sha256.as_deref() != Some(digest_file(checker)?.as_str()) {
            return Err("trusted checker digest changed during preparation".into());
        }
        monitor.check_now()?;
        preparation_remaining(started.elapsed())?;
        Ok(())
    })();
    let sidecar = serde_json::json!({
        "schema": "ripr.materialized_build_preparation.v1", "setup_only": true,
        "source_parent": options.source_parent, "reviewed_tree": options.reviewed_tree,
        "disposable_commit": state.disposable_commit,
        "root": normalize_path(root), "duration_ms": started.elapsed().as_millis(),
        "total_setup_seconds_limit": MATERIALIZED_PREPARATION_TIMEOUT.as_secs(),
        "settlement_reserve_seconds": PREPARATION_SETTLEMENT_RESERVE.as_secs(),
        "cargo_build_jobs": 1, "stream_bytes_limit": MAX_STREAM_BYTES,
        "known_path_growth_bytes_limit": PREPARATION_STORAGE_GROWTH,
        "known_paths": storage.as_ref().map(|monitor| monitor.roots()),
        "observed_peak_growth_bytes": storage.as_ref().map(|monitor| monitor.peak_growth()),
        "storage_scope": "observed known task target/build/evidence paths; no hard quota, between-scan, open-unlinked or outside-path guarantee",
        "process_scope": "owned cooperative Linux group or Windows Job; arbitrary escaping-process containment is not claimed",
        "failure_reason": outcome.as_ref().err(), "classes": rows,
        "governed_warmups_and_listings_unchanged": true,
        "policy_acceptance_credit": false,
    });
    let bytes = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    let retained = if bytes.len() <= 64 * 1024 {
        write_new_file(&evidence_root.join("build-preparation.json"), &bytes)
    } else {
        Err("preparation sidecar exceeds finite retention bound".into())
    };
    match (outcome, retained) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(reason), Ok(())) | (Ok(()), Err(reason)) => Err(reason),
        (Err(reason), Err(retention)) => Err(format!(
            "{reason}; preparation evidence retention failed: {retention}"
        )),
    }
}

#[cfg(test)]
mod materialized_build_preparation_tests {
    use super::*;

    #[test]
    fn materialized_build_preparation_uses_one_total_deadline_with_settlement_reserve() {
        for (elapsed, remaining) in [
            (0, 570),
            (1, 569),
            (200, 370),
            (270, 300),
            (300, 270),
            (301, 269),
            (569, 1),
        ] {
            assert_eq!(
                preparation_remaining(Duration::from_secs(elapsed)),
                Ok(Duration::from_secs(remaining))
            );
        }
        for seconds in [570, 600, 601, u64::MAX] {
            assert_eq!(
                preparation_remaining(Duration::from_secs(seconds)),
                Err("materialized preparation total setup deadline exhausted".to_string())
            );
        }
    }

    #[test]
    fn materialized_build_preparation_refuses_incomplete_or_unknown_capture() {
        let mut output = TimedBoundedOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::ZERO,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        };
        assert!(preparation_output_failure(&output, None).is_some());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            output.status = Some(std::process::ExitStatus::from_raw(0));
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            output.status = Some(std::process::ExitStatus::from_raw(0));
        }
        assert!(preparation_output_failure(&output, None).is_none());

        assert_eq!(
            preparation_output_failure(&output, Some("owned settlement unknown")).as_deref(),
            Some("owned settlement unknown")
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            output.status = Some(std::process::ExitStatus::from_raw(1 << 8));
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            output.status = Some(std::process::ExitStatus::from_raw(1));
        }
        assert!(
            preparation_output_failure(&output, None)
                .is_some_and(|reason| reason.contains("successfully"))
        );
        output.timed_out = true;
        assert!(
            preparation_output_failure(&output, None)
                .is_some_and(|reason| reason.contains("deadline"))
        );
        output.timed_out = false;
        output.stdout_truncated = true;
        assert!(
            preparation_output_failure(&output, None)
                .is_some_and(|reason| reason.contains("incomplete"))
        );
    }
}
