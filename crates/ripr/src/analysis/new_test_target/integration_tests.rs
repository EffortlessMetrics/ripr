use crate::analysis::ClassifiedSeam;
use crate::analysis::facts::build_index;
use crate::analysis::new_test_target::admit_new_integration_test;
use crate::analysis::new_test_target::{
    NewTestKind, NewTestProposalBlocker, NewTestProposalProvenance,
};
#[cfg(unix)]
use crate::analysis::repair_route::RepairRouteReadiness;
use crate::analysis::repair_route::{RepairTargetSelection, repair_packet_eligibility};
use crate::analysis::seam_inventory::inventory_classified_seams_at;
use crate::analysis::seams::SeamKind;
use crate::analysis::workspace::discover_rust_files;
use crate::app::repair_attempt::edit_cage_policy_from_packet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct FixtureRoot(PathBuf);

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn claim_root(label: &str) -> Result<FixtureRoot, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("ripr-4576-{label}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&root).map_err(|error| format!("create fixture root: {error}"))?;
    Ok(FixtureRoot(root))
}

fn write_file(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents).map_err(|error| format!("write {relative}: {error}"))
}

fn public_library_source() -> &'static str {
    r#"/// Ordinary public crate-root item.
#[inline]
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}
"#
}

fn private_library_source() -> &'static str {
    r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}
"#
}

fn unrelated_smoke_test() -> &'static str {
    r#"#[test]
fn crate_compiles() {
    assert!(true);
}
"#
}

fn write_ordinary_package(
    root: &Path,
    package: &str,
    autotests: Option<bool>,
) -> Result<(), String> {
    let autotests_line = match autotests {
        Some(value) => format!("autotests = {value}\n"),
        None => String::new(),
    };
    write_file(
        root,
        "Cargo.toml",
        &format!(
            "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{autotests_line}"
        ),
    )?;
    write_file(root, "src/lib.rs", public_library_source())?;
    write_file(root, "tests/smoke.rs", unrelated_smoke_test())
}

