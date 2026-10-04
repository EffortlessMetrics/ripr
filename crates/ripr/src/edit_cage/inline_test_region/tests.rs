use super::{
    InlineTestRegionRejectReason, InlineTestRegionStatus, authority_from_source,
    capture_inline_test_region_authority, observe_inline_test_region,
    validate_inline_test_region_edit,
};
use crate::testing::fixture_git::fixture_git_ok as git_ok;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LIB: &str = r#"pub fn price(cents: i32) -> i32 {
    cents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing() {
        assert_eq!(price(1), 1);
    }
}
"#;

const ADDED_TEST: &str = r#"pub fn price(cents: i32) -> i32 {
    cents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing() {
        assert_eq!(price(1), 1);
    }

    #[test]
    fn added() {
        assert_eq!(price(2), 2);
    }
}
"#;

fn authority(source: &str) -> Result<super::InlineTestRegionAuthority, String> {
    authority_from_source("src/lib.rs", source, "tests", "pkg:test".to_string(), &[])
        .map_err(|error| format!("fixture source must yield one exact tests region: {error:?}"))
}

fn assert_rejected(
    before: &str,
    after: &str,
    expected: InlineTestRegionRejectReason,
) -> Result<(), String> {
    let verdict = validate_inline_test_region_edit(before, &authority(before)?, after);
    if verdict.status != InlineTestRegionStatus::Rejected {
        return Err(format!("expected rejected {expected:?}, got {:?}", verdict));
    }
    if verdict.reason != Some(expected) {
        return Err(format!(
            "expected reason {expected:?}, got {:?}",
            verdict.reason
        ));
    }
    Ok(())
}

fn observe_failure(source: &str) -> Result<super::InlineTestRegionError, String> {
    match observe_inline_test_region(source, "tests") {
        Err(error) => Ok(error),
        Ok(_) => Err("expected region observation to fail".to_string()),
    }
}

fn authority_failure(relative: &str, source: &str) -> Result<super::InlineTestRegionError, String> {
    match authority_from_source(relative, source, "tests", "pkg:test".to_string(), &[]) {
        Err(error) => Ok(error),
        Ok(_) => Err(format!("expected authority capture to fail for {relative}")),
    }
}

/// Weaker file-level oracle used only to prove the region check is
/// discriminating: it accepts any same-file parseable edit that adds a
/// `fn` anywhere. A production-and-test combined edit must pass this and fail
/// the region cage.
fn file_level_only_would_admit(before: &str, after: &str) -> bool {
    if before == after {
        return false;
    }
    crate::analysis::parse_clean_source_file(after).is_some()
        && after.contains("fn ")
        && after.len() > before.len()
}

#[test]
fn adding_one_test_function_inside_the_named_module_is_admitted() -> Result<(), String> {
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, ADDED_TEST);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    assert_eq!(verdict.reason, None);
    Ok(())
}

#[test]
fn changing_the_production_owner_in_the_same_file_is_rejected() -> Result<(), String> {
    let after = LIB.replace("cents\n", "cents + 1\n");
    assert_rejected(LIB, &after, InlineTestRegionRejectReason::ProductionEdit)
}

#[test]
fn adding_a_production_helper_beside_the_test_module_is_rejected() -> Result<(), String> {
    let after = format!("{LIB}\nfn helper() {{}}\n");
    assert_rejected(LIB, &after, InlineTestRegionRejectReason::ProductionEdit)
}

#[test]
fn changing_cfg_test_while_adding_a_test_is_rejected() -> Result<(), String> {
    let after = ADDED_TEST.replace(
        concat!("#[", "cfg(test)]"),
        "#[cfg(all(test, feature = \"x\"))]",
    );
    assert_rejected(
        LIB,
        &after,
        InlineTestRegionRejectReason::ModuleDeclarationChanged,
    )
}

#[test]
fn changing_module_visibility_while_adding_a_test_is_rejected() -> Result<(), String> {
    let after = ADDED_TEST.replace("mod tests {", "pub mod tests {");
    assert_rejected(
        LIB,
        &after,
        InlineTestRegionRejectReason::ModuleDeclarationChanged,
    )
}

