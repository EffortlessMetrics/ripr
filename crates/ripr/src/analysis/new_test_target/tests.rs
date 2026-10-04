use crate::analysis::ClassifiedSeam;
use crate::analysis::new_test_target::{
    NewTestKind, NewTestProposalBlocker, NewTestProposalProvenance, NewTestTargetAdmission,
    admit_new_inline_unit_test, validate_inline_region_edit,
};
use crate::analysis::repair_route::{RepairTargetSelection, repair_packet_eligibility};
use crate::analysis::rust_index::{self, RustIndex};
use crate::analysis::seam_inventory::{
    inventory_classified_seams_at, inventory_compact_classified_seams_at_with_config,
};
use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use crate::analysis::syntax::{
    governed_cfg_test_modules, inline_unit_module_layout, production_owner_module_path,
};
use crate::config::RiprConfig;
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
        std::env::temp_dir().join(format!("ripr-4784-{label}-{}-{stamp}", std::process::id()));
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

fn private_owner_with_inline_tests() -> &'static str {
    r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
"#
}

fn private_owner_without_tests() -> &'static str {
    r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}
"#
}

fn existing_related_test_source() -> &'static str {
    r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premium_customer_gets_discount() {
        let total = discounted_total(100, 10);
        assert!(total > 0);
    }
}
"#
}

fn write_library(root: &Path, package: &str, lib: &str) -> Result<(), String> {
    write_file(
        root,
        "Cargo.toml",
        &format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )?;
    write_file(root, "src/lib.rs", lib)
}

fn library_case(label: &str, lib: &str) -> Result<(FixtureRoot, Vec<ClassifiedSeam>), String> {
    let root = claim_root(label)?;
    write_library(&root.0, "pricing", lib)?;
    let classified = inventory_classified_seams_at(&root.0)?;
    Ok((root, classified))
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

fn missing_blocker(entry: &ClassifiedSeam) -> Result<NewTestProposalBlocker, String> {
    entry
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.blocker)
        .ok_or_else(|| "Missing should name a typed blocker".to_string())
}

fn assert_missing_with(
    entry: &ClassifiedSeam,
    expected: NewTestProposalBlocker,
) -> Result<(), String> {
    let readiness = repair_packet_eligibility(entry).readiness;
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "expected Missing, got {:?}",
            readiness.target_selection
        ));
    }
    let blocker = missing_blocker(entry)?;
    if blocker != expected {
        return Err(format!(
            "expected {} blocker, got {}",
            expected.as_str(),
            blocker.as_str()
        ));
    }
    Ok(())
}

fn admit_from_source(file: &str, source: &str) -> NewTestTargetAdmission {
    let mut index = RustIndex::default();
    index.insert_file_only(
        PathBuf::from(file),
        rust_index::summarize_file(PathBuf::from(file), source.to_string()),
    );
    let seam = RepoSeam::new(
        file,
        "discounted_total",
        SeamKind::PredicateBoundary,
        0,
        1,
        "amount >= threshold",
        RequiredDiscriminator::BoundaryValue {
            description: "amount >= threshold".to_string(),
        },
        ExpectedSink::ReturnValue,
    );
    admit_new_inline_unit_test(&seam, &index)
}

#[test]
fn governed_cfg_test_modules_skip_nested_and_fn_local_modules() -> Result<(), String> {
    let source = r#"
fn discounted_total(amount: i32, threshold: i32) -> i32 {
    #[cfg(test)]
    mod nested_in_fn {
        fn helper() {}
    }
    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    mod inner {
        fn helper() {}
    }
}
"#;
    let modules = governed_cfg_test_modules(source)
        .ok_or_else(|| "parser-valid source should yield governed modules".to_string())?;
    if modules
        .iter()
        .map(|module| (module.name.as_str(), module.parent_modules.as_slice()))
        .collect::<Vec<_>>()
        != vec![("tests", [].as_slice())]
    {
        return Err(format!(
            "fn-nested and cfg-test-nested modules must not compete as anchors: {modules:?}"
        ));
    }
    Ok(())
}

