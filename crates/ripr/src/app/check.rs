use super::{CheckInput, CheckOutput};
use crate::analysis::{
    AnalysisResult, run_analysis_with_oracle_policy, run_repo_analysis_with_oracle_policy,
    run_worktree_analysis_with_oracle_policy,
};
use crate::config::RiprConfig;
use crate::domain::LanguageId;
use crate::domain::Summary;
use std::path::PathBuf;
use std::process::Command;

mod options_builder;
mod output_builder;

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
    run_check(input, config, AnalysisMode::Diff)
}

pub(crate) fn check_workspace_worktree_with_config(
    input: CheckInput,
    config: &RiprConfig,
) -> Result<CheckOutput, String> {
    run_check(input, config, AnalysisMode::Worktree)
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
    run_check(input, config, AnalysisMode::Repo)
}

/// Build a minimal [`CheckOutput`] for repo seam-driven rendering.
///
/// The seam inventory, repo exposure, agent packet, SARIF seam, and
/// seam-native badge renderers read only `output.root` plus auxiliary
/// disk artifacts as needed, so this avoids running `run_repo_analysis`
/// to compute legacy `Findings` those formats discard. The rest of the
/// fields are populated for schema-consistency only.
pub fn repo_seam_inventory_input(input: CheckInput) -> CheckOutput {
    output_builder::check_output_from_analysis(
        input,
        AnalysisResult {
            summary: Summary::default(),
            findings: Vec::new(),
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
        },
    )
}

enum AnalysisMode {
    Diff,
    Worktree,
    Repo,
}

fn run_check(
    mut input: CheckInput,
    config: &RiprConfig,
    mode: AnalysisMode,
) -> Result<CheckOutput, String> {
    // Managed producer mode (Campaign 31 Phase D, #1407): when
    // [perl] producer = "perllsp", invoke the perl-lsp binary to generate
    // a fact packet, then consume it automatically. NO silent invocation
    // unless explicitly configured.
    let perl_config = config.perl();
    if let Some(producer) = perl_config.producer()
        && producer == "perllsp"
        && input.perl_facts_path.is_none()
    {
        let packet_path = invoke_perl_lsp_producer(perl_config, &input)?;
        input.perl_facts_path = Some(packet_path);
    }

    let options = options_builder::analysis_options_from_input_and_config(&input, config);

    // Build the language list from config. When --perl-facts is provided,
    // automatically add Perl to the enabled list (the user explicitly opted in
    // by supplying a packet path). Campaign 31, #1429.
    let mut languages = config.languages().enabled().to_vec();
    if options.perl_facts_path.is_some() && !languages.contains(&LanguageId::Perl) {
        languages.push(LanguageId::Perl);
    }

    let analysis = match mode {
        AnalysisMode::Diff => {
            run_analysis_with_oracle_policy(&options, config.oracles(), &languages)?
        }
        AnalysisMode::Worktree => {
            run_worktree_analysis_with_oracle_policy(&options, config.oracles(), &languages)?
        }
        AnalysisMode::Repo => {
            run_repo_analysis_with_oracle_policy(&options, config.oracles(), &languages)?
        }
    };

    Ok(output_builder::check_output_from_analysis(input, analysis))
}

/// Invoke the `perl-lsp` binary to generate a fact packet.
///
/// Managed producer mode (Campaign 31 Phase D, #1407). Invokes:
/// ```text
/// perllsp --ripr-facts --ripr-schema ripr-perl-facts-v1 --ripr-root <root>
///   --ripr-out <cache_dir>/<hash>.json
/// ```
///
/// Captures stderr for diagnostics. Returns the packet path on success.
fn invoke_perl_lsp_producer(
    perl_config: &crate::config::PerlConfig,
    input: &CheckInput,
) -> Result<PathBuf, String> {
    let executable = perl_config
        .executable()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("perllsp"));

    let cache_dir = perl_config
        .cache_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("target/ripr/perl-facts"));

    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("failed to create Perl facts cache dir: {e}"))?;

    let root_str = input.root.display().to_string();
    let packet_hash = format!("{:016x}", simple_hash(&root_str));
    let packet_path = cache_dir.join(format!("{packet_hash}.json"));

    let _timeout_ms = perl_config.timeout_ms();
    let result = Command::new(&executable)
        .arg("--ripr-facts")
        .arg("--ripr-schema")
        .arg("ripr-perl-facts-v1")
        .arg("--ripr-root")
        .arg(&root_str)
        .arg("--ripr-out")
        .arg(packet_path.display().to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    // Note: full timeout enforcement (spawned-thread + join_timeout) lands
    // with the process-policy gate. For now the producer runs without a
    // timeout; the timeout_ms config is read but not enforced. This is safe
    // because managed mode is explicit opt-in only.

    match result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "perllsp ripr-facts failed (exit {:?}): {stderr}",
                    output.status.code()
                ));
            }
            if !packet_path.exists() {
                return Err(format!(
                    "perllsp ripr-facts completed but packet not found at {}",
                    packet_path.display()
                ));
            }
            Ok(packet_path)
        }
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Err(format!(
            "perllsp ripr-facts timed out after {_timeout_ms}ms"
        )),
        Err(e) => Err(format!(
            "failed to invoke perllsp at {}: {e}",
            executable.display()
        )),
    }
}

/// Simple deterministic hash for cache file naming (not cryptographic).
fn simple_hash(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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

        assert_eq!(output.schema_version, "0.2");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.summary.findings, output.findings.len());
        assert!(output.findings.iter().any(|finding| finding.id
            == "probe:crates_ripr_examples_sample_src_lib.rs:error_path:a776c683"));
        Ok(())
    }

    #[test]
    fn check_workspace_repo_runs_repo_use_case_from_input() -> Result<(), String> {
        let mut input = sample_diff_input();
        input.diff_file = None;

        let output = check_workspace_repo(input)?;

        assert_eq!(output.schema_version, "0.2");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.root, sample_root());
        Ok(())
    }

    #[test]
    fn repo_seam_inventory_input_synthesizes_minimal_output_without_analysis() {
        let input = sample_diff_input();
        let output = repo_seam_inventory_input(input);

        assert_eq!(output.schema_version, "0.2");
        assert_eq!(output.tool, "ripr");
        assert_eq!(output.mode, Mode::Draft);
        assert_eq!(output.root, sample_root());
        assert_eq!(output.summary, Summary::default());
        assert!(output.findings.is_empty());
    }
}
