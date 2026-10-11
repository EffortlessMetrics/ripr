//! Inactive committed request intake. A supported policy requests complete
//! generation; it never grants execution, saved-proof or publication authority.

use crate::domain::GitObjectId;
use crate::git::CompleteGitEnvironment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(super) const POLICY_PATH: &str = ".ripr/complete-evidence.json";
const REQUEST_SCHEMA: &str = "ripr.complete_request.v1";
const POLICY_BYTES: usize = 4096;
const PREFLIGHT_DURATION: Duration = Duration::from_secs(30);
const DRAIN_GRACE: Duration = Duration::from_secs(5);
const PREFLIGHT_CALLS: usize = 16;
const METADATA_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub(super) enum RequestedRoute {
    Ordinary,
    Complete(Box<CompleteRequest>),
}

/// Serializable DATA only. Deserializing this does not select a route or mint
/// the worker's VerifiedWholeInput.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct CommittedRequestBinding {
    pub(super) path: String,
    pub(super) git_mode: String,
    pub(super) blob_oid: String,
    pub(super) original_bytes: Vec<u8>,
    pub(super) sha256: String,
    pub(super) effective_request: String,
    pub(super) schema_version: String,
    pub(super) profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: String,
    request: String,
    profile: String,
}

impl CommittedRequestBinding {
    /// Bounded DATA capture through the sole existing policy decoder.
    pub(super) fn from_original_blob(
        blob_oid: &str,
        original_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        let oid = GitObjectId::parse(blob_oid).map_err(|error| error.to_string())?;
        if oid.as_str() != blob_oid {
            return Err("complete request blob identity is not canonical".into());
        }
        let policy = parse_policy(&original_bytes)?;
        let binding = Self {
            path: POLICY_PATH.into(),
            git_mode: "100644".into(),
            blob_oid: blob_oid.into(),
            sha256: sha256(&original_bytes),
            original_bytes,
            effective_request: policy.request,
            schema_version: policy.schema_version,
            profile: policy.profile,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.path != POLICY_PATH || self.git_mode != "100644" {
            return Err("complete request policy path or mode differs".into());
        }
        GitObjectId::parse(&self.blob_oid).map_err(|error| error.to_string())?;
        let policy = parse_policy(&self.original_bytes)?;
        if self.sha256 != sha256(&self.original_bytes)
            || self.effective_request != policy.request
            || self.schema_version != policy.schema_version
            || self.profile != policy.profile
        {
            return Err("complete request binding differs from original policy bytes".into());
        }
        Ok(())
    }
}

/// Original literals, committed request bytes and repository identities. This
/// is intake DATA, not an analyzer capability. No Deserialize/Default/Clone.
#[derive(Debug)]
pub(super) struct CompleteRequest {
    requested_head: String,
    invocation_repository: PathBuf,
    work_tree: PathBuf,
    git_directory: PathBuf,
    common_directory: PathBuf,
    head_commit: GitObjectId,
    head_tree: GitObjectId,
    binding: CommittedRequestBinding,
    preflight: Preflight,
}

impl CompleteRequest {
    pub(super) fn binding(&self) -> &CommittedRequestBinding {
        &self.binding
    }

    pub(super) fn head_commit(&self) -> &GitObjectId {
        &self.head_commit
    }

    pub(super) fn head_tree(&self) -> &GitObjectId {
        &self.head_tree
    }

    /// Re-resolve the ORIGINAL literal. Equal trees do not excuse commit
    /// movement or deleting the request; absence cannot downgrade this route.
    /// This consumes the request for one bounded postflight phase. The actual
    /// worker supplies its held overall deadline; it cannot reset that lifetime.
    pub(super) fn validate_current(self, worker_deadline: Instant) -> Result<(), String> {
        let current = match select_request_with_deadline(
            &self.invocation_repository,
            &self.requested_head,
            worker_deadline,
        )? {
            RequestedRoute::Complete(request) => *request,
            RequestedRoute::Ordinary => {
                return Err("committed complete request disappeared after selection".into());
            }
        };
        if current.head_commit != self.head_commit
            || current.head_tree != self.head_tree
            || current.binding != self.binding
            || current.work_tree != self.work_tree
            || current.git_directory != self.git_directory
            || current.common_directory != self.common_directory
        {
            return Err("committed complete request or repository identity changed".into());
        }
        Ok(())
    }

