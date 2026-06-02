use super::CheckInput;
use crate::analysis::AnalysisOptions;

pub(super) fn analysis_options_from_input(input: &CheckInput) -> AnalysisOptions {
    AnalysisOptions {
        root: input.root.clone(),
        base: input.base.clone(),
        diff_file: input.diff_file.clone(),
        mode: input.mode.analysis_mode(),
        include_unchanged_tests: input.include_unchanged_tests,
    }
}