fn boundary_entry(classified: &[ClassifiedSeam]) -> Result<&ClassifiedSeam, String> {
    classified
        .iter()
        .find(|entry| {
            entry.seam.kind() == SeamKind::PredicateBoundary
                && entry.seam.owner().contains("discounted_total")
        })
        .ok_or_else(|| {
            format!(
                "expected a discounted_total predicate seam, got {}",
                classified
                    .iter()
                    .map(|entry| format!(
                        "{}:{}:{}",
                        entry.seam.file().display(),
                        entry.seam.owner(),
                        entry.seam.kind().as_str()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn integration_admission_for(
    root: &Path,
    entry: &ClassifiedSeam,
) -> Result<crate::analysis::new_test_target::NewTestTargetAdmission, String> {
    let files = discover_rust_files(root)?;
    let index = build_index(root, &files)?;
    Ok(admit_new_integration_test(&entry.seam, &index))
}

fn proposed_file_display(selection: &RepairTargetSelection) -> Result<String, String> {
    match selection {
        RepairTargetSelection::Proposed(proposal) => {
            Ok(proposal.file.to_string_lossy().replace('\\', "/"))
        }
        other => Err(format!("expected Proposed, got {other:?}")),
    }
}

/// Positive: ordinary public library + established `tests/` layout earns one
/// Integration proposal. A test-only constructor is not this path — inventory
/// builds a RustIndex and readiness consumes it.
#[test]
fn public_library_with_established_tests_layout_earns_integration_proposal() -> Result<(), String> {
    let root = claim_root("public-layout")?;
    write_ordinary_package(&root.0, "pricing", None)?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    let RepairTargetSelection::Proposed(proposal) = &readiness.target_selection else {
        return Err(format!(
            "expected Proposed integration target, got {:?}",
            readiness.target_selection
        ));
    };
    if proposal.kind != NewTestKind::Integration {
        return Err(format!("expected Integration, got {:?}", proposal.kind));
    }
    if proposal.provenance != NewTestProposalProvenance::ProducerOwned {
        return Err(format!(
            "expected producer-owned provenance, got {:?}",
            proposal.provenance
        ));
    }
    if !proposal.owner.contains("discounted_total") {
        return Err(format!(
            "proposal owner missed the public item: {}",
            proposal.owner
        ));
    }
    let file = proposal.file.to_string_lossy().replace('\\', "/");
    if file.starts_with("src/") || !file.ends_with(".rs") {
        return Err(format!(
            "proposed file is not a new integration test file: {file}"
        ));
    }
    if root.0.join(&proposal.file).exists() {
        return Err(format!("proposal named an already-existing file: {file}"));
    }
    if readiness.test_target.is_some() {
        return Err("Proposed must not carry an Existing test_target identity".to_string());
    }
    if !readiness.is_repair_ready() {
        return Err(format!("proposal route was not ready: {readiness:?}"));
    }
    let serialized =
        serde_json::to_string(proposal).map_err(|error| format!("serialize proposal: {error}"))?;
    if serialized.contains("amount - 10") || serialized.contains("assert_eq") {
        return Err(format!(
            "proposal invented an expected value or test body: {serialized}"
        ));
    }
    Ok(())
}

/// Negative: the same public owner with Cargo autodiscovery disabled cannot
/// earn a new integration file without a Cargo.toml change.
#[test]
fn disabled_autotests_without_explicit_target_stays_missing() -> Result<(), String> {
    let root = claim_root("autotests-off")?;
    write_ordinary_package(&root.0, "pricing", Some(false))?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    if matches!(
        readiness.target_selection,
        RepairTargetSelection::Proposed(_)
    ) {
        return Err(format!(
            "autotests=false must not earn a proposal: {:?}",
            readiness.target_selection
        ));
    }
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing, got {:?}",
            readiness.target_selection
        ));
    }
    let missing = readiness.missing_evidence.join(" | ");
    if !missing.contains("autotests") && !missing.contains("discovery") {
        return Err(format!(
            "Missing must name the Cargo discovery blocker, got {missing:?}"
        ));
    }
    Ok(())
}

/// Negative: a private owner would require a production visibility edit.
#[test]
fn private_owner_stays_missing_without_an_inline_unit_proposal() -> Result<(), String> {
    let root = claim_root("private-owner")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_file(&root.0, "src/lib.rs", private_library_source())?;
    write_file(&root.0, "tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;
    let integration = integration_admission_for(&root.0, entry)?;

    if integration.blocker != Some(NewTestProposalBlocker::PrivateOwner) {
        return Err(format!(
            "Integration producer must name PrivateOwner, got {:?}",
            integration.blocker
        ));
    }
    if let RepairTargetSelection::Proposed(proposal) = &readiness.target_selection {
        return Err(format!(
            "private owner must not earn {:?} {:?}",
            proposal.kind, proposal.file
        ));
    }
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing, got {:?}",
            readiness.target_selection
        ));
    }
    Ok(())
}

/// Negative: a `pub fn` inside a private module is not a crate-root public
/// library item. An integration test cannot name it, so the route stays
/// Missing instead of proposing an unbuildable file.
#[test]
fn public_fn_inside_private_module_stays_missing() -> Result<(), String> {
    let root = claim_root("private-module")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_file(
        &root.0,
        "src/lib.rs",
        r#"mod hidden {
    pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
        if amount >= threshold {
            amount - 10
        } else {
            amount
        }
    }
}
"#,
    )?;
    write_file(&root.0, "tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;
    let integration = integration_admission_for(&root.0, entry)?;

    if integration.blocker != Some(NewTestProposalBlocker::PrivateOwner) {
        return Err(format!(
            "Integration producer must name PrivateOwner, got {:?}",
            integration.blocker
        ));
    }
    if let RepairTargetSelection::Proposed(proposal) = &readiness.target_selection {
        return Err(format!(
            "private-module owner must not earn {:?} {}",
            proposal.kind,
            proposal.file.display()
        ));
    }
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing, got {:?}",
            readiness.target_selection
        ));
    }
    Ok(())
}

/// Positive: a documented, attributed crate-root `pub fn` in a declared
/// `[lib] path` still earns one Integration proposal.
#[test]
fn documented_public_item_on_declared_lib_path_earns_integration_proposal() -> Result<(), String> {
    let root = claim_root("declared-lib-path")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/api.rs\"\n",
    )?;
    write_file(&root.0, "src/api.rs", public_library_source())?;
    write_file(&root.0, "tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    let file = proposed_file_display(&readiness.target_selection)?;
    if file.starts_with("src/") || !file.ends_with(".rs") {
        return Err(format!(
            "declared lib path proposed a non-test file: {file}"
        ));
    }
    if !readiness.is_repair_ready() {
        return Err(format!(
            "documented public item on declared lib path was not ready: {readiness:?}"
        ));
    }
    Ok(())
}

