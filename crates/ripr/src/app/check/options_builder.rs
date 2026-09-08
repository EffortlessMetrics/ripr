use super::CheckInput;
use crate::analysis::AnalysisOptions;
use crate::config::RiprConfig;

pub(crate) fn analysis_options_from_input_and_config(
    input: &CheckInput,
    config: &RiprConfig,
) -> AnalysisOptions {
    AnalysisOptions {
        root: input.root.clone(),
        base: input.base.clone(),
        diff_file: input.diff_file.clone(),
        mode: input.mode.analysis_mode(),
        include_unchanged_tests: input.include_unchanged_tests,
        resolve_tsconfig_paths: config.typescript().resolve_tsconfig_paths(),
        perl_facts_path: input.perl_facts_path.clone(),
        git_timeout: input.git_timeout,
        git_candidate: input.git_candidate.clone(),
        production_like_targets: config.analysis().production_like_targets().clone(),
        test_harnesses: config.analysis().test_harnesses().to_vec(),
        resolved_subject_identity: None,
    }
}
