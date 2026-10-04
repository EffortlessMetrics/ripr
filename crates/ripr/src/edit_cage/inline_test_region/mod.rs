//! Producer-neutral cage for one existing inline Rust test-module region.
//!
//! This slice answers whether an external before/after edit stayed inside one
//! exact pre-existing test-required inline module body. It does not select a
//! repair target, generate a test, or flip actionability. Cfg-test recognition
//! is consumed from [`crate::analysis::cfg_predicates`]; this module does not
//! add a second lexical detector.

mod observe;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged inline-test region cage; #4784 and RepairAttempt consume it next"
    )
)]
mod validate;

use std::io::Read as _;
use std::ops::Range;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{canonical_repository_root, normalize_repo_relative_path};

#[cfg(test)]
pub(crate) use validate::validate_inline_test_region_edit;

/// Allowed operation for V1: insert test-role items into the named body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InlineTestRegionOperation {
    InsertTestItem,
}

/// Checkout-portable identity: no absolute path, no volatile line numbers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineTestRegionPortableIdentity {
    pub(crate) relative_file: String,
    pub(crate) module_path: String,
    pub(crate) cfg_basis_digest: String,
    pub(crate) header_anchor_digest: String,
}

/// Exact region authority for one existing inline test module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineTestRegionAuthority {
    pub(crate) portable: InlineTestRegionPortableIdentity,
    pub(crate) source_digest: String,
    pub(crate) package_identity: String,
    pub(crate) header_range: Range<usize>,
    pub(crate) body_range: Range<usize>,
    pub(crate) allowed_operations: Vec<InlineTestRegionOperation>,
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InlineTestRegionStatus {
    Admitted,
    Rejected,
    NotARepair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InlineTestRegionRejectReason {
    StaleSourceDigest,
    StaleModuleAnchor,
    ProductionEdit,
    ModuleDeclarationChanged,
    CfgBasisChanged,
    WrongModule,
    UnsupportedModuleKind,
    Unparseable,
    MissingRegion,
    AmbiguousRegion,
    PathEscape,
    WrongRoot,
    NonTestSubject,
    ExistingAuthorityRewritten,
    NotPureInsertion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InlineTestRegionVerdict {
    pub(crate) status: InlineTestRegionStatus,
    pub(crate) reason: Option<InlineTestRegionRejectReason>,
    pub(crate) unauthorized_range: Option<Range<usize>>,
}

impl InlineTestRegionVerdict {
    pub(crate) fn rejected(
        reason: InlineTestRegionRejectReason,
        unauthorized_range: Option<Range<usize>>,
    ) -> Self {
        Self {
            status: InlineTestRegionStatus::Rejected,
            reason: Some(reason),
            unauthorized_range,
        }
    }

    pub(crate) fn not_a_repair(reason: InlineTestRegionRejectReason) -> Self {
        Self {
            status: InlineTestRegionStatus::NotARepair,
            reason: Some(reason),
            unauthorized_range: None,
        }
    }

    pub(crate) fn admitted() -> Self {
        Self {
            status: InlineTestRegionStatus::Admitted,
            reason: None,
            unauthorized_range: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InlineTestRegionError {
    Path(String),
    Unsupported {
        reason: InlineTestRegionRejectReason,
    },
}

impl InlineTestRegionError {
    pub(crate) fn reason(&self) -> Option<InlineTestRegionRejectReason> {
        match self {
            Self::Path(_) => Some(InlineTestRegionRejectReason::PathEscape),
            Self::Unsupported { reason } => Some(*reason),
        }
    }
}

/// Observe one unique existing inline test-module region from source bytes.
pub(crate) fn observe_inline_test_region(
    source: &str,
    module_name: &str,
) -> Result<observe::ObservedInlineTestRegion, InlineTestRegionError> {
    observe::observe_inline_test_region(source, module_name)
}

/// Build an authority from already-read source bytes and a repo-relative path.
pub(crate) fn authority_from_source(
    relative_file: &str,
    source: &str,
    module_name: &str,
    package_identity: String,
    generated_file_patterns: &[String],
) -> Result<InlineTestRegionAuthority, InlineTestRegionError> {
    let relative_file = normalize_repo_relative_path(Path::new(relative_file))
        .map_err(InlineTestRegionError::Path)?;
    if crate::analysis::is_test_surface_path(&relative_file) {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::UnsupportedModuleKind,
        });
    }
    if crate::analysis::is_generated_rust_file_with_patterns(
        Path::new(&relative_file),
        generated_file_patterns,
    ) {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::UnsupportedModuleKind,
        });
    }
    let observed = observe_inline_test_region(source, module_name)?;
    Ok(authority_from_observed(
        relative_file,
        source,
        observed,
        package_identity,
    ))
}

/// Capture authority from a repository root and a contained source file.
pub(crate) fn capture_inline_test_region_authority(
    root: &Path,
    relative_file: &str,
    module_name: &str,
    generated_file_patterns: &[String],
) -> Result<InlineTestRegionAuthority, InlineTestRegionError> {
    let root = canonical_repository_root(root).map_err(InlineTestRegionError::Path)?;
    let relative_file = normalize_repo_relative_path(Path::new(relative_file))
        .map_err(InlineTestRegionError::Path)?;
    let source = read_contained_regular_file(&root, &relative_file)?;
    let package_identity = nearest_package_identity(&root, &relative_file);
    authority_from_source(
        &relative_file,
        &source,
        module_name,
        package_identity,
        generated_file_patterns,
    )
}

fn authority_from_observed(
    relative_file: String,
    source: &str,
    observed: observe::ObservedInlineTestRegion,
    package_identity: String,
) -> InlineTestRegionAuthority {
    InlineTestRegionAuthority {
        portable: InlineTestRegionPortableIdentity {
            relative_file,
            module_path: observed.module_path,
            cfg_basis_digest: observed.cfg_basis_digest,
            header_anchor_digest: observed.header_anchor_digest,
        },
        source_digest: digest_bytes(source.as_bytes()),
        package_identity,
        header_range: observed.header_range,
        body_range: observed.body_range,
        allowed_operations: vec![InlineTestRegionOperation::InsertTestItem],
        limitations: vec![
            "cargo_target_identity_not_established: V1 binds package via nearest Cargo.toml, not cargo metadata target names".to_string(),
            "no_producer_admission: this cage does not select an InlineUnit target or flip actionability".to_string(),
        ],
    }
}

fn read_contained_regular_file(
    root: &Path,
    relative_file: &str,
) -> Result<String, InlineTestRegionError> {
    let path = contained_regular_file(root, relative_file)?;
    let walked = std::fs::symlink_metadata(&path).map_err(|error| {
        InlineTestRegionError::Path(format!("inspect {}: {error}", path.display()))
    })?;
    if walked.file_type().is_symlink() {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::PathEscape,
        });
    }
    let file = super::open_regular_file_no_follow(&path).map_err(|error| {
        InlineTestRegionError::Path(format!("open {}: {error}", path.display()))
    })?;
    let opened = file.metadata().map_err(|error| {
        InlineTestRegionError::Path(format!("inspect opened {}: {error}", path.display()))
    })?;
    if !opened.is_file() || !super::same_file_object(&walked, &opened) {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::PathEscape,
        });
    }
    let limit = super::MAX_CAPTURE_FILE_BYTES.saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).map_err(|error| {
        InlineTestRegionError::Path(format!("read {}: {error}", path.display()))
    })?;
    if bytes.len() as u64 > super::MAX_CAPTURE_FILE_BYTES {
        return Err(InlineTestRegionError::Path(format!(
            "read {}: file exceeds capture bound",
            path.display()
        )));
    }
    String::from_utf8(bytes)
        .map_err(|error| InlineTestRegionError::Path(format!("read {}: {error}", path.display())))
}

fn contained_regular_file(
    root: &Path,
    relative_file: &str,
) -> Result<PathBuf, InlineTestRegionError> {
    let mut current = root.to_path_buf();
    for component in relative_file.split('/') {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            InlineTestRegionError::Path(format!("inspect {}: {error}", current.display()))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(InlineTestRegionError::Unsupported {
                reason: InlineTestRegionRejectReason::PathEscape,
            });
        }
    }
    if !current.starts_with(root) {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::WrongRoot,
        });
    }
    Ok(current)
}

fn nearest_package_identity(root: &Path, relative_file: &str) -> String {
    let mut directory = root.join(relative_file);
    directory.pop();
    loop {
        let manifest = directory.join("Cargo.toml");
        if let Ok(bytes) = std::fs::read(&manifest) {
            let relative = manifest
                .strip_prefix(root)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| "Cargo.toml".to_string());
            return format!("{relative}:{}", digest_bytes(&bytes));
        }
        if directory == root {
            break;
        }
        if !directory.pop() {
            break;
        }
    }
    format!(
        "directory:{}",
        Path::new(relative_file)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .replace('\\', "/")
    )
}

pub(crate) fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