/// Existing safe target still wins over a new-file proposal.
#[test]
fn existing_related_integration_test_is_preferred() -> Result<(), String> {
    let root = claim_root("existing-preferred")?;
    write_ordinary_package(&root.0, "pricing", None)?;
    write_file(
        &root.0,
        "tests/pricing.rs",
        r#"#[test]
fn premium_customer_gets_discount() {
    let total = pricing::discounted_total(100, 10);
    assert!(total > 0);
}
"#,
    )?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    match &readiness.target_selection {
        RepairTargetSelection::Existing(target) => {
            let file = target.file().to_string_lossy().replace('\\', "/");
            if file.starts_with("src/") {
                return Err(format!(
                    "existing target left the integration layout: {file}"
                ));
            }
        }
        other => {
            return Err(format!("expected Existing related test, got {other:?}"));
        }
    }
    Ok(())
}

/// A same-named public owner in a sibling package is not a substitute.
#[test]
fn multi_package_workspace_selects_the_owner_package() -> Result<(), String> {
    let root = claim_root("workspace-packages")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\n",
    )?;
    write_file(
        &root.0,
        "alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_file(&root.0, "alpha/src/lib.rs", public_library_source())?;
    write_file(&root.0, "alpha/tests/smoke.rs", unrelated_smoke_test())?;
    write_file(
        &root.0,
        "beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_file(&root.0, "beta/src/lib.rs", public_library_source())?;
    write_file(&root.0, "beta/tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let alpha = classified
        .iter()
        .find(|entry| {
            entry.seam.kind() == SeamKind::PredicateBoundary
                && entry
                    .seam
                    .file()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .starts_with("alpha/")
                && entry.seam.owner().contains("discounted_total")
        })
        .ok_or_else(|| "expected alpha discounted_total seam".to_string())?;
    let readiness = repair_packet_eligibility(alpha).readiness;
    let file = proposed_file_display(&readiness.target_selection)?;
    if !file.starts_with("alpha/") || file.contains("/src/") {
        return Err(format!("alpha owner proposed a non-alpha file: {file}"));
    }
    if file.contains("beta/") {
        return Err(format!("sibling package captured the proposal: {file}"));
    }
    Ok(())
}

/// An external new-file edit binds back to the original proposal path and
/// becomes Existing. RIPR does not write the test.
#[test]
fn external_new_file_binds_to_the_original_proposal() -> Result<(), String> {
    let root = claim_root("after-edit")?;
    write_ordinary_package(&root.0, "pricing", None)?;

    let before = inventory_classified_seams_at(&root.0)?;
    let before_entry = boundary_entry(&before)?;
    let before_readiness = repair_packet_eligibility(before_entry).readiness;
    let proposed = match &before_readiness.target_selection {
        RepairTargetSelection::Proposed(proposal) => proposal.file.clone(),
        other => {
            return Err(format!(
                "expected a proposal before the edit, got {other:?}"
            ));
        }
    };
    if root.0.join(&proposed).exists() {
        return Err("proposal file already existed before the external edit".to_string());
    }

    write_file(
        &root.0,
        &proposed.to_string_lossy(),
        r#"#[test]
fn discounted_total_at_threshold() {
    assert_eq!(pricing::discounted_total(10, 10), 0);
}
"#,
    )?;

    let after = inventory_classified_seams_at(&root.0)?;
    let after_entry = boundary_entry(&after)?;
    let after_readiness = repair_packet_eligibility(after_entry).readiness;
    match &after_readiness.target_selection {
        RepairTargetSelection::Existing(target) => {
            let file = target.file().to_string_lossy().replace('\\', "/");
            let expected = proposed.to_string_lossy().replace('\\', "/");
            if file != expected {
                return Err(format!(
                    "after-edit Existing {file} did not bind {expected}"
                ));
            }
        }
        other => {
            return Err(format!(
                "external new test must become Existing, got {other:?}"
            ));
        }
    }
    Ok(())
}

/// The proposal names a test-only edit surface. The existing cage rejects
/// turning the production owner file into the authored target.
#[test]
fn proposal_edit_surface_is_test_only_and_cage_rejects_production() -> Result<(), String> {
    let root = claim_root("cage")?;
    write_ordinary_package(&root.0, "pricing", None)?;
    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;
    let file = proposed_file_display(&readiness.target_selection)?;

    let accepted = serde_json::json!({
        "seam_id": entry.seam.id().as_str(),
        "allowed_edit_surface": [file],
        "forbidden_files": ["src/lib.rs", "Cargo.toml"]
    })
    .to_string();
    edit_cage_policy_from_packet(&accepted, entry.seam.id().as_str())
        .map_err(|error| format!("proposal file must be a cage-legal test surface: {error}"))?;

    let refused = serde_json::json!({
        "seam_id": entry.seam.id().as_str(),
        "allowed_edit_surface": ["src/lib.rs"],
        "forbidden_files": []
    })
    .to_string();
    if edit_cage_policy_from_packet(&refused, entry.seam.id().as_str()).is_ok() {
        return Err("production src/lib.rs must not become the authored edit target".to_string());
    }
    Ok(())
}

/// Existing vs Proposed remain distinct typed yields (#3076).
#[test]
fn existing_and_proposed_route_yields_stay_distinct() -> Result<(), String> {
    let proposed_root = claim_root("yield-proposed")?;
    write_ordinary_package(&proposed_root.0, "pricing", None)?;
    let proposed = repair_packet_eligibility(boundary_entry(&inventory_classified_seams_at(
        &proposed_root.0,
    )?)?)
    .readiness;

    let existing_root = claim_root("yield-existing")?;
    write_ordinary_package(&existing_root.0, "pricing", None)?;
    write_file(
        &existing_root.0,
        "tests/pricing.rs",
        r#"#[test]
fn premium_customer_gets_discount() {
    let total = pricing::discounted_total(100, 10);
    assert!(total > 0);
}
"#,
    )?;
    let existing = repair_packet_eligibility(boundary_entry(&inventory_classified_seams_at(
        &existing_root.0,
    )?)?)
    .readiness;

    assert!(
        matches!(
            proposed.target_selection,
            RepairTargetSelection::Proposed(_)
        ),
        "no-existing-test yield must be Proposed: {:?}",
        proposed.target_selection
    );
    assert!(
        matches!(
            existing.target_selection,
            RepairTargetSelection::Existing(_)
        ),
        "existing-test yield must stay Existing: {:?}",
        existing.target_selection
    );
    assert_ne!(
        std::mem::discriminant(&proposed.target_selection),
        std::mem::discriminant(&existing.target_selection)
    );
    Ok(())
}

/// Negative: a retained `src/lib.rs` is not a Cargo library target when
/// `autolib = false` and no explicit `[lib]` table exists. File presence
/// alone must not earn a proposal.
#[test]
fn retained_lib_rs_with_autolib_false_and_no_explicit_lib_stays_missing() -> Result<(), String> {
    let root = claim_root("autolib-off")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2024\"\nautolib = false\n",
    )?;
    write_file(&root.0, "src/lib.rs", public_library_source())?;
    write_file(&root.0, "tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    if matches!(
        readiness.target_selection,
        RepairTargetSelection::Proposed(_)
    ) {
        return Err(format!(
            "autolib=false without [lib] must not earn a proposal: {:?}",
            readiness.target_selection
        ));
    }
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing, got {:?}",
            readiness.target_selection
        ));
    }
    let missing = readiness.missing_evidence.join(" | ");
    if !missing.contains("library") && !missing.contains("unresolved") {
        return Err(format!(
            "Missing must name the library-target blocker, got {missing:?}"
        ));
    }
    Ok(())
}

