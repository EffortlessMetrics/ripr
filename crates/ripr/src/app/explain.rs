use super::CheckInput;
use super::check_workspace_with_config;
use super::selector::select_finding;
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
    match select_finding(&output.findings, selector) {
        Some(finding) => Ok(output::human::render_finding_with_config(finding, config)),
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
    fn explain_finding_with_input_renders_selected_finding() -> Result<(), String> {
        let rendered = explain_finding_with_input(
            sample_diff_input(),
            "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path",
        )?;

        assert!(rendered.contains("Static exposure"));
        assert!(rendered.contains("no_static_path"));
        assert!(rendered.contains("InvoiceError::InvalidCurrency"));
        Ok(())
    }

    #[test]
    fn explain_finding_public_wrapper_reports_invalid_root() {
        let result = explain_finding(Path::new("missing-ripr-root-for-explain"), "probe:missing");

        assert!(
            result
                .err()
                .is_some_and(|err| err.contains("failed to run git diff"))
        );
    }
}
