use super::check_workspace_with_config;
use super::selector::selector_matches_location;
use super::{CheckInput, OutputFormat};
use crate::config::RiprConfig;
use crate::output;
use std::path::Path;

/// Produces a compact JSON context packet for one selected finding.
pub fn collect_context(
    root: &Path,
    selector: &str,
    max_related_tests: usize,
) -> Result<String, String> {
    collect_context_with_input(
        CheckInput {
            root: root.to_path_buf(),
            format: OutputFormat::Json,
            ..CheckInput::default()
        },
        selector,
        max_related_tests,
    )
}

/// Like [`collect_context`] but allows overriding the full check input.
pub fn collect_context_with_input(
    input: CheckInput,
    selector: &str,
    max_related_tests: usize,
) -> Result<String, String> {
    collect_context_with_config(input, selector, max_related_tests, &RiprConfig::default())
}

pub(crate) fn collect_context_with_config(
    input: CheckInput,
    selector: &str,
    max_related_tests: usize,
    config: &RiprConfig,
) -> Result<String, String> {
    let input = CheckInput {
        format: OutputFormat::Json,
        ..input
    };
    let output = check_workspace_with_config(input, config)?;
    let selected = output
        .findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding));

    match selected {
        Some(finding) => Ok(output::json::render_context_packet(
            finding,
            max_related_tests,
        )),
        None => Err(format!("no finding matched {selector:?}")),
    }
}
