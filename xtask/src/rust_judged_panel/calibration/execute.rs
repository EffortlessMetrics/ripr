//! Managed calibration execution using owned process/timeout authority (#4795).
//!
//! PATH or a mutable latest binary is not identity. The runner must be an
//! absolute hashed path. This module does not spawn `std::process::Command`
//! itself; it reuses `crate::run`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::python_judged_panel_replay::sha256_hex;
use crate::run::capture_bytes_in_dir_with_timeout;

use super::classify::{AttemptFacts, compile_failed_from_output};
use super::{RERUN, Selector, SubjectCounts};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExecutionPlan {
    pub(super) runner_path: PathBuf,
    pub(super) runner_sha256: String,
    pub(super) argv: Vec<String>,
    pub(super) cwd: PathBuf,
    pub(super) timeout: Duration,
    pub(super) selector: Selector,
    pub(super) subjects: SubjectCounts,
    pub(super) equivalent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RawExecution {
    pub(super) exit_code: Option<i32>,
    pub(super) timed_out: bool,
    pub(super) compile_failed: bool,
    pub(super) process_failed: bool,
    pub(super) stdout_sha256: String,
    pub(super) stderr_sha256: String,
}

pub(super) trait ProcessRunner {
    fn run(&self, plan: &ExecutionPlan) -> Result<RawExecution, String>;
}

pub(super) struct OwnedProcessRunner;

impl ProcessRunner for OwnedProcessRunner {
    fn run(&self, plan: &ExecutionPlan) -> Result<RawExecution, String> {
        bind_runner_identity(plan)?;
        let output = capture_bytes_in_dir_with_timeout(
            &plan.runner_path,
            &plan.argv,
            &plan.cwd,
            &[],
            &["HTTP_PROXY", "HTTPS_PROXY", "RIPR_BIN"],
            plan.timeout,
            "rust judged-panel calibration",
        )
        .map_err(|error| format!("{error}\nrerun: {RERUN}"))?;
        Ok(RawExecution {
            exit_code: output.status.and_then(|status| status.code()),
            timed_out: output.timed_out,
            compile_failed: compile_failed_from_output(&output.stderr, &output.stdout),
            process_failed: output.status.is_none() && !output.timed_out,
            stdout_sha256: format!("sha256:{}", sha256_hex(&output.stdout)),
            stderr_sha256: format!("sha256:{}", sha256_hex(&output.stderr)),
        })
    }
}

pub(super) fn bind_runner_identity(plan: &ExecutionPlan) -> Result<(), String> {
    if !plan.runner_path.is_absolute() {
        return Err(format!(
            "calibration runner `{}` is not an absolute identity; PATH or a command name is not authority\nrerun: {RERUN}",
            plan.runner_path.display()
        ));
    }
    let bytes = std::fs::read(&plan.runner_path).map_err(|error| {
        format!(
            "read calibration runner `{}`: {error}\nrerun: {RERUN}",
            plan.runner_path.display()
        )
    })?;
    let actual = format!("sha256:{}", sha256_hex(&bytes));
    if actual != plan.runner_sha256 {
        return Err(format!(
            "calibration runner `{}` digest `{actual}` does not match supplied identity `{}`\nrerun: {RERUN}",
            plan.runner_path.display(),
            plan.runner_sha256
        ));
    }
    Ok(())
}

pub(super) fn attempt_from_execution(
    plan: &ExecutionPlan,
    raw: &RawExecution,
    identity_ok: bool,
    selector_ok: bool,
) -> AttemptFacts {
    AttemptFacts {
        identity_ok,
        selector_ok,
        equivalent: plan.equivalent,
        timed_out: raw.timed_out,
        compile_failed: raw.compile_failed,
        process_failed: raw.process_failed,
        exit_code: raw.exit_code,
        intended: plan.subjects.intended,
        discovered: plan.subjects.discovered,
        selected: plan.subjects.selected,
        executed: plan.subjects.executed,
    }
}

pub(super) fn hash_runner(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "read calibration runner `{}`: {error}\nrerun: {RERUN}",
            path.display()
        )
    })?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}
