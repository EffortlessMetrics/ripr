use super::CheckInput;
use crate::analysis::AnalysisOptions;
use crate::config::RiprConfig;

pub(super) fn analysis_options_from_input_and_config(
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
    }
}
