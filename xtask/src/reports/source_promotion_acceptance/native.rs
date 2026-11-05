use super::{digest, hex};
use serde::{Deserialize, de::DeserializeOwned};
use std::path::Path;
use std::time::Duration;

const REPO: &str = "EffortlessMetrics/ripr-swarm";
const MAX_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeDecision {
    pub(super) reference: String,
    pub(super) body_sha256: String,
    author: String,
    author_association: String,
    body: String,
}
impl NativeDecision {
    pub(super) fn validate(&self, owner: u64) -> Result<(), String> {
        comment_id(&self.reference, owner)?;
        hex(&self.body_sha256, 64)?;
        if self.body.len() > MAX_BYTES
            || self.author.trim().is_empty()
            || !matches!(
                self.author_association.as_str(),
                "OWNER" | "MEMBER" | "COLLABORATOR"
            )
            || digest(self.body.as_bytes()) != self.body_sha256
        {
            return Err("native acceptance issuer or raw body digest is invalid".into());
        }
        Ok(())
    }
    pub(super) fn payload<T: DeserializeOwned>(&self) -> Result<T, String> {
        let normalized = self.body.replace("\r\n", "\n");
        let mut blocks = normalized.split("```ripr-release-acceptance\n");
        let _ = blocks.next();
        let rest = blocks
            .next()
            .ok_or_else(|| "native decision has no release-acceptance block".to_string())?;
        if blocks.next().is_some() {
            return Err("native decision repeats release-acceptance block".into());
        }
        let (json, _) = rest
            .split_once("\n```")
            .ok_or_else(|| "unterminated native acceptance block".to_string())?;
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
    if id.is_empty() || id.starts_with('0') || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "native decision requires exact {REPO} #{owner} comment"
        ));
    }
    id.parse()
        .map_err(|error| format!("native comment ID: {error}"))
}
pub(super) fn decode(reference: &str, owner: u64, bytes: &[u8]) -> Result<NativeDecision, String> {
    let id = comment_id(reference, owner)?;
    if bytes.len() > MAX_BYTES {
        return Err("native acceptance exceeds byte budget".into());
    }
    let comment: Comment = serde_json::from_slice(bytes)
        .map_err(|error| format!("native comment response: {error}"))?;
    if comment.id != id
        || comment.html_url != reference
        || comment.issue_url != format!("https://api.github.com/repos/{REPO}/issues/{owner}")
    {
        return Err("native decision identity/location differs".into());
    }
    let decision = NativeDecision {
        reference: reference.into(),
        body_sha256: digest(comment.body.as_bytes()),
        author: comment.user.login,
        author_association: comment.author_association,
        body: comment.body,
    };
    decision.validate(owner)?;
    Ok(decision)
}
pub(super) fn read(root: &Path, reference: &str, owner: u64) -> Result<Vec<u8>, String> {
    let id = comment_id(reference, owner)?;
    let args = [
        "api",
        "--hostname",
        "github.com",
        "--method",
        "GET",
        &format!("repos/{REPO}/issues/comments/{id}"),
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let output = crate::run::capture_output_in_dir_with_timeout_bounded(
        Path::new("gh"),
        &args,
        &[],
        root,
        Duration::from_secs(30),
        MAX_BYTES,
        "native release acceptance read",
    )?;
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || output.stderr.len() > 64 * 1024
        || output.status.is_none_or(|status| !status.success())
    {
        return Err(
            "native acceptance read failed, timed out or exceeded budget; no fallback".into(),
        );
    }
    Ok(output.stdout.into_bytes())
}
