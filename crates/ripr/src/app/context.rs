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
    let navigation = super::finding_navigation(&input, None, false);
    let output = check_workspace_with_config(input, config)?;
    match select_finding(&output.findings, selector) {
        Some(finding) => Ok(output::json::render_context_packet_with_explain_command(
            finding,
            max_related_tests,
            Some(navigation.explain_command(&finding.id)),
        )),
        None => Err(format!(
            "no finding matched {selector:?}; run `ripr check --json` to list available finding ids"
        )),
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
    // As in `explain --from`, the navigation names the artifact: replaying
    // it keeps the verified identity without re-running the pipeline.
    let navigation = super::finding_navigation(&input, Some(artifact_path), false);
    match select_finding(&findings, selector) {
        Some(finding) => Ok(output::json::render_context_packet_with_explain_command(
            finding,
            max_related_tests,
            Some(navigation.explain_command(&finding.id)),
        )),
        None => Err(format!(
            "no finding matched {selector:?}; run `ripr check --json` to list available finding ids"
        )),
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

    /// #3952: the witness's explain command replays the input identity that
    /// produced the finding. Without it, `ripr explain` re-analyzes the
    /// default branch instead of the `--diff` (or `--base`) the user chose.
    #[test]
    fn context_explain_command_carries_the_input_diff() -> Result<(), String> {
        let input = sample_diff_input();
        let diff = input
            .diff_file
            .as_ref()
            .map(|path| crate::agent::loop_commands::shell_arg(&path.display().to_string()))
            .ok_or("sample input must carry a diff file")?;
        let selector = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:a776c683";
        let rendered = collect_context_with_input(input, selector, 2)?;
        let packet: serde_json::Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse packet: {err}"))?;
        let command = packet
            .pointer("/witness/explain_command")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("fixture must render a witness: {rendered}"))?;
        if !command.starts_with("ripr explain --root ")
            || !command.contains(&format!("--diff {diff}"))
            || !command.ends_with(selector)
        {
            return Err(format!("explain command must replay --diff: {command}"));
        }
        Ok(())
    }

    #[test]
    fn collect_context_public_wrapper_reports_invalid_root() {
        let result = collect_context(
            Path::new("missing-ripr-root-for-context"),
            "probe:missing",
            1,
        );

        // #3952: a missing root must fail as a missing root, not as an
        // unresolvable default base.
        assert!(
            result
                .err()
                .is_some_and(|err| err.contains("does not exist or is not a directory"))
        );
    }
}
