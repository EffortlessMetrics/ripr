//! Versioned phase receipt for the review-comments orchestration boundary.
//!
//! The receipt describes execution state only.  It does not promote advisory
//! static evidence into a proof, and an incomplete receipt never means that
//! the requested review is clean or complete.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const REVIEW_COMMENTS_RECEIPT_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ReviewCommentsReceiptLimitation {
    pub(crate) category: String,
    pub(crate) repair_route: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ReviewCommentsRunReceipt {
    pub(crate) schema_version: &'static str,
    pub(crate) status: &'static str,
    pub(crate) root_identity: String,
    pub(crate) base_sha: String,
    pub(crate) head_sha: String,
    pub(crate) configured_timeout_ms: u64,
    pub(crate) last_completed_phase: Option<String>,
    pub(crate) active_phase: Option<String>,
    pub(crate) completed_artifacts: Vec<String>,
    pub(crate) missing_artifacts: Vec<String>,
    pub(crate) reusable_cache_identity: String,
    pub(crate) limitations: Vec<ReviewCommentsReceiptLimitation>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) atomic_write_status: &'static str,
}

impl ReviewCommentsRunReceipt {
    pub(crate) fn new(
        root: &Path,
        base: &str,
        head: &str,
        timeout_ms: u64,
        expected_artifacts: &[String],
    ) -> Self {
        let root_identity = canonical_root_identity(root);
        let base_sha = resolve_revision(root, base);
        let head_sha = resolve_revision(root, head);
        let reusable_cache_identity = reusable_cache_identity(&root_identity, &base_sha, &head_sha);
        Self {
            schema_version: REVIEW_COMMENTS_RECEIPT_SCHEMA_VERSION,
            status: "in_progress",
            root_identity,
            base_sha,
            head_sha,
            configured_timeout_ms: timeout_ms,
            last_completed_phase: None,
            active_phase: Some("input_validation".to_string()),
            completed_artifacts: Vec::new(),
            missing_artifacts: expected_artifacts.to_vec(),
            reusable_cache_identity,
            limitations: Vec::new(),
            non_claims: vec![
                "static review guidance is advisory evidence only".to_string(),
                "no complete route inventory is claimed until status is complete".to_string(),
            ],
            atomic_write_status: "not_written",
        }
    }

    pub(crate) fn phase(&mut self, completed: &str, active: &str) {
        self.last_completed_phase = Some(completed.to_string());
        self.active_phase = Some(active.to_string());
    }

    pub(crate) fn complete(&mut self, artifacts: &[String]) {
        self.status = "complete";
        self.last_completed_phase = Some("artifact_io".to_string());
        self.active_phase = None;
        self.completed_artifacts = artifacts.to_vec();
        self.missing_artifacts.clear();
    }

    pub(crate) fn write_atomic(&mut self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|err| format!("create receipt parent {} failed: {err}", parent.display()))?;

        let temp_path = atomic_temp_path(path);
        let mut committed = self.clone();
        committed.atomic_write_status = "committed";
        let json = serde_json::to_vec_pretty(&committed)
            .map_err(|err| format!("serialize review-comments receipt failed: {err}"))?;
        fs::write(&temp_path, json)
            .map_err(|err| format!("write review-comments receipt temp failed: {err}"))?;
        if let Err(err) = fs::rename(&temp_path, path) {
            if err.kind() != std::io::ErrorKind::AlreadyExists {
                let _ = fs::remove_file(&temp_path);
                return Err(format!("publish review-comments receipt failed: {err}"));
            }
            fs::remove_file(path).map_err(|remove_err| {
                format!("replace review-comments receipt failed: {remove_err}")
            })?;
            fs::rename(&temp_path, path).map_err(|rename_err| {
                format!("publish review-comments receipt failed: {rename_err}")
            })?;
        }
        self.atomic_write_status = "committed";
        Ok(())
    }

    pub(crate) fn path_for_output(output: &Path) -> PathBuf {
        output.with_file_name("run-receipt.json")
    }
}

pub(crate) fn attach_to_json(
    rendered: &str,
    receipt: &ReviewCommentsRunReceipt,
) -> Result<String, String> {
    let mut value: Value = serde_json::from_str(rendered).map_err(|err| {
        format!("parse review-comments JSON for receipt attachment failed: {err}")
    })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "review-comments JSON must be an object".to_string())?;
    object.insert(
        "run_receipt".to_string(),
        serde_json::to_value(receipt)
            .map_err(|err| format!("serialize review-comments receipt failed: {err}"))?,
    );
    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render review-comments JSON with receipt failed: {err}"))
}

fn canonical_root_identity(root: &Path) -> String {
    let normalized = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn reusable_cache_identity(root: &str, base: &str, head: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ripr-review-comments\0");
    hasher.update(root.as_bytes());
    hasher.update([0]);
    hasher.update(base.as_bytes());
    hasher.update([0]);
    hasher.update(head.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn resolve_revision(root: &Path, revision: &str) -> String {
    let object = format!("{revision}^{{commit}}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", &object])
        .output();
    output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| revision.to_string())
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "receipt.json".to_string());
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}