#[test]
fn cfg_all_test_still_counts_as_one_governed_module() -> Result<(), String> {
    let source = r#"
fn discounted_total(amount: i32, threshold: i32) -> i32 { amount }

#[cfg(all(test))]
mod unit {
    use super::*;
}
"#;
    let modules = governed_cfg_test_modules(source)
        .ok_or_else(|| "parser-valid source should yield governed modules".to_string())?;
    if modules.len() != 1 {
        return Err(format!("expected one module, got {modules:?}"));
    }
    if modules[0].name != "unit" || !modules[0].is_inline {
        return Err(format!("expected inline unit module, got {modules:?}"));
    }
    Ok(())
}

#[test]
fn production_owner_module_path_excludes_cfg_test_modules() -> Result<(), String> {
    let source = r#"
mod inner {
    fn discounted_total(amount: i32, threshold: i32) -> i32 { amount }

    #[cfg(test)]
    mod tests {
        use super::*;
    }
}
"#;
    let path = production_owner_module_path(source, 3)
        .ok_or_else(|| "owner on line 3 should resolve".to_string())?;
    if path != ["inner".to_string()] {
        return Err(format!("expected [inner], got {path:?}"));
    }
    Ok(())
}

#[test]
fn inline_unit_module_layout_matches_the_per_seam_queries_from_one_parse() -> Result<(), String> {
    // The memoized layout replaces one `governed_cfg_test_modules` and one
    // `production_owner_module_path` parse per seam. Pin the answers the
    // per-seam queries gave: cfg-test modules excluded from owner paths,
    // nested production modules kept in order, and the first function in
    // source order winning when two start on one line.
    let source = r#"
mod outer {
    mod inner {
        fn deep(amount: i32) -> i32 { amount }
    }
    fn first() -> i32 { 1 } mod same_line { fn second() {} }

    #[cfg(test)]
    mod tests {
        fn helper() {}
    }
}
fn top() {}
"#;
    let layout = inline_unit_module_layout(source)
        .ok_or_else(|| "parser-valid source should yield a layout".to_string())?;
    let (modules, paths) = (layout.modules, layout.owner_module_paths);
    let expected_modules = governed_cfg_test_modules(source)
        .ok_or_else(|| "parser-valid source should yield governed modules".to_string())?;
    if modules != expected_modules {
        return Err(format!(
            "modules diverged: {modules:?} vs {expected_modules:?}"
        ));
    }
    let owned = |names: &[&str]| {
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
    };
    let expected = [
        (4, owned(&["outer", "inner"])),
        (6, owned(&["outer"])),
        (10, owned(&["outer"])),
        (13, owned(&[])),
    ];
    for (line, path) in &expected {
        if paths.get(line) != Some(path) {
            return Err(format!(
                "line {line}: expected {path:?}, got {:?}",
                paths.get(line)
            ));
        }
        if production_owner_module_path(source, *line).as_ref() != Some(path) {
            return Err(format!(
                "line {line}: per-seam query disagrees with the layout"
            ));
        }
    }
    if paths.len() != expected.len() {
        return Err(format!("unexpected function lines: {paths:?}"));
    }
    if inline_unit_module_layout("fn broken( {").is_some() {
        return Err("a parse failure must yield no layout".to_string());
    }
    Ok(())
}

