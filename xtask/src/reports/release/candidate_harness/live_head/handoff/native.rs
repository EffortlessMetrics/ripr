//! Read-only GitHub adapter. The existing gh credential context stays owned by
//! the operator; this command never signs in, grants access or writes GitHub.
use super::super::digest;
use crate::run::{ByteCaptureBudget, capture_bytes_in_dir_with_budget};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::Path;
use std::time::Duration;

const REPO: &str = "EffortlessMetrics/ripr-swarm";
const MAX_NATIVE_BYTES: usize = 1024 * 1024;
const PAYLOAD_MARKER: &str = "```ripr-release-acceptance\n";

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeDecision {
    pub(super) reference: String,
    pub(super) body_sha256: String,
    author: String,
    author_association: String,
    // Retain the exact body for executable replay and later observed equality.
    body: String,
}

impl NativeDecision {
    pub(super) fn payload<T: DeserializeOwned>(&self) -> Result<T, String> {
        // Parse either GitHub newline form without changing retained raw bytes.
        let normalized = self.body.replace("\r\n", "\n");
        let mut blocks = normalized.split(PAYLOAD_MARKER);
        let _preamble = blocks.next();
        let rest = blocks
            .next()
            .ok_or_else(|| "native decision has no release-acceptance block".to_string())?;
        if blocks.next().is_some() {
            return Err("native decision repeats the release-acceptance block".to_string());
        }
        let (json, _) = rest
            .split_once("\n```")
            .ok_or_else(|| "native release-acceptance block is unterminated".to_string())?;
        serde_json::from_str(json).map_err(|error| format!("native acceptance payload: {error}"))
    }
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    html_url: String,
    issue_url: String,
    body: String,
    user: Author,
    author_association: String,
}
#[derive(Deserialize)]
struct Author {
    login: String,
}

fn comment_id(reference: &str, owner: u64) -> Result<u64, String> {
    let prefix = format!("https://github.com/{REPO}/issues/{owner}#issuecomment-");
    let id = reference.strip_prefix(&prefix).unwrap_or_default();
    if id.is_empty() || id.starts_with('0') || !id.bytes().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "native decision must be an exact {REPO} #{owner} comment"
        ));
    }
    id.parse()
        .map_err(|error| format!("native comment ID exceeds supported range: {error}"))
}

pub(super) fn read_native(
    root: &Path,
    reference: &str,
    owner: u64,
) -> Result<NativeDecision, String> {
    let id = comment_id(reference, owner)?;
    let endpoint = format!("repos/{REPO}/issues/comments/{id}");
    let args = [
        "api",
        "--hostname",
        "github.com",
        "--method",
        "GET",
        &endpoint,
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let result = capture_bytes_in_dir_with_budget(
        Path::new("gh"),
        &args,
        (root, None),
        &[],
        ByteCaptureBudget {
            timeout: Duration::from_secs(30),
            stdout_bytes: MAX_NATIVE_BYTES,
            stderr_bytes: 64 * 1024,
        },
        "native release acceptance read",
    )?;
    if result.timed_out || result.status.is_none_or(|status| !status.success()) {
        return Err(
            "native release acceptance read failed or timed out; no retained-input fallback"
                .to_string(),
        );
    }
    decode_native(reference, owner, &result.stdout)
}

pub(super) fn decode_native(
    reference: &str,
    owner: u64,
    bytes: &[u8],
) -> Result<NativeDecision, String> {
    let id = comment_id(reference, owner)?;
    if bytes.len() > MAX_NATIVE_BYTES {
        return Err("native release acceptance exceeds byte budget".to_string());
    }
    let comment: Comment = serde_json::from_slice(bytes)
        .map_err(|error| format!("native comment response: {error}"))?;
    if comment.id != id
        || comment.html_url != reference
        || comment.issue_url != format!("https://api.github.com/repos/{REPO}/issues/{owner}")
        || comment.user.login.trim().is_empty()
        || !matches!(
            comment.author_association.as_str(),
            "OWNER" | "MEMBER" | "COLLABORATOR"
        )
    {
        return Err("native release decision identity or trusted issuer differs".to_string());
    }
    Ok(NativeDecision {
        reference: reference.to_string(),
        body_sha256: digest(comment.body.as_bytes()),
        body: comment.body,
        author: comment.user.login,
        author_association: comment.author_association,
    })
}
