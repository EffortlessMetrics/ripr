use super::CheckInput;
use super::check_workspace_with_config;
use super::selector::selector_matches_location;
use crate::config::RiprConfig;
use crate::output;
use std::path::Path;

/// Computes findings and renders a single selected finding in human format.
///
/// The selector can be either a finding identifier (for example
/// `probe:path_to_file.rs:42:family`) or a `file:line` location.
pub fn explain_finding(root: &Path, selector: &str) -> Result<String, String> {
    explain_finding_with_input(
        CheckInput {
            root: root.to_path_buf(),
            ..CheckInput::default()
        },
        selector,
    )
}

/// Like [`explain_finding`] but allows overriding the full check input.
pub fn explain_finding_with_input(input: CheckInput, selector: &str) -> Result<String, String> {
    explain_finding_with_config(input, selector, &RiprConfig::default())
}

pub(crate) fn explain_finding_with_config(
    input: CheckInput,
    selector: &str,
    config: &RiprConfig,
) -> Result<String, String> {
    let output = check_workspace_with_config(input, config)?;
    let selected = output
        .findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding));

    match selected {
        Some(finding) => Ok(output::human::render_finding_with_config(finding, config)),
        None => Err(format!("no finding matched {selector:?}")),
    }
}