/// Positive: a same-file private owner plus one exact inline cfg(test) module
/// earns InlineUnit from the production inventory path.
#[test]
fn same_file_private_owner_with_one_inline_module_earns_inline_unit() -> Result<(), String> {
    let (_root, classified) = library_case("private-inline", private_owner_with_inline_tests())?;
    let entry = boundary_entry(&classified)?;
    let readiness = repair_packet_eligibility(entry).readiness;
    let RepairTargetSelection::Proposed(proposal) = &readiness.target_selection else {
        return Err(format!(
            "expected Proposed inline unit, got {:?}",
            readiness.target_selection
        ));
    };
    if proposal.kind != NewTestKind::InlineUnit {
        return Err(format!("expected InlineUnit, got {:?}", proposal.kind));
    }
    if proposal.provenance != NewTestProposalProvenance::ProducerOwned {
        return Err(format!(
            "expected producer-owned provenance, got {:?}",
            proposal.provenance
        ));
    }
    let file = proposal.file.to_string_lossy().replace('\\', "/");
    if file != "src/lib.rs" {
        return Err(format!(
            "proposal file should be the owner source, got {file}"
        ));
    }
    if !proposal.owner.contains("discounted_total") {
        return Err(format!(
            "proposal owner missed the item: {}",
            proposal.owner
        ));
    }
    let region = entry
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "proposal must carry #4783-shaped region authority".to_string())?;
    if region.module_name != "tests" {
        return Err(format!("expected module tests, got {}", region.module_name));
    }
    if !region.parent_modules.is_empty() {
        return Err(format!(
            "crate-root tests should have empty parent_modules, got {:?}",
            region.parent_modules
        ));
    }
    if readiness.test_target.is_some() {
        return Err("Proposed must not carry an Existing test_target identity".to_string());
    }
    if !readiness
        .present_evidence
        .iter()
        .any(|fact| fact.contains("producer-owned new inline unit test proposal"))
    {
        return Err(format!(
            "proposal should be present evidence, got {:?}",
            readiness.present_evidence
        ));
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

#[test]
fn unique_inline_module_need_not_be_named_tests() -> Result<(), String> {
    let (_root, classified) = library_case(
        "unit-name",
        r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod unit {
    use super::*;
}
"#,
    )?;
    let RepairTargetSelection::Proposed(proposal) =
        repair_packet_eligibility(boundary_entry(&classified)?)
            .readiness
            .target_selection
    else {
        return Err("unique module named unit must still earn InlineUnit".to_string());
    };
    let region = classified
        .iter()
        .find(|entry| entry.seam.owner().contains("discounted_total"))
        .and_then(|entry| entry.evidence.new_test_target.as_ref())
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "unit module must carry region identity".to_string())?;
    if region.module_name != "unit" {
        return Err(format!("expected module unit, got {}", region.module_name));
    }
    if proposal.kind != NewTestKind::InlineUnit {
        return Err(format!("expected InlineUnit, got {:?}", proposal.kind));
    }
    Ok(())
}

#[test]
fn nested_private_owner_with_same_module_tests_earns_inline_unit() -> Result<(), String> {
    let (_root, classified) = library_case(
        "nested-same",
        r#"mod inner {
    fn discounted_total(amount: i32, threshold: i32) -> i32 {
        if amount >= threshold {
            amount - 10
        } else {
            amount
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
    }
}
"#,
    )?;
    let inner = classified
        .iter()
        .find(|entry| {
            entry.seam.kind() == SeamKind::PredicateBoundary
                && entry.seam.owner().contains("inner")
                && entry.seam.owner().contains("discounted_total")
        })
        .ok_or_else(|| "expected inner::discounted_total seam".to_string())?;
    let RepairTargetSelection::Proposed(proposal) =
        &repair_packet_eligibility(inner).readiness.target_selection
    else {
        return Err(format!(
            "same-module nested tests must earn InlineUnit, got {:?}",
            repair_packet_eligibility(inner).readiness.target_selection
        ));
    };
    let region = inner
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "nested proposal must carry region".to_string())?;
    if region.parent_modules != ["inner".to_string()] {
        return Err(format!(
            "expected parent_modules [inner], got {:?}",
            region.parent_modules
        ));
    }
    if proposal.kind != NewTestKind::InlineUnit {
        return Err(format!("expected InlineUnit, got {:?}", proposal.kind));
    }
    Ok(())
}