/// Positive: an explicit `[lib]` table still admits the package when
/// `autolib = false`. Cargo keeps the declared library; only autodiscovery
/// is suppressed.
#[test]
fn explicit_lib_under_autolib_false_still_earns_integration_proposal() -> Result<(), String> {
    let root = claim_root("autolib-off-explicit-lib")?;
    write_file(
        &root.0,
        "Cargo.toml",
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2024\"\nautolib = false\n\n[lib]\npath = \"src/lib.rs\"\n",
    )?;
    write_file(&root.0, "src/lib.rs", public_library_source())?;
    write_file(&root.0, "tests/smoke.rs", unrelated_smoke_test())?;

    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;

    let file = proposed_file_display(&readiness.target_selection)?;
    if file.starts_with("src/") || !file.ends_with(".rs") {
        return Err(format!(
            "explicit [lib] under autolib=false proposed a non-test file: {file}"
        ));
    }
    if !readiness.is_repair_ready() {
        return Err(format!(
            "explicit [lib] under autolib=false was not ready: {readiness:?}"
        ));
    }
    Ok(())
}

/// Negative: a dangling leaf at every safe candidate is occupancy, not
/// absence. `exists`/`try_exists` follow the link and would treat the
/// path as free; no-follow metadata must refuse it.
///
/// Inventory cannot read a dangling `tests/*.rs` leaf, so the seam is
/// established first; the production producer then re-admits against
/// that index after the leaves are occupied.
#[cfg(unix)]
#[test]
fn dangling_proposed_file_symlink_is_not_a_new_safe_target() -> Result<(), String> {
    let root = claim_root("dangling-symlink")?;
    write_ordinary_package(&root.0, "pricing", None)?;
    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let before =
        proposed_file_display(&repair_packet_eligibility(entry).readiness.target_selection)?;
    if !before.ends_with("tests/discounted_total.rs") {
        return Err(format!(
            "fixture did not propose the first candidate: {before}"
        ));
    }

    let readiness = readiness_after_occupying_proposed_leaves(
        &root.0,
        entry,
        &[
            "tests/discounted_total.rs",
            "tests/discounted_total_boundary.rs",
        ],
    )?;

    if let RepairTargetSelection::Proposed(proposal) = &readiness.target_selection {
        return Err(format!(
            "dangling proposed-file symlink must not be admitted as {}: {:?}",
            proposal.file.display(),
            proposal.kind
        ));
    }
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing after occupied candidate leaves, got {:?}",
            readiness.target_selection
        ));
    }
    let missing = readiness.missing_evidence.join(" | ");
    if !missing.contains("collid") && !missing.contains("exist") && !missing.contains("path") {
        return Err(format!(
            "Missing must name the occupied-leaf blocker, got {missing:?}"
        ));
    }
    Ok(())
}

