use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};

#[derive(Serialize)]
struct Stage {
    elapsed_ms: u128,
    name: &'static str,
    input_line: Option<usize>,
    io_error_kind: Option<String>,
}

#[derive(Default, Serialize)]
struct Progress {
    stages: Vec<Stage>,
    truncated: bool,
}

pub(super) struct Observation {
    started: Instant,
    executable: PathBuf,
    executable_sha256: String,
    child_pid: u32,
    hold_input_until_exit: bool,
    progress: Mutex<Progress>,
}

impl Observation {
    pub(super) fn executable_custody(path: &Path) -> Result<(PathBuf, String), String> {
        let absolute = path
            .canonicalize()
            .map_err(|error| format!("resolve worktree-built MCP executable: {error}"))?;
        let mut file = std::fs::File::open(&absolute)
            .map_err(|error| format!("open worktree-built MCP executable: {error}"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|error| format!("hash worktree-built MCP executable: {error}"))?;
            if count == 0 {
                break;
            }
            let bytes = buffer
                .get(..count)
                .ok_or_else(|| "executable reader exceeded its buffer".to_string())?;
            hash.update(bytes);
        }
        Ok((absolute, format!("{:x}", hash.finalize())))
    }

    pub(super) fn new(
        started: Instant,
        executable: PathBuf,
        executable_sha256: String,
        child_pid: u32,
        hold_input_until_exit: bool,
    ) -> Self {
        Self {
            started,
            executable,
            executable_sha256,
            child_pid,
            hold_input_until_exit,
            progress: Mutex::new(Progress::default()),
        }
    }

    pub(super) fn record(
        &self,
        name: &'static str,
        input_line: Option<usize>,
        error: Option<std::io::ErrorKind>,
    ) {
        if let Ok(mut progress) = self.progress.lock() {
            if progress.stages.len() < 64 {
                progress.stages.push(Stage {
                    elapsed_ms: self.started.elapsed().as_millis(),
                    name,
                    input_line,
                    io_error_kind: error.map(|kind| format!("{kind:?}")),
                });
            } else {
                progress.truncated = true;
            }
        }
    }

    /// Test custody only: absolute executable identity is never MCP wire data.
    pub(super) fn finish(&self, timed_out: bool, exit_code: Option<i32>) -> Result<String, String> {
        let progress = self
            .progress
            .lock()
            .map_err(|_error| "MCP observation lock unavailable".to_string())?;
        let value = serde_json::json!({
            "executable": self.executable,
            "executable_sha256_before_launch": self.executable_sha256,
            "child_pid": self.child_pid,
            "elapsed_ms": self.started.elapsed().as_millis(),
            "deadline_seconds": 10,
            "timed_out": timed_out,
            "exit_code": exit_code,
            "hold_input_until_exit": self.hold_input_until_exit,
            "progress": &*progress,
            "scope": "Observed test process and IO stages; no startup or protocol failure cause inferred"
        });
        let encoded = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("encode MCP observation: {error}"))?;
        if let Some(directory) = std::env::var_os("RIPR_3088_STDIO_OBSERVATIONS") {
            let path = PathBuf::from(directory).join(format!("child-{}.json", self.child_pid));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| format!("create MCP process observation: {error}"))?;
            std::io::Write::write_all(&mut file, encoded.as_bytes())
                .map_err(|error| format!("retain MCP process observation: {error}"))?;
        }
        Ok(encoded)
    }
}
