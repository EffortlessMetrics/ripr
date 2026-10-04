//! Producer-owned `NewTestTargetProposal` admission.
//!
//! InlineUnit (#4784) may earn one exact insertion into an already-governed
//! inline cfg-test module. Integration (#4576) may earn one new `tests/` file
//! for a crate-root public library item with established autodiscovery.
//! Neither invents expected values or generates a test body.

use crate::analysis::facts::FunctionSourceRole;
use crate::analysis::language::is_generated_rust_file_with_patterns;
use crate::analysis::rust_index::{self, FunctionSummary, RustIndex};
use crate::analysis::seams::{RepoSeam, SeamKind};
use crate::analysis::syntax::{GovernedCfgTestModule, inline_unit_module_layout};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

mod integration;
#[cfg(test)]
mod integration_tests;
mod region;
#[cfg(test)]
mod tests;

pub(crate) use region::InlineTestRegionAuthority;
#[cfg(test)]
pub(crate) use region::validate_inline_region_edit;

const SAFE_NEW_INLINE_UNIT_EVIDENCE: &str = "producer-owned new inline unit test proposal";

pub(crate) use integration::admit_new_integration_test;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct NewTestTargetAdmission {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proposal: Option<NewTestTargetProposal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) region: Option<InlineTestRegionAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) blocker: Option<NewTestProposalBlocker>,
}

impl NewTestTargetAdmission {
    pub(crate) fn missing_reason(&self) -> Option<String> {
        self.blocker
            .as_ref()
            .map(|blocker| format!("new test proposal blocked: {}", blocker.as_str()))
    }

