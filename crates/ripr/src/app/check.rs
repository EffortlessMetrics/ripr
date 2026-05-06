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