#[test]
fn rewriting_module_braces_while_adding_a_test_is_rejected() -> Result<(), String> {
    let after = ADDED_TEST.replace("mod tests {", "mod tests\n{");
    assert_rejected(
        LIB,
        &after,
        InlineTestRegionRejectReason::ModuleDeclarationChanged,
    )
}

#[test]
fn renaming_the_module_while_adding_a_test_is_rejected() -> Result<(), String> {
    let after = ADDED_TEST.replace("mod tests {", "mod unit {");
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Rejected);
    assert_eq!(
        verdict.reason,
        Some(InlineTestRegionRejectReason::MissingRegion),
        "renaming the named module removes the region rather than silently retargeting"
    );
    Ok(())
}

#[test]
fn inserting_into_a_sibling_test_module_not_named_by_the_authority_is_rejected()
-> Result<(), String> {
    let before = r#"pub fn price() -> i32 { 1 }

#[cfg(test)]
mod tests {
}

#[cfg(test)]
mod extra {
}
"#;
    let after = r#"pub fn price() -> i32 { 1 }

#[cfg(test)]
mod tests {
}

#[cfg(test)]
mod extra {
    #[test]
    fn added() {}
}
"#;
    assert_rejected(before, after, InlineTestRegionRejectReason::ProductionEdit)
}

#[test]
fn inserting_into_a_nested_test_module_not_named_by_the_authority_is_rejected() -> Result<(), String>
{
    let before = r#"pub fn price() -> i32 { 1 }

#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod nested {
    }
}
"#;
    let after = r#"pub fn price() -> i32 { 1 }

#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod nested {
        #[test]
        fn added() {}
    }
}
"#;
    assert_rejected(
        before,
        after,
        InlineTestRegionRejectReason::UnsupportedModuleKind,
    )
}

#[test]
fn unique_nested_named_region_can_still_be_the_authority() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

mod wrapper {
    #[cfg(test)]
    mod tests {
    }
}
"#;
    let after = r#"pub fn price() -> i32 { 1 }

mod wrapper {
    #[cfg(test)]
    mod tests {
        #[test]
        fn added() {}
    }
}
"#;
    let authority =
        authority_from_source("src/lib.rs", source, "tests", "pkg:test".to_string(), &[])
            .map_err(|error| format!("{error:?}"))?;
    assert_eq!(authority.portable.module_path, "wrapper::tests");
    let verdict = validate_inline_test_region_edit(source, &authority, after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn out_of_line_test_module_is_unsupported() -> Result<(), String> {
    let source = "pub fn price() {}\n#[cfg(test)]\nmod tests;\n";
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion)
    );
    Ok(())
}

#[test]
fn cfg_all_test_unix_is_admitted_through_cfg_predicates() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

#[cfg(all(test, unix))]
mod tests {
}
"#;
    let after = r#"pub fn price() -> i32 { 1 }

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn added() {}
}
"#;
    let authority =
        authority_from_source("src/lib.rs", source, "tests", "pkg:test".to_string(), &[])
            .map_err(|error| format!("{error:?}"))?;
    let verdict = validate_inline_test_region_edit(source, &authority, after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn cfg_not_test_does_not_create_a_region() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

#[cfg(not(test))]
mod tests {
}
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion)
    );
    Ok(())
}

#[test]
fn unattributed_inline_module_is_not_a_test_region() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

mod tests {
    #[test]
    fn existing() {}
}
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion)
    );
    Ok(())
}

#[test]
fn macro_body_is_not_an_observed_inline_region() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

macro_rules! gen_tests {
    () => {
        #[cfg(test)]
        mod tests {}
    };
}

gen_tests!();
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion)
    );
    Ok(())
}

#[test]
fn comment_lookalike_cfg_test_does_not_create_a_region() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

// #[cfg(test)]
mod tests {
    pub fn not_a_test() {}
}
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion)
    );
    Ok(())
}

#[test]
fn stale_source_digest_rejects_even_a_legal_insertion() -> Result<(), String> {
    let mut authority = authority(LIB)?;
    authority.source_digest = "stale".to_string();
    let verdict = validate_inline_test_region_edit(LIB, &authority, ADDED_TEST);
    assert_eq!(verdict.status, InlineTestRegionStatus::Rejected);
    assert_eq!(
        verdict.reason,
        Some(InlineTestRegionRejectReason::StaleSourceDigest)
    );
    Ok(())
}

