//! Producer-owned Integration `NewTestTargetProposal` admission (#4576).
//!
//! Ordinary public-library owners with an established `tests/` layout earn
//! one new integration-test file. This module does not invent expected
//! values or generate the test body.

use super::{
    NewTestKind, NewTestProposalBlocker, NewTestProposalProvenance, NewTestTargetAdmission,
    NewTestTargetProposal, is_relative_without_parent, normalize_relative,
};
use crate::analysis::facts::FunctionSourceRole;
use crate::analysis::rust_index::{self, FunctionSummary, RustIndex};
use crate::analysis::seams::{RepoSeam, SeamKind};
use std::path::{Path, PathBuf};

const SAFE_NEW_INTEGRATION_TEST_EVIDENCE: &str = "producer-owned new integration test proposal";

pub(super) fn integration_present_reason() -> &'static str {
    SAFE_NEW_INTEGRATION_TEST_EVIDENCE
}

/// Admit one Integration proposal from indexed package, visibility, and
/// Cargo discovery facts. Callers that already have a safe Existing target
/// still invoke this so Missing reasons stay typed; ranking prefers Existing.
pub(crate) fn admit_new_integration_test(
    seam: &RepoSeam,
    index: &RustIndex,
) -> NewTestTargetAdmission {
    match try_admit_new_integration_test(seam, index) {
        Ok(proposal) => NewTestTargetAdmission {
            proposal: Some(proposal),
            region: None,
            blocker: None,
        },
        Err(blocker) => NewTestTargetAdmission {
            proposal: None,
            region: None,
            blocker: Some(blocker),
        },
    }
}

fn try_admit_new_integration_test(
    seam: &RepoSeam,
    index: &RustIndex,
) -> Result<NewTestTargetProposal, NewTestProposalBlocker> {
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

    let owner_fn = rust_index::find_owner_function(index, seam.file(), seam.display_line())
        .ok_or(NewTestProposalBlocker::OwnerUnresolved)?;
    if owner_fn.source_role != FunctionSourceRole::Production {
        return Err(NewTestProposalBlocker::OwnerUnresolved);
    }

    let authority = index
        .workspace_authority
        .as_ref()
        .ok_or(NewTestProposalBlocker::PathUnsafe)?;
    let package = owning_package(&authority.root, seam.file())?;
    owner_is_public_library_item(index, owner_fn, &package)?;
    if !package.has_library_target {
        return Err(NewTestProposalBlocker::LibraryTargetUnresolved);
    }
    if !package.autotests {
        return Err(NewTestProposalBlocker::AutotestsDisabled);
    }
    if !package.has_established_tests_layout {
        return Err(NewTestProposalBlocker::MissingIntegrationLayout);
    }

    let proposed = proposed_integration_file(&authority.root, &package, owner_fn)?;
    if !is_root_contained_new_test_file(&authority.root, &proposed) {
        return Err(NewTestProposalBlocker::PathUnsafe);
    }

    Ok(NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: proposed,
        owner: format!("{}::{}", package.library_crate_name, owner_fn.name),
        provenance: NewTestProposalProvenance::ProducerOwned,
    })
}

struct PackageFacts {
    package_dir: PathBuf,
    library_root: PathBuf,
    library_crate_name: String,
    autotests: bool,
    has_library_target: bool,
    has_established_tests_layout: bool,
}

fn owning_package(
    root: &Path,
    relative_file: &Path,
) -> Result<PackageFacts, NewTestProposalBlocker> {
    let mut cursor = relative_file.parent().map(Path::to_path_buf);
    while let Some(directory) = cursor {
        let relative_manifest = directory.join("Cargo.toml");
        let manifest = root.join(&relative_manifest);
        if manifest.is_file() {
            let text = std::fs::read_to_string(&manifest)
                .map_err(|_read| NewTestProposalBlocker::LibraryTargetUnresolved)?;
            let value = text
                .parse::<toml::Table>()
                .map_err(|_parse| NewTestProposalBlocker::LibraryTargetUnresolved)?;
            let Some(package) = value.get("package").and_then(toml::Value::as_table) else {
                cursor = directory.parent().map(Path::to_path_buf);
                continue;
            };
            let package_name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .ok_or(NewTestProposalBlocker::LibraryTargetUnresolved)?
                .to_string();
            let autotests = package
                .get("autotests")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true);
            let explicit_lib = value.get("lib").and_then(toml::Value::as_table);
            let autolib = package
                .get("autolib")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true);
            let lib_name = explicit_lib
                .and_then(|lib| lib.get("name"))
                .and_then(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| package_name.replace('-', "_"));
            let lib_path = explicit_lib
                .and_then(|lib| lib.get("path"))
                .and_then(toml::Value::as_str)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("src/lib.rs"));
            let lib_source = root.join(&directory).join(&lib_path);
            let has_library_target = if explicit_lib.is_some() || autolib {
                lib_source.is_file()
            } else {
                false
            };
            let tests_dir = root.join(&directory).join("tests");
            let has_established_tests_layout = tests_dir.is_dir()
                && std::fs::read_dir(&tests_dir)
                    .map(|entries| {
                        entries.filter_map(Result::ok).any(|entry| {
                            entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs")
                        })
                    })
                    .unwrap_or(false);
            let library_root = if directory.as_os_str().is_empty() {
                normalize_relative(&lib_path)
            } else {
                normalize_relative(&directory.join(&lib_path))
            };
            return Ok(PackageFacts {
                package_dir: directory,
                library_root,
                library_crate_name: lib_name.replace('-', "_"),
                autotests,
                has_library_target,
                has_established_tests_layout,
            });
        }
        if directory.as_os_str().is_empty() {
            break;
        }
        cursor = directory.parent().map(Path::to_path_buf);
    }
    Err(NewTestProposalBlocker::LibraryTargetUnresolved)
}

