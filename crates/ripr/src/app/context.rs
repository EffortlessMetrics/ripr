use super::check_workspace_with_config;
use super::selector::select_finding;
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

pub fn collect_context_with_config(
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
    match select_finding(&output.findings, selector) {
        Some(finding) => Ok(output::json::render_context_packet(
            finding,
            max_related_tests,
        )),
        None => Err(format!("no finding matched {selector:?}")),
    }
}

/// Like [`collect_context_with_config`] but loads the finding set from a
/// previously written check artifact (`--from`, RIPR-SPEC-0140) instead of
/// re-running the pipeline. The artifact identity gate is fail-closed; on a
/// verified hit, selection and rendering are identical to the fresh path.
/// `max_related_tests` is a render-time knob: it is not part of the artifact
/// identity and is honored fresh, including beyond the `check --json`
/// render cap, because the artifact stores the uncapped related-tests list.
pub(crate) fn collect_context_from_artifact(
    input: CheckInput,
    selector: &str,
    max_related_tests: usize,
    config: &RiprConfig,
    artifact_path: &Path,
    asserted_base: Option<&str>,
) -> Result<String, String> {
    let findings = super::check_artifact::load_findings_for_reuse(
        artifact_path,
        &input,
        config,
        asserted_base,
    )?;
    match select_finding(&findings, selector) {
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
            "probe:crates_ripr_examples_sample_src_lib.rs:error_path:a776c683",
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
