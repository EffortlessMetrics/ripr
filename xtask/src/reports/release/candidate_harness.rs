//! Qualification-only source/package custody. Raw inputs are not admitted handles.
mod archive;
mod input;
mod live_head;
pub(crate) use live_head::handoff::{AdmittedHandoff, HandoffInput, HandoffReceipt};
mod source;
pub(crate) use archive::{AttributedArchive, CandidateExecution};
pub(crate) use source::AdmittedSource;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QualificationInput {
    controller_root: PathBuf,
    source_root: PathBuf,
    artifact: PathBuf,
    approved_manifest_digest: Option<String>,
}

impl QualificationInput {
    pub(crate) fn new(
        controller_root: PathBuf,
        source_root: PathBuf,
        artifact: PathBuf,
    ) -> Result<Self, String> {
        if !safe_artifact_path(&artifact) {
            return Err(
                "candidate artifact must be an ordinary controller-relative path".to_string(),
            );
        }
        for root in [&controller_root, &source_root] {
            if root.as_os_str().is_empty() {
                return Err("qualification roots must not be blank".to_string());
            }
            #[cfg(windows)]
            if !root.is_absolute()
                && (root.has_root()
                    || matches!(root.components().next(), Some(Component::Prefix(_))))
            {
                return Err("qualification roots must be ordinary relative or fully qualified absolute paths".to_string());
            }
        }
        Ok(Self {
            controller_root,
            source_root,
            artifact,
            approved_manifest_digest: None,
        })
    }

    /// Explicit direct #1609 mode. This digest must come from the accepted
    /// #1609 handoff, never be calculated from an unreviewed producer output.
    pub(crate) fn with_approved_manifest_digest(mut self, digest: String) -> Result<Self, String> {
        live_head::require_hex("accepted manifest digest", &digest, 64)?;
        self.approved_manifest_digest = Some(digest);
        Ok(self)
    }

    pub(crate) fn approved_manifest_digest(&self) -> Option<&str> {
        self.approved_manifest_digest.as_deref()
    }

    pub(crate) fn controller_root(&self) -> &Path {
        &self.controller_root
    }
    pub(crate) fn source_root(&self) -> &Path {
        &self.source_root
    }
    pub(crate) fn artifact(&self) -> &Path {
        &self.artifact
    }
}

fn safe_artifact_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    !text.is_empty()
        && !text.contains('\\')
        && !text.contains(':')
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !text
            .split('/')
            .any(|part| part == "." || part == ".." || part.is_empty())
}