#[test]
fn inline_tests_helper_function_is_not_a_custom_harness() -> Result<(), String> {
    let (_root, classified) = library_case(
        "cfg-helper",
        r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_amount() -> i32 {
        10
    }
}
"#,
    )?;
    match repair_packet_eligibility(boundary_entry(&classified)?)
        .readiness
        .target_selection
    {
        RepairTargetSelection::Proposed(proposal) if proposal.kind == NewTestKind::InlineUnit => {
            Ok(())
        }
        other => Err(format!(
            "cfg(test) helper must not be CustomHarness, got {other:?}"
        )),
    }
}

/// Existing exact related test remains preferred over an inline proposal.
#[test]
fn existing_exact_test_is_preferred_over_inline_proposal() -> Result<(), String> {
    let (_root, classified) = library_case("existing-preferred", existing_related_test_source())?;
    match repair_packet_eligibility(boundary_entry(&classified)?)
        .readiness
        .target_selection
    {
        RepairTargetSelection::Existing(_) => Ok(()),
        other => Err(format!("existing test must stay Existing, got {other:?}")),
    }
}

/// V1 does not create a missing inline test module.
#[test]
fn no_test_module_stays_missing() -> Result<(), String> {
    let (_root, classified) = library_case("no-module", private_owner_without_tests())?;
    assert_missing_with(
        boundary_entry(&classified)?,
        NewTestProposalBlocker::NoTestModule,
    )
}

/// Two plausible inline modules stay Missing as ambiguous.
#[test]
fn two_inline_modules_stay_missing_as_ambiguous() -> Result<(), String> {
    let (_root, classified) = library_case(
        "ambiguous",
        r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}

#[cfg(test)]
mod more_tests {
    use super::*;
}
"#,
    )?;
    assert_missing_with(
        boundary_entry(&classified)?,
        NewTestProposalBlocker::AmbiguousModule,
    )
}

/// Out-of-line `mod tests;` is a typed limitation, not an insertion target.
#[test]
fn out_of_line_test_module_is_a_typed_limitation() -> Result<(), String> {
    let root = claim_root("out-of-line")?;
    write_library(
        &root.0,
        "pricing",
        r#"fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests;
"#,
    )?;
    write_file(
        &root.0,
        "src/tests.rs",
        "#[test]\nfn crate_compiles() { assert!(true); }\n",
    )?;
    let classified = inventory_classified_seams_at(&root.0)?;
    assert_missing_with(
        boundary_entry(&classified)?,
        NewTestProposalBlocker::OutOfLineModule,
    )
}

/// A sibling source file with a same-named owner cannot supply the target.
#[test]
fn sibling_file_same_named_owner_cannot_supply_the_target() -> Result<(), String> {
    let root = claim_root("sibling")?;
    write_library(
        &root.0,
        "pricing",
        r#"mod other;

fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}
"#,
    )?;
    write_file(
        &root.0,
        "src/other.rs",
        r#"pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
"#,
    )?;
    let classified = inventory_classified_seams_at(&root.0)?;
    let lib_entry = classified
        .iter()
        .find(|entry| {
            entry.seam.kind() == SeamKind::PredicateBoundary
                && entry.seam.file().to_string_lossy().replace('\\', "/") == "src/lib.rs"
                && entry.seam.owner().contains("discounted_total")
        })
        .ok_or_else(|| "expected lib.rs discounted_total seam".to_string())?;
    let readiness = repair_packet_eligibility(lib_entry).readiness;
    if !matches!(readiness.target_selection, RepairTargetSelection::Missing) {
        return Err(format!(
            "sibling tests module must not supply lib.rs owner, got {:?}",
            readiness.target_selection
        ));
    }
    Ok(())
}