    /// Resolve all endpoints through this request's SAME aggregate preflight.
    /// Large source, diff and configuration reads require later native startup.
    pub(super) fn resolve_whole_subject(
        &mut self,
        options: &super::PrEvidenceOptions,
    ) -> Result<CompleteSubject, String> {
        if options.check
            || !options.base_explicit
            || options.head != self.requested_head
            || options.base.is_empty()
            || options.base.len() > POLICY_BYTES
            || options.base.contains('\0')
        {
            return Err("complete whole-subject producer request is invalid".into());
        }
        let root_text = options.root.as_str();
        if root_text.len() > POLICY_BYTES || root_text.contains('\0') {
            return Err("complete whole-subject root literal admission exceeded".into());
        }
        let root = std::fs::canonicalize(super::command_root_path(
            &self.invocation_repository,
            root_text,
        ))
        .map_err(|error| format!("complete whole-subject root is unavailable: {error}"))?;
        if root != self.work_tree {
            return Err("complete whole-head-v1 requires the whole work-tree root".into());
        }
        // One canonical merge base is required. The literals only enter the
        // rev-parse expression after --end-of-options; later argv use OIDs.
        let base_expression = format!("{}^{{commit}}", options.base);
        let base_commit = oid_output(&self.probe_subject(&[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &base_expression,
        ])?)?;
        let base_tree_expression = format!("{}^{{tree}}", base_commit.as_str());
        let base_tree = oid_output(&self.probe_subject(&[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &base_tree_expression,
        ])?)?;
        let head = self.head_commit.as_str().to_string();
        let origin_commit = oid_output(&self.probe_subject(&[
            "merge-base",
            "--all",
            base_commit.as_str(),
            &head,
        ])?)?;
        let origin_tree_expression = format!("{}^{{tree}}", origin_commit.as_str());
        let origin_tree = oid_output(&self.probe_subject(&[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &origin_tree_expression,
        ])?)?;
        Ok(CompleteSubject {
            invocation_repository: self.invocation_repository.clone(),
            root,
            work_tree: self.work_tree.clone(),
            base_commit,
            head_commit: self.head_commit.clone(),
            base_tree,
            head_tree: self.head_tree.clone(),
            origin_commit,
            origin_tree,
            requested: options.clone(),
        })
    }

    /// Reobserve original literals once under the held worker deadline.
    /// A missing policy cannot downgrade a previously selected route.
    pub(super) fn validate_whole_current(
        self,
        subject: &CompleteSubject,
        worker_deadline: Instant,
    ) -> Result<(), String> {
        let mut current = match select_request_with_deadline(
            &self.invocation_repository,
            &self.requested_head,
            worker_deadline,
        )? {
            RequestedRoute::Complete(request) => *request,
            RequestedRoute::Ordinary => {
                return Err("committed complete request disappeared after selection".into());
            }
        };
        if current.invocation_repository != self.invocation_repository
            || current.head_commit != self.head_commit
            || current.head_tree != self.head_tree
            || subject.head_commit != self.head_commit
            || subject.head_tree != self.head_tree
            || current.binding != self.binding
            || current.work_tree != self.work_tree
            || current.git_directory != self.git_directory
            || current.common_directory != self.common_directory
        {
            return Err("committed complete request or repository identity changed".into());
        }
        let observed = current.resolve_whole_subject(subject.requested_options())?;
        if &observed != subject {
            return Err("complete whole-subject identities or original options changed".into());
        }
        Ok(())
    }

    /// Full-subject identity probes consume the SAME remaining call/time/output
    /// budget as selection. Large raw/source capture is a later native-worker
    /// phase and cannot use this method's finite metadata admission.
    pub(super) fn probe_subject(&mut self, args: &[&str]) -> Result<Vec<u8>, String> {
        self.preflight.git(
            &self.invocation_repository,
            args,
            POLICY_BYTES,
            CompleteGitEnvironment::WholeInput,
        )
    }
}

/// Bound canonical subject DATA. It is never an analyzer or publication grant.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompleteSubject {
    pub(super) invocation_repository: PathBuf,
    pub(super) root: PathBuf,
    pub(super) work_tree: PathBuf,
    pub(super) base_commit: GitObjectId,
    pub(super) head_commit: GitObjectId,
    pub(super) base_tree: GitObjectId,
    pub(super) head_tree: GitObjectId,
    pub(super) origin_commit: GitObjectId,
    pub(super) origin_tree: GitObjectId,
    requested: super::PrEvidenceOptions,
}

impl CompleteSubject {
    pub(super) fn requested_options(&self) -> &super::PrEvidenceOptions {
        &self.requested
    }

