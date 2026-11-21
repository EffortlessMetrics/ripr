use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(crate) const WORKSPACE_STATUS_SCHEMA_VERSION: &str = "ripr-workspace-status-v1";
const REPOSITORY_MARKERS: &[&str] = &[
    ".git",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "Makefile.PL",
    "Build.PL",
    "cpanfile",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkspaceStatus {
    pub(crate) schema_version: &'static str,
    pub(crate) workspace_state: WorkspaceState,
    pub(crate) root: RootStatus,
    pub(crate) configuration: ConfigurationStatus,
    pub(crate) trust: TrustStatus,
    pub(crate) authority: AuthorityStatus,
    pub(crate) claim_boundary: &'static str,
    pub(crate) limitations: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceState {
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RootStatus {
    pub(crate) state: RootState,
    pub(crate) source: RootSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) identity: Option<String>,
    pub(crate) repository_markers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error_code: Option<RootErrorCode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootState {
    Validated,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootSource {
    Explicit,
    CurrentDirectory,
    AncestorDiscovery,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootErrorCode {
    CurrentDirectoryUnavailable,
    RootMissing,
    RootNotDirectory,
    RootCanonicalizeFailed,
    RepositoryMarkerMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ConfigurationStatus {
    pub(crate) project_config_state: ProjectConfigState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectConfigState {
    BuiltInDefaultsOnly,
    DetectedNotLoaded,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TrustStatus {
    pub(crate) project_config_trust: ProjectConfigTrust,
    pub(crate) effective_access: EffectiveAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectConfigTrust {
    NotEstablished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectiveAccess {
    ReadOnlyStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct AuthorityStatus {
    pub(crate) source_edit_capability: NoAuthority,
    pub(crate) verification_execution_capability: NoAuthority,
    pub(crate) mutation_execution_capability: NoAuthority,
    pub(crate) model_provider: NoAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoAuthority {
    None,
}

impl WorkspaceStatus {
    pub fn resolve(explicit_root: Option<PathBuf>) -> Self {
        let resolved = resolve_root(explicit_root);
        let workspace_state = if resolved.root.state == RootState::Validated {
            WorkspaceState::Ready
        } else {
            WorkspaceState::Unavailable
        };
        let configuration = ConfigurationStatus {
            project_config_state: resolved.project_config_state,
        };

        Self {
            schema_version: WORKSPACE_STATUS_SCHEMA_VERSION,
            workspace_state,
            root: resolved.root,
            configuration,
            trust: TrustStatus {
                project_config_trust: ProjectConfigTrust::NotEstablished,
                effective_access: EffectiveAccess::ReadOnlyStatus,
            },
            authority: AuthorityStatus {
                source_edit_capability: NoAuthority::None,
                verification_execution_capability: NoAuthority::None,
                mutation_execution_capability: NoAuthority::None,
                model_provider: NoAuthority::None,
            },
            claim_boundary: concat!(
                "Read-only static workspace discovery. This surface does not edit source, ",
                "execute verification or mutation, load project-local provider configuration, ",
                "or claim runtime correctness."
            ),
            limitations: vec![
                "workspace identity is host-local and not a portable repository identifier",
                "project-local ripr.toml is detected but not loaded by workspace discovery",
                "client launch does not establish project-configuration trust",
                "status does not run analysis or refresh evidence",
            ],
        }
    }
}

struct ResolvedRoot {
    root: RootStatus,
    project_config_state: ProjectConfigState,
}

fn resolve_root(explicit_root: Option<PathBuf>) -> ResolvedRoot {
    match explicit_root {
        Some(root) => validate_root(root, RootSource::Explicit),
        None => match std::env::current_dir() {
            Ok(current_dir) => discover_root(current_dir),
            Err(_error) => unavailable_root(RootErrorCode::CurrentDirectoryUnavailable),
        },
    }
}

fn discover_root(current_dir: PathBuf) -> ResolvedRoot {
    let mut candidate = current_dir.as_path();
    let mut nearest_project_root = None;
    let mut first = true;
    loop {
        if candidate.join(".git").exists() {
            let source = if first {
                RootSource::CurrentDirectory
            } else {
                RootSource::AncestorDiscovery
            };
            return validate_root(candidate.to_path_buf(), source);
        }
        if nearest_project_root.is_none() && has_non_git_repository_marker(candidate) {
            let source = if first {
                RootSource::CurrentDirectory
            } else {
                RootSource::AncestorDiscovery
            };
            nearest_project_root = Some((candidate.to_path_buf(), source));
        }
        let Some(parent) = candidate.parent() else {
            return match nearest_project_root {
                Some((root, source)) => validate_root(root, source),
                None => unavailable_root(RootErrorCode::RepositoryMarkerMissing),
            };
        };
        candidate = parent;
        first = false;
    }
}

fn validate_root(root: PathBuf, source: RootSource) -> ResolvedRoot {
    if !root.exists() {
        return unavailable_root_with_source(source, RootErrorCode::RootMissing);
    }
    if !root.is_dir() {
        return unavailable_root_with_source(source, RootErrorCode::RootNotDirectory);
    }
    let canonical = match root.canonicalize() {
        Ok(canonical) => canonical,
        Err(_error) => {
            return unavailable_root_with_source(source, RootErrorCode::RootCanonicalizeFailed);
        }
    };
    let markers = repository_markers(&canonical);
    if markers.is_empty() {
        return unavailable_root_with_source(source, RootErrorCode::RepositoryMarkerMissing);
    }
    let project_config_state = if canonical.join("ripr.toml").is_file() {
        ProjectConfigState::DetectedNotLoaded
    } else {
        ProjectConfigState::BuiltInDefaultsOnly
    };
    ResolvedRoot {
        root: RootStatus {
            state: RootState::Validated,
            source,
            identity: Some(root_identity(&canonical)),
            repository_markers: markers,
            error_code: None,
        },
        project_config_state,
    }
}

fn unavailable_root(error_code: RootErrorCode) -> ResolvedRoot {
    unavailable_root_with_source(RootSource::Unavailable, error_code)
}

fn unavailable_root_with_source(source: RootSource, error_code: RootErrorCode) -> ResolvedRoot {
    ResolvedRoot {
        root: RootStatus {
            state: RootState::Unavailable,
            source,
            identity: None,
            repository_markers: Vec::new(),
            error_code: Some(error_code),
        },
        project_config_state: ProjectConfigState::Unavailable,
    }
}

fn has_non_git_repository_marker(root: &Path) -> bool {
    // Project markers are files; an empty directory named `Cargo.toml` is
    // not a usable project (#3525 review). `.git` keeps its file-or-dir
    // handling elsewhere.
    REPOSITORY_MARKERS
        .iter()
        .filter(|marker| **marker != ".git")
        .any(|marker| root.join(marker).is_file())
}

fn repository_markers(root: &Path) -> Vec<String> {
    REPOSITORY_MARKERS
        .iter()
        .filter(|marker| root.join(marker).is_file())
        .map(|marker| (*marker).to_string())
        .collect()
}

fn root_identity(root: &Path) -> String {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

    let normalized = root.to_string_lossy().replace('\\', "/");
    let digest = Sha256::digest(normalized.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    format!("root:sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_root(label: &str) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ripr-mcp-workspace-status-{label}-{}-{counter}",
            std::process::id()
        ))
    }

    #[test]
    fn explicit_repository_root_is_hashed_and_never_serialized() -> Result<(), String> {
        let root = temporary_root("ready");
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(root.join("ripr.toml"), "mode = \"draft\"\n")
            .map_err(|error| error.to_string())?;

        let status = WorkspaceStatus::resolve(Some(root.clone()));
        let encoded = serde_json::to_string(&status).map_err(|error| error.to_string())?;
        let canonical = root
            .canonicalize()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();

        if status.workspace_state != WorkspaceState::Ready {
            return Err("expected a ready workspace status".to_string());
        }
        if status.configuration.project_config_state != ProjectConfigState::DetectedNotLoaded {
            return Err("ripr.toml must be detected without being loaded".to_string());
        }
        if status.trust.project_config_trust != ProjectConfigTrust::NotEstablished
            || status.trust.effective_access != EffectiveAccess::ReadOnlyStatus
        {
            return Err("workspace launch must not imply project-configuration trust".to_string());
        }
        if encoded.contains(&canonical) {
            return Err("serialized status leaked the canonical root path".to_string());
        }
        if status
            .root
            .identity
            .as_deref()
            .is_none_or(|value| !value.starts_with("root:sha256:"))
        {
            return Err("root identity must be a bounded hash".to_string());
        }

        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn invalid_explicit_root_fails_closed_without_path_disclosure() -> Result<(), String> {
        let root = temporary_root("missing");
        let status = WorkspaceStatus::resolve(Some(root.clone()));
        let encoded = serde_json::to_string(&status).map_err(|error| error.to_string())?;

        if status.workspace_state != WorkspaceState::Unavailable {
            return Err("missing root must be unavailable".to_string());
        }
        if status.root.error_code != Some(RootErrorCode::RootMissing) {
            return Err("missing root must carry root_missing".to_string());
        }
        let rejected_root = root.to_string_lossy();
        if encoded.contains(rejected_root.as_ref()) {
            return Err("unavailable status leaked the rejected path".to_string());
        }
        Ok(())
    }
}