#[test]
fn rewriting_an_existing_test_is_rejected() -> Result<(), String> {
    let after = LIB.replace("price(1), 1", "price(1), 2");
    assert_rejected(
        LIB,
        &after,
        InlineTestRegionRejectReason::ExistingAuthorityRewritten,
    )
}

#[test]
fn line_movement_after_capture_invalidates_the_old_authority() -> Result<(), String> {
    let moved = format!("\n{LIB}");
    let previous = authority(LIB)?;
    let recaptured = authority(&moved)?;
    assert_eq!(
        previous.portable, recaptured.portable,
        "portable identity must ignore volatile line numbers"
    );
    assert_ne!(previous.header_range, recaptured.header_range);
    assert_ne!(previous.source_digest, recaptured.source_digest);
    let later = moved.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    #[test]\n    fn added() {}",
    );
    let stale = validate_inline_test_region_edit(&moved, &previous, &later);
    assert_eq!(stale.status, InlineTestRegionStatus::Rejected);
    assert_eq!(
        stale.reason,
        Some(InlineTestRegionRejectReason::StaleSourceDigest)
    );
    let current = validate_inline_test_region_edit(&moved, &recaptured, &later);
    assert_eq!(current.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn whitespace_only_insertion_is_not_a_completed_repair() -> Result<(), String> {
    let after = LIB.replace("    use super::*;\n", "    use super::*;\n\n");
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::NotARepair);
    assert_eq!(
        verdict.reason,
        Some(InlineTestRegionRejectReason::NonTestSubject)
    );
    Ok(())
}

#[test]
fn two_inline_test_modules_named_tests_are_ambiguous() -> Result<(), String> {
    let source = r#"
mod a {
    #[cfg(test)]
    mod tests {}
}
mod b {
    #[cfg(test)]
    mod tests {}
}
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::AmbiguousRegion)
    );
    Ok(())
}

#[test]
fn tests_layout_path_is_not_an_inline_region() -> Result<(), String> {
    let error = authority_failure("tests/pricing.rs", LIB)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::UnsupportedModuleKind)
    );
    Ok(())
}

#[test]
fn generated_path_is_unsupported() -> Result<(), String> {
    let error = authority_failure("src/generated.rs", LIB)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::UnsupportedModuleKind)
    );
    Ok(())
}

#[test]
fn traversal_relative_path_is_rejected_before_diff_admission() -> Result<(), String> {
    let error = authority_failure("../src/lib.rs", LIB)?;
    assert!(
        error.reason() == Some(InlineTestRegionRejectReason::PathEscape)
            || matches!(error, super::InlineTestRegionError::Path(_))
    );
    Ok(())
}

#[test]
fn portable_identity_survives_relocated_roots_while_containment_stays_exact() -> Result<(), String>
{
    let first = git_lib_fixture("region-root-a")?;
    let second = git_lib_fixture("region-root-b")?;
    fs::write(first.root.join("src/lib.rs"), LIB).map_err(|e| e.to_string())?;
    fs::write(second.root.join("src/lib.rs"), LIB).map_err(|e| e.to_string())?;
    git_ok(&first.root, &["add", "src/lib.rs"])?;
    git_ok(&second.root, &["add", "src/lib.rs"])?;
    git_ok(&first.root, &["commit", "-qm", "lib"])?;
    git_ok(&second.root, &["commit", "-qm", "lib"])?;
    let left = capture_inline_test_region_authority(&first.root, "src/lib.rs", "tests", &[])
        .map_err(|e| format!("{e:?}"))?;
    let right = capture_inline_test_region_authority(&second.root, "src/lib.rs", "tests", &[])
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(left.portable, right.portable);
    assert_eq!(left.source_digest, right.source_digest);
    assert_ne!(first.root, second.root);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_escape_cannot_redirect_the_allowed_region() -> Result<(), String> {
    let fixture = git_lib_fixture("region-symlink")?;
    let outside = std::env::temp_dir().join(format!(
        "ripr-inline-region-outside-{}-{}",
        std::process::id(),
        nanos()
    ));
    fs::create_dir_all(&outside).map_err(|e| e.to_string())?;
    fs::write(outside.join("lib.rs"), LIB).map_err(|e| e.to_string())?;
    let link = fixture.root.join("src/lib.rs");
    fs::remove_file(&link).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(outside.join("lib.rs"), &link).map_err(|e| e.to_string())?;
    let error =
        match capture_inline_test_region_authority(&fixture.root, "src/lib.rs", "tests", &[]) {
            Err(error) => error,
            Ok(_) => return Err("symlink must fail closed".to_string()),
        };
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::PathEscape)
    );
    let _ = fs::remove_dir_all(outside);
    Ok(())
}