    pub(super) fn pinned_options(&self) -> super::PrEvidenceOptions {
        let mut options = self.requested.clone();
        options.base = self.base_commit.as_str().into();
        options.head = self.head_commit.as_str().into();
        options
    }
}

#[derive(Debug)]
struct Preflight {
    deadline: Instant,
    calls: usize,
    observed_metadata: usize,
}

impl Preflight {
    fn new() -> Self {
        Self::within(Instant::now() + PREFLIGHT_DURATION)
    }

    fn within(overall_deadline: Instant) -> Self {
        Self {
            deadline: overall_deadline.min(Instant::now() + PREFLIGHT_DURATION),
            calls: 0,
            observed_metadata: 0,
        }
    }

    fn git(
        &mut self,
        repo: &Path,
        args: &[&str],
        stream_limit: usize,
        environment: CompleteGitEnvironment,
    ) -> Result<Vec<u8>, String> {
        if self.calls >= PREFLIGHT_CALLS {
            return Err("complete request aggregate Git call admission exceeded".into());
        }
        let reservation = stream_limit
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(self.observed_metadata))
            .ok_or("complete request metadata accounting overflow")?;
        if reservation > METADATA_BYTES {
            return Err("complete request aggregate metadata admission exceeded".into());
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let timeout = remaining
            .checked_sub(DRAIN_GRACE)
            .filter(|timeout| !timeout.is_zero())
            .ok_or("complete request aggregate preflight deadline exhausted")?;
        self.calls += 1;
        let output = crate::git::run_git_complete_output_with_deadline_and_limit(
            repo,
            args,
            timeout,
            stream_limit,
            environment,
        )
        .map_err(|error| format!("complete request Git probe failed: {error}"))?;
        self.observed_metadata = self
            .observed_metadata
            .checked_add(output.stdout.len())
            .and_then(|bytes| bytes.checked_add(output.stderr.len()))
            .ok_or("complete request metadata accounting overflow")?;
        if self.observed_metadata > METADATA_BYTES {
            return Err("complete request aggregate metadata admission exceeded".into());
        }
        if Instant::now() >= self.deadline {
            return Err("complete request aggregate preflight deadline exhausted".into());
        }
        if !output.status.success() {
            return Err(format!(
                "complete request Git probe failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }
}

/// The policy always belongs to the repository root of the requested commit,
/// independent of a selected analysis subroot or the dirty live policy file.
pub(super) fn select_request(repo: &Path, requested_head: &str) -> Result<RequestedRoute, String> {
    select_request_with_deadline(repo, requested_head, Instant::now() + PREFLIGHT_DURATION)
}

pub(super) fn select_request_with_deadline(
    repo: &Path,
    requested_head: &str,
    overall_deadline: Instant,
) -> Result<RequestedRoute, String> {
    let mut preflight = Preflight::within(overall_deadline);
    let work_tree = path_output(
        &preflight.git(
            repo,
            &["rev-parse", "--show-toplevel"],
            8192,
            CompleteGitEnvironment::Selector,
        )?,
        "work tree",
    )?;
    let expression = format!("{requested_head}^{{commit}}");
    let head_commit = oid_output(&preflight.git(
        repo,
        &["rev-parse", "--verify", "--end-of-options", &expression],
        POLICY_BYTES,
        CompleteGitEnvironment::Selector,
    )?)?;
    let tree_expression = format!("{}^{{tree}}", head_commit.as_str());
    let head_tree = oid_output(&preflight.git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &tree_expression,
        ],
        POLICY_BYTES,
        CompleteGitEnvironment::Selector,
    )?)?;
    let entry = preflight.git(
        repo,
        &[
            "ls-tree",
            "--full-tree",
            "-z",
            head_tree.as_str(),
            "--",
            POLICY_PATH,
        ],
        POLICY_BYTES,
        CompleteGitEnvironment::Selector,
    )?;
    let Some(blob_oid) = policy_entry(&entry)? else {
        return Ok(RequestedRoute::Ordinary);
    };
    // Complete-only literal admission is deliberately AFTER genuine absence.
    if requested_head.is_empty()
        || requested_head.len() > POLICY_BYTES
        || requested_head.contains('\0')
    {
        return Err("complete request head literal admission exceeded".into());
    }
    let size = preflight.git(
        repo,
        &["cat-file", "-s", blob_oid.as_str()],
        POLICY_BYTES,
        CompleteGitEnvironment::WholeInput,
    )?;
    let size = line(&size)?
        .parse::<usize>()
        .map_err(|error| format!("complete request blob size is invalid: {error}"))?;
    if size == 0 || size > POLICY_BYTES {
        return Err("complete request policy exceeds the 4096-byte admission".into());
    }
    let original_bytes = preflight.git(
        repo,
        &["cat-file", "blob", blob_oid.as_str()],
        POLICY_BYTES,
        CompleteGitEnvironment::WholeInput,
    )?;
    if original_bytes.len() != size {
        return Err("complete request policy length changed during capture".into());
    }
    parse_policy(&original_bytes)?;
    let directories = preflight.git(
        repo,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
        ],
        8192,
        CompleteGitEnvironment::WholeInput,
    )?;
    let (current_work_tree, git_directory, common_directory) = directory_tuple(&directories)?;
    let current_head = oid_output(&preflight.git(
        repo,
        &["rev-parse", "--verify", "--end-of-options", &expression],
        POLICY_BYTES,
        CompleteGitEnvironment::WholeInput,
    )?)?;
    let current_tree = oid_output(&preflight.git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &tree_expression,
        ],
        POLICY_BYTES,
        CompleteGitEnvironment::WholeInput,
    )?)?;
    let current_entry = preflight.git(
        repo,
        &[
            "ls-tree",
            "--full-tree",
            "-z",
            current_tree.as_str(),
            "--",
            POLICY_PATH,
        ],
        POLICY_BYTES,
        CompleteGitEnvironment::WholeInput,
    )?;
    if current_work_tree != work_tree
        || current_head != head_commit
        || current_tree != head_tree
        || policy_entry(&current_entry)?.as_ref() != Some(&blob_oid)
    {
        return Err(
            "complete request repository, head, tree or policy changed during selection".into(),
        );
    }
    let invocation_repository = std::fs::canonicalize(repo).map_err(|error| error.to_string())?;
    let work_tree = std::fs::canonicalize(work_tree).map_err(|error| error.to_string())?;
    let git_directory = std::fs::canonicalize(git_directory).map_err(|error| error.to_string())?;
    let common_directory =
        std::fs::canonicalize(common_directory).map_err(|error| error.to_string())?;
    if !invocation_repository.starts_with(&work_tree)
        || !git_directory.is_dir()
        || !common_directory.is_dir()
    {
        return Err("complete request repository directory identity is invalid".into());
    }
    let binding = CommittedRequestBinding::from_original_blob(blob_oid.as_str(), original_bytes)?;
    Ok(RequestedRoute::Complete(Box::new(CompleteRequest {
        requested_head: requested_head.into(),
        invocation_repository,
        work_tree,
        git_directory,
        common_directory,
        head_commit,
        head_tree,
        binding,
        preflight,
    })))
}