fn owner_is_public_library_item(
    index: &RustIndex,
    owner_fn: &FunctionSummary,
    package: &PackageFacts,
) -> Result<(), NewTestProposalBlocker> {
    if normalize_relative(&owner_fn.file) != package.library_root {
        return Err(NewTestProposalBlocker::PrivateOwner);
    }
    let Some(facts) = index.files.get(&owner_fn.file) else {
        return Err(NewTestProposalBlocker::OwnerUnresolved);
    };
    if facts.used_lexical_fallback {
        return Err(NewTestProposalBlocker::OwnerUnresolved);
    }
    if !owner_symbol_is_crate_root(owner_fn) {
        return Err(NewTestProposalBlocker::PrivateOwner);
    }
    if function_item_is_crate_public(&facts.source, owner_fn.start_line) {
        Ok(())
    } else {
        Err(NewTestProposalBlocker::PrivateOwner)
    }
}

fn owner_symbol_is_crate_root(owner_fn: &FunctionSummary) -> bool {
    let expected = format!(
        "{}::{}",
        crate::analysis::stable_path_text(&owner_fn.file),
        owner_fn.name
    );
    owner_fn.id.0 == expected
}

fn function_item_is_crate_public(source: &str, start_line: usize) -> bool {
    let declaration = source
        .lines()
        .nth(start_line.saturating_sub(1))
        .unwrap_or("")
        .trim();
    crate_public_fn_declaration(declaration)
}

fn crate_public_fn_declaration(declaration: &str) -> bool {
    let Some(after_pub) = declaration.strip_prefix("pub") else {
        return false;
    };
    if after_pub.starts_with('(') {
        return false;
    }
    let rest = after_pub.trim_start();
    let rest = rest
        .strip_prefix("async")
        .map(str::trim_start)
        .unwrap_or(rest);
    let rest = rest
        .strip_prefix("const")
        .map(str::trim_start)
        .unwrap_or(rest);
    let rest = rest
        .strip_prefix("unsafe")
        .map(str::trim_start)
        .unwrap_or(rest);
    rest.starts_with("fn")
        && rest
            .as_bytes()
            .get(2)
            .is_none_or(|byte| byte.is_ascii_whitespace())
}

fn proposed_integration_file(
    root: &Path,
    package: &PackageFacts,
    owner_fn: &FunctionSummary,
) -> Result<PathBuf, NewTestProposalBlocker> {
    let stem = sanitize_test_file_stem(&owner_fn.name)?;
    let candidates = [
        format!("tests/{stem}.rs"),
        format!("tests/{stem}_boundary.rs"),
    ];
    for relative in candidates {
        let workspace_relative = if package.package_dir.as_os_str().is_empty() {
            PathBuf::from(&relative)
        } else {
            package.package_dir.join(&relative)
        };
        if leaf_is_absent(&root.join(&workspace_relative))? {
            return Ok(normalize_relative(&workspace_relative));
        }
    }
    Err(NewTestProposalBlocker::FileCollision)
}

fn sanitize_test_file_stem(name: &str) -> Result<String, NewTestProposalBlocker> {
    let stem: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        return Err(NewTestProposalBlocker::PathUnsafe);
    }
    Ok(stem)
}

fn is_root_contained_new_test_file(root: &Path, relative: &Path) -> bool {
    if !is_relative_without_parent(relative) {
        return false;
    }
    let normalized = relative.to_string_lossy().replace('\\', "/");
    if !rust_index::is_test_file(relative) {
        return false;
    }
    if !normalized.ends_with(".rs") {
        return false;
    }
    let forbidden = [
        "target/",
        ".git/",
        "vendor/",
        "node_modules/",
        "fixtures/",
        "generated/",
    ];
    if forbidden
        .iter()
        .any(|prefix| normalized.starts_with(prefix) || normalized.contains(&format!("/{prefix}")))
    {
        return false;
    }
    let full = root.join(relative);
    match leaf_is_absent(&full) {
        Ok(true) => {}
        Ok(false) | Err(_) => return false,
    }
    let Some(parent) = full.parent() else {
        return false;
    };
    let Ok(canonical_root) = root.canonicalize() else {
        return false;
    };
    parent
        .canonicalize()
        .is_ok_and(|canonical| canonical.starts_with(&canonical_root))
}

/// No-follow occupancy: only `NotFound` is a genuinely new leaf.
fn leaf_is_absent(path: &Path) -> Result<bool, NewTestProposalBlocker> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(NewTestProposalBlocker::PathUnsafe),
    }
}