#[test]
fn case_folding_alias_does_not_match_the_stored_relative_path() -> Result<(), String> {
    let authority = authority(LIB)?;
    assert_eq!(authority.portable.relative_file, "src/lib.rs");
    let alias = authority_from_source("SRC/LIB.RS", LIB, "tests", "pkg:test".to_string(), &[])
        .map_err(|error| format!("{error:?}"))?;
    assert_ne!(
        alias.portable.relative_file,
        authority.portable.relative_file
    );
    Ok(())
}

#[test]
fn adding_a_test_and_changing_production_is_the_discriminating_negative() -> Result<(), String> {
    let after = ADDED_TEST.replace("cents\n", "cents + 1\n");
    assert!(
        file_level_only_would_admit(LIB, &after),
        "file-level allowlist must accept the combined production-and-test edit"
    );
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Rejected);
    assert_eq!(
        verdict.reason,
        Some(InlineTestRegionRejectReason::ProductionEdit)
    );
    Ok(())
}

#[test]
fn removing_the_region_check_would_let_the_production_negative_pass() {
    let after = ADDED_TEST.replace("cents\n", "cents + 1\n");
    assert!(
        file_level_only_would_admit(LIB, &after),
        "control 12: without region containment the production-edit negative passes"
    );
    assert!(!file_level_only_would_admit(LIB, LIB));
}

#[test]
fn unparseable_after_source_is_rejected() -> Result<(), String> {
    assert_rejected(
        LIB,
        "fn not rust {",
        InlineTestRegionRejectReason::Unparseable,
    )
}

#[test]
fn use_only_insertion_is_not_a_completed_repair() -> Result<(), String> {
    let after = LIB.replace(
        "    use super::*;\n",
        "    use super::*;\n    use core::fmt;\n",
    );
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::NotARepair);
    assert_eq!(
        verdict.reason,
        Some(InlineTestRegionRejectReason::NonTestSubject)
    );
    Ok(())
}

#[test]
fn use_plus_test_function_is_admitted() -> Result<(), String> {
    let with_use = LIB.replace(
        "    use super::*;\n",
        "    use super::*;\n    use core::fmt;\n",
    );
    let after = with_use.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    #[test]\n    fn added() {}",
    );
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn const_item_inside_the_named_module_is_not_test_role_evidence() -> Result<(), String> {
    let after = LIB.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    const N: i32 = 1;",
    );
    assert_rejected(LIB, &after, InlineTestRegionRejectReason::NonTestSubject)
}

#[test]
fn nested_module_item_inside_the_named_region_is_unsupported() -> Result<(), String> {
    let after = LIB.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    mod helpers {}",
    );
    assert_rejected(
        LIB,
        &after,
        InlineTestRegionRejectReason::UnsupportedModuleKind,
    )
}

#[test]
fn inserting_a_test_before_existing_items_is_admitted() -> Result<(), String> {
    let after = LIB.replace(
        "    use super::*;\n",
        "    #[test]\n    fn added() {}\n    use super::*;\n",
    );
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn helper_function_inside_the_named_module_is_test_role_evidence() -> Result<(), String> {
    let after = LIB.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    fn helper() -> i32 { 1 }",
    );
    let verdict = validate_inline_test_region_edit(LIB, &authority(LIB)?, &after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn function_local_cfg_test_module_is_not_a_region() -> Result<(), String> {
    let source = r#"pub fn price() -> i32 { 1 }

pub fn helper() {
    #[cfg(test)]
    mod tests {
        #[test]
        fn never_registered() {}
    }
}
"#;
    let error = observe_failure(source)?;
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::MissingRegion),
        "a function-local module is not a rustc test harness subject"
    );
    Ok(())
}