fn parse_policy(bytes: &[u8]) -> Result<Policy, String> {
    if bytes.is_empty() || bytes.len() > POLICY_BYTES || bytes.contains(&0) {
        return Err("complete request policy byte admission failed".into());
    }
    std::str::from_utf8(bytes)
        .map_err(|error| format!("complete request policy is not UTF-8: {error}"))?;
    let policy: Policy = serde_json::from_slice(bytes)
        .map_err(|error| format!("complete request policy is invalid: {error}"))?;
    if policy.schema_version != REQUEST_SCHEMA
        || policy.request != "complete"
        || policy.profile != "whole-head-v1"
    {
        return Err("complete request policy version, request or profile is unsupported".into());
    }
    Ok(policy)
}

fn policy_entry(bytes: &[u8]) -> Result<Option<GitObjectId>, String> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let record = bytes
        .strip_suffix(&[0])
        .filter(|record| !record.contains(&0))
        .ok_or("complete request policy must have one exact NUL-delimited Git record")?;
    let tab = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or("complete request policy Git record lacks a path")?;
    let metadata = &record[..tab];
    let path = &record[tab + 1..];
    if path != POLICY_PATH.as_bytes() {
        return Err("complete request Git record names another path".into());
    }
    let fields = std::str::from_utf8(metadata)
        .map_err(|error| error.to_string())?
        .split(' ')
        .collect::<Vec<_>>();
    if fields.len() != 3 || fields[0] != "100644" || fields[1] != "blob" {
        return Err("complete request policy must be a regular committed 100644 blob".into());
    }
    GitObjectId::parse(fields[2])
        .map(Some)
        .map_err(|error| error.to_string())
}

fn line(bytes: &[u8]) -> Result<&str, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let text = text.strip_suffix('\r').unwrap_or(text);
    if text.is_empty() || text.contains(['\n', '\r', '\0']) {
        return Err("complete request Git metadata must be one nonempty line".into());
    }
    Ok(text)
}