/// The first occupied dangling leaf is skipped; a genuinely absent
/// second candidate may still earn a proposal.
#[cfg(unix)]
#[test]
fn dangling_first_candidate_skips_to_a_free_boundary_file() -> Result<(), String> {
    let root = claim_root("dangling-first")?;
    write_ordinary_package(&root.0, "pricing", None)?;
    let classified = inventory_classified_seams_at(&root.0)?;
    let entry = boundary_entry(&classified)?;
    let readiness =
        readiness_after_occupying_proposed_leaves(&root.0, entry, &["tests/discounted_total.rs"])?;
    let file = proposed_file_display(&readiness.target_selection)?;
    if file.ends_with("tests/discounted_total.rs") {
        return Err(format!(
            "first-candidate dangling symlink was treated as a new file: {file}"
        ));
    }
    if !file.ends_with("tests/discounted_total_boundary.rs") {
        return Err(format!("expected the free boundary candidate, got {file}"));
    }
    if leaf_is_present(&root.0.join(&file))? {
        return Err(format!("second candidate was not a free leaf: {file}"));
    }
    if !readiness.is_repair_ready() {
        return Err(format!(
            "free second candidate was not ready: {readiness:?}"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn readiness_after_occupying_proposed_leaves(
    root: &Path,
    entry: &ClassifiedSeam,
    occupied: &[&str],
) -> Result<RepairRouteReadiness, String> {
    let files = discover_rust_files(root)?;
    let index = build_index(root, &files)?;
    for relative in occupied {
        dangling_symlink(root, relative)?;
    }
    let mut occupied_entry = entry.clone();
    occupied_entry.evidence.new_test_target = Some(admit_new_integration_test(&entry.seam, &index));
    Ok(repair_packet_eligibility(&occupied_entry).readiness)
}

#[cfg(unix)]
fn dangling_symlink(root: &Path, relative: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    std::os::unix::fs::symlink("ripr-4576-missing-target", &path)
        .map_err(|error| format!("symlink {relative}: {error}"))
}

#[cfg(unix)]
fn leaf_is_present(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("inspect {}: {error}", path.display())),
    }
}
