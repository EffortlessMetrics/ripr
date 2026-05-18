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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;
    use std::path::PathBuf;

    fn sample_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample")
    }

    fn sample_diff_input() -> CheckInput {
        let root = sample_root();
        CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("example.diff")),
            mode: Mode::Draft,
            ..CheckInput::default()
        }
    }

    #[test]
    fn collect_context_with_input_renders_selected_finding_packet() -> Result<(), String> {
        let rendered = collect_context_with_input(
            sample_diff_input(),
            "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path",
            2,
        )?;

        assert!(rendered.contains("\"tool\": \"ripr\""));
        assert!(rendered.contains("\"family\": \"error_path\""));
        assert!(rendered.contains("\"missing_discriminators\""));
        assert!(rendered.contains("InvoiceError::InvalidCurrency"));
        Ok(())
    }

    #[test]
    fn collect_context_public_wrapper_reports_invalid_root() {
        let result = collect_context(
            Path::new("missing-ripr-root-for-context"),
            "probe:missing",
            1,
        );

        assert!(
            result
                .err()
                .is_some_and(|err| err.contains("failed to run git diff"))
        );
    }
}