/// Private nested-module owner is not accessible from a crate-root tests module.
#[test]
fn nested_private_owner_without_same_module_tests_stays_missing() -> Result<(), String> {
    let (_root, classified) = library_case(
        "nested-private",
        r#"mod inner {
    fn discounted_total(amount: i32, threshold: i32) -> i32 {
        if amount >= threshold {
            amount - 10
        } else {
            amount
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
"#,
    )?;
    let inner = classified
        .iter()
        .find(|entry| {
            entry.seam.kind() == SeamKind::PredicateBoundary
                && entry.seam.owner().contains("inner")
                && entry.seam.owner().contains("discounted_total")
        })
        .ok_or_else(|| "expected inner::discounted_total seam".to_string())?;
    assert_missing_with(inner, NewTestProposalBlocker::OwnerInaccessible)
}

/// External in-region test addition binds back as Existing.
#[test]
fn external_inline_test_addition_binds_to_existing() -> Result<(), String> {
    let root = claim_root("after-edit")?;
    write_library(&root.0, "pricing", private_owner_with_inline_tests())?;
    let before = inventory_classified_seams_at(&root.0)?;
    let before_entry = boundary_entry(&before)?;
    let before_readiness = repair_packet_eligibility(before_entry).readiness;
    if !matches!(
        before_readiness.target_selection,
        RepairTargetSelection::Proposed(_)
    ) {
        return Err(format!(
            "expected a proposal before the edit, got {:?}",
            before_readiness.target_selection
        ));
    }
    let region = before_entry
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "before-edit proposal must carry a region".to_string())?
        .clone();
    let before_source = fs::read_to_string(root.0.join("src/lib.rs"))
        .map_err(|error| format!("read before source: {error}"))?;
    let after_source = format!(
        "{}{}{}",
        &before_source[..region.close_brace_start],
        "\n    #[test]\n    fn discounted_total_at_threshold() {\n        assert_eq!(discounted_total(10, 10), 0);\n    }\n",
        &before_source[region.close_brace_start..]
    );
    validate_inline_region_edit(&before_source, &after_source, &region)
        .map_err(|blocker| format!("in-region edit must be admitted: {}", blocker.as_str()))?;
    write_file(&root.0, "src/lib.rs", &after_source)?;

    let after = inventory_classified_seams_at(&root.0)?;
    let after_entry = boundary_entry(&after)?;
    match repair_packet_eligibility(after_entry)
        .readiness
        .target_selection
    {
        RepairTargetSelection::Existing(target) => {
            let file = target.file().to_string_lossy().replace('\\', "/");
            if file != "src/lib.rs" {
                return Err(format!("after-edit Existing file was {file}"));
            }
            Ok(())
        }
        other => Err(format!(
            "external inline test must become Existing, got {other:?}"
        )),
    }
}

/// Production text in the same file is rejected by the carried region authority.
#[test]
fn production_edit_in_the_same_file_is_rejected() -> Result<(), String> {
    let (_root, classified) = library_case("production-edit", private_owner_with_inline_tests())?;
    let entry = boundary_entry(&classified)?;
    let region = entry
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "proposal must carry a region".to_string())?;
    let before = private_owner_with_inline_tests();
    let after = before.replacen("amount - 10", "amount - 11", 1);
    match validate_inline_region_edit(before, &after, region) {
        Err(NewTestProposalBlocker::ProductionEdit) => Ok(()),
        other => Err(format!(
            "production edit must be ProductionEdit, got {other:?}"
        )),
    }
}

#[test]
fn stale_source_digest_rejects_the_region_edit() -> Result<(), String> {
    let (_root, classified) = library_case("stale-digest", private_owner_with_inline_tests())?;
    let region = boundary_entry(&classified)?
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "proposal must carry a region".to_string())?;
    match validate_inline_region_edit("fn other() {}", private_owner_with_inline_tests(), region) {
        Err(NewTestProposalBlocker::StaleSource) => Ok(()),
        other => Err(format!(
            "stale before-text must be StaleSource, got {other:?}"
        )),
    }
}

#[test]
fn renamed_inline_module_invalidates_the_region() -> Result<(), String> {
    let (_root, classified) = library_case("renamed-module", private_owner_with_inline_tests())?;
    let region = boundary_entry(&classified)?
        .evidence
        .new_test_target
        .as_ref()
        .and_then(|admission| admission.region.as_ref())
        .ok_or_else(|| "proposal must carry a region".to_string())?
        .clone();
    let after = private_owner_with_inline_tests().replace("mod tests", "mod unit");
    match validate_inline_region_edit(private_owner_with_inline_tests(), &after, &region) {
        Err(NewTestProposalBlocker::AmbiguousModule | NewTestProposalBlocker::ProductionEdit) => {
            Ok(())
        }
        other => Err(format!(
            "renamed module must not remain a valid region, got {other:?}"
        )),
    }
}