    pub(crate) fn present_reason(&self) -> &'static str {
        match self.proposal.as_ref().map(|proposal| proposal.kind) {
            Some(NewTestKind::Integration) => integration::integration_present_reason(),
            _ => SAFE_NEW_INLINE_UNIT_EVIDENCE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NewTestProposalBlocker {
    NoTestModule,
    AmbiguousModule,
    OutOfLineModule,
    CustomHarness,
    GeneratedOrVendor,
    OwnerUnresolved,
    OwnerInaccessible,
    LexicalFallback,
    PathUnsafe,
    StaleSource,
    ProductionEdit,
    InlineUnitOutOfScope,
    PrivateOwner,
    AutotestsDisabled,
    MissingIntegrationLayout,
    LibraryTargetUnresolved,
    FileCollision,
}

impl NewTestProposalBlocker {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NoTestModule => "no governed inline cfg-test module exists in the owner file",
            Self::AmbiguousModule => {
                "more than one governed inline test module or insertion anchor is plausible"
            }
            Self::OutOfLineModule => {
                "the only test module is out-of-line; V1 does not follow mod tests;"
            }
            Self::CustomHarness => {
                "owner file is a custom test harness, not an ordinary inline module"
            }
            Self::GeneratedOrVendor => {
                "owner file is generated, vendored, or otherwise outside the governed source set"
            }
            Self::OwnerUnresolved => "production owner is unresolved",
            Self::OwnerInaccessible => {
                "the selected inline test module does not have ordinary Rust access to the owner"
            }
            Self::LexicalFallback => {
                "owner file used lexical fallback; module region identity is not established"
            }
            Self::PathUnsafe => "owner source path is not a root-contained production file",
            Self::StaleSource => {
                "region authority source digest does not match the supplied before text"
            }
            Self::ProductionEdit => {
                "edit changed production text outside the governed inline test-module region"
            }
            Self::InlineUnitOutOfScope => {
                "inline unit insertion is out of scope for this seam or file"
            }
            Self::PrivateOwner => {
                "owner is private and would require a production visibility change"
            }
            Self::AutotestsDisabled => "cargo autotests discovery is disabled",
            Self::MissingIntegrationLayout => "package has no established tests/ layout",
            Self::LibraryTargetUnresolved => "exact public library target is unresolved",
            Self::FileCollision => "proposed integration file collides with an existing path",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct NewTestTargetProposal {
    pub(crate) kind: NewTestKind,
    pub(crate) file: PathBuf,
    pub(crate) owner: String,
    pub(crate) provenance: NewTestProposalProvenance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NewTestKind {
    InlineUnit,
    Integration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NewTestProposalProvenance {
    ProducerOwned,
}

/// Prefer a producer-owned Integration file when the owner is a crate-root
/// public library item; otherwise keep the landed InlineUnit producer.
/// When both stay Missing, keep Integration's Cargo/layout blockers. A
/// PrivateOwner Integration refusal does not replace an InlineUnit module
/// reason: private items can still earn a same-file unit proposal.
pub(crate) fn admit_new_test_target(
    seam: &RepoSeam,
    index: &RustIndex,
    layouts: &InlineUnitLayoutMemo,
) -> NewTestTargetAdmission {
    let integration = admit_new_integration_test(seam, index);
    if integration.proposal.is_some() {
        return integration;
    }
    let inline = admit_new_inline_unit_test_with(seam, index, layouts);
    if inline.proposal.is_some() {
        return inline;
    }
    match integration.blocker {
        Some(
            NewTestProposalBlocker::AutotestsDisabled
            | NewTestProposalBlocker::MissingIntegrationLayout
            | NewTestProposalBlocker::LibraryTargetUnresolved
            | NewTestProposalBlocker::FileCollision,
        ) => integration,
        _ => inline,
    }
}

/// Admit one InlineUnit proposal from indexed source-role and parser-backed
/// module facts. Callers that already have a related observer still invoke
/// this so Missing reasons stay typed; ranking prefers an admitted Existing
/// target, a refused DirectOwnerCall stays Missing rather than Proposed, and
/// an advisory related observer does not block an independently admitted
/// proposal.
#[cfg(test)]
pub(crate) fn admit_new_inline_unit_test(
    seam: &RepoSeam,
    index: &RustIndex,
) -> NewTestTargetAdmission {
    admit_new_inline_unit_test_with(seam, index, &InlineUnitLayoutMemo::default())
}

fn admit_new_inline_unit_test_with(
    seam: &RepoSeam,
    index: &RustIndex,
    layouts: &InlineUnitLayoutMemo,
) -> NewTestTargetAdmission {
    match try_admit_new_inline_unit_test(seam, index, layouts) {
        Ok((proposal, region)) => NewTestTargetAdmission {
            proposal: Some(proposal),
            region: Some(region),
            blocker: None,
        },
        Err(blocker) => NewTestTargetAdmission {
            proposal: None,
            region: None,
            blocker: Some(blocker),
        },
    }
}

fn try_admit_new_inline_unit_test(
    seam: &RepoSeam,
    index: &RustIndex,
    layouts: &InlineUnitLayoutMemo,
) -> Result<(NewTestTargetProposal, InlineTestRegionAuthority), NewTestProposalBlocker> {
    if !matches!(
        seam.kind(),
        SeamKind::PredicateBoundary
            | SeamKind::ErrorVariant
            | SeamKind::ReturnValue
            | SeamKind::FieldConstruction
            | SeamKind::MatchArm
    ) {
        return Err(NewTestProposalBlocker::InlineUnitOutOfScope);
    }
    if rust_index::is_test_file(seam.file()) {
        return Err(NewTestProposalBlocker::InlineUnitOutOfScope);
    }
    if path_is_generated_or_vendor(seam.file()) {
        return Err(NewTestProposalBlocker::GeneratedOrVendor);
    }
    if !is_relative_without_parent(seam.file()) {
        return Err(NewTestProposalBlocker::PathUnsafe);
    }

    let owner_fn = rust_index::find_owner_function(index, seam.file(), seam.display_line())
        .ok_or(NewTestProposalBlocker::OwnerUnresolved)?;
    if owner_fn.source_role != FunctionSourceRole::Production {
        return Err(NewTestProposalBlocker::OwnerUnresolved);
    }

    let facts = rust_index::find_file_facts(index, seam.file())
        .ok_or(NewTestProposalBlocker::OwnerUnresolved)?;
    if facts.used_lexical_fallback {
        return Err(NewTestProposalBlocker::LexicalFallback);
    }
    if facts
        .functions
        .iter()
        .any(|function| function.source_role == FunctionSourceRole::HarnessHelper)
    {
        return Err(NewTestProposalBlocker::CustomHarness);
    }

    let layout = layouts.layout(seam.file(), &facts.source);
    let modules = layout
        .modules
        .as_deref()
        .ok_or(NewTestProposalBlocker::LexicalFallback)?;
    let region = unique_inline_region(seam.file(), &facts.source, owner_fn, &layout, modules)?;

    Ok((
        NewTestTargetProposal {
            kind: NewTestKind::InlineUnit,
            file: normalize_relative(seam.file()),
            owner: seam.owner().to_string(),
            provenance: NewTestProposalProvenance::ProducerOwned,
        },
        region,
    ))
}

fn unique_inline_region(
    file: &Path,
    source: &str,
    owner_fn: &FunctionSummary,
    layout: &InlineUnitFileLayout,
    modules: &[GovernedCfgTestModule],
) -> Result<InlineTestRegionAuthority, NewTestProposalBlocker> {
    let owner_modules = layout
        .owner_module_paths
        .get(&owner_fn.start_line)
        .ok_or(NewTestProposalBlocker::OwnerUnresolved)?;
    let inline = modules
        .iter()
        .filter(|module| module.is_inline)
        .collect::<Vec<_>>();
    let out_of_line = modules
        .iter()
        .filter(|module| !module.is_inline)
        .collect::<Vec<_>>();

    if inline.is_empty() {
        if !out_of_line.is_empty() {
            return Err(NewTestProposalBlocker::OutOfLineModule);
        }
        return Err(NewTestProposalBlocker::NoTestModule);
    }
    if inline.len() != 1 || !out_of_line.is_empty() {
        return Err(NewTestProposalBlocker::AmbiguousModule);
    }
    let module = inline[0];
    if &module.parent_modules != owner_modules {
        return Err(NewTestProposalBlocker::OwnerInaccessible);
    }
    let body_start = module
        .body_start
        .ok_or(NewTestProposalBlocker::AmbiguousModule)?;
    let close_brace_start = module
        .close_brace_start
        .ok_or(NewTestProposalBlocker::AmbiguousModule)?;
    if body_start > close_brace_start || close_brace_start > source.len() {
        return Err(NewTestProposalBlocker::AmbiguousModule);
    }
    Ok(InlineTestRegionAuthority {
        file: normalize_relative(file),
        module_name: module.name.clone(),
        parent_modules: module.parent_modules.clone(),
        body_start,
        close_brace_start,
        source_digest: layout.source_digest.clone(),
    })
}

/// Parser-backed facts an InlineUnit admission reads from one file. Each
/// depends only on the file's source, so one parse and one digest per file
/// serve every seam in it; per seam, admission used to parse the whole file
/// twice and hash it once, which dominated cold `ripr pilot` time.
#[derive(Debug)]
struct InlineUnitFileLayout {
    /// `None` when the file is not parser-valid.
    modules: Option<Vec<GovernedCfgTestModule>>,
    /// Enclosing non-cfg-test modules of each function, by start line.
    owner_module_paths: BTreeMap<usize, Vec<String>>,
    source_digest: String,
}

/// Per-file [`InlineUnitFileLayout`] memo for one evidence pass. The pass
/// borrows the index immutably, so a file's source cannot change while the
/// memo lives.
#[derive(Debug, Default)]
pub(crate) struct InlineUnitLayoutMemo {
    layouts: RefCell<BTreeMap<PathBuf, Rc<InlineUnitFileLayout>>>,
}

impl InlineUnitLayoutMemo {
    fn layout(&self, file: &Path, source: &str) -> Rc<InlineUnitFileLayout> {
        if let Some(layout) = self.layouts.borrow().get(file) {
            return Rc::clone(layout);
        }
        let (modules, owner_module_paths) = match inline_unit_module_layout(source) {
            Some(layout) => (Some(layout.modules), layout.owner_module_paths),
            None => (None, BTreeMap::new()),
        };
        let layout = Rc::new(InlineUnitFileLayout {
            modules,
            owner_module_paths,
            source_digest: region::source_digest(source),
        });
        self.layouts
            .borrow_mut()
            .insert(file.to_path_buf(), Rc::clone(&layout));
        layout
    }
}

fn path_is_generated_or_vendor(path: &Path) -> bool {
    if is_generated_rust_file_with_patterns(path, &[]) {
        return true;
    }
    path.components().any(|component| {
        let Component::Normal(value) = component else {
            return false;
        };
        matches!(
            value.to_string_lossy().as_ref(),
            "vendor" | "vendored" | "node_modules" | "target" | ".git" | "fixtures"
        )
    })
}

pub(super) fn is_relative_without_parent(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

pub(super) fn normalize_relative(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}
