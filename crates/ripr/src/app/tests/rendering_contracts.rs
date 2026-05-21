use super::{OutputFormat, check_output_with, render_check_with_config, sample_finding};
use crate::domain::Summary;

#[test]
fn summary_default_is_empty() {
    let summary = Summary::default();
    assert_eq!(summary.findings, 0);
    assert_eq!(summary.exposed, 0);
    assert_eq!(summary.weakly_exposed, 0);
}

#[test]
fn configured_finding_severity_applies_to_human_json_and_github() -> Result<(), String> {
    let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
    let config =
        crate::config::tests_only_parse("[severity.findings]\nweakly_exposed = \"info\"\n")?;

    let human = render_check_with_config(&output, &OutputFormat::Human, &config)?;
    let json = render_check_with_config(&output, &OutputFormat::Json, &config)?;
    let github = render_check_with_config(&output, &OutputFormat::Github, &config)?;

    if !human.contains("INFO src/lib.rs:1") {
        return Err(format!("human severity was not configured: {human}"));
    }
    if !json.contains("\"severity\": \"info\"") {
        return Err(format!("json severity was not configured: {json}"));
    }
    if !github.starts_with("::notice ") {
        return Err(format!("github severity was not configured: {github}"));
    }
    Ok(())
}