fn path_output(bytes: &[u8], label: &str) -> Result<PathBuf, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() || text.contains(['\n', '\r', '\0']) {
        return Err("complete request directory path contains a control delimiter".into());
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(format!("complete request {label} must be absolute"));
    }
    Ok(path)
}

fn directory_tuple(bytes: &[u8]) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let mut lines = text.split('\n');
    let mut next = || {
        let text = lines
            .next()
            .ok_or("complete request directory identity is missing")?;
        path_output(text.as_bytes(), "repository directory")
    };
    let work_tree = next()?;
    let git_directory = next()?;
    let common_directory = next()?;
    if lines.next().is_some() {
        return Err("complete request directory identity has extra records".into());
    }
    Ok((work_tree, git_directory, common_directory))
}

fn oid_output(bytes: &[u8]) -> Result<GitObjectId, String> {
    GitObjectId::parse(line(bytes)?).map_err(|error| error.to_string())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture_git::{fixture_git_ok, remove_fixture_tree};
    use std::sync::atomic::{AtomicU64, Ordering};

    const VALID: &str = r#"{"schema_version":"ripr.complete_request.v1","request":"complete","profile":"whole-head-v1"}"#;
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = remove_fixture_tree(&self.0);
        }
    }
    impl Fixture {
        fn new() -> Result<Self, String> {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let repo = std::env::temp_dir().join(format!(
                "ripr-complete-request-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&repo).map_err(|error| error.to_string())?;
            let fixture = Self(repo);
            fixture.git(&[
                "-c",
                "init.templateDir=",
                "init",
                "--quiet",
                "-b",
                "request",
            ])?;
            fixture.git(&["config", "--local", "user.name", "RIPR request fixture"])?;
            fixture.git(&["config", "--local", "user.email", "request@example.invalid"])?;
            fixture.git(&["config", "--local", "commit.gpgsign", "false"])?;
            std::fs::write(fixture.0.join("source.rs"), "pub const VALUE: u8 = 1;\n")
                .map_err(|error| error.to_string())?;
            fixture.git(&["add", "--", "source.rs"])?;
            fixture.git(&["commit", "--quiet", "-m", "ordinary base"])?;
            Ok(fixture)
        }
        fn git(&self, args: &[&str]) -> Result<(), String> {
            fixture_git_ok(&self.0, args)
        }
        fn policy(&self, bytes: &[u8]) -> Result<(), String> {
            std::fs::create_dir_all(self.0.join(".ripr")).map_err(|error| error.to_string())?;
            std::fs::write(self.0.join(POLICY_PATH), bytes).map_err(|error| error.to_string())?;
            self.git(&["add", "--", POLICY_PATH])?;
            self.git(&["commit", "--quiet", "-m", "committed request"])
        }
        fn oid(&self, literal: &str) -> Result<String, String> {
            let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
                &self.0,
                &["rev-parse", "--verify", "--end-of-options", literal],
                Duration::from_secs(5),
                POLICY_BYTES,
            )
            .map_err(|error| error.to_string())?;
            if !output.status.success() {
                return Err("fixture OID probe failed".into());
            }
            Ok(line(&output.stdout)?.into())
        }
    }
    fn refusal<T>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(_) => Err("invalid complete request was accepted".into()),
            Err(error) => Ok(error),
        }
    }
    fn complete(route: RequestedRoute) -> Result<CompleteRequest, String> {
        match route {
            RequestedRoute::Complete(request) => Ok(*request),
            RequestedRoute::Ordinary => Err("supported committed policy selected ordinary".into()),
        }
    }

    #[test]
    fn committed_request_ignores_dirty_policy_and_selected_subroot() -> Result<(), String> {
        let fixture = Fixture::new()?;
        std::fs::create_dir_all(fixture.0.join(".ripr")).map_err(|error| error.to_string())?;
        std::fs::write(fixture.0.join(POLICY_PATH), VALID).map_err(|error| error.to_string())?;
        assert!(matches!(
            select_request(&fixture.0, "HEAD")?,
            RequestedRoute::Ordinary
        ));
        fixture.policy(VALID.as_bytes())?;
        std::fs::create_dir_all(fixture.0.join("nested")).map_err(|error| error.to_string())?;
        std::fs::write(fixture.0.join(POLICY_PATH), b"dirty invalid decoy")
            .map_err(|error| error.to_string())?;
        let mut request = complete(select_request(&fixture.0.join("nested"), "HEAD")?)?;
        assert_eq!(request.binding().original_bytes, VALID.as_bytes());
        assert_eq!(request.head_commit().as_str(), fixture.oid("HEAD")?);
        assert_eq!(request.head_tree().as_str(), fixture.oid("HEAD^{tree}")?);
        request.binding().validate()?;
        let tree =
            request.probe_subject(&["rev-parse", "--verify", "--end-of-options", "HEAD^{tree}"])?;
        assert_eq!(&oid_output(&tree)?, request.head_tree());
        std::fs::remove_file(fixture.0.join(POLICY_PATH)).map_err(|error| error.to_string())?;
        request.validate_current(Instant::now() + PREFLIGHT_DURATION)?;
        Ok(())
    }

    #[test]
    fn unknown_invalid_duplicate_and_oversized_committed_policy_refuse() -> Result<(), String> {
        for (invalid, expected) in [
            (b"{}".as_slice(), "policy is invalid"),
            (
                br#"{"schema_version":"future","request":"complete","profile":"whole-head-v1"}"#.as_slice(),
                "is unsupported",
            ),
            (
                br#"{"schema_version":"ripr.complete_request.v1","request":"ordinary","profile":"whole-head-v1"}"#.as_slice(),
                "is unsupported",
            ),
            (
                br#"{"schema_version":"ripr.complete_request.v1","request":"complete","profile":"whole-head-v1","extra":true}"#.as_slice(),
                "policy is invalid",
            ),
            (
                br#"{"schema_version":"ripr.complete_request.v1","request":"complete","request":"complete","profile":"whole-head-v1"}"#.as_slice(),
                "policy is invalid",
            ),
            (b"\xff".as_slice(), "is not UTF-8"),
            (b"\0".as_slice(), "byte admission failed"),
        ] {
            let fixture = Fixture::new()?;
            fixture.policy(invalid)?;
            let error = refusal(select_request(&fixture.0, "HEAD"))?;
            assert!(error.contains(expected), "{error}");
        }
        let fixture = Fixture::new()?;
        fixture.policy(&vec![b' '; POLICY_BYTES + 1])?;
        let error = refusal(select_request(&fixture.0, "HEAD"))?;
        assert!(error.contains("4096-byte"), "{error}");
        Ok(())
    }

    #[test]
    fn selected_request_refuses_deletion_and_same_tree_head_movement() -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.policy(VALID.as_bytes())?;
        let request = complete(select_request(&fixture.0, "HEAD")?)?;
        fixture.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "same tree new head",
        ])?;
        assert_eq!(request.head_tree().as_str(), fixture.oid("HEAD^{tree}")?);
        let error = refusal(request.validate_current(Instant::now() + PREFLIGHT_DURATION))?;
        assert!(error.contains("changed"), "{error}");
        let request = complete(select_request(&fixture.0, "HEAD")?)?;
        fixture.git(&["rm", "--quiet", "--", POLICY_PATH])?;
        fixture.git(&["commit", "--quiet", "-m", "remove request"])?;
        assert!(matches!(
            select_request(&fixture.0, "HEAD")?,
            RequestedRoute::Ordinary
        ));
        let error = refusal(request.validate_current(Instant::now() + PREFLIGHT_DURATION))?;
        assert!(error.contains("disappeared"), "{error}");
        Ok(())
    }

    #[test]
    fn explicit_commit_and_replacement_object_cannot_hide_committed_request() -> Result<(), String>
    {
        let fixture = Fixture::new()?;
        let ordinary = fixture.oid("HEAD")?;
        fixture.policy(VALID.as_bytes())?;
        let requested = fixture.oid("HEAD")?;
        fixture.git(&["replace", &requested, &ordinary])?;
        let request = complete(select_request(&fixture.0, &requested)?)?;
        assert_eq!(request.head_commit().as_str(), requested);
        assert_eq!(request.binding().original_bytes, VALID.as_bytes());
        fixture.git(&["update-ref", "refs/heads/request", &ordinary])?;
        request.validate_current(Instant::now() + PREFLIGHT_DURATION)?;
        assert!(matches!(
            select_request(&fixture.0, "HEAD")?,
            RequestedRoute::Ordinary
        ));
        Ok(())
    }

    #[test]
    fn policy_git_entry_and_original_byte_binding_are_closed() -> Result<(), String> {
        let oid = "0123456789012345678901234567890123456789";
        for mode in ["100755", "120000", "160000", "040000"] {
            let entry = format!("{mode} blob {oid}\t{POLICY_PATH}\0");
            let error = refusal(policy_entry(entry.as_bytes()))?;
            assert!(error.contains("regular committed 100644 blob"), "{error}");
        }
        for (entry, expected) in [
            (format!("100644 blob {oid}\tother.json\0"), "another path"),
            (format!("100644 blob {oid}\t{POLICY_PATH}"), "NUL-delimited"),
            (
                format!("100644 blob {oid}\t{POLICY_PATH}\0extra\0"),
                "NUL-delimited",
            ),
            (
                format!("100644 tree {oid}\t{POLICY_PATH}\0"),
                "regular committed 100644 blob",
            ),
        ] {
            let error = refusal(policy_entry(entry.as_bytes()))?;
            assert!(error.contains(expected), "{error}");
        }
        let error = refusal(path_output(b"/repo.git\r\n", "fixture directory"))?;
        assert!(error.contains("control delimiter"), "{error}");
        let fixture = Fixture::new()?;
        fixture.policy(VALID.as_bytes())?;
        let mut request = complete(select_request(&fixture.0, "HEAD")?)?;
        request.binding.original_bytes.push(b' ');
        let error = refusal(request.binding.validate())?;
        assert!(error.contains("binding differs"), "{error}");
        Ok(())
    }

    #[test]
    fn exhausted_preflight_cannot_spawn_or_infer_absence() -> Result<(), String> {
        let mut preflight = Preflight::new();
        preflight.calls = PREFLIGHT_CALLS;
        let error = refusal(preflight.git(
            Path::new("nonexistent-request-repository"),
            &[],
            POLICY_BYTES,
            CompleteGitEnvironment::Selector,
        ))?;
        assert!(error.contains("call admission"), "{error}");
        preflight.calls = 0;
        preflight.deadline = Instant::now();
        let error = refusal(preflight.git(
            Path::new("nonexistent-request-repository"),
            &[],
            POLICY_BYTES,
            CompleteGitEnvironment::Selector,
        ))?;
        assert!(error.contains("deadline"), "{error}");
        preflight.deadline = Instant::now() + PREFLIGHT_DURATION;
        preflight.observed_metadata = METADATA_BYTES;
        let error = refusal(preflight.git(
            Path::new("nonexistent-request-repository"),
            &[],
            POLICY_BYTES,
            CompleteGitEnvironment::Selector,
        ))?;
        assert!(error.contains("metadata admission"), "{error}");
        let fixture = Fixture::new()?;
        fixture.policy(VALID.as_bytes())?;
        let request = complete(select_request(&fixture.0, "HEAD")?)?;
        let error = refusal(request.validate_current(Instant::now()))?;
        assert!(error.contains("deadline"), "{error}");
        complete(select_request(&fixture.0, "HEAD")?)?
            .validate_current(Instant::now() + PREFLIGHT_DURATION)?;
        Ok(())
    }
    #[test]
    fn whole_subject_uses_unique_actual_origin_and_preserves_requested_options()
    -> Result<(), String> {
        let fixture = Fixture::new()?;
        let base = fixture.oid("HEAD")?;
        fixture.git(&["branch", "complete-base", &base])?;
        fixture.policy(VALID.as_bytes())?;
        let head = fixture.oid("HEAD")?;
        fixture.git(&["checkout", "--quiet", "complete-base"])?;
        std::fs::write(fixture.0.join("source.rs"), "pub const VALUE: u8 = 3;\n")
            .map_err(|error| error.to_string())?;
        fixture.git(&["add", "--", "source.rs"])?;
        fixture.git(&["commit", "--quiet", "-m", "divergent base"])?;
        let divergent = fixture.oid("HEAD")?;
        let divergent_tree = fixture.oid("HEAD^{tree}")?;
        fixture.git(&["checkout", "--quiet", "request"])?;
        let options = super::super::PrEvidenceOptions {
            base: "refs/heads/complete-base".into(),
            base_explicit: true,
            ..super::super::PrEvidenceOptions::default()
        };
        let mut request = complete(select_request(&fixture.0, "HEAD")?)?;
        let subject = request.resolve_whole_subject(&options)?;
        assert_eq!(subject.base_commit.as_str(), divergent);
        assert_eq!(subject.head_commit.as_str(), head);
        assert_eq!(subject.origin_commit.as_str(), base);
        assert_eq!(subject.base_tree.as_str(), divergent_tree);
        assert_ne!(subject.origin_tree, subject.base_tree);
        assert_eq!(subject.requested_options(), &options);
        let pinned = subject.pinned_options();
        assert_eq!(pinned.base, divergent);
        assert_eq!(pinned.head, head);
        assert_eq!(pinned.root, options.root);
        assert_eq!(pinned.base_explicit, options.base_explicit);
        request.validate_whole_current(&subject, Instant::now() + PREFLIGHT_DURATION)?;
        Ok(())
    }

    #[test]
    fn whole_subject_refuses_same_tree_base_movement_and_recovers() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let base = fixture.oid("HEAD")?;
        fixture.git(&["branch", "complete-base", &base])?;
        fixture.policy(VALID.as_bytes())?;
        let options = super::super::PrEvidenceOptions {
            base: "refs/heads/complete-base".into(),
            base_explicit: true,
            ..super::super::PrEvidenceOptions::default()
        };
        let mut request = complete(select_request(&fixture.0, "HEAD")?)?;
        let subject = request.resolve_whole_subject(&options)?;
        fixture.git(&["checkout", "--quiet", "complete-base"])?;
        fixture.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "same tree moved base",
        ])?;
        assert_eq!(fixture.oid("HEAD^{tree}")?, subject.base_tree.as_str());
        assert_ne!(fixture.oid("HEAD")?, subject.base_commit.as_str());
        fixture.git(&["checkout", "--quiet", "request"])?;
        let error =
            refusal(request.validate_whole_current(&subject, Instant::now() + PREFLIGHT_DURATION))?;
        assert!(error.contains("whole-subject identities"), "{error}");
        let mut recovered = complete(select_request(&fixture.0, "HEAD")?)?;
        let current = recovered.resolve_whole_subject(&options)?;
        assert_ne!(current.base_commit, subject.base_commit);
        assert_eq!(current.base_tree, subject.base_tree);
        recovered.validate_whole_current(&current, Instant::now() + PREFLIGHT_DURATION)?;
        Ok(())
    }

    #[test]
    fn whole_subject_refuses_invalid_requests_before_new_git_probes() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let base = fixture.oid("HEAD")?;
        fixture.policy(VALID.as_bytes())?;
        std::fs::create_dir_all(fixture.0.join("nested")).map_err(|error| error.to_string())?;
        let valid = super::super::PrEvidenceOptions {
            base,
            base_explicit: true,
            ..super::super::PrEvidenceOptions::default()
        };
        let mut request = complete(select_request(&fixture.0, "HEAD")?)?;
        let before = request.preflight.calls;
        let mut cases = Vec::new();
        let mut wrong_head = valid.clone();
        wrong_head.head = request.head_commit().as_str().into();
        cases.push(wrong_head);
        let mut implicit = valid.clone();
        implicit.base_explicit = false;
        cases.push(implicit);
        let mut check = valid.clone();
        check.check = true;
        cases.push(check);
        let mut bad_base = valid.clone();
        bad_base.base.push('\0');
        cases.push(bad_base);
        let mut subroot = valid.clone();
        subroot.root = "nested".into();
        cases.push(subroot);
        for options in cases {
            let error = refusal(request.resolve_whole_subject(&options))?;
            assert!(error.starts_with("complete whole-"), "{error}");
            assert_eq!(request.preflight.calls, before);
        }
        let subject = request.resolve_whole_subject(&valid)?;
        let error = refusal(request.validate_whole_current(&subject, Instant::now()))?;
        assert!(error.contains("deadline"), "{error}");
        Ok(())
    }

    #[test]
    fn stale_request_cannot_accept_rebound_same_tree_head_subject_data() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let base = fixture.oid("HEAD")?;
        fixture.policy(VALID.as_bytes())?;
        let options = super::super::PrEvidenceOptions {
            base,
            base_explicit: true,
            ..super::super::PrEvidenceOptions::default()
        };
        let mut original = complete(select_request(&fixture.0, "HEAD")?)?;
        let old_subject = original.resolve_whole_subject(&options)?;
        fixture.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "same tree moved head",
        ])?;
        let mut fresh = complete(select_request(&fixture.0, "HEAD")?)?;
        let rebound = fresh.resolve_whole_subject(&options)?;
        assert_ne!(rebound.head_commit, old_subject.head_commit);
        assert_eq!(rebound.head_tree, old_subject.head_tree);
        let error = refusal(
            original.validate_whole_current(&rebound, Instant::now() + PREFLIGHT_DURATION),
        )?;
        assert!(
            error.contains("committed complete request or repository identity changed"),
            "{error}"
        );
        fresh.validate_whole_current(&rebound, Instant::now() + PREFLIGHT_DURATION)?;
        Ok(())
    }
}
