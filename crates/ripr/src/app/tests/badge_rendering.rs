use super::{
    OutputFormat, check_output_with, check_output_with_temp_seam_workspace, render_check,
    sample_finding,
};

#[test]
fn render_check_dispatches_badge_json_format() -> Result<(), String> {
    let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
    let rendered = render_check(&output, &OutputFormat::BadgeJson)?;

    assert!(rendered.contains("\"schema_version\": \"0.5\""));
    assert!(rendered.contains("\"kind\": \"ripr\""));
    assert!(rendered.contains("\"scope\": \"diff\""));
    assert!(rendered.contains("\"basis\": \"finding_exposure\""));
    assert!(rendered.contains("\"counts\":"));
    assert!(rendered.contains("\"reason_counts\":"));
    assert!(rendered.contains("\"policy\":"));
    assert!(rendered.contains("\"duplicate_activation_and_oracle_shape\": 0"));
    Ok(())
}

#[test]
fn render_check_dispatches_badge_shields_format() -> Result<(), String> {
    let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
    let rendered = render_check(&output, &OutputFormat::BadgeShields)?;

    assert!(rendered.contains("\"schemaVersion\": 1"));
    assert!(rendered.contains("\"label\":"));
    assert!(rendered.contains("\"message\":"));
    assert!(rendered.contains("\"color\":"));
    for forbidden in [
        "\"counts\"",
        "\"reason_counts\"",
        "\"policy\"",
        "\"kind\"",
        "\"status\"",
        "\"scope\"",
        "\"basis\"",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "Shields projection must not contain `{forbidden}`"
        );
    }
    Ok(())
}

#[test]
fn badge_render_message_has_no_denominator_or_coverage_framing() -> Result<(), String> {
    let output = check_output_with(vec![
        sample_finding("src/a.rs", 1),
        sample_finding("src/b.rs", 2),
    ]);
    for format in [OutputFormat::BadgeJson, OutputFormat::BadgeShields] {
        let rendered = render_check(&output, &format)?;
        let lower = rendered.to_ascii_lowercase();
        assert!(
            !rendered.contains("\"message\": \"") || {
                let after = rendered.split("\"message\": \"").nth(1).unwrap_or("");
                let value_end = after.find('"').unwrap_or(after.len());
                let value = &after[..value_end];
                !value.contains('/')
            }
        );
        assert!(!lower.contains("coverage"));
        assert!(!lower.contains("uncovered"));
    }
    Ok(())
}

#[test]
fn render_check_repo_badge_json_paints_scope_repo() -> Result<(), String> {
    let output = check_output_with_temp_seam_workspace(vec![sample_finding("src/lib.rs", 1)])?;
    let rendered = render_check(&output, &OutputFormat::RepoBadgeJson)?;

    assert!(rendered.contains("\"schema_version\": \"0.5\""));
    assert!(rendered.contains("\"scope\": \"repo\""));
    assert!(rendered.contains("\"basis\": \"canonical_actionable_gap\""));
    assert!(!rendered.contains("\"scope\": \"diff\""));
    assert!(rendered.contains("\"kind\": \"ripr\""));
    assert!(rendered.contains("\"analyzed_findings\": 0"));
    assert!(!rendered.contains("\"analyzed_seams\": 0"));
    let _ = std::fs::remove_dir_all(&output.root);
    Ok(())
}

#[test]
fn render_check_repo_badge_shields_stays_four_fields_without_scope_leak() -> Result<(), String> {
    let output = check_output_with_temp_seam_workspace(vec![sample_finding("src/lib.rs", 1)])?;
    let rendered = render_check(&output, &OutputFormat::RepoBadgeShields)?;

    assert!(!rendered.contains("\"scope\""));
    assert!(!rendered.contains("\"basis\""));
    let top_level_keys = rendered
        .lines()
        .filter(|line| line.starts_with("  \""))
        .count();
    assert_eq!(top_level_keys, 4);
    for forbidden in [
        "\"counts\"",
        "\"reason_counts\"",
        "\"policy\"",
        "\"kind\"",
        "\"status\"",
        "\"schema_version\"",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "Shields projection must not contain `{forbidden}`"
        );
    }
    let _ = std::fs::remove_dir_all(&output.root);
    Ok(())
}

#[test]
fn render_check_badge_plus_missing_test_efficiency_is_neutral() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("ripr-badge-plus-missing-{stamp}"));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("create temp dir: {e}"))?;

    let mut output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
    output.root = tmp.clone();

    let rendered = render_check(&output, &OutputFormat::BadgePlusJson)?;
    assert!(rendered.contains(r#""kind": "ripr_plus""#));
    assert!(rendered.contains(r#""message": "needs test-efficiency""#));
    assert!(rendered.contains(r#""color": "lightgrey""#));
    assert!(rendered.contains("test-efficiency.json"));
    assert!(rendered.contains("docs/BADGE_ADOPTION.md"));
    assert!(!rendered.contains("cargo xtask test-efficiency-report"));

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}