#[test]
fn file_level_region_stays_unique_beside_a_function_local_lookalike() -> Result<(), String> {
    let before = r#"pub fn price() -> i32 { 1 }

pub fn helper() {
    #[cfg(test)]
    mod tests {}
}

#[cfg(test)]
mod tests {
}
"#;
    let after = r#"pub fn price() -> i32 { 1 }

pub fn helper() {
    #[cfg(test)]
    mod tests {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn added() {}
}
"#;
    let authority =
        authority_from_source("src/lib.rs", before, "tests", "pkg:test".to_string(), &[])
            .map_err(|error| format!("{error:?}"))?;
    assert_eq!(authority.portable.module_path, "tests");
    let verdict = validate_inline_test_region_edit(before, &authority, after);
    assert_eq!(verdict.status, InlineTestRegionStatus::Admitted);
    Ok(())
}

#[test]
fn inner_cfg_not_test_plus_a_function_is_rejected() -> Result<(), String> {
    let with_inner = LIB.replace(
        "mod tests {\n    use super::*;",
        concat!("mod tests {\n    #![", "cfg(not(test))]\n    use super::*;"),
    );
    let after = with_inner.replace(
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }",
        "    fn existing() {\n        assert_eq!(price(1), 1);\n    }\n\n    #[test]\n    fn added() {}",
    );
    assert!(
        file_level_only_would_admit(LIB, &after),
        "item-delta-only matching would treat an inner cfg plus fn as a legal insertion"
    );
    assert_rejected(LIB, &after, InlineTestRegionRejectReason::CfgBasisChanged)
}

#[test]
fn configured_generated_pattern_rejects_a_non_conventional_path() -> Result<(), String> {
    let admitted = authority_from_source(
        "src/autogen_output.rs",
        LIB,
        "tests",
        "pkg:test".to_string(),
        &[],
    );
    if admitted.is_err() {
        return Err(
            "src/autogen_output.rs must not match built-in generated naming without a pattern"
                .to_string(),
        );
    }
    let error = match authority_from_source(
        "src/autogen_output.rs",
        LIB,
        "tests",
        "pkg:test".to_string(),
        &["autogen_output.rs".to_string()],
    ) {
        Err(error) => error,
        Ok(_) => {
            return Err("configured generated pattern must fail closed".to_string());
        }
    };
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::UnsupportedModuleKind)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn fifo_at_the_captured_path_is_rejected_without_blocking() -> Result<(), String> {
    let fixture = git_lib_fixture("region-fifo")?;
    let path = fixture.root.join("src/lib.rs");
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    git_ok(
        &fixture.root,
        &["-c", "alias.make-fifo=!mkfifo", "make-fifo", "src/lib.rs"],
    )?;
    let started = std::time::Instant::now();
    let error =
        match capture_inline_test_region_authority(&fixture.root, "src/lib.rs", "tests", &[]) {
            Err(error) => error,
            Ok(_) => return Err("FIFO must fail closed".to_string()),
        };
    if started.elapsed() >= std::time::Duration::from_secs(2) {
        return Err("FIFO capture blocked waiting for a writer".to_string());
    }
    assert_eq!(
        error.reason(),
        Some(InlineTestRegionRejectReason::PathEscape)
    );
    Ok(())
}

struct GitLibFixture {
    root: PathBuf,
}

impl Drop for GitLibFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn git_lib_fixture(name: &str) -> Result<GitLibFixture, String> {
    let root = std::env::temp_dir().join(format!(
        "ripr-inline-region-{name}-{}-{}",
        std::process::id(),
        nanos()
    ));
    fs::create_dir_all(root.join("src")).map_err(|e| format!("create src: {e}"))?;
    fs::write(root.join("src/lib.rs"), "pub fn price() {}\n").map_err(|e| e.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
    )
    .map_err(|e| e.to_string())?;
    git_ok(&root, &["-c", "init.templateDir=", "init", "-q"])?;
    git_ok(&root, &["config", "user.email", "ripr@example.invalid"])?;
    git_ok(&root, &["config", "user.name", "RIPR Test"])?;
    git_ok(&root, &["config", "commit.gpgSign", "false"])?;
    git_ok(&root, &["add", "."])?;
    git_ok(&root, &["commit", "-qm", "baseline"])?;
    Ok(GitLibFixture { root })
}
