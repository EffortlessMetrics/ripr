use super::{CheckInput, CheckOutput};
use crate::analysis::{
    AnalysisOptions, run_analysis_with_oracle_policy, run_repo_analysis_with_oracle_policy,
};
use crate::config::RiprConfig;
use crate::domain::Summary;

/// Runs the end-to-end static exposure analysis for a workspace.
///
/// # Errors
///
/// Returns `Err(String)` when diff acquisition, syntax indexing, or static
/// analysis cannot complete for the requested workspace/input pair.
///
/// # Examples
///
/// ```no_run
/// use ripr::{check_workspace, CheckInput};
///
/// let output = check_workspace(CheckInput::default())?;
/// println!("schema={}, findings={}", output.schema_version, output.findings.len());
/// # Ok::<(), String>(())
/// ```
pub fn check_workspace(input: CheckInput) -> Result<CheckOutput, String> {
    check_workspace_with_config(input, &RiprConfig::default())
}

pub(crate) fn check_workspace_with_config(
    input: CheckInput,
    config: &RiprConfig,
) -> Result<CheckOutput, String> {
    let options = AnalysisOptions {
        root: input.root.clone(),
        base: input.base.clone(),
        diff_file: input.diff_file.clone(),
        mode: input.mode.analysis_mode(),
        include_unchanged_tests: input.include_unchanged_tests,
    };
    let analysis = run_analysis_with_oracle_policy(&options, config.oracles())?;
    Ok(CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        base: input.base,
        summary: analysis.summary,
        findings: analysis.findings,
    })
}

/// Runs the repo-baseline static exposure analysis for a workspace. This
/// seeds probes from every currently-probeable production syntax shape
/// rather than from a diff. Use this when the answer to "is the repo's
/// static exposure clean?" should not depend on the contents of
/// `git diff origin/main...HEAD`.
///
/// # Errors
///
/// Returns `Err(String)` when repository traversal, syntax indexing, or
/// classification cannot complete for the requested workspace.
pub fn check_workspace_repo(input: CheckInput) -> Result<CheckOutput, String> {
    check_workspace_repo_with_config(input, &RiprConfig::default())
}

pub(crate) fn check_workspace_repo_with_config(
    input: CheckInput,
    config: &RiprConfig,
) -> Result<CheckOutput, String> {
    let options = AnalysisOptions {
        root: input.root.clone(),
        base: input.base.clone(),
        diff_file: input.diff_file.clone(),
        mode: input.mode.analysis_mode(),
        include_unchanged_tests: input.include_unchanged_tests,
    };
    let analysis = run_repo_analysis_with_oracle_policy(&options, config.oracles())?;
    Ok(CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        base: input.base,
        summary: analysis.summary,
        findings: analysis.findings,
    })
}

/// Build a minimal [`CheckOutput`] for repo seam-driven rendering.
///
/// The seam inventory, repo exposure, agent packet, SARIF seam, and
/// seam-native badge renderers read only `output.root` plus auxiliary
/// disk artifacts as needed, so this avoids running `run_repo_analysis`
/// to compute legacy `Findings` those formats discard. The rest of the
/// fields are populated for schema-consistency only.
pub fn repo_seam_inventory_input(input: CheckInput) -> CheckOutput {
    CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        base: input.base,
        summary: Summary::default(),
        findings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, OutputFormat};
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
            format: OutputFormat::Json,
            ..CheckInput::default()
        }
    }

    #[test]
    fn check_workspace_runs_diff_use_case_from_input() -> Result<(), String> {
        let output = check_workspace(sample_diff_input())?;

        assert_eq!(output.schema_version, "0.1");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.summary.findings, output.findings.len());
        assert!(
            output.findings.iter().any(|finding| finding.id
                == "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path")
        );
        Ok(())
    }

    #[test]
    fn check_workspace_repo_runs_repo_use_case_from_input() -> Result<(), String> {
        let mut input = sample_diff_input();
        input.diff_file = None;

        let output = check_workspace_repo(input)?;

        assert_eq!(output.schema_version, "0.1");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.root, sample_root());
        Ok(())
    }

    #[test]
    fn repo_seam_inventory_input_synthesizes_minimal_output_without_analysis() {
        let input = sample_diff_input();
        let output = repo_seam_inventory_input(input);

        assert_eq!(output.schema_version, "0.1");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.root, sample_root());
        assert_eq!(output.summary, Summary::default());
        assert!(output.findings.is_empty());
    }
}