/// Existing and Proposed remain distinct typed yields.
#[test]
fn existing_and_proposed_inline_yields_stay_distinct() -> Result<(), String> {
    let (_proposed_root, proposed_classified) =
        library_case("yield-proposed", private_owner_with_inline_tests())?;
    let proposed = repair_packet_eligibility(boundary_entry(&proposed_classified)?).readiness;

    let (_existing_root, existing_classified) =
        library_case("yield-existing", existing_related_test_source())?;
    let existing = repair_packet_eligibility(boundary_entry(&existing_classified)?).readiness;

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
    let proposed_json = serde_json::to_value(&proposed.target_selection)
        .map_err(|error| format!("serialize proposed: {error}"))?;
    let existing_json = serde_json::to_value(&existing.target_selection)
        .map_err(|error| format!("serialize existing: {error}"))?;
    if proposed_json.get("proposed").is_none() {
        return Err(format!(
            "proposed JSON lost the Proposed arm: {proposed_json}"
        ));
    }
    if existing_json.get("existing").is_none() {
        return Err(format!(
            "existing JSON lost the Existing arm: {existing_json}"
        ));
    }
    Ok(())
}

/// Equivalent checkout spelling does not enter the portable proposal identity.
#[test]
fn proposal_file_identity_is_root_relative() -> Result<(), String> {
    let (_root, classified) = library_case("relative-id", private_owner_with_inline_tests())?;
    let RepairTargetSelection::Proposed(proposal) =
        repair_packet_eligibility(boundary_entry(&classified)?)
            .readiness
            .target_selection
    else {
        return Err("expected Proposed".to_string());
    };
    let file = proposal.file.to_string_lossy();
    if file.contains(':') || Path::new(file.as_ref()).is_absolute() || file.contains("ripr-4784") {
        return Err(format!(
            "proposal file leaked a checkout path: {}",
            proposal.file.display()
        ));
    }
    Ok(())
}

#[test]
fn compact_inventory_does_not_admit_inline_unit_proposals() -> Result<(), String> {
    let root = claim_root("compact")?;
    write_library(&root.0, "pricing", private_owner_with_inline_tests())?;
    let compact =
        inventory_compact_classified_seams_at_with_config(&root.0, &RiprConfig::default())?;
    let entry = boundary_entry(&compact)?;
    if entry.evidence.new_test_target.is_some() {
        return Err("compact evidence must omit new-test proposals".to_string());
    }
    Ok(())
}

#[test]
fn tests_dir_harness_file_is_out_of_scope() {
    let admission = admit_from_source("tests/pricing.rs", private_owner_with_inline_tests());
    assert_eq!(
        admission.blocker,
        Some(NewTestProposalBlocker::InlineUnitOutOfScope)
    );
    assert!(admission.proposal.is_none());
}

#[test]
fn generated_source_path_is_a_typed_limitation() {
    let admission = admit_from_source(
        "src/generated/pricing.rs",
        private_owner_with_inline_tests(),
    );
    assert_eq!(
        admission.blocker,
        Some(NewTestProposalBlocker::GeneratedOrVendor)
    );
}

#[test]
fn vendor_source_path_is_a_typed_limitation() {
    let admission = admit_from_source(
        "vendor/pricing/src/lib.rs",
        private_owner_with_inline_tests(),
    );
    assert_eq!(
        admission.blocker,
        Some(NewTestProposalBlocker::GeneratedOrVendor)
    );
}

#[test]
fn parent_dir_escape_is_path_unsafe() {
    let admission = admit_from_source("../src/lib.rs", private_owner_with_inline_tests());
    assert_eq!(admission.blocker, Some(NewTestProposalBlocker::PathUnsafe));
}
